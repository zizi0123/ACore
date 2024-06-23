#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod lang_item;
mod syscall;
mod file;
mod config;

use syscall::*;
use config::*;
use allocator::GlobalBuddyAllocator;

extern crate alloc;

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

// implement global allocator traits to enable use of alloc crate
#[global_allocator]
static USER_HEAP_ALLOCATOR: GlobalBuddyAllocator = unsafe {
    GlobalBuddyAllocator::new(USER_HEAP_SIZE, USER_HEAP_GRANULARITY)
};

#[alloc_error_handler]
// panic when heap allocation error occurs
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    init_heap_allocator();
    exit(main());
    panic!("unreachable after sys_exit!");
}

#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    println!("Hello, world!");
    panic!("Cannot find main!");
}

fn init_heap_allocator() {
    println!("start init user heap!");
    unsafe {
        println!("heap start: {:x}, size: {:x}", HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
        USER_HEAP_ALLOCATOR.init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
}

pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}
pub fn yield_() -> isize {
    sys_yield()
}
pub fn get_time() -> isize {
    sys_get_time()
}

pub fn sleep(time: usize) {
    let start_time = sys_get_time() as usize;
    while sys_get_time() as usize - start_time < time {
        sys_yield();
    }
}

pub fn sbrk(size: i32) -> isize {
    sys_sbrk(size)
}

pub fn getpid() -> isize {
    sys_getpid()
}
pub fn fork() -> isize {
    sys_fork()
}
pub fn exec(path: &str) -> isize {
    sys_exec(path)
}

// wait for any child process to exit
pub fn wait(exit_code: &mut i32) -> isize {
    loop { // busy waiting
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => { // child process is still running
                yield_();
            }
            exit_pid => return exit_pid,
        }
    }
}

// wait for a specific child process to exit
pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop { // busy waiting
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => { // child process is still running
                yield_();
            }
            exit_pid => return exit_pid,
        }
    }
}

pub fn pm_service(result1: isize, result2: usize, arg: &mut i32) -> isize {
    return sys_pm_service(result1, result2, arg as *mut _);
}


