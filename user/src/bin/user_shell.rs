#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8; // '\n'
const CR: u8 = 0x0du8; // '\r'
const DL: u8 = 0x7fu8; // delete
const BS: u8 = 0x08u8; // backspace '\b'

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const PURPLE: &str = "\x1b[35m";
const RESET: &str = "\x1b[0m";

extern crate alloc;

use user_lib::console::getchar;
use user_lib::{exec, fork, waitpid};
use alloc::string::String;


#[no_mangle]
pub fn main() -> i32 {
    println!("{}ACore user shell start!{}", PURPLE, RESET);
    let mut line: String = String::new();
    print!("$ ");
    loop {
        let c = getchar();
        match c {
            LF | CR => { // Enter
                println!("");
                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork();
                    if pid == 0 {
                        // child process
                        if exec(line.as_str()) == -1 {
                            println!("{}no such program: {}{}", RED, RESET, line.as_str());
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!(
                            "{}Process {} exited with code {}{}",
                            GREEN, pid, exit_code, RESET
                        );
                    }
                    line.clear();
                }
                print!("$ ");
            }
            BS | DL => { // Backspace or Delete
                if !line.is_empty() {
                    // clear the last character in console
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                // display the character in console
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
