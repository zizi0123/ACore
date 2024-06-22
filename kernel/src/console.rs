// os/src/console.rs

use crate::sbi::console_putchar;
use core::fmt::{self, Write};

struct Stdout;

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

// 调用 write_fmt 方法，将格式化的字符串打印到终端。
pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap(); 
}


// 实现 core::fmt::Write trait 的 write_str 方法，此后可以使用 write_fmt 方法
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            console_putchar(c as usize);
        }
        Ok(())
    }
}


