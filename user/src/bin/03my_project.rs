#![no_std]
#![no_main]

use core::arch::asm;

#[macro_use]
extern crate user_lib;

#[no_mangle]
fn main() -> i32 {
    for i in 1..10 {
        for j in 1..=i {
            print!("{}*{}={}\t", j, i, j * i)
        }
        println!("")
    }
    
    // println!("C!");
    // println!("*****");
    // println!("*");
    // println!("*");
    // println!("*****");
    // unsafe {
    //     asm!("sret");
    // }
    0
}