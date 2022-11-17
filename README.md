# 2022秋《操作系统》课程实验报告实验-1

20301174-万兴全

## 一、实验步骤 

本实验的主要目的是构建一个独立的不依赖于rust标准库的可执行程序。

1. 创建一个Rust项目

首先进入已配置的好环境的容器，并进入/mnt目录。

```sh
cd mnt
cargo new os --bin

```

![image-20221030135830652](OSLab_1.assets/image-20221030135830652.png)

之前创过了但是没整对，重做一次可以

![image-20221101104513205](OSLab_1.assets/image-20221101104513205.png)

执行如下命令查看应用运行结果。

```sh
cd os
cargo build
cargo run
```

![image-20221101104556351](OSLab_1.assets/image-20221101104556351.png)

2. 移除标准库依赖

（1）首先，需要修改target为riscv64
在os/.cargo目录下创建config文件，并增加如下内容：

```sh
os/.cargo/config

[build]
target = "riscv64gc-unknown-none-elf"
```

![image-20221101104829285](OSLab_1.assets/image-20221101104829285.png)

（2）修改main.rs文件
在 main.rs 的开头分别加入如下内容：

```rust
#![no_std]
#![no_main]
```

![image-20221031234849903](OSLab_1.assets/image-20221031234849903.png)

![image-20221101105000542](OSLab_1.assets/image-20221101105000542.png)

同时，因为标准库 std 中提供了 panic 的处理函数 #[panic_handler]，所以还需要实现panic handler。
具体增加如下内容：

```rust
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

![image-20221031234957559](OSLab_1.assets/image-20221031234957559.png)

注意还需要删除main函数。

![image-20221031235015081](OSLab_1.assets/image-20221031235015081.png)

修改完后执行cargo build进行编译。如若出现编译错误，请尝试执行如下命令安装相关软件包。

```sh
rustup target add riscv64gc-unknown-none-elf
cargo install cargo-binutils
rustup component add llvm-tools-preview
rustup component add rust-src
```

![image-20221101110848341](OSLab_1.assets/image-20221101110848341.png)

修改完成的可以编译通过的代码如下：

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

雀食是这样

修改完成代码后，请提交git，并注意在commit的注释中加入自己的学号。
同时，建议在自己本机提交，而不是在docker镜像中提交。

![image-20221101110940409](OSLab_1.assets/image-20221101110940409.png)

![image-20221101111102824](OSLab_1.assets/image-20221101111102824.png)



（3）分析独立的可执行程序
执行如下命令分析移除标准库后的独立可执行程序。

```sh
file target/riscv64gc-unknown-none-elf/debug/os
rust-readobj -h target/riscv64gc-unknown-none-elf/debug/os
rust-objdump -S target/riscv64gc-unknown-none-elf/debug/os
```

通过分析可以发现编译生成的二进制程序是一个空程序，这是因为编译器找不到入口函数，所以没有生成后续的代码。

![image-20221101111138109](OSLab_1.assets/image-20221101111138109.png)

3. 用户态可执行的环境

（1）增加入口函数
我们还需要增加入口函数，rust编译器要找的入口函数是 _start() 。
因此，我们可以在main.rs中增加如下内容：

```rust
#[no_mangle]
extern "C" fn _start() {
    loop{};
}
```

然后重新编译。

![image-20221101111223490](OSLab_1.assets/image-20221101111223490.png)

接着，通过如下命令：

```sh
qemu-riscv64 target/riscv64gc-unknown-none-elf/debug/os
```

![image-20221101111319345](OSLab_1.assets/image-20221101111319345.png)

执行编译生成的程序，可以发现是在执行一个死循环，也即无任何输出，程序也不结束。

![image-20221101111400132](OSLab_1.assets/image-20221101111400132.png)

如果把loop注释掉，然后重新编译执行的话，会发现出现了Segmentation fault。这是因为目前程序还缺少一个正确的退出机制。



![image-20221101003203786](OSLab_1.assets/image-20221101003203786.png)

![image-20221101111508198](OSLab_1.assets/image-20221101111508198.png)

接着，我们实现程序的退出机制。

（2）实现退出机制
实现应用程序退出，在main.rs中增加如下代码：

```rust
use core::arch::asm;

const SYSCALL_EXIT: usize = 93;

fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!("ecall",
             in("x10") args[0],
             in("x11") args[1],
             in("x12") args[2],
             in("x17") id,
             lateout("x10") ret
        );
    }
    ret
}

pub fn sys_exit(xstate: i32) -> isize {
    syscall(SYSCALL_EXIT, [xstate as usize, 0, 0])
}

#[no_mangle]
extern "C" fn _start() {
    sys_exit(9);
}
```

![image-20221101003432081](OSLab_1.assets/image-20221101003432081.png)



修改完后，再重新编译和执行就可以发现程序能够正常退出了。

![image-20221101111717361](OSLab_1.assets/image-20221101111717361.png)

（3）实现输出支持
首先，封装一下对SYSCALL_WRITE系统调用。这个是Linux操作系统内核提供的系统调用，其ID就是SYSCALL_WRITE。

```rust
const SYSCALL_WRITE: usize = 64;

pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
  syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}
```

然后，实现基于 Write Trait 的数据结构，并完成 Write Trait 所需要的 write_str 函数，并用 print 函数进行包装。

```rust
struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        sys_write(1, s.as_bytes());
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}
```



最后，实现基于 print 函数，实现Rust语言 格式化宏 ( formatting macros )。

```rust
use core::fmt::{self, Write};

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}
```



同时在入口函数_start增加println输出。

```rust
println!("Hello, world!");
```

![image-20221101005604231](OSLab_1.assets/image-20221101005604231.png)

编译并通过如下命令执行，就可以看到独立的可执行程序已经支持输出显示了。

```sh
qemu-riscv64 target/riscv64gc-unknown-none-elf/debug/os
```

![image-20221101112158770](OSLab_1.assets/image-20221101112158770.png)

## 二、思考问题

### （1）  为什么称最后实现的程序为独立的可执行程序，它和标准的程序有什么区别？

最后实现的这个程序是独立于操作系统的可执行程序，意味着所有依赖于操作系统的库我们都不使用，但是不依赖于操作系统的Rust语言特性还是可以使用的；

而标准的程序可以使用依赖于操作系统的库。

### （2）  实现和编译独立可执行程序的目的是什么？

我们的计划是用rust语言编写自己的操作系统，我们就不应该使用任何与操作系统相关的库，因此我们必须禁用标准库引用。我们的目的就是让我们的程序不依赖操作系统可以独立运行，从而实现一个我们自己的操作系统。

## 三、Git提交截图

仅本地git记录

![image-20221101112404495](OSLab_1.assets/image-20221101112404495.png)

![image-20221101112437383](OSLab_1.assets/image-20221101112437383.png)

![image-20221101112451194](OSLab_1.assets/image-20221101112451194.png)

## 四、其他说明

做了两遍，第一次没连着做，后面把os目录放容器的根目录下了。。。不过结果不太影响；

后来又重做一次，完全符合要求（在mnt目录），用时45分钟。

