#![no_std]
#![no_main]

// use core::arch::asm;

#[macro_use]
extern crate user_lib;

#[no_mangle]
fn main() -> i32 {
    println!("I will find prime numbers from 1 to 100:");
    const N: i32 = 100;
    let mut count = 0;
    for i in 2..=N {
        let mut temp = true;
        for j in 2..i / 2 + 1 {
            if i % j == 0 {
                temp = false;
                break;
            }
        }
        if temp {
            count += 1;
            print!("{} ", i);
            if count % 5 == 0 {
                println!("");
            }
        }
    }
    0
}