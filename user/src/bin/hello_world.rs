#![no_std]  //告诉编译器无需链接标准库 std，仅链接 core 库
#![no_main] //告诉编译器无需完成初始化工作（需依赖于标准库）

#[macro_use]
extern crate user_lib;

#[no_mangle] //告诉编译器不要更改函数名称
pub fn main() -> i32 {
    println!("Hello world, this is zizi's first user program!");
    return 0;
}
