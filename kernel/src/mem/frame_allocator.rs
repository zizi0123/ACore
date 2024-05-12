use super::page_table::*;
use crate::config::*;
use crate::sync::*;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::lazy_static;

pub trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PPN>;
    fn dealloc(&mut self, ppn: PPN);
}

pub struct StackFrameAllocator {
    start: PPN,
    end: PPN,
    recycled: Vec<PPN>,
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            start: PPN::new(),
            end: PPN::new(),
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Option<PPN> {
        let result: Option<PPN>;
        if let Some(ppn) = self.recycled.pop() {
            result = Some(ppn);
        } else {
            if self.start == self.end {
                println!("FrameAllocator: no available frame!");
                return None;
            } else {
                self.start.0 += 1;
                result = Some(PPN(self.start.0 - 1));
            }
        }
        let ppn = result.unwrap();
        //clear the page
        let page = ppn.get_page();
        for i in 0..4096 {
            page[i] = 0;
        }
        return result;
    }

    fn dealloc(&mut self, ppn: PPN) {
        if ppn >= self.start || self.recycled.iter().find(|&x| *x == ppn).is_some() {
            panic!("Frame ppn = {:#x} has not been allocated!", ppn.0);
        }
        self.recycled.push(ppn);
    }
}

impl StackFrameAllocator {
    pub fn init(&mut self, start: PPN, end: PPN) {
        self.start = start;
        self.end = end;
        println!(
            "FrameAllocator: start ppn:{:#x}, end ppn:{:#x}, total frame number = {}",
            self.start.0,self.end.0,self.end.0 - self.start.0)
    }
}

//this struct tracks a frame's lifetime
pub struct FrameTracker {
    pub ppn: PPN,
}

impl FrameTracker {
    pub fn new(ppn: PPN) -> Self {
        //todo page cleaning  why????
        Self { ppn }
    }
}

//when a FrameTracker is recycled, the drop function will be called by compiler
impl Drop for FrameTracker {
    fn drop(&mut self) {
        FRAME_ALLOCATOR.exclusive_access().dealloc(self.ppn);
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<StackFrameAllocator> =
        unsafe { UPSafeCell::new(StackFrameAllocator::new()) };
}

pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel(); //the end of kernel space get from linker
    }
    let start = (PhyAddr::from(ekernel as usize)).to_up_ppn();
    let end = (PhyAddr::from(MEMORY_END as usize)).to_down_ppn();
    FRAME_ALLOCATOR.exclusive_access().init(start, end);
}

pub fn alloc_frame() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(|ppn| FrameTracker::new(ppn))
}

#[allow(unused)]
/// a simple test for frame allocator
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = alloc_frame().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = alloc_frame().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}
