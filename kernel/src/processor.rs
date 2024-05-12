use core::arch::asm;

pub fn hartid() -> usize {
    let hartid: usize;
    unsafe { asm!("csrr {0}, mhartid", out(reg)hartid) }
    return hartid;
}
