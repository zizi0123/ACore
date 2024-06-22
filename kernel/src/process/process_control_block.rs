use super::context::TaskContext;
use super::pid::{pid_alloc, KernelStack, PidWrapper};
use super::loader::open_app_file;
use crate::config::TRAP_CONTEXT_START_VA;
use crate::mem::address_space::{copy_address_space, user_space_from_elf, AddressSpace, KERNEL_SPACE};
use crate::mem::page_table::{PhyAddr, VirtAddr, PPN};
use crate::trap::{trap_handler, TrapContext};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use sync::UPSafeCell;

#[derive(Copy, Clone, PartialEq)]
pub enum ProcessStatus {
    Ready,   // 准备运行 （当程序被加载入内存初始化时，它的初始状态即为Ready）
    Running, // 正在运行
    Exited,  // 已退出
}

pub struct ProcessControlBlock {
    // immutable
    pub pid: PidWrapper,
    pub kernel_stack: KernelStack,
    // mutable
    pub inner: UPSafeCell<ProcessControlBlockInner>,
}

// to implement inner mutability of a immutable reference
pub struct ProcessControlBlockInner {
    pub status: ProcessStatus,
    pub task_ctx: TaskContext,
    pub trap_ctx_ppn: PPN,
    pub address_space: AddressSpace,
    pub heap_bottom: usize,
    pub program_brk: usize,                     // heap top
    pub parent: Option<Weak<ProcessControlBlock>>, // Weak reference won't add reference count
    pub children: Vec<Arc<ProcessControlBlock>>, // when the reference cnt of Arc = 0, it will be recycleed. Any task can't exist if there is no reference from parent (except the init task).
    pub user_stack_start: usize,
    pub exit_code: i32,
}

impl ProcessControlBlock {
    pub fn new(elf_data: &[u8]) -> Self {
        // build user space, and get trap context ppn, sp of user stack
        let (user_space, user_sp, elf_entry_point) = user_space_from_elf(elf_data);
        let trap_ctx_ppn = user_space
            .translate(VirtAddr::from(TRAP_CONTEXT_START_VA).to_down_vpn())
            .unwrap();

        // alloctate pid
        let pid = pid_alloc();
        let pid_numer = pid.0;
        // allocate kernel stack
        let kernel_stack = KernelStack::new(&pid);
        let kernel_stack_top = kernel_stack.get_top();

        let task_context: TaskContext = TaskContext::new(kernel_stack_top);

        let task_control_block = Self {
            pid,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    trap_ctx_ppn,
                    user_stack_start: user_sp.into(),
                    heap_bottom: user_sp.into(),
                    program_brk: user_sp.into(),
                    task_ctx: task_context,
                    status: ProcessStatus::Ready,
                    address_space: user_space,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                })
            },
        };

        // initiate trap context
        // when an app was initially built, the entry after trap is the entry point of the app
        task_control_block.inner.exclusive_access().set_trap_ctx(
            elf_entry_point,
            user_sp.into(),
            KERNEL_SPACE.exclusive_access().get_satp(),
            kernel_stack_top,
            trap_handler as usize,
        );

        println!(
            "process {} created, entry va = {:#x}",
            pid_numer, elf_entry_point
        );
        task_control_block
    }

    pub fn get_pid(&self) -> usize {
        self.pid.0
    }

    pub fn fork(self: &Arc<ProcessControlBlock>) -> Arc<ProcessControlBlock> {
        let mut parent_inner = self.inner.exclusive_access();

        // copy address space and get new trap context ppn
        let child_address_space = copy_address_space(&parent_inner.address_space);
        let child_trap_ctx_ppn = child_address_space
            .translate(VirtAddr::from(TRAP_CONTEXT_START_VA).to_down_vpn())
            .unwrap();

        // allocate pid and kernel stack
        let child_pid = pid_alloc();
        let child_kernel_stack = KernelStack::new(&child_pid);
        let child_kernel_stack_top = child_kernel_stack.get_top();

        let child_task_context = TaskContext::new(child_kernel_stack_top);

        let child_task_control_block = Arc::new(Self {
            pid: child_pid,
            kernel_stack: child_kernel_stack,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    trap_ctx_ppn: child_trap_ctx_ppn,
                    user_stack_start: parent_inner.user_stack_start,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    task_ctx: child_task_context,
                    status: ProcessStatus::Ready,
                    address_space: child_address_space,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                })
            },
        });

        parent_inner.children.push(child_task_control_block.clone());

        // only need to change kernel stack top, other info have been copyed when copy address space
        child_task_control_block
            .inner
            .exclusive_access()
            .get_trap_ctx()
            .kernel_sp = child_kernel_stack_top;

        return child_task_control_block;
    }

    pub fn exec(&self, elf_data: &[u8]) {
        let (user_space, user_sp, elf_entry_point) = user_space_from_elf(elf_data);
        let trap_ctx_ppn = user_space
            .translate(VirtAddr::from(TRAP_CONTEXT_START_VA).to_down_vpn())
            .unwrap();

        let mut inner = self.inner.exclusive_access();
        inner.address_space = user_space;
        inner.trap_ctx_ppn = trap_ctx_ppn;

        inner.set_trap_ctx(
            elf_entry_point,
            user_sp.into(),
            KERNEL_SPACE.exclusive_access().get_satp(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
    }
}

impl ProcessControlBlockInner {
    fn set_trap_ctx(
        &self,
        entry: usize, // entry point of the app after trap
        sp: usize,    // the va of the current app's user stack sp
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) {
        let ctx_addr: PhyAddr = self.trap_ctx_ppn.into();
        let trap_cx_ptr: &'static mut TrapContext =
            unsafe { (ctx_addr.0 as *mut TrapContext).as_mut().unwrap() };
        *trap_cx_ptr = TrapContext::new(
            entry,
            sp.into(),
            kernel_satp,
            kernel_sp,
            trap_handler as usize,
        );
    }

    pub fn get_trap_ctx(&self) -> &'static mut TrapContext {
        let ctx_addr: PhyAddr = self.trap_ctx_ppn.into();
        return unsafe { (ctx_addr.0 as *mut TrapContext).as_mut().unwrap() };
    }

    pub fn has_exited(&self) -> bool {
        return self.status == ProcessStatus::Exited
    }

    /// change the location of the program break. return None if failed.
    pub fn change_program_brk(&mut self, size: i32) -> Option<usize> {
        let old_break = self.program_brk;
        let new_brk = self.program_brk as isize + size as isize;
        if new_brk < self.heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            self.address_space.shrink_heap_to(
                VirtAddr::from(self.heap_bottom),
                VirtAddr::from(new_brk as usize),
            )
        } else {
            self.address_space.append_heap_to(
                VirtAddr::from(self.heap_bottom),
                VirtAddr::from(new_brk as usize),
            )
        };
        if result {
            self.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }
}

lazy_static! {
    pub static ref INITPROC: Arc<ProcessControlBlock> = Arc::new({
        let data = open_app_file("initproc").unwrap();
        ProcessControlBlock::new(data)
    });
}
