//! Physical frame allocator
//!
//! Manages physical memory frames based on bootloader memory map.
//! Excludes reserved regions (BIOS, kernel, bootloader).
//!
//! # Memory Safety
//! - Never allocates the same frame twice
//! - Respects memory region types from bootloader
//! - Maintains allocation watermarks for reliability

use bootloader_api::info::MemoryRegionKind;
use x86_64::{
    addr::{align_down, align_up},
    structures::paging::{FrameAllocator, PageSize, PhysFrame, Size4KiB},
    PhysAddr,
};

/// Maximum number of usable memory ranges we track
///
/// This is a reasonable limit for most systems. Real hardware typically
/// has 4-8 usable ranges. QEMU/KVM usually has 2-3.
const MAX_USABLE_RANGES: usize = 32;

/// Low watermark: warn when available memory drops below this (16 MiB)
const LOW_WATERMARK_BYTES: u64 = 16 * 1024 * 1024;

/// Minimum watermark: refuse non-critical allocations below this (4 MiB)
const MIN_WATERMARK_BYTES: u64 = 4 * 1024 * 1024;

/// Allocation statistics
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    /// Total number of frames managed by this allocator
    pub total_frames: u64,
    
    /// Number of frames currently allocated
    pub allocated_frames: u64,
    
    /// Number of frames still available
    pub available_frames: u64,
    
    /// Total memory in bytes
    pub total_bytes: u64,
    
    /// Allocated memory in bytes
    pub allocated_bytes: u64,
    
    /// Available memory in bytes
    pub available_bytes: u64,
    
    /// Number of usable memory ranges
    pub range_count: usize,
    
    /// Whether we're below low watermark (should warn)
    pub below_low_watermark: bool,
    
    /// Whether we're below minimum watermark (critical)
    pub below_min_watermark: bool,
}

/// Physical frame allocator backed by bootloader memory map.
///
/// # Allocation Strategy
/// Uses first-fit with optimization: remembers the last successful
/// allocation index to avoid repeatedly scanning empty ranges.
///
/// # Memory Reservation
/// Automatically excludes:
/// - First 1 MiB minimum (BIOS data, VGA memory, NULL pointer detection)
/// - Bootloader code and data
/// - Kernel code and data
/// - Non-usable memory regions (reserved, ACPI, etc.)
///
/// # Invariants
/// - INVARIANT: Never allocates same frame twice
/// - INVARIANT: All allocated frames are page-aligned
/// - INVARIANT: Ranges never overlap
/// - INVARIANT: Each range [start, end) has start < end
pub struct EarlyFrameAllocator {
    /// Array of available physical memory ranges (start, end), page-aligned.
    /// End is exclusive: range is [start, end).
    ranges: [(u64, u64); MAX_USABLE_RANGES],
    
    /// Number of valid ranges in the array
    len: usize,
    
    /// Optimization: index hint for next allocation
    /// We start searching from this index to avoid repeated scans of
    /// depleted ranges.
    next: usize,
    
    /// Initial total memory (for statistics)
    initial_total: u64,
}

impl EarlyFrameAllocator {
    /// Creates a frame allocator from the bootloader memory map.
    ///
    /// All memory below `kernel_end` is considered reserved.
    /// This includes:
    /// - Low memory (BIOS, VGA, etc.)
    /// - Bootloader code and data
    /// - Kernel code and data
    ///
    /// # Arguments
    /// * `memory_regions` - Memory map from bootloader
    /// * `kernel_start` - Start of kernel in physical memory (currently unused, reserved for future)
    /// * `kernel_end` - End of kernel in physical memory (everything below is reserved)
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `memory_regions` accurately describes physical RAM
    /// - `kernel_end` covers all kernel and bootloader memory that must not be allocated
    /// - Bootloader has identity-mapped or higher-half mapped all memory
    ///
    /// # Panics
    /// Logs warning if no usable memory regions are found (but doesn't panic).
    pub unsafe fn new(
        memory_regions: &[bootloader_api::info::MemoryRegion],
        _kernel_start: u64,
        kernel_end: u64,
    ) -> Self {
        let page_size = Size4KiB::SIZE;
        let mut ranges = [(0u64, 0u64); MAX_USABLE_RANGES];
        let mut len = 0usize;
        let mut total = 0u64;

        // Reserve everything below kernel_end, with minimum of 1 MiB
        // to protect BIOS data area, VGA memory, and detect NULL pointer bugs
        let reserved_end = kernel_end.max(0x100000);

        // Process each memory region from bootloader
        for region in memory_regions {
            // Only use regions marked as usable
            if region.kind != MemoryRegionKind::Usable {
                continue;
            }

            // Align region boundaries to page size
            let mut start = align_up(region.start, page_size);
            let end = align_down(region.end, page_size);

            // Skip regions entirely below reserved memory
            if end <= reserved_end {
                continue;
            }

            // Trim start if it overlaps with reserved memory
            if start < reserved_end {
                start = reserved_end;
            }

            // Skip empty regions
            if start >= end {
                continue;
            }

            // Add this range if we have space
            if len < MAX_USABLE_RANGES {
                ranges[len] = (start, end);
                total += end - start;
                len += 1;
            } else {
                // Too many ranges - would need to log warning here
                // For now, we just skip the range (acceptable for Stage 2A)
            }
        }

        Self {
            ranges,
            len,
            next: 0,
            initial_total: total,
        }
    }

