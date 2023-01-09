# 2022秋《操作系统》课程实验报告实验-7

20301174-万兴全[VanXQ/VanOS at lab7 (github.com)](https://github.com/VanXQ/VanOS/tree/lab7)

## 一、实验步骤 

本实验的主要目的是实现进程及进程的管理。

1. 修改应用程序

（1）增加重要的系统调用

fork系统调用会创建一个新的进程；waitpid系统调用是的当前进程等待子进程结束，回收其资源并获得返回值；getpid系统调用获得当前进程的信息；exec系统调用将当前的进程地址空间清空并加载一个特定的可执行文件，然后返回用户态执行；read系统调用从文件中读取一段内容到缓冲区，主要目的是为了实现user shell。

首先，修改user/src/syscall.rs增加上述系统调用。

```rust
//user/src/syscall.rs

const SYSCALL_READ: usize = 63;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;

pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(SYSCALL_READ, [fd, buffer.as_mut_ptr() as usize, buffer.len()])
}

pub fn sys_getpid() -> isize {
    syscall(SYSCALL_GETPID, [0, 0, 0])
}

pub fn sys_fork() -> isize {
    syscall(SYSCALL_FORK, [0, 0, 0])
}

pub fn sys_exec(path: &str) -> isize {
    syscall(SYSCALL_EXEC, [path.as_ptr() as usize, 0, 0])
}

pub fn sys_waitpid(pid: isize, exit_code: *mut i32) -> isize {
    syscall(SYSCALL_WAITPID, [pid as usize, exit_code as usize, 0])
}
```

接着，在user/src/lib.rs封装系统调用为应用程序使用的形式。

```rust
//user/src/lib.rs

pub fn read(fd: usize, buf: &mut [u8]) -> isize { sys_read(fd, buf) }

pub fn getpid() -> isize { sys_getpid() }
pub fn fork() -> isize { sys_fork() }
pub fn exec(path: &str) -> isize { sys_exec(path) }
pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => { yield_(); }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => { yield_(); }
            // -1 or a real pid
            exit_pid => return exit_pid,
        }
    }
}

pub fn sleep(period_ms: usize) {
    let start = sys_get_time();
    while sys_get_time() < start + period_ms as isize {
        sys_yield();
    }
}
```

（2）实现用户初始程序initproc

```rust
//user/src/bin/initproc.rs

#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{
    fork,
    wait,
    exec,
    yield_,
};

#[no_mangle]
fn main() -> i32 {
    if fork() == 0 {
        exec("user_shell\0");
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -1 {
                yield_();
                continue;
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid,
                exit_code,
            );
        }
    }
    0
}
```

（3）实现shell程序

首先基于sys_read系统调用封装能够从标准输入读取一个字符的函数getchar。

```rust
//user/src/console.rs 

use super::read;

const STDIN: usize = 0;

pub fn getchar() -> u8 {
    let mut c = [0u8; 1];
    read(STDIN, &mut c);
    c[0]
}
```

然后，实现user shell程序。

```rust
//user/src/bin/user_shell.rs

#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;

use alloc::string::String;
use user_lib::{fork, exec, waitpid};
use user_lib::console::getchar;

#[no_mangle]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    print!(">> ");
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork();
                    if pid == 0 {
                        // child process
                        if exec(line.as_str()) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
```

因为Rust的可边长字符串类型String基于动态内存分配，因此还需要在用户库user_lib中支持动态内存分配。

```rust
//usr/src/lib.rs
#![feature(alloc_error_handler)]

use buddy_system_allocator::LockedHeap;

const USER_HEAP_SIZE: usize = 16384;
static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    exit(main());
}
```


其他应用程序的实现就不再这里一一列出，请参考示例代码。

![image-20221208224419990](OSLab_7.assets/image-20221208224419990.png)

2. 在内核中增加系统调用

首先，修改os/src/syscall/mod.rs增加fork、waitpid、getpid、read系统调用。

```rust
//os/src/syscall/mod.rs

const SYSCALL_READ: usize = 63;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;

mod fs;
mod process;

use fs::*;
use process::*;

pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    match syscall_id {
        SYSCALL_READ => sys_read(args[0], args[1] as *const u8, args[2]),
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(),
        SYSCALL_GETPID => sys_getpid(),
        SYSCALL_FORK => sys_fork(),
        SYSCALL_EXEC => sys_exec(args[0] as *const u8),
        SYSCALL_WAITPID => sys_waitpid(args[0] as isize, args[1] as *mut i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
```


然后，修改os/src/syscall/fs.rs，实现sys_read系统调用。

```rust
//os/src/syscall/fs.rs

use crate::task::{current_user_token, suspend_current_and_run_next};
use crate::sbi::console_getchar;
const FD_STDIN: usize = 0;


pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDIN => {
            assert_eq!(len, 1, "Only support len = 1 in sys_read!");
            let mut c: usize;
            loop {
                c = console_getchar();
                if c == 0 {
                    suspend_current_and_run_next();
                    continue;
                } else {
                    break;
                }
            }
            let ch = c as u8;
            let mut buffers = translated_byte_buffer(current_user_token(), buf, len);
            unsafe { buffers[0].as_mut_ptr().write_volatile(ch); }
            1
        }
        _ => {
            panic!("Unsupported fd in sys_read!");
        }
    }
}
```

其中，suspend_current_and_run_next函数是暂停当前的任务并切换到下一个任务，具体实现将在后面介绍。

然后，修改os/src/syscall/process.rs实现其他系统调用。

```rust
//os/src/syscall/process.rs

use crate::task::{
    suspend_current_and_run_next,
    exit_current_and_run_next,
    current_task,
    current_user_token,
    add_task,
};

use crate::mm::{
    translated_str,
    translated_refmut,
};
use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // find a child process

// ---- access current TCB exclusively
let mut inner = task.inner_exclusive_access();
if inner.children
    .iter()
    .find(|p| {pid == -1 || pid as usize == p.getpid()})
    .is_none() {
    return -1;
    // ---- release current PCB
}
let pair = inner.children
    .iter()
    .enumerate()
    .find(|(_, p)| {
        // ++++ temporarily access child PCB lock exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
if let Some((idx, _)) = pair {
    let child = inner.children.remove(idx);
    // confirm that child will be deallocated after removing from children list
    assert_eq!(Arc::strong_count(&child), 1);
    let found_pid = child.getpid();
    // ++++ temporarily access child TCB exclusively
    let exit_code = child.inner_exclusive_access().exit_code;
    // ++++ release child PCB
    *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
    found_pid as isize
} else {
    -2
}
// ---- release current PCB lock automatically

}


```

![image-20221208225021169](OSLab_7.assets/image-20221208225021169.png)

3. 应用的链接与加载

（1）基于名字的应用链接

因为实现exec系统调用需要根据应用程序的名字获取ELF格式的数据，因此需要修改链接和加载接口。

修改编译链接辅助文件os/build.rs。

```rust
//os/build.rs

writeln!(f, r#"
    .global _app_names
_app_names:"#)?;
    for app in apps.iter() {
        writeln!(f, r#"    .string "{}""#, app)?;
    }
```


（2）基于名字的应用加载

应用加载子模块loader.rs会用一个全局可见的只读向量APP_NAMES按照顺序吧所有应用的名字保存在内存中。

```rust
//os/src/loader.rs

use alloc::vec::Vec;
use lazy_static::*;

lazy_static! {
    static ref APP_NAMES: Vec<&'static str> = {
        let num_app = get_num_app();
        extern "C" { fn _app_names(); }
        let mut start = _app_names as usize as *const u8;
        let mut v = Vec::new();
        unsafe {
            for _ in 0..num_app {
                let mut end = start;
                while end.read_volatile() != '\0' as u8 {
                    end = end.add(1);
                }
                let slice = core::slice::from_raw_parts(start, end as usize - start as usize);
                let str = core::str::from_utf8(slice).unwrap();
                v.push(str);
                start = end.add(1);
            }
        }
        v
    };
}

#[allow(unused)]
pub fn get_app_data_by_name(name: &str) -> Option<&'static [u8]> {
    let num_app = get_num_app();
    (0..num_app)
        .find(|&i| APP_NAMES[i] == name)
        .map(|i| get_app_data(i))
}

pub fn list_apps() {
    println!("/**** APPS ****");
    for app in APP_NAMES.iter() {
        println!("{}", app);
    }
    println!("**************/");
}
```

![image-20221208225602441](OSLab_7.assets/image-20221208225602441.png)

4. 进程标识符与内核栈

（1）实现进程标识符
进程标识应当是唯一的，我们将其抽象为一个PidHandle类型。

```rust
//os/src/task/pid.rs

pub struct PidHandle(pub usize);
```

类似于之前的物理页帧的管理，我们实现一个进程标识符分配器PID_ALLOCATOR。

```rust
//os/src/task/pid.rs

struct PidAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl PidAllocator {
    pub fn new() -> Self {
        PidAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }
    pub fn alloc(&mut self) -> PidHandle {
        if let Some(pid) = self.recycled.pop() {
            PidHandle(pid)
        } else {
            self.current += 1;
            PidHandle(self.current - 1)
        }
    }
    pub fn dealloc(&mut self, pid: usize) {
        assert!(pid < self.current);
        assert!(
            self.recycled.iter().find(|ppid| **ppid == pid).is_none(),
            "pid {} has been deallocated!", pid
        );
        self.recycled.push(pid);
    }
}

lazy_static! {
    static ref PID_ALLOCATOR : UPSafeCell<PidAllocator> = unsafe {
        UPSafeCell::new(PidAllocator::new())
    };
}
```

我们还需要封装一个全局的进程标识分配接口pid_alloc。

```rust
//os/src/task/pid.rs

pub fn pid_alloc() -> PidHandle {
    PID_ALLOCATOR.exclusive_access().alloc()
}
```

同时，为了允许资源的自动回收，还需要为PidHandle实现Drop Trait。

```rust
//os/src/task/pid.rs

impl Drop for PidHandle {
    fn drop(&mut self) {
        //println!("drop pid {}", self.0);
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}
```

（2）在内核栈中保存进程标识符

重新定义内核栈。

```rust
//os/src/task/pid.rs

use alloc::vec::Vec;
use lazy_static::*;
use crate::sync::UPSafeCell;
use crate::mm::{KERNEL_SPACE, MapPermission, VirtAddr};
use crate::config::{
    PAGE_SIZE,
    TRAMPOLINE,
    KERNEL_STACK_SIZE,
};


pub struct KernelStack {
    pid: usize,
}
```


实现如下方法。

```rust
/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

impl KernelStack {
    pub fn new(pid_handle: &PidHandle) -> Self {
        let pid = pid_handle.0;
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(pid);
        KERNEL_SPACE
            .exclusive_access()
            .insert_framed_area(
                kernel_stack_bottom.into(),
                kernel_stack_top.into(),
                MapPermission::R | MapPermission::W,
            );
        KernelStack {
            pid: pid_handle.0,
        }
    }
    #[allow(unused)]
    pub fn push_on_top<T>(&self, value: T) -> *mut T where
        T: Sized, {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe { *ptr_mut = value; }
        ptr_mut
    }
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.pid);
        kernel_stack_top
    }
}
```

同时也需要实现KernelStack 的Drop Trait以便KernelStack生命周期结束时回收相应的物理页帧。

```rust
impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.pid);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
    }
}
```

相应的，还需要修改os/src/mm/memory_set.rs

```rust
impl MemorySet {
    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self.areas.iter_mut().enumerate()
            .find(|(_, area)| area.vpn_range.get_start() == start_vpn) {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
        }
    }
}
```

![image-20221208225803480](OSLab_7.assets/image-20221208225803480.png)


5. 修改实现进程控制块

修改原本的TaskControlBlock实现进程控制块的功能。

```rust
//os/src/task/task.rs

pub struct TaskControlBlock {
    // immutable
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    // mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
}
```

TaskControlBlockInner实现以下方法：

```rust
impl TaskControlBlockInner {
    /*
    pub fn get_task_cx_ptr2(&self) -> *const usize {
        &self.task_cx_ptr as *const usize
    }
    */
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}
```

TaskControlBlock实现以下方法：

```rust
impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    pub fn new(elf_data: &[u8]) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // push a task context which goes to trap_return to the top of kernel stack
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn,
                base_size: user_sp,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set,
                parent: None,
                children: Vec::new(),
                exit_code: 0,
            })},
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
    pub fn exec(&self, elf_data: &[u8]) {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

​    // **** access inner exclusively
​    let mut inner = self.inner_exclusive_access();
​    // substitute memory_set
​    inner.memory_set = memory_set;
​    // update trap_cx ppn
​    inner.trap_cx_ppn = trap_cx_ppn;
​    // initialize trap_cx
​    let trap_cx = inner.get_trap_cx();
​    *trap_cx = TrapContext::app_init_context(
​        entry_point,
​        user_sp,
​        KERNEL_SPACE.exclusive_access().token(),
​        self.kernel_stack.get_top(),
​        trap_handler as usize,
​    );
​    // **** release inner automatically
}
pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
​    // ---- access parent PCB exclusively
​    let mut parent_inner = self.inner_exclusive_access();
​    // copy user space(include trap context)
​    let memory_set = MemorySet::from_existed_user(
​        &parent_inner.memory_set
​    );
​    let trap_cx_ppn = memory_set
​        .translate(VirtAddr::from(TRAP_CONTEXT).into())
​        .unwrap()
​        .ppn();
​    // alloc a pid and a kernel stack in kernel space
​    let pid_handle = pid_alloc();
​    let kernel_stack = KernelStack::new(&pid_handle);
​    let kernel_stack_top = kernel_stack.get_top();
​    let task_control_block = Arc::new(TaskControlBlock {
​        pid: pid_handle,
​        kernel_stack,
​        inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
​            trap_cx_ppn,
​            base_size: parent_inner.base_size,
​            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
​            task_status: TaskStatus::Ready,
​            memory_set,
​            parent: Some(Arc::downgrade(self)),
​            children: Vec::new(),
​            exit_code: 0,
​        })},
​    });
​    // add child
​    parent_inner.children.push(task_control_block.clone());
​    // modify kernel_sp in trap_cx
​    // **** access children PCB exclusively
​    let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
​    trap_cx.kernel_sp = kernel_stack_top;
​    // return
​    task_control_block
​    // ---- release parent PCB automatically
​    // **** release children PCB automatically
}
pub fn getpid(&self) -> usize {
​    self.pid.0
}}
```

同时修改TaskStatus的状态。

```rust
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running,
    Zombie,
}
```

![image-20221208230752660](OSLab_7.assets/image-20221208230752660.png)


6. 实现任务管理器

修改任务管理器，将部分任务管理功能移到处理器管理中。

```rust
//os/src/task/manager.rs

use crate::sync::UPSafeCell;
use super::TaskControlBlock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;

pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    pub fn new() -> Self {
        Self { ready_queue: VecDeque::new(), }
    }
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> = unsafe {
        UPSafeCell::new(TaskManager::new())
    };
}

pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
```

![image-20221208230836284](OSLab_7.assets/image-20221208230836284.png)


7. 增加处理器管理结构

实现处理器管理结构Processor，完成从任务管理器分离的维护CPU状态的部分功能。

```rust
//os/src/task/processor.rs

use super::{TaskContext, TaskControlBlock};
use alloc::sync::Arc;
use lazy_static::*;
use super::{fetch_task, TaskStatus};
use super::__switch;
use crate::trap::TrapContext;
use crate::sync::UPSafeCell;

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe {
        UPSafeCell::new(Processor::new())
    };
}

pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(
                    idle_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
        }
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().inner_exclusive_access().get_trap_cx()
}

pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(
            switched_task_cx_ptr,
            idle_task_cx_ptr,
        );
    }
}
```

![image-20221208230931565](OSLab_7.assets/image-20221208230931565.png)


8. 创建初始进程

内核初始化完成后，将会调用task子模块的add_initproc将初始进程initproc加入任务管理器。在这之前要初始化初始进程的进程控制块。

```rust
//os/src/task/mod.rs

use crate::loader::get_app_data_by_name;
use manager::add_task;

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(
        TaskControlBlock::new(get_app_data_by_name("initproc").unwrap())
    );
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}
```




9. 进程调度机制

通过调用 task 子模块提供的 suspend_current_and_run_next 函数可以暂停当前任务并切换到另外一个任务。因为进程概念的引入，其实现需要更改。

```rust
//os/src/task/mod.rs

pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

// ---- access current TCB exclusively
let mut task_inner = task.inner_exclusive_access();
let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
// Change status to Ready
task_inner.task_status = TaskStatus::Ready;
drop(task_inner);
// ---- release current PCB

// push back to ready queue.
add_task(task);
// jump to scheduling cycle
schedule(task_cx_ptr);

}
```

![image-20221208231132142](OSLab_7.assets/image-20221208231132142.png)


10. 进程的生成机制

在内核中只有初始进程initproc是手动生成的，其他的进程由初始进程直接或间接fork出来，然后再调用exec系统调用加载并执行可执行文件。所以，进程的生成机制由fork和exec两个系统调用来完成。

实现fork系统调用最关键的是为子进程创建一个和父进程几乎相同的地址空间。具体实现如下。

```rust
//os/src/mm/memory_set.rs

impl MapArea {
    pub fn from_another(another: &MapArea) -> Self {
        Self {
            vpn_range: VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }
}

impl MemorySet {
    pub fn from_existed_user(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // copy data sections/trap_context/user_stack
        for area in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, None);
            // copy data from another space
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn.get_bytes_array().copy_from_slice(src_ppn.get_bytes_array());
            }
        }
        memory_set
    }
}
```

接着，实现 TaskControlBlock::fork 来从父进程的进程控制块创建一份子进程的控制块。实现如下：

```rust
//os/src/task/task.rs
impl TaskControlBlock {
        pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        // ---- access parent PCB exclusively
        let mut parent_inner = self.inner_exclusive_access();
        // copy user space(include trap context)
        let memory_set = MemorySet::from_existed_user(
            &parent_inner.memory_set
        );
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn,
                base_size: parent_inner.base_size,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
            })},
        });
        // add child
        parent_inner.children.push(task_control_block.clone());
        // modify kernel_sp in trap_cx
        // **** access children PCB exclusively
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // return
        task_control_block
        // ---- release parent PCB automatically
        // **** release children PCB automatically
    }
}
```


然后，实现exec系统调用。

```rust
//os/src/task/task.rs

impl TaskControlBlock {
    

pub fn exec(&self, elf_data: &[u8]) {
    // memory_set with elf program headers/trampoline/trap context/user stack
    let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
    let trap_cx_ppn = memory_set
        .translate(VirtAddr::from(TRAP_CONTEXT).into())
        .unwrap()
        .ppn();

​    // **** access inner exclusively
​    let mut inner = self.inner_exclusive_access();
​    // substitute memory_set
​    inner.memory_set = memory_set;
​    // update trap_cx ppn
​    inner.trap_cx_ppn = trap_cx_ppn;
​    // initialize trap_cx
​    let trap_cx = inner.get_trap_cx();
​    *trap_cx = TrapContext::app_init_context(
​        entry_point,
​        user_sp,
​        KERNEL_SPACE.exclusive_access().token(),
​        self.kernel_stack.get_top(),
​        trap_handler as usize,
​    );
​    // **** release inner automatically
}

}
```

有了exec系统调用后，sys_exec的实现就很容易理解了。Sys_exec的实现还依赖于对页表的修改。

```rust
//os/src/mm/page_table.rs

impl PageTable {
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor())
            .map(|pte| {
                //println!("translate_va:va = {:?}", va);
                let aligned_pa: PhysAddr = pte.ppn().into();
                //println!("translate_va:pa_align = {:?}", aligned_pa);
                let offset = va.page_offset();
                let aligned_pa_usize: usize = aligned_pa.into();
                (aligned_pa_usize + offset).into()
            })
    }
}

pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    loop {
        let ch: u8 = *(page_table.translate_va(VirtAddr::from(va)).unwrap().get_mut());
        if ch == 0 {
            break;
        } else {
            string.push(ch as char);
            va += 1;
        }
    }
    string
}

pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    //println!("into translated_refmut!");
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    //println!("translated_refmut: before translate_va");
    page_table.translate_va(VirtAddr::from(va)).unwrap().get_mut()
}
```


在sys_exec系统调用后，trap_handler原来的上下文cx失效了。为此，在syscall分发之后，还需要重新获取trap上下文。具体修改代码如下：

```rust
//os/src/trap/mod.rs


#[no_mangle]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let cx = current_trap_cx();
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            // get system call return value
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]);
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault) |
        Trap::Exception(Exception::StorePageFault) |
        Trap::Exception(Exception::InstructionFault) |
        Trap::Exception(Exception::InstructionPageFault) |
        Trap::Exception(Exception::LoadFault) |
        Trap::Exception(Exception::LoadPageFault) => {
            println!(
                "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, core dumped.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            // page fault exit code
            exit_current_and_run_next(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, core dumped.");
            // illegal instruction exit code
            exit_current_and_run_next(-3);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!("Unsupported trap {:?}, stval = {:#x}!", scause.cause(), stval);
        }
    }
    trap_return();
}
```





11. 进程资源回收机制

当应用调用 sys_exit 系统调用主动退出或者出错由内核终止之后，会在内核中调用 exit_current_and_run_next 函数退出当前进程并切换到下一个进程。

相比之前的实现，exit_current_and_run_next增加了一个退出码作为参数。

```rust
// os/src/mm/memory_set.rs

impl MemorySet {
    pub fn recycle_data_pages(&mut self) {
        self.areas.clear();
    }
}
```



```rust
//os/src/task/mod.rs

pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();
    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

// ++++++ access initproc TCB exclusively
{
    let mut initproc_inner = INITPROC.inner_exclusive_access();
    for child in inner.children.iter() {
        child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
        initproc_inner.children.push(child.clone());
    }
}
// ++++++ release parent PCB

inner.children.clear();
// deallocate user space
inner.memory_set.recycle_data_pages();
drop(inner);
// **** release current PCB
// drop task manually to maintain rc correctly
drop(task);
// we do not have to save task context
let mut _unused = TaskContext::zero_init();
schedule(&mut _unused as *mut _);

}
```

同时，父进程通过 sys_waitpid 系统调用来回收子进程的资源并收集它的一些信息。

最后，修改main.rs。

```rust
//os/src/main.rs

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    mm::init();
    println!("[kernel] back to world!");
    mm::remap_test();
    task::add_initproc();
    println!("after initproc!");
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    loader::list_apps();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}
```

至此，具有进程管理功能的操作系统实现完成。

![image-20221219185022004](OSLab_7.assets/image-20221219185022004.png)

![image-20221219184939089](OSLab_7.assets/image-20221219184939089.png)

![image-20221219194250760](OSLab_7.assets/image-20221219194250760.png)

## 二、思考问题

### （1）分析应用的链接与加载是如何实现的；

链接：应用通过应用名链接到队列中。主要通过完成以下两个任务实现：

```
符号解析：将每个符号引用（变量与函数引用）刚好和一个符号定义联系起来。
重定位：编辑器和汇编器生成从地址0开始的代码和数据节。在汇编器生成的可重定位目标程序中，数据和代码的位置都是局部的逻辑地址，而想要在系统中执行这些指令，就需要以某种方式将这些逻辑地址重定位为实际的存储器地址。链接器通过将每个符号定义与一个存储器位置联系起来，然后修改所有对这些符号的引用，使得它们指向这个存储器位置，从而重定位这些节。
```

加载：由加载器将可执行目标文件中的代码和数据从磁盘拷贝到存储器中，然后跳转到程序的入口点来运行该程序。实际上由于虚拟存储器机制的存在，除了一些头部信息，加载器在加载过程中并没有执行从磁盘到存储器的实际数据拷贝，只是简单的将可执行文件映射到虚拟地址空间。直到程序真正运行，即CPU引用一个被映射的虚拟地址时才会触发缺页中断进行拷贝，而这是由页面调度机制自动完成的。

### （2）分析进程标识符、进程控制块是如何设计和实现的；

进程标识符：定义pidhandle使标识唯一，并为其分配内存。分装一个全局进程标识分配接口，设置droptrait调用先前实现的dealloc方法实现自动回收资源，在内核栈中定义标识符数据结构是现在内存栈中保存进程标识符，实现kernelstack的droptrait，使kernelstack生命周期结束时回收对应物理页帧.

进程控制块：taskcontrolblockinner数据结构，实现进程间的调用分配与释放。

### （3）分析任务管理是如何实现的；

在`task.rs`中，TaskControlBlockInner实现了任务的出入栈操作，以此实现任务管理。

### （4）分析进程的调度、生成、以及进程资源的回收是如何实现的。

进程的调度：通过调用task中提供的suspend_current_and_run_next函数，暂停当前任务切换到另一个任务。

进程的生成：在内核中只有initproc是手动生成的，其他的进程由初始进程复制出来，然后经系统调用加载并执行可执行文件，即fork和exec两个系统调用。fork的核心是为子进程创建一个和父进程相似的地址空间，TaskControlBlock::fork从父进程的进程控制块中创建一份子进程控制块，通过exec调用。

资源回收机制：当应调用sys_exit系统调用主动退出或者由内核终止时，在内核中调用exit_current_and run_next函数退出进程切换到另一个进程，同时通过sys_waitpid这一系统调用来回收进程资源并收集信息。

## 三、Git提交截图

![](OSLab_7.assets/image-20221219185531271.png)

![image-20221219185535929](OSLab_7.assets/image-20221219185535929.png)

![image-20221219185547278](OSLab_7.assets/image-20221219185547278.png)

![image-20221219185558483](OSLab_7.assets/image-20221219185558483.png)

![image-20221219185607449](OSLab_7.assets/image-20221219185607449.png)

![image-20221219185617302](OSLab_7.assets/image-20221219185617302.png)

![image-20221219185629818](OSLab_7.assets/image-20221219185629818.png)

![image-20221219185638878](OSLab_7.assets/image-20221219185638878.png)





## 四、其他说明

终于做完辣！感谢老师，感谢助教！
