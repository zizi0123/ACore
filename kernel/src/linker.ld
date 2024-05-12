OUTPUT_ARCH(riscv) /* 设置目标平台 */
ENTRY(_start) /* 设置程序的入口点为文件 entry.asm 中定义的全局符号 _start */
BASE_ADDRESS = 0x80000000; /* 内核初始化代码应被放置的位置 */


/* . 表示当前地址，linker 会从它指向的位置开始往下放置从输入的目标文件中收集来的段 */
SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;
    
    /* 在可执行文件中生成段 .text，按序放置花括号中内容。* 为通配符。 */
    stext = .; /* 定义标签 stext，其值为当前地址 */
    .text : {
        *(.text.entry)
        . = ALIGN(4K);
        strampoline = .;
        *(.text.trampoline);
        . = ALIGN(4K);
        ASSERT(. - strampoline == 0x1000, "error: trampoline size is not a page size");
        *(.text .text.*)
    }

    /* 在可执行文件中生成段 .rodata */
    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    /* 在可执行文件中生成段 .data */
    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    /* 在可执行文件中生成段 .bss */
    . = ALIGN(4K);
    edata = .;
    sbss_with_stack = .;
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}