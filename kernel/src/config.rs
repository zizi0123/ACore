//constants used in ACore

use core::mem::size_of;

pub const UART_BASE: usize = 0x1000_0000;
pub const RBR: usize = 0x00;
pub const THR: usize = 0x00;
pub const IER: usize = 0x01;
pub const FCR: usize = 0x02;
pub const LSR: usize = 0x05; 

pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
pub const KERNEL_HEAP_GRANULARITY: usize = size_of::<usize>();

pub const PAGE_SIZE_BITS :usize = 12;
pub const PAGE_SIZE : usize = 1 << PAGE_SIZE_BITS;
pub const PA_WIDTH_SV39 : usize= 56;
pub const VA_WIDTH_SV39 : usize = 39;

pub const TRAMPOLINE_START_VA: usize = !0 - PAGE_SIZE + 1;
pub const TRAP_CONTEXT_START_VA: usize = TRAMPOLINE_START_VA - PAGE_SIZE;

pub const USER_STACK_SIZE: usize = 4096 * 8;
pub const KERNEL_STACK_SIZE: usize = 4096 * 8;

pub const MM_DERICT_MAP: &[(usize, usize)] = &[
    (VIRT_TEST, 0x00_2000), // VIRT_TEST/RTC  in virt machine
    (UART_BASE, 0x1000)
];

pub const INTERRUPT_PERIOD: usize = 5000000;

pub const CLINT: usize = 0x200_0000;
pub const CLINT_MTIMECMP: usize = CLINT + 0x4000;
pub const CLINT_MTIME: usize = CLINT + 0xBFF8;



pub const FD_STDIN: usize = 0;
pub const FD_STDOUT: usize = 1;


// QEMU config
pub const VIRT_TEST: usize = 0x10_0000;
pub const CLOCK_FREQ: usize = 12500000;
pub const MEMORY_END: usize = 0x8800_0000;

pub const GREEN : &str = "\x1b[32m";
pub const RED : &str = "\x1b[31m";
pub const RESET: &str = "\x1b[0m"; // 重置颜色设置
