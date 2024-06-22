# TRAMPOLINE

.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

    .section .text.trampoline
    .globl __user_trap
    .globl __user_return
    .align 2 # risc-v spec requirement
    # When an app traps into kernel, hardware will jump to here. (by stvec)
__user_trap:
    # swap sp and sscratch, now sp -> trap context, sscratch -> user stack
    # sp -> the bottom of user's kernel stack, the start of trap context
    csrrw sp, sscratch, sp 
    # save general registers to stack
    sd x1, 1*8(sp) 
    sd x3, 3*8(sp) # skip x2(sp)
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr

    # save sstatus, sepc, sscratch to stack
    csrr t0, sstatus
    csrr t1, sepc
    csrr t2, sscratch 
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    sd t2, 2*8(sp)

    # (the following satp, trap_handler, kernel_sp are stored in trap context when building this task's task ctrl block)
    # load kernel_satp into t0 
    ld t0, 34*8(sp)
    # load trap_handler into t1
    ld t1, 36*8(sp)
    # sp -> kernel_sp in kernel space
    ld sp, 35*8(sp)
    # switch to kernel space
    csrw satp, t0
    sfence.vma
    # jump to trap_handler
    jr t1

    # When back from S mode, we have args:
    # a0 = trap context address of app
    # a1 = user space satp
__user_return:
    # switch to user space
    csrw satp, a1
    sfence.vma
    # save trap context in user space in sscratch
    csrw sscratch, a0
    # sp -> trap context in user space(after allocated), sscratch -> user stack sp
    mv sp, a0 
    # restore sstatus/sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore general purpose registers except x0/sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # back to user stack
    ld sp, 2*8(sp)
    sret