// 功能：将内存中缓冲区中的数据写入文件。
// 参数：`fd` 表示待写入文件的文件描述符；
//      `buf` 表示内存中缓冲区的起始地址；
//      `len` 表示内存中缓冲区的长度。
// 返回值：返回成功写入的长度。
// syscall ID：64
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize { //only support stdout now
    if fd == 1 {
        let slice = unsafe { core::slice::from_raw_parts(buf, len) };
        let str = core::str::from_utf8(slice).unwrap();
        print!("{}", str);
        return len as isize
    } else {
        panic!("Unsupported fd in sys_write!");
    }
}
