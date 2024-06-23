use crate::mem::page_table::physical_bytes_of_user_ptr;
use crate::sbi;
use crate::process::scheduler::get_current_satp;
use crate::config::{FD_STDOUT, FD_STDIN};

// return the number of bytes written successfully
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDOUT => {
            let buffers = physical_bytes_of_user_ptr(get_current_satp(), buf, len);
            for buffer in buffers {
                print!("{}", core::str::from_utf8(buffer).unwrap());
            }
            len as isize
        },
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}

// only support read from stdin, len = 1
pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    match fd {
        FD_STDIN => {
            assert_eq!(len, 1, "Only support len = 1 in sys_read!");
            let c = sbi::console_getchar();
            let mut bytes = physical_bytes_of_user_ptr(get_current_satp(), buf, len);
            unsafe {
                bytes[0].as_mut_ptr().write_volatile(c); // write to the physical memory of user space
            }
            return 1
        }
        _ => {
            panic!("Unsupported fd in sys_read!");
        }
    }
}