

use crate::config::BUDDY_ALLOCATOR_ORDER_SIZE;
use crate::mem::linked_list::LinkedList;
use core::alloc::{GlobalAlloc, Layout};
use core::cmp::*;
use core::mem::size_of;
use core::ptr::null_mut;
use crate::sync::UPSafeCell;


struct BuddyAllocator {
    free_list: [LinkedList; BUDDY_ALLOCATOR_ORDER_SIZE],

    user: usize, // the total size user asked for

    allocated: usize, // the total size allocated

    total: usize, // the total size of the memory

    granularity: usize, // the granularity of the memory
}


impl BuddyAllocator{
    pub const fn new(size: usize, gran: usize) -> Self {
        let new_allocator = Self {
            free_list: [LinkedList::new(); BUDDY_ALLOCATOR_ORDER_SIZE],
            user: 0,
            allocated: 0,
            total: size,
            granularity: gran,
        };
        return new_allocator;
    }

    pub unsafe fn init(&mut self, mut start: usize, mut size: usize) {
        start = (start + self.granularity - 1) & (!self.granularity + 1); //make start up aligned to granularity
        size = size & (!self.granularity + 1); //make size down aligned to granularity
        self.total += size;
        let mut order: usize;
        while size > 0 {
            //the order of the memory block to add. start is aligned to 2^order
            order = min(
                prev_power_of_two(size).trailing_zeros() as usize,
                start.trailing_zeros() as usize,
            );
            self.free_list[order].push(start);
            start += 1 << order;
            size -= 1 << order;
        }
    }

    
    unsafe fn split(&mut self, order: usize, target_order: usize) {
        //split the block of target_order until block of order appears
        let mut addr: usize;
        let mut buddy_addr1: usize;
        let mut buddy_addr2: usize;
        let mut new_order = target_order;
        while new_order > order {
            addr = self.free_list[new_order].pop();
            new_order -= 1;
            buddy_addr1 = addr;
            buddy_addr2 = addr + (1 << new_order);
            self.free_list[new_order].push(buddy_addr1);
            self.free_list[new_order].push(buddy_addr2);
        }
    }

    
    unsafe fn merge(&mut self, mut addr: usize, mut order: usize) {
        let mut buddy_addr: usize;
        while order < BUDDY_ALLOCATOR_ORDER_SIZE {
            buddy_addr = addr ^ (1 << order); //address has been aligned to 2^order
            if self.free_list[order].is_empty() {
                self.free_list[order].push(addr);
                return;
            } else {
                if self.free_list[order].search_and_delete(buddy_addr){
                    addr = min(addr, buddy_addr);
                    order += 1;
                } else {
                    self.free_list[order].push(addr);
                    return;
                }
            }
        }
    }

    #[no_mangle]
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let align = max(self.granularity, layout.align());
        let size = max(layout.size().next_power_of_two(), align); //size is a power of 2
        let order = size.trailing_zeros() as usize; //the order of the memory block to allocate
        for i in order..BUDDY_ALLOCATOR_ORDER_SIZE {
            if !self.free_list[i].is_empty() {
                if i > order {
                    self.split(order, i);
                }
                assert!(!self.free_list[order].is_empty());
                self.user += layout.size();
                self.allocated += size;
                let ans = self.free_list[order].pop() as *mut u8;
                return ans;
            }
        }
        return null_mut();
    }


    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let align = max(self.granularity, layout.align());
        let size = max(layout.size().next_power_of_two(), align); //size is a power of 2
        let order = size.trailing_zeros() as usize; //the order of the memory block to allocate
        if self.free_list[order].is_empty() {
            self.free_list[order].push(ptr as usize);
        } else {
            self.merge(ptr as usize, order);
        }
    }

}

pub struct GlobalBuddyAllocator {
    allocator : UPSafeCell<BuddyAllocator>
}

impl GlobalBuddyAllocator {
    pub const unsafe fn new(size: usize, gran: usize) -> Self {
        Self {
            allocator: UPSafeCell::new(BuddyAllocator::new(size, gran)),
        }
    }

    pub unsafe fn init(&self, start: usize, size: usize) {
        self.allocator.exclusive_access().init(start, size);
    }
}

unsafe impl GlobalAlloc for GlobalBuddyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        return self.allocator.exclusive_access().alloc(layout);
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.allocator.exclusive_access().dealloc(ptr,layout);
    }
    
}

fn prev_power_of_two(num: usize) -> usize {
    1 << (8 * (size_of::<usize>()) - num.leading_zeros() as usize - 1)
}