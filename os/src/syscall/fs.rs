//! File and filesystem-related syscalls
use crate::fs::{open_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};

/* 
//use core::str::from_utf8;
use crate::task::add_name_name;
use crate::task::del_name_name;
use crate::task::get_link_num;
use crate::task::get_real_name;
use crate::task::update_add_name_fd;
use crate::fs::StatMode;
*/
use crate::fs::linkat;
use crate::fs::unlinkat;
use crate::fs::get_inode_id_from_name;
use crate::fs::state;
use crate::fs::StatMode;
use crate::mm::memory_set::virt_to_pyh;

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) { 
        let inode_id = get_inode_id_from_name(path.as_str());
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();     // fd  ->  inode_id
        inner.fdtoinode[fd]=inode_id as i32;
        inner.fd_table[fd] = Some(inode);
        fd as isize               
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let pd = virt_to_pyh(_st as usize);
    println!("here 0.1");
    let task = current_task().unwrap();
    println!("here 0.2");
    let task_inner = task.inner_exclusive_access();
    println!("here 0.3");
    unsafe{
        let pdad:*mut Stat = pd as *mut Stat;
        println!("here 3");
         let (nlink,_is) = state(task_inner.fdtoinode[_fd] as u64);
        println!("here 4");
        (*pdad).dev=0;
        println!("here 5");
        (*pdad).ino=task_inner.fdtoinode[_fd] as u64;
        println!("here 6");
        (*pdad).nlink=nlink;
        println!("here 7");
    //     if is{
        (*pdad).mode=StatMode::FILE;
    //     }else {
    //         (*_st).mode=StatMode::DIR;
    //    }
    }
    println!("here 8");
    0
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_name = translated_str(token, _old_name); //   \0
    let new_name = translated_str(token, _new_name);
    //let old_name = my_translated_str(_old_name); //   \0
    //let new_name = my_translated_str(_new_name);
    return linkat(old_name.as_str(),new_name.as_str());
}
/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let name = translated_str(token,_name);
    //let name = my_translated_str(_name);
    return unlinkat(name.as_str());
}
/* 
fn my_translated_str(ptr :*const u8) -> String {
    let mut len = 0;
    unsafe {
        while *ptr.offset(len as isize) != 0 {
            len += 1;
        }
        len+=1;
        let ptr_slice = core::slice::from_raw_parts(ptr,len);
        let str = from_utf8(ptr_slice).unwrap();// &str
        println!("slice:: {:?}",str);
        return String::from(str);
    }
}
*/