//os /src/timer.rs
use riscv::register::time;

pub fn get_time() -> usize {
    time::read()
}

const MSEC_PER_SEC: usize = 1000;

pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}