pub mod allocator;
pub mod page_table;
pub mod frame_allocator;
pub mod address_space;

use frame_allocator::{frame_allocator_test,init_frame_allocator};
use address_space::{KERNEL_SPACE,test_space};
use allocator::{init_heap_allocator,heap_test};

pub fn init() {

    init_heap_allocator();

    heap_test();

    init_frame_allocator();

    frame_allocator_test();

    KERNEL_SPACE.exclusive_access().activate(); // use SV39 mode and set the root ppn into satp register

    test_space();

}