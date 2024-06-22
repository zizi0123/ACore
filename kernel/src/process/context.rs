use crate::trap::trap_return;


#[repr(C)]
#[derive(Clone, Copy)]
// save status of the task when it is switched out
pub struct TaskContext {
    /// return address when the task is switched back
    ra: usize,
    /// kernel stack sp of task
    sp: usize,
    /// callee saved registers:  s 0..11
    s: [usize; 12],
}

impl TaskContext {
    // init task context, set kernel stack of an new app
    // set ra = trap_return, when start running the app, it will jump to trap_return to get into U mode after switch.
    pub fn new(sp: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp,
            s: [0; 12],
        }
    }

    // return a task context will all registers set to 0, for use in __switch when the first app is started
    pub fn empty() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }


    
}
