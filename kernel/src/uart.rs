use crate::config::{FCR, LSR, RBR, THR, UART_BASE};

macro_rules! Reg {
    ($reg:expr) => {
        (UART_BASE + $reg) as *mut u8
    };
}

macro_rules! read_reg {
    ($reg:expr) => {
        unsafe { *Reg!($reg) }
    };
}

macro_rules! write_reg {
    ($reg:expr, $val:expr) => {
        unsafe { *Reg!($reg) = $val }
    };
}

pub fn init() {
    // 禁用 FIFO
    write_reg!(FCR, 0);
}

pub fn console_putchar(c: usize) {
    write_reg!(THR, c as u8);
}

// return zero if no more bytes are present.
pub fn console_getchar() -> u8 {
    let tmp = 0x01;
    if read_reg!(LSR) & tmp == 0{
        return 0;
    }
    return read_reg!(RBR);
}
