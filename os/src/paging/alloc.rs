use x86_64::structures::paging::{PhysFrame, Size4KiB};

pub trait PhysAllocator {
    fn alloc(&mut self) -> Option<PhysFrame<Size4KiB>>;
}

impl PhysAllocator for EarlyFrameAllocator {
    fn alloc(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate_frame()
    }
}
