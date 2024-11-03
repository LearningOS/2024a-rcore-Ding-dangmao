//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
    },
};
use crate::timer::get_time_ms;
use crate::timer::get_time_us;
use crate::mm::memory_set::virt_to_pyh;
use crate::mm::MapPermission;
use crate::mm::memory_set::mmp;
use crate::mm::memory_set::unmap;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let pd=virt_to_pyh(_ts as usize);
    let us = get_time_us();
    unsafe {
        let pdad:*mut TimeVal = pd as *mut TimeVal;
        *pdad = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
        };
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    //同理
    let pd=virt_to_pyh(_ti as usize);
    unsafe{
        let inner = crate::task::TASK_MANAGER.inner.exclusive_access();
        let current = inner.current_task;

        let pdad:*mut TaskInfo = pd as *mut TaskInfo;
        (*pdad).status=TaskStatus::Running;
        (*pdad).time=get_time_ms()-inner.tasks[current].time;
        (*pdad).syscall_times.copy_from_slice(&inner.tasks[current].syscall_times);
        drop(inner);
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    //_port转换-->MapPermissin   //左移一位
    //_start  ---   _start+len上取整
    //构造MapArea
    if (_port & !0x7 !=0) || (_port & 0x7 == 0){
        return -1;
    }
    let mut d = _port;
    let mut permissions = MapPermission::empty();
    if d&1==1 {
        permissions.insert(MapPermission::R);
    }
    d>>=1;
    if d&1==1 {
        permissions.insert(MapPermission::W);
    }
    d>>=1;
    if d&1==1 {
        permissions.insert(MapPermission::X);
    }
    permissions.insert(MapPermission::U);
    return mmp(_start,_start+_len,permissions);
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    return unmap(_start,_start+_len);
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
