#![no_std]
extern crate alloc;

mod heap_allocator;
mod config;
mod linked_list;

pub use heap_allocator::GlobalBuddyAllocator;

