use super::context::TaskContext;
use super::kernel_stack_alloc::KernelStack;
use super::loader::open_app_file;
use super::scheduler::init_process;
use crate::config::TRAP_CONTEXT_START_VA;
use crate::mem::address_space::{copy_address_space, user_space_from_elf, AddressSpace, KERNEL_SPACE};
use crate::mem::page_table::{PhyAddr, VirtAddr, PPN};
use crate::trap::{trap_handler, TrapContext};
use crate::config::{GREEN, RESET};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use sync::UPSafeCell;

pub fn init() {
    add_task(INIT_TASK.get_pid(), INIT_TASK.clone());
    println!("{}init process has been build and added to PM{}", GREEN, RESET);
}

pub fn add_task(pid: usize, task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add_task(pid, task);
}

pub fn get_task(pid: usize) -> Option<Arc<TaskControlBlock>> {
    return TASK_MANAGER.exclusive_access().get_task(pid);
}

pub fn remove_task(pid: usize) {
    TASK_MANAGER.exclusive_access().remove_task(pid);
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

pub struct TaskManager {
    ready_tasks: BTreeMap<usize, Arc<TaskControlBlock>>
}

impl TaskManager {
    fn new() -> Self {
        Self {
            ready_tasks: BTreeMap::new(),
        }
    }

    fn add_task(&mut self, pid: usize, task: Arc<TaskControlBlock>) {
        self.ready_tasks.insert(pid, task);
    }

    fn remove_task(&mut self, pid: usize) {
        self.ready_tasks.remove(&pid);
    }

    fn get_task(&self, pid: usize) -> Option<Arc<TaskControlBlock>> {
        self.ready_tasks.get(&pid).map(Arc::clone)
    }
}

pub struct TaskControlBlock {
    // immutable
    pub pid: usize,
    pub kernel_stack: KernelStack,
    // mutable
    pub inner: UPSafeCell<TaskControlBlockInner>,
}

// to implement inner mutability of a immutable reference
pub struct TaskControlBlockInner {
    pub task_ctx: TaskContext,
    pub trap_ctx_ppn: PPN,
    pub address_space: AddressSpace,
    pub heap_bottom: usize,
    pub program_brk: usize, // heap top
    pub user_stack_start: usize,
}

impl TaskControlBlock {
    pub fn new(elf_data: &[u8], pid: usize) -> Self {

        // build user space, and get trap context ppn, sp of user stack
        let (user_space, user_sp, elf_entry_point) = user_space_from_elf(elf_data);
        let trap_ctx_ppn = user_space
            .translate(VirtAddr::from(TRAP_CONTEXT_START_VA).to_down_vpn())
            .unwrap();

        // allocate kernel stack
        let kernel_stack = KernelStack::new(pid);
        let kernel_stack_top = kernel_stack.get_top();

        let task_context: TaskContext = TaskContext::new(kernel_stack_top);

        let task_control_block = Self {
            pid,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_ctx_ppn,
                    user_stack_start: user_sp.into(),
                    heap_bottom: user_sp.into(),
                    program_brk: user_sp.into(),
                    task_ctx: task_context,
                    address_space: user_space,
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
            "task {} created, entry va = {:#x}",
            pid, elf_entry_point
        );
        task_control_block
    }

    pub fn fork(self: &Arc<TaskControlBlock>, child_pid: usize) -> Arc<TaskControlBlock> {
        let parent_inner = self.inner.exclusive_access();

        // copy address space and get new trap context ppn
        let child_address_space = copy_address_space(&parent_inner.address_space);
        let child_trap_ctx_ppn = child_address_space
            .translate(VirtAddr::from(TRAP_CONTEXT_START_VA).to_down_vpn())
            .unwrap();

        // allocate pid and kernel stack
        let child_kernel_stack = KernelStack::new(child_pid);
        let child_kernel_stack_top = child_kernel_stack.get_top();

        let child_task_context = TaskContext::new(child_kernel_stack_top);

        let child_task_control_block = Arc::new(Self {
            pid: child_pid,
            kernel_stack: child_kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_ctx_ppn: child_trap_ctx_ppn,
                    user_stack_start: parent_inner.user_stack_start,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    task_ctx: child_task_context,
                    address_space: child_address_space,
                })
            },
        });

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

    pub fn get_pid(&self) -> usize {
        self.pid
    }
}

impl TaskControlBlockInner {
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
    pub static ref INIT_TASK: Arc<TaskControlBlock> = Arc::new({
        let data = open_app_file("initproc").unwrap();
        let pid = init_process();
        TaskControlBlock::new(data, pid)
    });
}


