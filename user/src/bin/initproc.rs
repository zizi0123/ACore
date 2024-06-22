// init process: executes the user shell and continuously recycles zombie processes in loop
#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const PURPLE: &str = "\x1b[35m";
const RESET: &str = "\x1b[0m";

use user_lib::{exec, fork, wait, yield_};

#[no_mangle]
fn main() -> i32 {
    let str1 = "[initproc] Hello, world!";
    let path = "user_shell\0";
    println!("{}{}{}", PURPLE, str1, RESET);
    println!("str1:{:#x}",str1.as_ptr() as usize);
    println!("path:{:#x}",path.as_ptr() as usize);
    if fork() == 0 {
        exec(&path);
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -1 {
                yield_();
                continue;
            }
            // println!(
            //     "[initproc] Released a zombie process, pid={}, exit_code={}",
            //     pid, exit_code,
            // );
        }
    }
    0
}
