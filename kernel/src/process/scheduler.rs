use super::context::TaskContext;
use super::switch::__switch;
use super::task_manager::{
    get_task, TaskControlBlock, INIT_TASK,
};
use crate::process::loader::open_app_file;
use crate::sbi;
use crate::syscall::process_manager::PM_SERVICE;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use sync::UPSafeCell;

const PROCESS_MANAGER_PID: usize = 0;

const PM_INIT: usize = 1;
const PM_FORK: usize = 2;
const PM_WAITPID: usize = 3;
const PM_SUSPEND: usize = 4;
const PM_EXIT: usize = 5;
const PM_FETCH: usize = 6;

pub struct Scheduler {
    current: Option<Arc<TaskControlBlock>>,
    empty_task_ctx: TaskContext, // this is an empty task context for switch
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            current: None,
            empty_task_ctx: TaskContext::empty(),
        }
    }

    fn get_empty_task_ctx_ptr(&mut self) -> *mut TaskContext {
        &mut self.empty_task_ctx as *mut TaskContext
    }

    fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        return self.current.take();
    }

    fn get_current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref SCHEDULER: UPSafeCell<Scheduler> = unsafe { UPSafeCell::new(Scheduler::new()) };
}

fn switch_out(switched_task_ctx_ptr: *mut TaskContext) {
    let mut scheduler = SCHEDULER.exclusive_access();
    let empty_task_ctx_ptr = scheduler.get_empty_task_ctx_ptr();
    drop(scheduler);
    unsafe {
        __switch(switched_task_ctx_ptr, empty_task_ctx_ptr);
    }
}

pub fn switch_in(switched_task_ctx_ptr: *mut TaskContext) {
    let cur_task = SCHEDULER.exclusive_access().take_current().unwrap();
    let mut cur_task_inner = cur_task.inner.exclusive_access();
    let cur_task_ctx_ptr = &mut cur_task_inner.task_ctx as *mut TaskContext;
    drop(cur_task_inner);
    unsafe {
        __switch(cur_task_ctx_ptr, switched_task_ctx_ptr);
    }
}

// build SCHEUDLER, and start the loop
// continuously fetch a ready task, and switch from empty task to it.
// when a task was switched out, the empty task will be switched in, and start to run this function, begin the next loop.
pub fn start_schedule() {
    loop {
        let mut scheduler = SCHEDULER.exclusive_access();
        if let Some(task) = fetch_ready_task() {
            let empty_task_ctx_ptr = scheduler.get_empty_task_ctx_ptr();
            // access coming task TCB exclusively
            let task_inner = task.inner.exclusive_access();
            let next_task_ctx_ptr = &task_inner.task_ctx as *const TaskContext;
            drop(task_inner);
            // release coming task TCB manually
            scheduler.current = Some(task);
            // release scheduler manually
            drop(scheduler);

            unsafe {
                __switch(empty_task_ctx_ptr, next_task_ctx_ptr);
            }
        }
    }
}

pub fn suspend_current_and_run_next() {
    let cur_task = SCHEDULER.exclusive_access().take_current().unwrap();
    let mut task_inner = cur_task.inner.exclusive_access();
    let cur_task_ctx_ptr = &mut task_inner.task_ctx as *mut TaskContext;
    drop(task_inner);
    suspend_current_process();
    switch_out(cur_task_ctx_ptr);
}

// tcb was not cleared when exit. parent process may access it to get exit code. it was clear in waitpid.
pub fn exit_current_and_run_next(exit_code: i32) {
    let cur_task = SCHEDULER.exclusive_access().take_current().unwrap();
    let pid = cur_task.get_pid();

    // maybe the empty task is exiting, we should shutdown the system
    if pid == INIT_TASK.get_pid() {
        println!(
            "[kernel] Init process exit with exit_code {} ...",
            exit_code
        );
        if exit_code != 0 {
            sbi::shutdown(true)
        } else {
            sbi::shutdown(false)
        }
    }

    exit_current_process(exit_code);
    let mut task_inner = cur_task.inner.exclusive_access();
    // release resources
    task_inner.address_space.clear();

    drop(task_inner);
    drop(cur_task); // drop task manually to maintain rc correctly

    // current task has been exited, use a new task to switch from
    let mut new_empty = TaskContext::empty();
    switch_out(&mut new_empty as *mut _);
}

pub fn get_current_satp() -> usize {
    return SCHEDULER
        .exclusive_access()
        .get_current()
        .unwrap()
        .inner
        .exclusive_access()
        .address_space
        .get_satp();
}

pub fn get_current_trap_ctx() -> &'static mut TrapContext {
    return SCHEDULER
        .exclusive_access()
        .get_current()
        .unwrap()
        .inner
        .exclusive_access()
        .get_trap_ctx();
}

pub fn get_current_task() -> Arc<TaskControlBlock> {
    return SCHEDULER.exclusive_access().get_current().unwrap();
}

pub fn change_program_brk(size: i32) -> Option<usize> {
    return SCHEDULER
        .exclusive_access()
        .get_current()
        .unwrap()
        .inner
        .exclusive_access()
        .change_program_brk(size);
}

pub fn get_pid() -> usize {
    return SCHEDULER
        .exclusive_access()
        .get_current()
        .unwrap()
        .get_pid();
}

lazy_static! {
    pub static ref PROCESS_MANAGER: Arc<TaskControlBlock> = Arc::new({
        let data = open_app_file("process_manager").unwrap();
        let pid = PROCESS_MANAGER_PID as usize; // this pid won't be allocated to other processes
        TaskControlBlock::new(data, pid)
    });
}

// the process asked for pm service, initialized randomly

fn call_pm_service(service_id: usize, arg: i32) {
    let mut pm_task_inner = PROCESS_MANAGER.inner.exclusive_access();
    let pm_task_ctx_ptr: *mut TaskContext = &mut pm_task_inner.task_ctx as *mut TaskContext;
    PM_SERVICE.exclusive_access().ask_service_id = service_id;
    PM_SERVICE.exclusive_access().arg = arg;
    PM_SERVICE.exclusive_access().user_tcb = get_current_task();
    switch_in(pm_task_ctx_ptr);
}
    

// build the INIT_PROCESS in pm, and return the pid of it.
pub fn init_process() -> usize {
    call_pm_service(PM_INIT, 0);
    return PM_SERVICE.exclusive_access().result1 as usize; // the pid of INIT_PROCESS
}

pub fn fork_process() -> usize {
    call_pm_service(PM_FORK, 0);
    return PM_SERVICE.exclusive_access().result1 as usize; // the pid of child process
}

pub fn waitpid_process(pid: isize) -> (isize, usize) {
    call_pm_service(PM_WAITPID, pid as i32);
    let pid = PM_SERVICE.exclusive_access().result1;
    let exit_code = PM_SERVICE.exclusive_access().result2;
    return (pid, exit_code);
}

pub fn suspend_current_process() {
    call_pm_service(PM_SUSPEND, 0);
}

pub fn exit_current_process(exit_code: i32) {
    call_pm_service(PM_EXIT, exit_code);
}

pub fn fetch_ready_task() -> Option<Arc<TaskControlBlock>> {
    call_pm_service(PM_FETCH, 0);
    let pid = PM_SERVICE.exclusive_access().result1;
    return get_task(pid as usize);
}
