// init process: executes the user shell and continuously recycles zombie processes in loop
#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

extern crate alloc;

const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

const PM_INIT: usize = 1;
const PM_FORK: usize = 2;
const PM_WAITPID: usize = 3;
const PM_SUSPEND: usize = 4;
const PM_EXIT: usize = 5;
const PM_FETCH: usize = 6;

use alloc::collections::VecDeque;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use sync::UPSafeCell;
use user_lib::pm_service;

// return: init process pid
fn init() -> isize {
    PROCESS_MANAGER
        .exclusive_access()
        .add_ready_process(INIT_PROCESS.clone());
    println!(
        "{}init process has been build and added to PM{}",
        GREEN, RESET
    );
    return INIT_PROCESS.get_pid() as isize;
}

// return: child pid
fn fork() -> isize {
    let parent_pcb = PROCESS_MANAGER.exclusive_access().get_current().unwrap();

    // allocate pid and kernel stack
    let child_pid = pid_alloc();

    let child_process_control_block = Arc::new(ProcessControlBlock {
        pid: child_pid,
        inner: unsafe {
            UPSafeCell::new(ProcessControlBlockInner {
                status: ProcessStatus::Ready,
                parent: Some(Arc::downgrade(&parent_pcb)),
                children: Vec::new(),
                exit_code: 0,
            })
        },
    });

    let mut parent_inner = parent_pcb.inner.exclusive_access();
    parent_inner
        .children
        .push(child_process_control_block.clone());
    drop(parent_inner);

    PROCESS_MANAGER
        .exclusive_access()
        .add_ready_process(child_process_control_block.clone());
    let child_pid = child_process_control_block.get_pid();

    return child_pid as isize;
}

// return: (wait result, exit code)
// no such child process: wait result = -1
// child process has not exited: wait result = -2
// else, wait result = pid of exited child process
fn waitpid(pid: isize) -> (isize, usize) {
    let parent_pcb = PROCESS_MANAGER.exclusive_access().get_current().unwrap();
    let mut parent_inner = parent_pcb.inner.exclusive_access();

    // no such child process
    if !parent_inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.get_pid())
    {
        return (-1, 0);
    }

    // get the exited child process
    let pair = parent_inner.children.iter().enumerate().find(|(_, p)| {
        p.inner.exclusive_access().has_exited() && (pid == -1 || pid as usize == p.get_pid())
    });
    if let Some((idx, _)) = pair {
        let child = parent_inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.get_pid() as isize;
        let exit_code = child.inner.exclusive_access().exit_code;
        return (found_pid, exit_code as usize);
    } else {
        return (-2, 0);
    }
    // ---- release parent pcb automatically
}

fn suspend_current_process() {
    let current_pcb = PROCESS_MANAGER.exclusive_access().take_current().unwrap();
    let mut current_inner = current_pcb.inner.exclusive_access();
    current_inner.status = ProcessStatus::Ready;
    PROCESS_MANAGER
        .exclusive_access()
        .add_ready_process(current_pcb.clone());
}

fn exit_current_process(exit_code: i32) {
    let current_pcb = PROCESS_MANAGER.exclusive_access().take_current().unwrap();
    let mut current_inner = current_pcb.inner.exclusive_access();
    current_inner.status = ProcessStatus::Exited;
    current_inner.exit_code = exit_code;

    // add parent of current process's children to INITPROC
    let mut initproc_inner = INIT_PROCESS.inner.exclusive_access();
    for child in current_inner.children.iter() {
        child.inner.exclusive_access().parent = Some(Arc::downgrade(&INIT_PROCESS)); // downgrade to Weak, won't add ref count
        initproc_inner.children.push(child.clone()); // clone to add ref count
    }
    drop(initproc_inner);

    current_inner.children.clear();

    drop(current_inner);
    drop(current_pcb);
}

// get a ready process from ready queue, and set it as current process, set status = running
fn fetch_ready_process() -> isize {
    return PROCESS_MANAGER
        .exclusive_access()
        .fetch_ready_process()
        .unwrap()
        .get_pid() as isize;
}

lazy_static! {
    pub static ref PROCESS_MANAGER: UPSafeCell<ProcessManager> =
        unsafe { UPSafeCell::new(ProcessManager::new()) };
}

lazy_static! {
    pub static ref INIT_PROCESS: Arc<ProcessControlBlock> = Arc::new(ProcessControlBlock::new());
}

pub struct ProcessManager {
    current_process: Option<Arc<ProcessControlBlock>>,
    ready_process_queue: VecDeque<Arc<ProcessControlBlock>>,
}

