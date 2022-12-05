// os/src/mm/mod.rs

mod heap_allocator;
mod page_table;

use page_table::{PTEFlags};


pub fn init() {
    heap_allocator::init_heap();
    heap_allocator::heap_test();
}

pub use page_table::{PageTableEntry};