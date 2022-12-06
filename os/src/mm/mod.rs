// os/src/mm/mod.rs

mod heap_allocator;
mod page_table;

mod address;
mod frame_allocator;


use page_table::{PTEFlags};
pub use page_table::{PageTableEntry};
pub use address::{PhysAddr, VirtAddr, PhysPageNum, VirtPageNum, StepByOne};
pub use frame_allocator::{FrameTracker, frame_alloc};

pub fn init() {
    heap_allocator::init_heap();
    heap_allocator::heap_test();
    frame_allocator::init_frame_allocator();
    frame_allocator::frame_allocator_test();
}

