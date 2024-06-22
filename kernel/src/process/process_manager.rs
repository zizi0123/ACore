use alloc::collections::VecDeque;
use alloc::sync::Arc;
use sync::UPSafeCell;
use crate::config::{GREEN, RESET};

use super::process_control_block::{ProcessControlBlock, INITPROC};
pub struct ProcessManager {
    ready_tasks_queue : VecDeque<Arc<ProcessControlBlock>>,
}
                                                                                                                                                                                                                                                                                          
impl ProcessManager {
    fn new() -> Self {
        Self {
            ready_tasks_queue: VecDeque::new(),
        }
    }

    fn add_ready_task(&mut self, task: Arc<ProcessControlBlock>) {
        self.ready_tasks_queue.push_back(task);
    }

    fn fetch_ready_task(&mut self) -> Option<Arc<ProcessControlBlock>> {
        self.ready_tasks_queue.pop_front()
    }
}

// init TASK_MANAGER, get total number of tasks, and create TaskControlBlock for each task
lazy_static! {
    pub static ref PROCESS_MANAGER: UPSafeCell<ProcessManager> =
        unsafe { UPSafeCell::new(ProcessManager::new()) };
}

pub fn init() {
    add_task(INITPROC.clone());
    println!("{}init process has been build and added to PM{}", GREEN, RESET);
}

pub fn add_task(task: Arc<ProcessControlBlock>) {
    PROCESS_MANAGER.exclusive_access().add_ready_task(task);
}

pub fn fetch_ready_task() -> Option<Arc<ProcessControlBlock>> {
    return PROCESS_MANAGER.exclusive_access().fetch_ready_task();
}





