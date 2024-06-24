use super::context::TaskContext;
use super::switch::__switch;
use super::task_manager::{get_task, TaskControlBlock, INIT_TASK};
use crate::process::loader::open_app_file;
use crate::sbi;
use crate::syscall::process_manager::PM_SERVICE;
use crate::trap::TrapContext;

use alloc::sync::Arc;
use sync::UPSafeCell;

const PROCESS_MANAGER_PID: usize = 0;

const PM_FORK: usize = 2;
const PM_WAITPID: usize = 3;
const PM_SUSPEND_AND_RUN_NEXT: usize = 4;
const PM_EXIT_AND_RUN_NEXT: usize = 5;
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

pub fn switch_in(new_task: Arc<TaskControlBlock>) {
    let mut scheduler = SCHEDULER.exclusive_access();
    let cur_task = scheduler.take_current().unwrap();
    scheduler.current = Some(new_task.clone());
    let mut cur_task_inner = cur_task.inner.exclusive_access();
    let cur_task_ctx_ptr = &mut cur_task_inner.task_ctx as *mut TaskContext;
    let new_task_inner = new_task.inner.exclusive_access();
    let new_task_ctx_ptr = &new_task_inner.task_ctx as *const TaskContext;
    drop(cur_task_inner);
    drop(new_task_inner);
    drop(scheduler);
    unsafe {
        __switch(cur_task_ctx_ptr, new_task_ctx_ptr);
    }
}

pub fn switch_in_pid(pid: usize) {
    let new_task = get_task(pid).unwrap();
    switch_in(new_task);
}

// build SCHEUDLER, and start the loop
// continuously fetch a ready task, and switch from empty task to it.
// when a task was switched out, the empty task will be switched in, and start to run this function, begin the next loop.
pub fn start_schedule() {
    // initially, switch to INIT task
    let mut scheduler = SCHEDULER.exclusive_access();
    let task = INIT_TASK.clone();
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
    let next_pid = suspend_current_and_run_next_process() as usize;
    switch_in_pid(next_pid);
}

// tcb was not cleared when exit. parent process may access it to get exit code. it was clear in waitpid.
pub fn exit_current_and_run_next(exit_code: i32) {
    let scheduler = SCHEDULER.exclusive_access();
    let cur_task = scheduler.get_current().unwrap();
    drop(scheduler);

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

    // release resources
    let mut cur_task_inner = cur_task.inner.exclusive_access();
    cur_task_inner.address_space.clear();
    drop(cur_task_inner); 

    // switch to next task
    let next_pid = exit_current_and_run_next_process(exit_code) as usize;
    switch_in_pid(next_pid);
}

pub fn get_current_satp() -> usize {
    let scheduler = SCHEDULER.exclusive_access();
    let cur_task = scheduler.get_current().unwrap();
    drop(scheduler);
    let cur_task_inner = cur_task.inner.exclusive_access();
    let satp = cur_task_inner.address_space.get_satp();
    drop(cur_task_inner);
    return satp;
}

pub fn get_current_trap_ctx() -> &'static mut TrapContext {
    let scheduler = SCHEDULER.exclusive_access();
    let cur_task = scheduler.get_current().unwrap();
    drop(scheduler);
    let cur_task_inner = cur_task.inner.exclusive_access();
    let trap_ctx = cur_task_inner.get_trap_ctx();
    drop(cur_task_inner);
    return trap_ctx;
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
    let mut pm_service = PM_SERVICE.exclusive_access();
    pm_service.ask_service_id = service_id;
    pm_service.arg = arg;
    pm_service.user_tcb = Some(get_current_task());
    drop(pm_service);
    switch_in(PROCESS_MANAGER.clone());
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

pub fn suspend_current_and_run_next_process() -> isize {
    call_pm_service(PM_SUSPEND_AND_RUN_NEXT, 0);
    let pid = PM_SERVICE.exclusive_access().result1;
    return pid;
}

pub fn exit_current_and_run_next_process(exit_code: i32) -> isize {
    call_pm_service(PM_EXIT_AND_RUN_NEXT, exit_code);
    let pid = PM_SERVICE.exclusive_access().result1;
    return pid;
}

pub fn fetch_ready_task() -> Option<Arc<TaskControlBlock>> {
    call_pm_service(PM_FETCH, 0);
    let pid = PM_SERVICE.exclusive_access().result1;
    return get_task(pid as usize);
}
