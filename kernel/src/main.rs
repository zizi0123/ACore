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
mod config;
mod mem;
mod syscall;
mod trap;
mod process;
mod time;


//将汇编代码 entry.asm 转化为字符串并通过 global_asm! 宏嵌入到代码中
use core::arch::global_asm;
//使用第三方包 riscv 提供的寄存器定义
use riscv::register::*;
use process::{process_manager, scheduler};
use core::arch::asm;
use process::loader;


global_asm!(include_str!("entry.asm")); 
// global_asm!(include_str!("time/handler.asm"));
global_asm!(include_str!("link_app.S"));


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
    //sstatus.sie is not set, so trap can not happen in S-mode, but can interrupt into S-mode when in U-mode. 
    sie::set_sext(); //enable external interrupt
    sie::set_stimer(); //enable time interrupt
    sie::set_ssoft(); //enable soft interrupt

    //physical memory protection
    pmpaddr0::write(0x3fffffffffffff);
    pmpcfg0::write(0xf);
    
    // time::init();

    //switch to S mode
    asm!("mret", options(noreturn));
}

#[no_mangle] //告诉编译器不要更改函数名称
extern "C" fn rust_main() -> ! {
    // .bss 段用于存放未初始化的全局变量，在程序开始运行前需要由操作系统清零
    clear_bss();

    println!("[kernel] Hello, world!");

    trap::init();

    mem::init();

    process_manager::init();

    loader::init();

    scheduler::init();

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
