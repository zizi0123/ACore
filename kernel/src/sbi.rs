// os/src/sbi.rs
// 用于内核与 RustSBI 通信

use crate::uart;

//向控制台输出单字符
pub fn console_putchar(c: usize) {
    uart::console_putchar(c);
}

const VIRT_TEST: usize = 0x100000;

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
