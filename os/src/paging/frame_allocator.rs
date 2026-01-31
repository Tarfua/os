//! Physical frame allocator
//!
//! Manages physical memory frames based on bootloader memory map.
//! Excludes reserved regions (BIOS, kernel, bootloader).

use bootloader_api::info::MemoryRegionKind;
use x86_64::{
    addr::{align_down, align_up},
    structures::paging::{FrameAllocator, PageSize, PhysFrame, Size4KiB},
    PhysAddr,
};

const MAX_USABLE_RANGES: usize = 32;

/// Physical frame allocator backed by bootloader memory map.
///
/// Only uses regions marked as `Usable`. Automatically excludes:
/// - First 1 MiB minimum (BIOS data, VGA memory, NULL detection)
/// - Bootloader code and data
/// - Kernel code and data
pub struct EarlyFrameAllocator {
    /// Array of available physical memory ranges (start, end), page-aligned.
    /// End is exclusive.
    ranges: [(u64, u64); MAX_USABLE_RANGES],
    /// Number of valid ranges
    len: usize,
    /// Optimization: index to try first on next allocation
    next: usize,
}

impl EarlyFrameAllocator {
    /// Creates a frame allocator from the bootloader memory map.
    ///
    /// All memory below `kernel_end` is reserved (BIOS, bootloader, kernel).
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `memory_regions` accurately describes physical RAM
    /// - `kernel_end` covers all kernel and bootloader memory
    pub unsafe fn new(
        memory_regions: &[bootloader_api::info::MemoryRegion],
        _kernel_start: u64,
        kernel_end: u64,
    ) -> Self {
        let page_size = Size4KiB::SIZE;
        let mut ranges = [(0u64, 0u64); MAX_USABLE_RANGES];
        let mut len = 0usize;

        // Reserve everything below kernel_end (minimum 1 MiB)
        let reserved_end = kernel_end.max(0x100000);

        for region in memory_regions {
            if region.kind != MemoryRegionKind::Usable {
                continue;
            }

            let mut start = align_up(region.start, page_size);
            let end = align_down(region.end, page_size);

            if end <= reserved_end {
                continue;
            }

            if start < reserved_end {
                start = reserved_end;
            }

            if start >= end {
                continue;
            }

            if len < MAX_USABLE_RANGES {
                ranges[len] = (start, end);
                len += 1;
            }
        }

        Self { ranges, len, next: 0 }
    }

    /// Returns the number of available memory ranges.
    #[inline]
    pub fn range_count(&self) -> usize {
        self.len
    }

    /// Returns total available memory in bytes (approximate).
    pub fn total_memory(&self) -> u64 {
        self.ranges[..self.len]
            .iter()
            .map(|(start, end)| end - start)
            .sum()
    }

    /// Returns allocated memory in bytes (approximate).
    pub fn allocated_memory(&self) -> u64 {
        let total = self.total_memory();
        let remaining: u64 = self.ranges[..self.len]
            .iter()
            .map(|(start, end)| end - start)
            .sum();
        total - remaining
    }
}

unsafe impl FrameAllocator<Size4KiB> for EarlyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let n = self.len;
        for j in 0..n {
            let i = (self.next + j) % n;
            let (start, end) = &mut self.ranges[i];
            if *start < *end {
                self.next = i;
                let addr = PhysAddr::new(*start);
                *start += Size4KiB::SIZE;
                return Some(PhysFrame::containing_address(addr));
            }
        }
        None
    }
}
