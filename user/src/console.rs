const STDIN: usize = 0;
const STDOUT: usize = 1;

use crate::syscall::{sys_read, sys_write};


pub extern "C" fn putchar(c: u8) {
    sys_write(STDOUT, &[c]);
}

pub extern "C" fn getchar() -> u8 {
    let mut buf = [0u8; 1];
    sys_read(STDIN, &mut buf);
    buf[0]
}