//constants used in ACore

use core::mem::size_of;

pub const MEMORY_END: usize = 0x8800_0000; 

pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
pub const KERNEL_HEAP_GRANULARITY: usize = size_of::<usize>();

pub const INTERRUPT_PERIOD: usize = 1000000;

pub const BUDDY_ALLOCATOR_ORDER_SIZE: usize = 32;

pub const PAGE_SIZE_BITS :usize = 12;
pub const PAGE_SIZE : usize = 1 << PAGE_SIZE_BITS;
pub const PA_WIDTH_SV39 : usize= 56;
pub const VA_WIDTH_SV39 : usize = 39;
pub const PPN_WIDTH_SV39 : usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
pub const VPN_WIDTH_SV39 : usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;

pub const TRAMPOLINE_VA: usize = !0 - PAGE_SIZE + 1;

