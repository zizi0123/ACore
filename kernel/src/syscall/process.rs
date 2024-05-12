// 功能：退出应用程序并将返回值告知批处理系统。
// 参数：`exit_code` 表示应用程序的返回值。
// 返回值：该系统调用不应该返回。
// syscall ID：93
pub fn sys_exit(xstate: i32) -> ! {
    println!("[kernel] Application exited with code {}", xstate);
    // run_next_app()
}