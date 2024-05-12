.section .text.trampoline
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm

.align 2 # risc-v spec requirement
__user_trap:
    csrrw sp, sscratch, sp # swap sp and sscratch, now sp->kernel stack, sscratch->user stack
    addi sp, sp, -34*8 # allocate a TrapContext on kernel stack
    sd x1, 1*8(sp) # save general-purpose registers
    # skip sp(x2), we will save it later
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    csrr t2, sscratch # the sp of user stack
    sd t2, 2*8(sp)
    mv a0, sp  # set argument of trap_handler(cx: &mut TrapContext)
    call trap_handler



    .macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

__user_return:
    # case1: start running app 
    # case2: back to U after call trap_handler
    mv sp, a0 # now sp->kernel stack(after allocated), sscratch->user stack
    # restore sstatus/sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    # restore general-purpuse registers except sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    addi sp, sp, 34*8 # release TrapContext on kernel stack
    csrrw sp, sscratch, sp # now sp->user stack, sscratch->kernel stack
    sret