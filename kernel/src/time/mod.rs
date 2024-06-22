use core::arch::asm;
use crate::config::{CLINT_MTIMECMP, CLOCK_FREQ, INTERRUPT_PERIOD};
use crate::sbi;
use riscv::register::*;

pub fn get_time() -> usize {
    return sbi::get_time();
}

/// get current time in microseconds
pub fn get_time_ms() -> usize {
    return time::read() / (CLOCK_FREQ / 1000);
}

/// set the next timer interrupt
pub fn set_next_trigger() {
    sbi::set_timer(get_time() + INTERRUPT_PERIOD);
}

static mut TIMER_SCRATCH: [usize; 5] = [0; 5];

#[no_mangle]
pub unsafe fn init() {
    set_next_trigger();
    
    // save timer_scratch pointer
    mscratch::write(&TIMER_SCRATCH as *const _ as usize);

    //set the entry address of M-mode time interrupt handler
    //set mode as direct, all exceptions set pc to BASE.
    mtvec::write(_time_int_handler as usize, mtvec::TrapMode::Direct);

    //enable M mode time interrupt
    mstatus::set_mie();

    //enable time interrupt
    mie::set_mtimer();
}


#[no_mangle]
pub extern "C" fn _time_int_handler() {
    unsafe {
        asm!(r#"
        .align 2
        
        # store the registers
        csrrw sp, mscratch, sp # now sp -> timer_scrath
        sd t0, 0(sp)
        sd t1, 8(sp)
        sd t2, 16(sp)

        # set next time
        mv t0, {mtimecmp}
        ld t1, 0(t0) # t1 = mtimecmp = current time
        mv t2, {interval}
        add t1, t1, t2
        sd t1, 0(t0) # t1 = next trigger

        # delegate to S-mode to do the remaining work: switch to next task
        li t0, 2
        csrw sip, t0

        # restore
        ld t0, 0(sp)
        ld t1, 8(sp)
        ld t2, 16(sp)
        csrrw sp, mscratch, sp

        mret
        "#, 
        mtimecmp = in(reg) CLINT_MTIMECMP,
        interval = in(reg) INTERRUPT_PERIOD,
        options(noreturn))
    }
}
