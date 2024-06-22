// syscalss about process management

use alloc::sync::Arc;

use crate::process::scheduler::{change_program_brk, exit_current_and_run_next, get_current_satp, get_current_task, get_pid, suspend_current_and_run_next};
use crate::time::get_time_ms;
use crate::process::process_manager::add_task;
use crate::mem::page_table;
use crate::process::loader::open_app_file;
use crate::config::{GREEN, RED, RESET};

pub fn sys_fork() -> isize {
    println!("{}[kernel] fork a new process{}", GREEN, RESET);
    let current_task = get_current_task();
    let new_task = current_task.fork();
    let new_pid = new_task.get_pid();

    // set the return value of child process to 0
    let trap_cx = new_task.inner.exclusive_access().get_trap_ctx();
    trap_cx.x[10] = 0;

    // add new task to scheduler
    add_task(new_task);

    return new_pid as isize;
}

// return 0 if success, -1 if no such file.
pub fn sys_exec(path: *const u8) -> isize {
    let satp = get_current_satp();
    let app_name = page_table::get_string(satp, path);
    println!("{}[kernel] exec app: {}{}", RED, app_name, RESET);
    if let Some(data) = open_app_file(app_name.as_str()) {
        let task = get_current_task();
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
    let task = get_current_task();
    let mut inner = task.inner.exclusive_access();

    // no such child process
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.get_pid())
    {
        return -1;
    }

    // find the exited child process
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        p.inner.exclusive_access().has_exited() && (pid == -1 || pid as usize == p.get_pid())
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.get_pid();
        let exit_code = child.inner.exclusive_access().exit_code;
        page_table::write_into(inner.address_space.get_satp(), exit_code_ptr, exit_code);
        return found_pid as isize
    } else {
        return -2;
    }
    // ---- release current PCB lock automatically
}

pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
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

