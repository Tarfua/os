//! Paging: structures and safe API over bootloader's page tables.
//!
//! Uses x86_64 types (PML4, PDPT, PD, PT via OffsetPageTable). All table/CR3
//! changes are in `unsafe` blocks; public API is map_region and init.

use bootloader_api::info::MemoryRegionKind;
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable,
        PageTableFlags as Flags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};
use x86_64::registers::control::Cr3;

const MAX_USABLE_RANGES: usize = 32;

/// Frame allocator backed by BootInfo memory map (Usable regions only, kernel range excluded).
pub struct BootInfoFrameAllocator {
    /// (start, end) physical addresses, page-aligned; end exclusive.
    ranges: [(u64, u64); MAX_USABLE_RANGES],
    len: usize,
}

impl BootInfoFrameAllocator {
    /// Builds allocator from boot_info. Excludes kernel and non-Usable regions.
    ///
    /// # Safety
    /// Caller must ensure `boot_info` is valid and memory_regions describe real physical RAM.
    pub unsafe fn new(
        memory_regions: &[bootloader_api::info::MemoryRegion],
        kernel_start: u64,
        kernel_end: u64,
    ) -> Self {
        let page_size = Size4KiB::SIZE;
        let mut ranges = [(0u64, 0u64); MAX_USABLE_RANGES];
        let mut len = 0usize;

        for region in memory_regions {
            if region.kind != MemoryRegionKind::Usable {
                continue;
            }
            let start = (region.start + page_size - 1) / page_size * page_size;
            let end = (region.end / page_size) * page_size;
            if start >= end {
                continue;
            }

            if kernel_end <= start || kernel_start >= end {
                if len < MAX_USABLE_RANGES {
                    ranges[len] = (start, end);
                    len += 1;
                }
            } else {
                let k_start = kernel_start.max(start).min(end);
                let k_end = kernel_end.max(start).min(end);
                if start < k_start && len < MAX_USABLE_RANGES {
                    ranges[len] = (start, k_start);
                    len += 1;
                }
                if k_end < end && len < MAX_USABLE_RANGES {
                    ranges[len] = (k_end, end);
                    len += 1;
                }
            }
        }

        Self { ranges, len }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        for i in 0..self.len {
            let (start, end) = &mut self.ranges[i];
            if *start < *end {
                let addr = PhysAddr::new(*start);
                *start += Size4KiB::SIZE;
                return Some(PhysFrame::containing_address(addr));
            }
        }
        None
    }
}

/// Result of paging init: mapper over bootloader tables and a frame allocator.
pub struct PagingState {
    pub mapper: OffsetPageTable<'static>,
    pub frame_allocator: BootInfoFrameAllocator,
}

/// Initializes paging state using the current CR3 (bootloader's tables). Identity mapping.
///
/// # Safety
/// Caller must ensure: paging is enabled, CR3 points to a valid PML4, and physical memory
/// at the PML4 frame is identity-mapped so we can read/write page tables.
pub unsafe fn init(boot_info: &'static bootloader_api::BootInfo) -> Option<PagingState> {
    let (frame, _) = Cr3::read();
    let pml4_phys = frame.start_address();
    let level_4_table: &'static mut PageTable = unsafe { &mut *(pml4_phys.as_u64() as *mut PageTable) };
    let mapper = unsafe { OffsetPageTable::new(level_4_table, VirtAddr::new(0)) };

    let kernel_start = boot_info.kernel_addr / Size4KiB::SIZE * Size4KiB::SIZE;
    let kernel_end = (boot_info.kernel_addr + boot_info.kernel_len + Size4KiB::SIZE - 1)
        / Size4KiB::SIZE
        * Size4KiB::SIZE;
    let frame_allocator = unsafe {
        BootInfoFrameAllocator::new(
            &*boot_info.memory_regions,
            kernel_start,
            kernel_end,
        )
    };

    Some(PagingState {
        mapper,
        frame_allocator,
    })
}

/// Maps a contiguous virtual range to physical frames. Caller must not overlap existing mappings.
///
/// # Safety
/// Unsafe: can create aliasing or invalid mappings if used incorrectly.
#[allow(dead_code)]
pub unsafe fn map_region(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BootInfoFrameAllocator,
    virt_start: VirtAddr,
    size: u64,
    flags: Flags,
) -> Result<(), &'static str> {
    let page_count = (size + Size4KiB::SIZE - 1) / Size4KiB::SIZE;
    let start_page = Page::containing_address(virt_start);
    for i in 0..page_count {
        let page = start_page + i as u64;
        let frame = frame_allocator.allocate_frame().ok_or("out of frames")?;
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| "map_to failed")?
                .flush();
        }
    }
    Ok(())
}
