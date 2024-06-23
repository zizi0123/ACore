use super::sys_yield;
use crate::process::{
    context::TaskContext,
    scheduler::switch_in,
    task_manager::{TaskControlBlock, INIT_TASK},
};
use crate::{mem::page_table, process::scheduler::get_current_task};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use sync::UPSafeCell;

const PM_NONE: usize = 0;
const PM_INIT: usize = 1;
const PM_FORK: usize = 2;
const PM_WAITPID: usize = 3;
const PM_SUSPEND: usize = 4;
const PM_EXIT: usize = 5;
const PM_FETCH: usize = 6;
const BUSY: usize = 1;
const IDLE: usize = 0;

// this is modified by scheduler, to ask for pm service
lazy_static! {
    pub static ref PM_SERVICE: UPSafeCell<PmService> = unsafe { UPSafeCell::new(PmService::new()) };
}
pub struct PmService {
    pub ask_service_id: usize,
    pub result1: isize,
    pub result2: usize,
    pub service_status: usize,
    pub arg: i32,
    pub user_tcb: Arc<TaskControlBlock>, // the user process that ask for pm service's task context
}

impl PmService {
    pub fn new() -> Self {
        Self {
            ask_service_id: PM_NONE,
            service_status: IDLE,
            arg: 0,
            result1: 0,
            result2: 0,
            user_tcb: INIT_TASK.clone(), // random init
        }
    }
}

pub fn process_manager_syscall(result1: isize, result2: usize, arg: *mut i32) -> isize {
    loop {
        let mut pm_service = PM_SERVICE.exclusive_access();
        if pm_service.service_status == BUSY {
            // finish service and return from user
            pm_service.service_status = IDLE;
            pm_service.ask_service_id = PM_NONE;
            pm_service.result1 = result1;
            pm_service.result2 = result2;
            let mut task_inner = pm_service.user_tcb.inner.exclusive_access();
            let task_ctx_ptr = &mut task_inner.task_ctx as *mut TaskContext;

            // switch back to user process, but not yield
            switch_in(task_ctx_ptr);
            continue;
        } else {
            let serice_id = pm_service.ask_service_id;
            match serice_id {
                PM_NONE => {
                    drop(pm_service);
                    sys_yield();
                }
                PM_INIT | PM_FORK | PM_SUSPEND | PM_FETCH => {
                    pm_service.service_status = BUSY;
                    return serice_id as isize;
                }
                PM_WAITPID | PM_EXIT => {
                    pm_service.service_status = BUSY;
                    let current_task = get_current_task();
                    let cur_task_inner = current_task.inner.exclusive_access();
                    page_table::write_into(
                        cur_task_inner.address_space.get_satp(),
                        arg,
                        pm_service.arg,
                    );
                    return serice_id as isize;
                }
                _ => {
                    panic!("Unknown service id: {}", serice_id);
                }
            }
        }
        panic!("unreach able code in process manager service!");
    }
}
