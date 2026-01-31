use x86_64::{
    structures::paging::{OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    VirtAddr,
};

pub struct PageTableRoot {
    pml4: PhysFrame<Size4KiB>,
    phys_offset: VirtAddr,
}

impl PageTableRoot {
    pub unsafe fn new(pml4: PhysFrame<Size4KiB>, phys_offset: VirtAddr) -> Self {
        Self { pml4, phys_offset }
    }

    pub unsafe fn mapper(&self) -> OffsetPageTable<'_> {
        let virt =
            self.phys_offset.as_u64() + self.pml4.start_address().as_u64();
        let table = &mut *(virt as *mut PageTable);
        OffsetPageTable::new(table, self.phys_offset)
    }

    pub fn frame(&self) -> PhysFrame<Size4KiB> {
        self.pml4
    }
}
