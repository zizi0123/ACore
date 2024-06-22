use allocator::GlobalBuddyAllocator;
use crate::config::*;

// the memory space for kernel heap, in .bss section
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

//implement global allocator traits to allow use of alloc crate
#[global_allocator]
static KERNEL_HEAP_ALLOCATOR: GlobalBuddyAllocator = unsafe {
    GlobalBuddyAllocator::new(KERNEL_HEAP_SIZE, KERNEL_HEAP_GRANULARITY)
};

#[alloc_error_handler]
/// panic when heap allocation error occurs
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}


pub fn init_heap_allocator() {
    println!("start init kernel heap!");
    unsafe {
        println!("heap start: {:#x}, end: {:#x}", HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE + HEAP_SPACE.as_ptr() as usize);
        KERNEL_HEAP_ALLOCATOR.init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

#[allow(unused)]
pub fn heap_test() {
    println!("start test heap!");

    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);

    let mut v: Vec<usize> = Vec::new();

    

    for i in 0..5 {
        v.push(i);
        println!("push {}", i);
    }


    for i in 0..5 {
        assert_eq!(v[i], i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}


