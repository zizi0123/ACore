//!Implementation of [`PidAllocator`]
use crate::config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE_START_VA};
use crate::mem::address_space::{MapType, SectionPermisson, KERNEL_SPACE};
use crate::mem::page_table::VirtAddr;

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE_START_VA - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

// the position of kernel stack in kernel space was specified by pid
pub struct KernelStack {
    pid: usize,
}

impl KernelStack {
    // allocate a kernel stack for a task
    pub fn new(pid: usize) -> Self {
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(pid);
        KERNEL_SPACE.exclusive_access().add_section(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            SectionPermisson::R | SectionPermisson::W,
            MapType::Framed,
            None,
        );
        KernelStack { pid }
    }

    #[allow(unused)]
    //Push a value on top of kernelstack
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }

    //Get the value on the top of kernelstack
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.pid);
        kernel_stack_top
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.pid);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .delete_section(kernel_stack_bottom_va.into());
    }
}
