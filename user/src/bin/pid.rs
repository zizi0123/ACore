#![no_std]
#![no_main]

extern crate user_lib;
extern crate alloc;

use alloc::vec::Vec;
use lazy_static::*;
use sync::UPSafeCell;

pub fn pid_alloc() -> PidWrapper {
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
    pub fn new() -> Self {
        PidAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> PidWrapper {
        let result: PidWrapper;
        if let Some(pid) = self.recycled.pop() {
            result = PidWrapper(pid);
        } else {
            result = PidWrapper(self.current);
            self.current += 1;
        }
        return result;
    }

    pub fn dealloc(&mut self, pid: usize) {
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






