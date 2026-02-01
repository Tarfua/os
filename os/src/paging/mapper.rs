//! Page mapping utilities
//!
//! Low-level functions for manipulating page tables with proper safety checks.
//!
//! # Safety Principles
//! - All addresses must be validated before mapping
//! - Physical memory offset must be used for accessing physical frames
//! - User/kernel separation must be enforced
//! - TLB must be flushed after mapping changes

use super::{PagingError, PagingResult};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, Page, PageSize, PageTableFlags as Flags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

/// Kernel/user address space split on x86_64 (canonical address boundary)
///
/// Addresses below this are user space, addresses at or above are kernel space.
pub const KERNEL_SPACE_START: u64 = 0xFFFF_8000_0000_0000;

/// Maximum user space address (exclusive)
pub const USER_SPACE_END: u64 = 0x0000_8000_0000_0000;

/// Zeros a physical frame using proper virtual addressing.
///
/// # Safety
/// - Frame must be valid and not currently in use
/// - Physical offset must correctly map physical memory
/// - Caller must ensure no concurrent access to this frame
#[inline]
pub unsafe fn zero_frame(frame: PhysFrame<Size4KiB>, phys_offset: VirtAddr) {
    // CRITICAL: Must use physical offset, not raw physical address
    let virt_addr = phys_offset.as_u64() + frame.start_address().as_u64();
    let ptr = virt_addr as *mut u8;
    
    // Zero the entire frame
    core::ptr::write_bytes(ptr, 0, Size4KiB::SIZE as usize);
}

/// Determines how virtual pages should be mapped to physical frames
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// Identity mapping: virtual address = physical address
    ///
    /// Used for kernel memory where VA == PA for simplicity.
    Identity,

    /// Allocate new physical frames for each page
    ///
    /// Used for most user memory allocations.
    Allocate,
}

/// Validates that an address is suitable for user space mapping.
///
/// Returns `Err` if the address is in kernel space.
#[inline]
pub fn validate_user_address(addr: VirtAddr) -> PagingResult<()> {
    if addr.as_u64() >= USER_SPACE_END {
        return Err(PagingError::KernelAddressInUserSpace { addr });
    }
    Ok(())
}

/// Validates that an address is properly aligned to page boundary.
#[inline]
pub fn validate_alignment(addr: VirtAddr) -> PagingResult<()> {
    if !addr.is_aligned(Size4KiB::SIZE) {
        return Err(PagingError::Misaligned {
            addr,
            required: Size4KiB::SIZE,
        });
    }
    Ok(())
}

/// Validates that a memory region is valid and doesn't overflow.
///
/// Checks:
/// - Size is non-zero
/// - Start + size doesn't overflow
/// - Resulting range is valid
pub fn validate_region(start: VirtAddr, size: u64) -> PagingResult<(VirtAddr, VirtAddr)> {
    // Size must be at least one page
    if size == 0 {
        return Err(PagingError::SizeTooSmall {
            provided: size,
            required: Size4KiB::SIZE,
        });
    }

    // Check for overflow
    let end_addr = start
        .as_u64()
        .checked_add(size)
        .ok_or(PagingError::SizeOverflow { start, size })?;

    let end = VirtAddr::new(end_addr);

    // Ensure the range doesn't span kernel/user boundary
    if start.as_u64() < USER_SPACE_END && end.as_u64() > USER_SPACE_END {
        return Err(PagingError::InvalidRange);
    }

    Ok((start, end))
}

/// Validates page table flags for user space mappings.
///
/// Ensures that user pages have USER_ACCESSIBLE set and don't have
/// inappropriate flags.
#[inline]
pub fn validate_user_flags(flags: Flags) -> PagingResult<()> {
    if !flags.contains(Flags::USER_ACCESSIBLE) {
        return Err(PagingError::InvalidFlags);
    }

    // User pages should not have GLOBAL flag
    if flags.contains(Flags::GLOBAL) {
        return Err(PagingError::InvalidFlags);
    }

    Ok(())
}

/// Validates page table flags for kernel space mappings.
///
/// Ensures that kernel pages don't have USER_ACCESSIBLE set.
#[inline]
pub fn validate_kernel_flags(flags: Flags) -> PagingResult<()> {
    if flags.contains(Flags::USER_ACCESSIBLE) {
        return Err(PagingError::InvalidFlags);
    }

    Ok(())
}

