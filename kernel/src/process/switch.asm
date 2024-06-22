# this function is used to switch between tasks when the kernel is processing a trap
# save the context of current task in a0 (*mut TaskContext), and load the context of next task in a1(*const TaskContext)


.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch
__switch:
    # used for switching between tasks, keep the context of current task and load the context of next task
    # __switch(
    #     current_task_cx_ptr: *mut TaskContext, in a0
    #     next_task_cx_ptr: *const TaskContext, in a1
    # )
    # save kernel stack of current task
    sd sp, 8(a0)
    # save ra & s0~s11 (task ctx) of current task
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr
    # restore ra & s0~s11 (task ctx) of next task
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # swictch to the kernel stack of next task
    ld sp, 8(a1)
    ret