impl ProcessManager {
    fn new() -> Self {
        Self {
            ready_process_queue: VecDeque::new(),
            current_process: None,
        }
    }

    fn get_current(&self) -> Option<Arc<ProcessControlBlock>> {
        self.current_process.as_ref().map(Arc::clone)
    }

    fn take_current(&mut self) -> Option<Arc<ProcessControlBlock>> {
        return self.current_process.take();
    }

    fn add_ready_process(&mut self, process: Arc<ProcessControlBlock>) {
        process.inner.exclusive_access().status = ProcessStatus::Ready;
        self.ready_process_queue.push_back(process);
    }

    fn fetch_ready_process(&mut self) -> Option<Arc<ProcessControlBlock>> {
        if let Some(pcb) = self.ready_process_queue.pop_front() {
            let mut process_inner = pcb.inner.exclusive_access();
            process_inner.status = ProcessStatus::Running;
            self.current_process = Some(pcb.clone());
            return Some(pcb.clone());
        } else {
            return None;
        }
    }
}

pub struct ProcessControlBlock {
    // immutable
    pub pid: PidWrapper,
    // mutable
    pub inner: UPSafeCell<ProcessControlBlockInner>,
}

impl ProcessControlBlock {
    fn get_pid(&self) -> usize {
        self.pid.0
    }

    fn new() -> Self {
        // alloctate pid
        let pid = pid_alloc();
        let pid_numer = pid.0;

        let process_control_block = Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    status: ProcessStatus::Ready,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                })
            },
        };

        println!("creat a new process, pid = {}", pid_numer);
        process_control_block
    }
}

pub struct ProcessControlBlockInner {
    pub status: ProcessStatus,
    pub parent: Option<Weak<ProcessControlBlock>>, // Weak reference won't add reference count
    pub children: Vec<Arc<ProcessControlBlock>>, // when the reference cnt of Arc = 0, it will be recycleed. Any task can't exist if there is no reference from parent (except the init task).
    pub exit_code: i32,
}

impl ProcessControlBlockInner {
    fn has_exited(&self) -> bool {
        return self.status == ProcessStatus::Exited;
    }
}

// process status
#[derive(Copy, Clone, PartialEq)]
pub enum ProcessStatus {
    Ready,   // 准备运行 （当程序被加载入内存初始化时，它的初始状态即为Ready）
    Running, // 正在运行
    Exited,  // 已退出
}

//pid
fn pid_alloc() -> PidWrapper {
    PID_ALLOCATOR.exclusive_access().alloc()
}

lazy_static! {
    pub static ref PID_ALLOCATOR: UPSafeCell<PidAllocator> =
        unsafe { UPSafeCell::new(PidAllocator::new()) };
}

pub struct PidAllocator {
    current: usize,
    recycled: Vec<usize>,
}

// a stack allocator to allocate different pid for each task
impl PidAllocator {
    fn new() -> Self {
        PidAllocator {
            current: 1,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> PidWrapper {
        let result: PidWrapper;
        if let Some(pid) = self.recycled.pop() {
            result = PidWrapper(pid);
        } else {
            result = PidWrapper(self.current);
            self.current += 1;
        }
        return result;
    }

    fn dealloc(&mut self, pid: usize) {
        assert!(pid < self.current);
        assert!(
            !self.recycled.iter().any(|ppid| *ppid == pid),
            "pid {} has been deallocated!",
            pid
        );
        self.recycled.push(pid);
    }
}

// a wrapper for pid to implement auto deallocation
pub struct PidWrapper(pub usize);

impl Drop for PidWrapper {
    // enable auto deallocation
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

#[no_mangle]
fn main() -> i32 {
    let mut arg: i32 = 0;
    let mut result1 = 0;
    let mut result2: usize = 0;
    // result: return value of last time
    // arg: argument of this time, for kernel to write in
    let mut service_id = pm_service(result1, result2, &mut arg) as usize;
    loop {
        match service_id {
            PM_INIT => {
                result1 = init();
                result2 = 0;
            }
            PM_FORK => {
                result1 = fork();
                result2 = 0;
            }
            PM_WAITPID => {
                (result1, result2) = waitpid(arg as isize);
            }
            PM_SUSPEND => {
                suspend_current_process();
                result1 = 0;
                result2 = 0;
            }
            PM_EXIT => {
                exit_current_process(arg);
                result1 = 0;
                result2 = 0;
            }
            PM_FETCH => {
                result1 = fetch_ready_process();
                result2 = 0;
            }
            _ => {
                panic!("Unknown service id: {}", service_id);
            }
        }
        service_id = pm_service(result1, result2, &mut arg) as usize;
    }
}