/// Maps a contiguous virtual range to physical memory.
///
/// This is the core mapping function. It performs comprehensive validation
/// and maps pages one at a time with proper TLB flushing.
///
/// # Arguments
/// * `mapper` - Page table mapper
/// * `frame_allocator` - Physical frame allocator
/// * `virt_start` - Starting virtual address (must be page-aligned)
/// * `size` - Size in bytes (will be rounded up to page size)
/// * `flags` - Page table flags for all pages in the range
/// * `map_type` - How to map pages (identity or allocate)
///
/// # Safety
/// - Can create invalid/aliasing mappings if misused
/// - Caller must not overlap existing mappings
/// - Caller must ensure flags are appropriate for the address range
/// - For identity mapping, caller must ensure physical memory is valid
/// - Must not be called concurrently for overlapping regions
///
/// # Errors
/// Returns error if:
/// - Addresses are misaligned
/// - Region is invalid or overflows
/// - Flags are invalid for the address range
/// - Frame allocation fails
/// - Mapping operation fails
pub unsafe fn map_region<M>(
    mapper: &mut M,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    virt_start: VirtAddr,
    size: u64,
    flags: Flags,
    map_type: MapType,
) -> PagingResult<()>
where
    M: Mapper<Size4KiB>,
{
    // Validate alignment
    validate_alignment(virt_start)?;

    // Validate region
    let (_start, _end) = validate_region(virt_start, size)?;

    // Validate flags based on address range
    if virt_start.as_u64() < USER_SPACE_END {
        // User space mapping
        validate_user_flags(flags)?;
    } else {
        // Kernel space mapping
        validate_kernel_flags(flags)?;
    }

    // INVARIANT: flags must always include PRESENT
    if !flags.contains(Flags::PRESENT) {
        return Err(PagingError::InvalidFlags);
    }

    // Calculate number of pages (round up)
    let page_count = (size + Size4KiB::SIZE - 1) / Size4KiB::SIZE;
    let start_page = Page::containing_address(virt_start);

    // Map each page
    for i in 0..page_count {
        let page = start_page + i;

        // Determine physical frame
        let frame = match map_type {
            MapType::Identity => {
                // Identity mapping: VA == PA
                PhysFrame::containing_address(PhysAddr::new(page.start_address().as_u64()))
            }
            MapType::Allocate => {
                // Allocate new frame
                frame_allocator
                    .allocate_frame()
                    .ok_or(PagingError::OutOfFrames)?
            }
        };

        // Perform mapping
        // SAFETY: Caller guarantees this is safe
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| PagingError::MapFailed)?
                .flush(); // Flush TLB for this page
        }
    }

    Ok(())
}

/// Maps a contiguous virtual range and zeros the allocated memory.
///
/// This is a convenience wrapper around `map_region` that also zeros
/// all allocated frames. Useful for allocating clean memory for stacks,
/// heaps, etc.
///
/// # Safety
/// Same safety requirements as `map_region`, plus:
/// - Physical offset must correctly map physical memory for zeroing
pub unsafe fn map_region_zeroed<M>(
    mapper: &mut M,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_offset: VirtAddr,
    virt_start: VirtAddr,
    size: u64,
    flags: Flags,
) -> PagingResult<()>
where
    M: Mapper<Size4KiB>,
{
    // Validate and map
    validate_alignment(virt_start)?;
    let (_start, _end) = validate_region(virt_start, size)?;

    if virt_start.as_u64() < USER_SPACE_END {
        validate_user_flags(flags)?;
    } else {
        validate_kernel_flags(flags)?;
    }

    if !flags.contains(Flags::PRESENT) {
        return Err(PagingError::InvalidFlags);
    }

    let page_count = (size + Size4KiB::SIZE - 1) / Size4KiB::SIZE;
    let start_page = Page::containing_address(virt_start);

    for i in 0..page_count {
        let page = start_page + i;

        // Allocate and zero frame
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(PagingError::OutOfFrames)?;

        // Zero the frame BEFORE mapping it
        // SAFETY: Frame is freshly allocated, no concurrent access
        unsafe {
            zero_frame(frame, phys_offset);
        }

        // Map the zeroed frame
        // SAFETY: Caller guarantees this is safe
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| PagingError::MapFailed)?
                .flush();
        }
    }

    Ok(())
}

// Stage 2B+: Will add unmap_region, remap_region, protect_region, etc.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_user_address() {
        // User space addresses should be valid
        assert!(validate_user_address(VirtAddr::new(0x1000)).is_ok());
        assert!(validate_user_address(VirtAddr::new(0x7FFF_FFFF_FFFF)).is_ok());

        // Kernel space addresses should be invalid for user
        assert!(validate_user_address(VirtAddr::new(KERNEL_SPACE_START)).is_err());
        assert!(validate_user_address(VirtAddr::new(0xFFFF_FFFF_FFFF_FFFF)).is_err());
    }

    #[test]
    fn test_validate_alignment() {
        // Page-aligned addresses
        assert!(validate_alignment(VirtAddr::new(0x0000)).is_ok());
        assert!(validate_alignment(VirtAddr::new(0x1000)).is_ok());
        assert!(validate_alignment(VirtAddr::new(0x2000)).is_ok());

        // Misaligned addresses
        assert!(validate_alignment(VirtAddr::new(0x0001)).is_err());
        assert!(validate_alignment(VirtAddr::new(0x0FFF)).is_err());
        assert!(validate_alignment(VirtAddr::new(0x1001)).is_err());
    }

    #[test]
    fn test_validate_region() {
        // Valid regions
        assert!(validate_region(VirtAddr::new(0x1000), 0x1000).is_ok());
        assert!(validate_region(VirtAddr::new(0x1000), 0x10000).is_ok());

        // Zero size
        assert!(validate_region(VirtAddr::new(0x1000), 0).is_err());

        // Overflow
        assert!(validate_region(VirtAddr::new(u64::MAX - 0x100), 0x1000).is_err());

        // Crossing kernel/user boundary
        assert!(validate_region(
            VirtAddr::new(USER_SPACE_END - 0x1000),
            0x2000
        )
        .is_err());
    }
}
