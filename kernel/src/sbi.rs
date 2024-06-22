// os/src/sbi.rs
// 用于内核与 RustSBI 通信

use core::ptr::write_volatile;

use crate::uart;
use crate::config::{CLINT_MTIME, CLINT_MTIMECMP, VIRT_TEST};

//向控制台输出单字符
pub fn console_putchar(c: usize) {
    uart::console_putchar(c);
}

pub fn console_getchar() -> u8 {
    loop{
        let c = uart::console_getchar();
        if c != 0 {
            return c;
        }
    }
}

pub fn shutdown(failure: bool) -> ! {
    unsafe {
        if failure {
            *(VIRT_TEST as *mut u32) = 0x5555;
        }else{
            *(VIRT_TEST as *mut u32) = 0x3333;
        }
    };
    unreachable!()
}


// 设置 mtimecmp 寄存器
pub fn set_timer(time: usize) {
    unsafe {
        write_volatile(CLINT_MTIMECMP as *mut usize, time);
    }
}

// 获取当前系统时间
pub fn get_time() -> usize {
    unsafe {
        let time = CLINT_MTIME as *const usize;
        time.read_volatile()
    }
}


