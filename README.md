# 2022秋《操作系统》课程实验报告实验-4

20301174-万兴全

已推送到[VanXQ/VanOS at lab4 (github.com)](https://github.com/VanXQ/VanOS/tree/lab4)

## 一、实验步骤 

本实验的主要目的是实现一个支持多道程序和协作式调度的操作系统。

1. 实现应用程序

多道程序操作系统的应用程序跟批处理操作系统的应用程序实现基本是一致的。主要的区别在于应用程序加载到内存中的位置不同。同时，在多道程序操作系统中，应用程序可以主动让出CPU切换到其他应用程序。协作式调度的意思就是应用程序在执行IO操作时主动让出CPU，从而使得CPU可以执行其他应用程序，最终提高CPU的使用效率。

（1）应用程序的放置

在批处理操作系统中，应用程序加载的内存位置是相同的。但是，在多道程序操作系统中，每个应用程序加载的位置是不同的。这也就是的链接脚本中linker.ld的BASE_ADDRESS是不同的。为此，我们需要编写一个脚本build.py，实现为每个应用程序定制链接脚本。

```python
// user/build.py
import os

base_address = 0x80400000
step = 0x20000
linker = 'src/linker.ld'

app_id = 0
apps = os.listdir('src/bin')
apps.sort()
for app in apps:
    app = app[:app.find('.')]
    lines = []
    lines_before = []
    with open(linker, 'r') as f:
        for line in f.readlines():
            lines_before.append(line)
            line = line.replace(hex(base_address), hex(base_address+step*app_id))
            lines.append(line)
    with open(linker, 'w+') as f:
        f.writelines(lines)
    os.system('cargo build --bin %s --release' % app)
    print('[build.py] application %s start with address %s' %(app, hex(base_address+step*app_id)))
    with open(linker, 'w+') as f:
        f.writelines(lines_before)
    app_id = app_id + 1
```

相应的，我们还需要修改user/Makefile文件。具体，请参考示例代码。主要的区别就在于调用了build.py执行编译。

修改为

```makefile
TARGET := riscv64gc-unknown-none-elf

MODE := release

APP_DIR := src/bin

TARGET_DIR := target/$(TARGET)/$(MODE)

APPS := $(wildcard $(APP_DIR)/*.rs)

ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))

BINS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(APPS))

OBJDUMP := rust-objdump --arch-name=riscv64

OBJCOPY := rust-objcopy --binary-architecture=riscv64

elf: $(APPS)

  @python3 build.py

binary: elf

  $(foreach elf, $(ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

build: binary

clean:

  @cargo clean

.PHONY: elf binary build clean
```

（2）增加yield系统调用

应用程序执行的过程通常是计算和IO间歇执行的。程序在执行IO的过程中如果仍然占用CPU的话，则会造成CPU资源的浪费。多道程序设计允许应用程序在执行IO操作时，应用程序主动让出CPU的权限让其他应用程序执行。yield系统调用就是使得应用程序能够主动让出CPU权限的系统调用。

首先，在user/src/syscall.rs中增加sys_yield系统调用。增加如下代码：

```rust
const SYSCALL_YIELD: usize = 124;

pub fn sys_yield() -> isize {
    syscall(SYSCALL_YIELD, [0, 0, 0])
}
```

然后，修改user/src/lib.rs增加yield_   用户库的封装。具体增加代码如下：

```rust
pub fn yield_() -> isize { sys_yield() }
```

（3）实现测试应用程序

在user/src/bin分别实现00write_a.rs，01write_b.rs，02write_c.rs三个测试应用程序，分别输出字母ABC。其中00write_a.rs的代码如下：

```rust
#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::yield_;

const WIDTH: usize = 10;
const HEIGHT: usize = 5;

#[no_mangle]
fn main() -> i32 {
    for i in 0..HEIGHT {
        for _ in 0..WIDTH { print!("A"); }
        println!(" [{}/{}]", i + 1, HEIGHT);
        yield_();
    }
    println!("Test write_a OK!");
    0
}
```

然后，在user目录下执行make build命令就可以编译多道程序操作系统的应用程序了。需要注意的是，因为目前操作系统还不支持加载以及yield系统调用，所以这个时候的操作系统还不支持运行这些应用程序。

![image-20221115111955509](OSLab_4.assets/image-20221115111955509.png)

![image-20221115111939682](OSLab_4.assets/image-20221115111939682.png)


2. 多道程序的加载

在批处理操作系统中，应用程序的加载和执行都是由batch子模块还处理的。而在多道程序操作系统中，应用程序的加载和执行分为两个模块来完成。其中，loader子模块负责应用程序的加载，task子模块负责应用的执行和切换。另外，不同于批处理操作系统，多道程序操作系统所用的应用程序在内核初始化的时候就一起加载到内存中。

首先，将一些常量分开到os/src/config.rs中，具体代码如下：

```rust
//os/src/config.rs

pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const MAX_APP_NUM: usize = 4;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;
```

然后，复用batch子模块中内核栈和用户栈的代码。需要注意的是内核栈的上下文信息中增加了任务的上下文信息，这部分将在后面任务部分详细说明。

```rust
//os/src/loader.rs

use core::arch::asm;

use crate::trap::TrapContext;
use crate::task::TaskContext;
use crate::config::*;

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [
    KernelStack { data: [0; KERNEL_STACK_SIZE], };
    MAX_APP_NUM
];

static USER_STACK: [UserStack; MAX_APP_NUM] = [
    UserStack { data: [0; USER_STACK_SIZE], };
    MAX_APP_NUM
];

impl KernelStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    pub fn push_context(&self, trap_cx: TrapContext, task_cx: TaskContext) -> &'static mut TaskContext {
        unsafe {
            let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
            *trap_cx_ptr = trap_cx;
            let task_cx_ptr = (trap_cx_ptr as usize - core::mem::size_of::<TaskContext>()) as *mut TaskContext;
            *task_cx_ptr = task_cx;
            task_cx_ptr.as_mut().unwrap()
        }
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}
```

最重要的部分就是加载应用程序。在os/src/loader.rs增加如下代码：

```rust
//os/src/loader.rs

fn get_base_i(app_id: usize) -> usize {
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

pub fn get_num_app() -> usize {
    extern "C" { fn _num_app(); }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

pub fn load_apps() {
    extern "C" { fn _num_app(); }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe {
        core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1)
    };
    // clear i-cache first
    unsafe { asm!("fence.i"); }
    // load apps
    for i in 0..num_app {
        let base_i = get_base_i(i);
        // clear region
        (base_i..base_i + APP_SIZE_LIMIT).for_each(|addr| unsafe {
            (addr as *mut u8).write_volatile(0)
        });
        // load app from data section to memory
        let src = unsafe {
            core::slice::from_raw_parts(app_start[i] as *const u8, app_start[i + 1] - app_start[i])
        };
        let dst = unsafe {
            core::slice::from_raw_parts_mut(base_i as *mut u8, src.len())
        };
        dst.copy_from_slice(src);
    }
}
```

可以看出应用程序被加载到get_base_i计算出来的物理地址上。

![image-20221115112136908](OSLab_4.assets/image-20221115112136908.png)

![image-20221115112311655](OSLab_4.assets/image-20221115112311655.png)


3. 任务的设计与实现

多道程序支持应用程序主动交出CPU的使用权。我们把一个计算执行过程称之为任务。一个应用程序的任务切换到另外一个应用程序的任务称为任务切换。任务切换过程中需要保存的恢复任务重新执行所需的寄存器、栈等内容称为任务的上线文。在批处理操作系统中，已经讲了Trap切换的概念。区别于Trap的切换，任务的切换不涉及到特权级的切换。

（1）任务的上下文
任务切换需要有任务上下文的支持。我们通过TaskContext数据结构来记录任务的上下文信息。

```rust
// os/src/task/context.rs

#[repr(C)]
pub struct TaskContext {
    ra: usize,
    s: [usize; 12],
}

impl TaskContext {
    pub fn goto_restore() -> Self {
        extern "C" { fn __restore(); }
        Self {
            ra: __restore as usize,
            s: [0; 12],
        }
    }
}
```

这里goto_restore通过调用__restore构造一个第一次进入用户态的Trap上下文。需要注意的是和批处理操作系统不一样的是__restore不在需要开头的mv sp, a0，因为后续的任务切换能够保证sp指向正确的地址。

（2）任务的运行状态及任务控制块
为了支持任务切换，我们需要在内核中维护任务的运行状态。具体实现在os/src/task/task.rs中：

```rust
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}
```

但是仅仅有任务的状态还是不够的，内核还需要保存更多的信息在任务控制块中。其定义如下：

```rust
//os/src/task/task.rs

#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    pub task_cx_ptr: usize,
    pub task_status: TaskStatus,
}

impl TaskControlBlock {
    pub fn get_task_cx_ptr2(&self) -> *const usize {
        &self.task_cx_ptr as *const usize
    }
}
```

任务控制块中维护了一个任务上下文的地址指针task_cx_ptr。因为任务切换的时候还需要使用这个指针，所以还提供了获取这个指针的方法get_task_cx_ptr2。

（3）任务切换

任务切换的主要过程是保存一个任务的上下文之后，任务进入暂停状态。同时，恢复另外一个任务的上下文并让其在CPU上继续执行。我们通过汇编代码来实现，具体如下：

```rust
// os/src/task/switch.S

.altmacro
.macro SAVE_SN n
    sd s\n, (\n+1)*8(sp)
.endm
.macro LOAD_SN n
    ld s\n, (\n+1)*8(sp)
.endm
    .section .text
    .globl __switch

__switch:
    # __switch(
    #     current_task_cx_ptr2: &*const TaskContext,
    #     next_task_cx_ptr2: &*const TaskContext
    # )
    # push TaskContext to current sp and save its address to where a0 points to
    addi sp, sp, -13*8
    sd sp, 0(a0)
    # fill TaskContext with ra & s0-s11
    sd ra, 0(sp)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr
    # ready for loading TaskContext a1 points to
    ld sp, 0(a1)
    # load registers in the TaskContext
    ld ra, 0(sp)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # pop TaskContext
    addi sp, sp, 13*8
    ret
```

为了更方便的调用__switch，还需要将它封装为Rust的函数。实现在os/src/task/switch.rs中。

```rust
//os/src/task/switch.rs

use core::arch::global_asm;

global_asm!(include_str!("switch.S"));

extern "C" {
    pub fn __switch(
        current_task_cx_ptr2: *const usize,
        next_task_cx_ptr2: *const usize
    );
}
```

（4）任务管理器

我们还需要一个全局的任务管理器来管理任务控制描述的应用程序。

```rust
//os/src/task/mod.rs

mod context;
mod switch;
mod task;

use crate::config::MAX_APP_NUM;
use crate::loader::{get_num_app, init_app_cx};
use core::cell::RefCell;
use lazy_static::*;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

pub struct TaskManager {
    num_app: usize,
    inner: RefCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

unsafe impl Sync for TaskManager {}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [
            TaskControlBlock { task_cx_ptr: 0, task_status: TaskStatus::UnInit };
            MAX_APP_NUM
        ];
        for i in 0..num_app {
            tasks[i].task_cx_ptr = init_app_cx(i) as * const _ as usize;
            tasks[i].task_status = TaskStatus::Ready;
        }
        TaskManager {
            num_app,
            inner: RefCell::new(TaskManagerInner {
                tasks,
                current_task: 0,
            }),
        }
    };
}

impl TaskManager {
    fn run_first_task(&self) {
        self.inner.borrow_mut().tasks[0].task_status = TaskStatus::Running;
        let next_task_cx_ptr2 = self.inner.borrow().tasks[0].get_task_cx_ptr2();
        let _unused: usize = 0;
        unsafe {
            __switch(
                &_unused as *const _,
                next_task_cx_ptr2,
            );
        }
    }

fn mark_current_suspended(&self) {
    let mut inner = self.inner.borrow_mut();
    let current = inner.current_task;
    inner.tasks[current].task_status = TaskStatus::Ready;
}

fn mark_current_exited(&self) {
    let mut inner = self.inner.borrow_mut();
    let current = inner.current_task;
    inner.tasks[current].task_status = TaskStatus::Exited;
}

fn find_next_task(&self) -> Option<usize> {
    let inner = self.inner.borrow();
    let current = inner.current_task;
    (current + 1..current + self.num_app + 1)
        .map(|id| id % self.num_app)
        .find(|id| {
            inner.tasks[*id].task_status == TaskStatus::Ready
        })
}

fn run_next_task(&self) {
    if let Some(next) = self.find_next_task() {
        let mut inner = self.inner.borrow_mut();
        let current = inner.current_task;
        inner.tasks[next].task_status = TaskStatus::Running;
        inner.current_task = next;
        let current_task_cx_ptr2 = inner.tasks[current].get_task_cx_ptr2();
        let next_task_cx_ptr2 = inner.tasks[next].get_task_cx_ptr2();
        core::mem::drop(inner);
        unsafe {
            __switch(
                current_task_cx_ptr2,
                next_task_cx_ptr2,
            );
        }
    } else {
        panic!("All applications completed!");
    }
}

}

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}
```

这里类似于批处理操作系统中的AppManager，也使用了lazy_static!宏。上述代码还调用了loader子模块的init_app_cx。因此，还需要在os/src/loader.rs增加如下代码：

```rust
// os/src/loader.rs

pub fn init_app_cx(app_id: usize) -> &'static TaskContext {
    KERNEL_STACK[app_id].push_context(
        TrapContext::app_init_context(get_base_i(app_id), USER_STACK[app_id].get_sp()),
        TaskContext::goto_restore(),
    )
}
```

分析此部分代码可以发现KernelStack先压入一个Trap上下文，然后再压入一个任务上下文。而这个任务上下文是通过 TaskContext::goto_restore()构造的。


4. 实现sys_yield和sys_exit系统调用

修改/os/src/syscall/process.rs，内容如下：

```rust
use crate::task::{
    suspend_current_and_run_next,
    exit_current_and_run_next,
};

pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}
```

同时，注意修改os/src/syscall/mod.rs增加sys_yield系统调用的处理。这两个系统调用都是基于task子模块提供的接口实现的。具体增加如下代码：

```rust
const SYSCALL_YIELD: usize = 124;

SYSCALL_YIELD => sys_yield(),
```




5. 修改其他部分代码

我们还需要注释掉trap子模块中run_next_app()部分的代码，也就是注释掉src/trap/mod.rs中run_next_app的代码。同时，特别注意注释掉trap.S中__restore 中的mv sp, a0。

最后，修改main.rs为如下内容：

```rust
#![no_std]
#![no_main]
#![feature(panic_info_message)]

use core::arch::global_asm;

#[macro_use]
mod console;
mod lang_items;
mod sbi;
mod syscall;
mod trap;
mod loader;
mod config;
mod task;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    });
}

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    trap::init();
    loader::load_apps();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}
```

至此，支持协作式调度的多道程序操作系统实现完成。

![image-20221115113300583](OSLab_4.assets/image-20221115113300583.png)

![image-20221115140020801](OSLab_4.assets/image-20221115140020801.png)

## 二、思考问题

### （1）分析应用程序是如何加载的；

config.rs中声明了内核使用的常数。其中，APP_BASE_ADDRESS为0x80400000 ，APP_SIZE_LIMIT为 0x20000 ，从APP_BASE_ADDRESS开始依次为每个应用预留一段空间，这意味着从0x80400000开始为每个应用预留的空间大小为0x20000。由于每个应用被加载到内存中的地址不同，因此他们的连接脚本linker.ld中的BASE_ADDRESS不同。于是通过build.py这个python脚本来设置应用地址，封装后通过loader.rs中的load_apps（）方法实现应用的加载。

### （2）分析多道程序如何设计和实现的；

在task/context.rs中定义了TaskContext标识应用执行上下文，定义TaskConyrolBlock的结构表示应用执行上下文的状态，TaskManager数据结构控制任务的切换和执行。定义sys_yield接口，并于syscall.rs中封装为yield函数。应用程序在用户态执行后，通过系统调用sys_yield使自己主动暂停，TaskManager数据结构中，定义了成员函数run_next_task实现任务控制块的切换，用switch的汇编脚本函数完成底层的任务上下文切换，最后通过sys_exit退出任务，结束。

### （3）分析所实现的多道程序操作系统中的任务是如何实现的，以及它和理论课程里的进程和线程有什么区别和联系。

任务的上下文使用TaskContext结构记录任务的上下文信息，任务的运行状态及任务控制块在内核中维护任务的运行状态，由TaskControlBlock结构表示应用执行上下文的状态，全局的任务管理器来管理任务控制描述的应用程序TaskManager，实现sys_yield来实现任务暂停；sys_exit来实现任务的停止。最后任务切换由switch的汇编脚本函数完成任务上下文切换。此次实验中仅实现多道程序，实质上为进程之间的切换。与线程相比，进程具有独立的地址空间，虽然开销大，但是健壮性要更好。

## 三、Git提交截图

![image-20221115132851951](OSLab_4.assets/image-20221115132851951.png)

![image-20221115132901300](OSLab_4.assets/image-20221115132901300.png)

![image-20221115132914374](OSLab_4.assets/image-20221115132914374.png)

![image-20221115132940872](OSLab_4.assets/image-20221115132940872.png)

![image-20221115133000358](OSLab_4.assets/image-20221115133000358.png)

![image-20221115133017959](OSLab_4.assets/image-20221115133017959.png)

![image-20221115133030242](OSLab_4.assets/image-20221115133030242.png)

![image-20221115133040406](OSLab_4.assets/image-20221115133040406.png)

![image-20221115133051695](OSLab_4.assets/image-20221115133051695.png)

![image-20221115133059388](OSLab_4.assets/image-20221115133059388.png)

![image-20221115133110496](OSLab_4.assets/image-20221115133110496.png)

## 四、其他说明

