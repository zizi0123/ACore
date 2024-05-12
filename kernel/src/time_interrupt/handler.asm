# kernel/src/time_interrupt/handler.asm
    .section .text.trap
    .global _time_int_handler   #告知编译器 _time_int_handler 是一个全局符号，可被其他目标文件使用
_time_int_handler:
