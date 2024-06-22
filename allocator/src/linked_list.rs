use core::ptr::null_mut;

#[derive(Clone, Copy)]
pub struct LinkedList {
    pub head: *mut usize,
}

unsafe impl Send for LinkedList {}

impl LinkedList {
    pub const fn new() -> Self {
        LinkedList { head: null_mut() }
    }

    pub unsafe fn push(&mut self, addr: usize) {
        *(addr as *mut usize) = self.head as usize;
        self.head = addr as *mut usize;
    }

    //attention when pop is called, the caller should ensure the list is not empty
    pub unsafe fn pop(&mut self) -> usize {
        if self.head.is_null() {
            panic!("pop from an empty linked list");
        }
        let addr = self.head as usize;
        self.head = *self.head as *mut usize;
        return addr;
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    pub unsafe fn search_and_delete(&mut self, addr: usize) -> bool {
        let mut p = self.head;
        if p == null_mut() {
            return false;
        }
        if p as usize == addr {
            self.head = *p as *mut usize;
            return true;
        }
        while p != null_mut() && *p as usize != addr {
            p = *p as *mut usize;
        }
        if p == null_mut() {
            return false;
        }
        *p = *(addr as *mut usize);
        return true;
    }
}
