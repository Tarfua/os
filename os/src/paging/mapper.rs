//! Page mapping utilities
//!
//! Low-level functions for manipulating page tables.

use super::{PagingError, PagingResult};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, Page, PageSize, PageTableFlags as Flags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

/// Zeros a physical frame.
///
/// # Safety
/// Frame must be valid and not currently in use.
#[inline]
pub unsafe fn zero_frame(frame: PhysFrame<Size4KiB>) {
    let ptr = frame.start_address().as_u64() as *mut u8;
    core::ptr::write_bytes(ptr, 0, Size4KiB::SIZE as usize);
}

/// Maps a contiguous virtual range.
///
/// If `identity` is true, each virtual page maps to the same physical address.
/// Otherwise, allocates new physical frames.
///
/// # Safety
/// - Can create invalid/aliasing mappings if misused
/// - Caller must not overlap existing mappings
/// - Use USER_ACCESSIBLE only for user space
pub enum MapType {
    Identity,
    Allocate,
}

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
    let page_count = (size + Size4KiB::SIZE - 1) / Size4KiB::SIZE;
    let start_page = Page::containing_address(virt_start);

    for i in 0..page_count {
        let page = start_page + i;
        let frame = match map_type {
            MapType::Identity => {
                PhysFrame::containing_address(
                    PhysAddr::new(page.start_address().as_u64())
                )
            }
            MapType::Allocate => {
                frame_allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?
            }
        };

        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| PagingError::MapFailed)?
                .flush();
        }
    }

    Ok(())
}

// Stage 2B+: Will add unmap_region, remap_region, etc.
