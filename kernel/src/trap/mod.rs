mod context;

use crate::{
    config::{RED, RESET, TRAMPOLINE_START_VA, TRAP_CONTEXT_START_VA},
    syscall::syscall,
    process::scheduler::{exit_current_and_run_next, get_current_satp, get_current_trap_ctx, suspend_current_and_run_next},
};
pub use context::TrapContext;
use core::arch::{asm, global_asm};
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    stval, stvec, sip,
};

global_asm!(include_str!("trap.asm"));

pub fn init() {
    extern "C" {
        fn __user_trap();
    }
    unsafe {
        stvec::write(__user_trap as usize, TrapMode::Direct); // Set the trap handler address
    }
}

#[no_mangle]
pub fn trap_handler() -> ! {
    // now we're in the kernel process a trap. if another trap happens, we'll jump to trap_from_kernel.
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }

    let scause = scause::read();
    let stval = stval::read();
    let trap_ctx = get_current_trap_ctx();

    // when exception or interrupt happens, CPU will write the cause to scause register
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // set the pc to the next instruction of ecall
            trap_ctx.sepc += 4; 
            // set a0 = result from syscall
            let result = syscall(
                trap_ctx.x[17],
                [trap_ctx.x[10], trap_ctx.x[11], trap_ctx.x[12]],
            ) as usize; 

            // trap context will change after exec 
            let current_trap_ctx = get_current_trap_ctx();
            current_trap_ctx.x[10] = result;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault) => {
            println!("{}[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.{}", RED, stval, trap_ctx.sepc, RESET);
            exit_current_and_run_next(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("{}[kernel] IllegalInstruction in application, kernel killed it.{}", RED, RESET);
            exit_current_and_run_next(-3);
        }
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            // clear the soft interrupt bit in sip
            let sip = sip::read().bits();
                unsafe {
                    asm! {"csrw sip, {sip}", sip = in(reg) sip ^ 2};
                }
            println!("{}[kernel] Time Interrupt occured, switch to next app.{}", RED, RESET);
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    trap_return();
}

#[no_mangle]
// when an app is loaded in the first time, after __switch to this app, kernel will jump to this function.

// set stvec -> trampoline
// get satp and trap context of user process
// __switch(trap_ctx_va, user_satp)
pub fn trap_return() -> ! {
    unsafe {
        stvec::write(TRAMPOLINE_START_VA as usize, TrapMode::Direct);
    }
    let trap_ctx_va = TRAP_CONTEXT_START_VA; // the trap context va in user space
    let user_satp = get_current_satp();
    extern "C" {
        fn __user_trap();
        fn __user_return();
    }
    // get the va of __user_return in user and kernel space
    let return_va = __user_return as usize - __user_trap as usize + TRAMPOLINE_START_VA;
    unsafe {
        asm!(
            "fence.i", // clear instruction cache
            "jr {return_va}", // jump to __user_return
            return_va = in(reg) return_va,
            in("a0") trap_ctx_va,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of usr page table
            options(noreturn)
        );
    }
}

#[no_mangle]
    pub fn trap_from_kernel() -> ! {
    panic!("a trap from kernel!");
}
