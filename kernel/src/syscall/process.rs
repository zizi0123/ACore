// syscalss about process management

use crate::process::scheduler::{change_program_brk, exit_current_and_run_next, fork_process, waitpid_process, get_current_satp, get_current_task, get_pid, suspend_current_and_run_next};
use crate::process::task_manager::{add_task, remove_task};
use crate::time::get_time_ms;
use crate::mem::page_table;
use crate::process::loader::open_app_file;
use crate::config::{GREEN, RESET};

pub fn sys_fork() -> isize {
    println!("{}[kernel] fork a new process{}", GREEN, RESET);
    let current_task = get_current_task();
    let child_pid = fork_process();
    let child_task = current_task.fork(child_pid);

    // set the return value of child process to 0
    let child_inner = child_task.inner.exclusive_access();
    let trap_cx = child_inner.get_trap_ctx();
    trap_cx.x[10] = 0;
    drop(child_inner);

    // add new task to scheduler
    add_task(child_pid, child_task);

    return child_pid as isize;
}

// return 0 if success, -1 if no such file.
pub fn sys_exec(path: *const u8) -> isize {
    let satp = get_current_satp();
    let app_name = page_table::get_string(satp, path);
    if let Some(data) = open_app_file(app_name.as_str()) {
        let task = get_current_task();
        println!("{}[kernel] exec app: {}, pid = {}{}", GREEN, app_name, task.pid, RESET);
        task.exec(data);
        0
    } else {
        -1
    }
}

// no such child process -> -1.
// child process is still running -> -2.
// else -> pid, and exit code of child process is kept in exit_code_ptr.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let current_task = get_current_task();
    let (pid, exit_code) = waitpid_process(pid);
    let cur_task_inner = current_task.inner.exclusive_access();
    if pid != -1 && pid != -2{
        page_table::write_into(cur_task_inner.address_space.get_satp(), exit_code_ptr, exit_code as i32);
        // remove task to release resources
        remove_task(pid as usize);
    }
    drop(cur_task_inner);
    return pid;
}

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}


pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

pub fn sys_sbrk(size: i32) -> isize {
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

pub fn sys_get_pid() -> isize {
    get_pid() as isize
}

