use alloc::sync::Arc;
use sync::UPSafeCell;
use crate::trap::TrapContext;
use crate::sbi;
use super::switch::__switch;
use super::process_manager::{fetch_ready_task, add_task};
use super::{context::TaskContext, process_control_block::{ProcessControlBlock, ProcessStatus, INITPROC}};

const EMPTY_PID: usize = 0;

pub struct Scheduler {
    current: Option<Arc<ProcessControlBlock>>,
    empty_task_ctx: TaskContext, // this is an empty task context for switch
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            current: None,
            empty_task_ctx: TaskContext::empty(),
        }
    }

    fn get_empty_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.empty_task_ctx as *mut TaskContext
    }

    fn take_current(&mut self) -> Option<Arc<ProcessControlBlock>> {
        return self.current.take();
    }

    fn get_current(&self) -> Option<Arc<ProcessControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref SCHEDULER: UPSafeCell<Scheduler> = unsafe { UPSafeCell::new(Scheduler::new()) };
}

fn switch_out(switched_task_cx_ptr: *mut TaskContext) {
    let mut scheduler = SCHEDULER.exclusive_access();
    let idle_task_cx_ptr = scheduler.get_empty_task_cx_ptr();
    drop(scheduler);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

// build SCHEUDLER, and start the loop
// continuously fetch a ready task, and switch from empty task to it.
// when a task was switched out, the empty task will be switched in, and start to run this function, begin the next loop. 
pub fn init() {
    loop {
        let mut scheduler = SCHEDULER.exclusive_access();
        if let Some(task) = fetch_ready_task() {
            let idle_task_cx_ptr = scheduler.get_empty_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner.exclusive_access();
            let next_task_cx_ptr = &task_inner.task_ctx as *const TaskContext;
            task_inner.status = ProcessStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            scheduler.current = Some(task);
            // release scheduler manually
            drop(scheduler);


            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}

pub fn suspend_current_and_run_next() {
    let task = SCHEDULER.exclusive_access().take_current().unwrap();
    let mut task_inner = task.inner.exclusive_access();
    let task_cx_ptr = &mut task_inner.task_ctx as *mut TaskContext;
    task_inner.status = ProcessStatus::Ready;
    drop(task_inner);
    add_task(task); // add it back to ready queue
    switch_out(task_cx_ptr);
}

pub fn exit_current_and_run_next(exit_code: i32) {
    let cur_pcb = SCHEDULER.exclusive_access().take_current().unwrap();
    let pid = cur_pcb.get_pid();

    // maybe the empty task is exiting, we should shutdown the system
    if pid == EMPTY_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        if exit_code != 0 {
            sbi::shutdown(true)
        } else {
            sbi::shutdown(false)
        }
    }

    // mark the task as exited
    let mut cur_pcb_inner = cur_pcb.inner.exclusive_access();
    cur_pcb_inner.status = ProcessStatus::Exited;
    cur_pcb_inner.exit_code = exit_code;

    // add parent of current task's children to INITPROC
    let mut initproc_inner = INITPROC.inner.exclusive_access();
    for child in cur_pcb_inner.children.iter() {
        child.inner.exclusive_access().parent = Some(Arc::downgrade(&INITPROC)); // downgrade to Weak, won't add ref count
        initproc_inner.children.push(child.clone()); // clone to add ref count
    }
    drop(initproc_inner);

    // release resources
    cur_pcb_inner.children.clear();
    cur_pcb_inner.address_space.clear();

    drop(cur_pcb_inner);
    drop(cur_pcb); // drop task manually to maintain rc correctly

    // switch from a 'new' empty to the 'special' empty process
    let mut new_empty = TaskContext::empty();
    switch_out(&mut new_empty as *mut _);
}

pub fn get_current_satp() -> usize {
    return SCHEDULER.exclusive_access().get_current().unwrap().inner.exclusive_access().address_space.get_satp();
}

pub fn get_current_trap_ctx() -> &'static mut TrapContext {
    return SCHEDULER.exclusive_access().get_current().unwrap().inner.exclusive_access().get_trap_ctx();
}

pub fn get_current_task() -> Arc<ProcessControlBlock> {
    return SCHEDULER.exclusive_access().get_current().unwrap();
}

pub fn change_program_brk(size: i32) -> Option<usize> {
    return SCHEDULER.exclusive_access().get_current().unwrap().inner.exclusive_access().change_program_brk(size)
}

pub fn get_pid() -> usize {
    return SCHEDULER.exclusive_access().get_current().unwrap().get_pid();
}