use crate::mem::page_table::get_bytes;
use crate::sbi;
use crate::process::scheduler::{get_current_satp, suspend_current_and_run_next};
use crate::config::{FD_STDOUT, FD_STDIN};

// return the number of bytes written successfully
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDOUT => {
            let buffers = get_bytes(get_current_satp(), buf, len);
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
            let mut c: u8;
            loop { // busy waiting until a byte is read
                c = sbi::console_getchar();
                if c == 0 { // no input
                    panic!("No input in sys_read!");
                    suspend_current_and_run_next();
                    continue;
                } else {
                    break;
                }
            }
            let mut buffers = get_bytes(get_current_satp(), buf, len);
            unsafe {
                buffers[0].as_mut_ptr().write_volatile(c);
            }
            return 1
        }
        _ => {
            panic!("Unsupported fd in sys_read!");
        }
    }
}