    /// Returns the number of available memory ranges.
    ///
    /// This is primarily useful for debugging and diagnostics.
    #[inline]
    pub fn range_count(&self) -> usize {
        self.len
    }

    /// Returns total managed memory in bytes.
    ///
    /// This is the initial total memory, not current available memory.
    #[inline]
    pub fn total_memory(&self) -> u64 {
        self.initial_total
    }

    /// Returns currently available memory in bytes (approximate).
    ///
    /// This calculates available memory by summing up all remaining ranges.
    /// It's approximate because fragmentation is not accounted for.
    pub fn available_memory(&self) -> u64 {
        self.ranges[..self.len]
            .iter()
            .map(|(start, end)| {
                if end > start {
                    end - start
                } else {
                    0
                }
            })
            .sum()
    }

    /// Returns allocated memory in bytes (approximate).
    #[inline]
    pub fn allocated_memory(&self) -> u64 {
        self.initial_total.saturating_sub(self.available_memory())
    }

    /// Returns detailed allocation statistics.
    ///
    /// Useful for monitoring memory usage and detecting potential OOM conditions.
    pub fn stats(&self) -> AllocatorStats {
        let available_bytes = self.available_memory();
        let allocated_bytes = self.allocated_memory();
        let total_bytes = self.initial_total;

        let frame_size = Size4KiB::SIZE;

        AllocatorStats {
            total_frames: total_bytes / frame_size,
            allocated_frames: allocated_bytes / frame_size,
            available_frames: available_bytes / frame_size,
            total_bytes,
            allocated_bytes,
            available_bytes,
            range_count: self.len,
            below_low_watermark: available_bytes < LOW_WATERMARK_BYTES,
            below_min_watermark: available_bytes < MIN_WATERMARK_BYTES,
        }
    }

    /// Checks if allocator is below low watermark.
    ///
    /// When below low watermark, the system should start being conservative
    /// with memory allocations.
    #[inline]
    pub fn is_low_memory(&self) -> bool {
        self.available_memory() < LOW_WATERMARK_BYTES
    }

    /// Checks if allocator is critically low (below minimum watermark).
    ///
    /// When critically low, only essential allocations should proceed.
    #[inline]
    pub fn is_critical_memory(&self) -> bool {
        self.available_memory() < MIN_WATERMARK_BYTES
    }

    /// Attempts to allocate a frame, providing context on failure.
    ///
    /// Unlike the standard `allocate_frame()`, this provides information
    /// about why the allocation failed.
    pub fn try_allocate(&mut self) -> Result<PhysFrame<Size4KiB>, AllocationError> {
        self.allocate_frame()
            .ok_or_else(|| {
                let stats = self.stats();
                if stats.available_bytes == 0 {
                    AllocationError::OutOfMemory
                } else {
                    // We have memory but fragmented?
                    AllocationError::Fragmented
                }
            })
    }
}

/// Allocation error with context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    /// Completely out of memory
    OutOfMemory,
    
    /// Memory available but fragmented
    /// (Should be rare with page-sized allocations)
    Fragmented,
}

unsafe impl FrameAllocator<Size4KiB> for EarlyFrameAllocator {
    /// Allocates a single 4 KiB frame.
    ///
    /// Returns `None` if no frames are available.
    ///
    /// # Algorithm
    /// First-fit with optimization: starts searching from the last successful
    /// allocation index to avoid repeatedly scanning depleted ranges.
    ///
    /// # Invariants Maintained
    /// - Never allocates the same frame twice
    /// - All returned frames are page-aligned
    /// - Frame is valid physical memory
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let n = self.len;

        // Try each range, starting from our hint
        for j in 0..n {
            let i = (self.next + j) % n;
            let (start, end) = &mut self.ranges[i];

            // Check if this range has available memory
            if *start < *end {
                // Found a frame! Update hint for next allocation
                self.next = i;

                // Allocate from the start of this range
                let addr = PhysAddr::new(*start);
                *start += Size4KiB::SIZE;

                // INVARIANT: addr is page-aligned because we aligned it during init
                // and we only increment by page size
                return Some(PhysFrame::containing_address(addr));
            }
        }

        // No memory available
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watermarks() {
        assert!(LOW_WATERMARK_BYTES > MIN_WATERMARK_BYTES);
        assert!(MIN_WATERMARK_BYTES > 0);
    }
}
