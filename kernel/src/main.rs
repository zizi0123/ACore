// os/src/main.rs
#![no_std]  //告诉编译器无需链接标准库 std，仅链接 core 库
#![no_main] //告诉编译器无需完成初始化工作（需依赖于标准库）
#![feature(
    panic_info_message, //通过 PanicInfo::message 获取报错信息
    naked_functions, 
    alloc_error_handler,
    step_trait
)] 

#[macro_use] //引入宏到当前作用域
mod console;

extern crate alloc;

#[macro_use]
extern crate lazy_static;

mod lang_items;
mod sbi;
mod uart;
mod processor;
mod config;
mod mem;
mod sync;
mod syscall;
mod trap;


//将汇编代码 entry.asm 转化为字符串并通过 global_asm! 宏嵌入到代码中
use core::{arch::global_asm, ptr::{read_volatile, write_volatile}};
//使用第三方包 riscv 提供的寄存器定义
use riscv::register::*;
use core::arch::asm;
use config::INTERRUPT_PERIOD;
use crate::mem::{address_space::KERNEL_SPACE, allocator::*, frame_allocator::init_frame_allocator};
use crate::mem::frame_allocator::frame_allocator_test;
use crate::mem::address_space::AddressSpace;

const MTIME :usize = 0x0200bff8;
const MTIMECMP :usize = 0x02004000;

global_asm!(include_str!("entry.asm")); 
global_asm!(include_str!("time_interrupt/handler.asm"));



#[no_mangle]
//switch from machine mode to supervisor mode
unsafe fn rust_start() -> ! {
    //M previous privilege -> supervisor, so we can return to S mode when mret.
    mstatus::set_mpp(mstatus::MPP::Supervisor);
    //M exception pc (the pc when exception occurs, used as return address) -> the entry address of supervisor mode
    mepc::write(rust_main as usize);

    //disable page table temporarily
    satp::write(0);

    //setting a bit in medeleg or mideleg will delegate the corresponding trap to lower privilege level
    let zero:usize = 0xffff;
    asm!("csrw medeleg, {0}",
         "csrw mideleg, {0}",
         in(reg) zero,
        );

    //set S mode interruption enable
    sie::set_sext(); //enable external interrupt
    sie::set_stimer(); //enable time interrupt
    sie::set_ssoft(); //enable soft interrupt

    //physical memory protection
    pmpaddr0::write(0x3fffffffffffff);
    pmpcfg0::write(0xf);
    
    //set time interrupt interval
    let hartid = processor::hartid();
    let mtime = (MTIME) as *mut usize;
    let timenow = read_volatile(mtime);
    let mtimecmp = (MTIMECMP + 8 * hartid) as *mut usize;
    write_volatile(mtimecmp, timenow + INTERRUPT_PERIOD);


    //set the entry address of time interrupt handler
    //function _time_int_handler is defined in handler.s
    //set mode as direct, all exceptions set pc to BASE.
    //attention why the handler func has to be asm?
    extern "C" {
        fn _time_int_handler();
    }
    mtvec::write(_time_int_handler as usize, mtvec::TrapMode::Direct);

    //enable M mode time interrupt
    mstatus::set_mie();

    //enable time interrupt
    mie::set_mtimer();

    //switch to S mode
    asm!("mret", options(noreturn));
}

#[no_mangle] //告诉编译器不要更改函数名称
extern "C" fn rust_main() -> ! {
    clear_bss();
    println!("Hello, world!");
    
    init_heap_allocator();

    // heap_test();

    init_frame_allocator();

    // frame_allocator_test();

    KERNEL_SPACE.exclusive_access().test_space();

    panic!("Shutdown machine!");


}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    });
}
