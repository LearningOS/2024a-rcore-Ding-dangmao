//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

use crate::config::BIG_STRIDE;

use crate::timer::get_time_ms;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            task_inner.stride+=BIG_STRIDE/(task_inner.prio as i32);
            if task_inner.first{
                task_inner.first=false;
                task_inner.time=get_time_ms() as usize;
            }
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

//记录系统调用
///
pub fn syscalladd(id: usize){
    let task=current_task().unwrap();
    let mut  task_inner=task.inner_exclusive_access();
    task_inner.syscall_times[id]+=1;
}
/* 
///新,原
pub fn add_name_name(stri1:&str,stri2: &str){
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let flag:bool=true;
    for (name1_,name2_) in nton.iter_mut() {
        if name1.as_str()==stri1 && name2.as_str()==stri2{
           flag=false;
        }
    }
    if flag{
     inner.nton.push((String::from(stri1),String::from(stri2)));
    }
}
///
pub fn del_name_name(stri: &str)->isize{
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let nton=&mut inner.nton;

    // 查找第二个元素等于 `s1` 的元组
    if let Some(index) = nton.iter().position(|(first, _)| first.as_str() == stri) {
        nton.remove(index);
        return 0; // 成功删除，返回 0
    } else {
        // 如果没有找到，返回 -1
        return -1;
    }
}
///
pub fn get_link_num(fd: i32)->u32{
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let nton=&mut inner.nton;
    let nametofd=&mut inner.nametofd;

    let mut filename:String=String::from("null");
    let mut num:u32=0;
    //找到文件描述符对应的文件名
    for (ref name,ref fd_) in nametofd.iter_mut() {
        if fd==(*fd_){
            filename=String::from((*name).as_str());
        }
    }
    //通过文件名找到所有硬链接
    let str_name=filename.as_str();
    for (_,ref name_) in nton.iter_mut() {
        if str_name==(*name_){
           num+=1;
        }
    }
    return num;
}
///
pub fn get_real_name(name:&str)->String{
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let nton=&mut inner.nton;

    for (ref name_,_) in nton.iter_mut() {
        if name==(*name_).0.as_str(){
           return String::from((*name).1);
        }
    }
    return String::from("kong");

}
///
pub fn update_add_name_fd(name:&str,fd:u32){
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let nametofd=&mut inner.nametofd;
    let mut flag:bool=true;
    for (ref mut name_,_) in nametofd.iter_mut() {
        //遍历寻找实际名称对应的元组位置,并更新文件描述符 
        if name==(*name_).0.as_str(){
           (*name_).1=fd;
           flag=false;
        }
    }
    //若没存过则存如文件名对应的文件描述符
    if(flag){
        nametofd.push((String::from(name),fd));
    }
}
    */