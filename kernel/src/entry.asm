# os/src/entry.asm
    .section .text.entry
    .global _start   #告知编译器 _start 是一个全局符号，可被其他目标文件使用
_start:
    la sp, boot_stack_top #把sp设置为栈的起始位置, 为 kernel 分配栈空间
    call rust_start

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound: #栈向低地址增长，此为增长的最低地址
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top: #栈向低地址增长，此为增长的最高地址
