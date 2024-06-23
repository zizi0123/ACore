const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_SBRK: usize = 214;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;
const SERVICE_PROCESS_MANAGER: usize = 511;


mod file_system;
mod process;
pub mod process_manager;

use file_system::*;
use process::*;
use process_manager::*;

// call the corresponding syscall function according to the syscall_id
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize { 
    match syscall_id {
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_GET_TIME => sys_get_time(),
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_READ => sys_read(args[0], args[1] as *mut u8, args[2]),
        SYSCALL_GETPID => sys_get_pid(),
        SYSCALL_SBRK => sys_sbrk(args[0] as i32),
        SYSCALL_FORK => sys_fork(),
        SYSCALL_EXEC => sys_exec(args[0] as *const u8),
        SYSCALL_WAITPID => sys_waitpid(args[0] as isize, args[1] as *mut i32),
        SERVICE_PROCESS_MANAGER => process_manager_syscall(args[0] as isize, args[1], args[2] as *mut i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}



