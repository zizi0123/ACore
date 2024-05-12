const UART_BASE: u64 = 0x1000_0000;
const RBR: u64 = 0x00;
const THR: u64 = 0x00;
const IER: u64 = 0x01;
const FCR: u64 = 0x02;


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

pub fn console_getchar() -> u8 {
    return read_reg!(RBR);
}
