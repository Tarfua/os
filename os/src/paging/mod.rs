//! Paging: structures and safe API over bootloader's page tables.
//!
//! Stage 2A: AddressSpace as first-class object (id + root PML4). Wrapper
//! around OffsetPageTable; kernel space identity-mapped in each AS.

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

/// Paging operation errors. Stage 2 will add OutOfMemory, MapConflict, InvalidAddress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    InvalidCr3,
    OutOfFrames,
    MapFailed,
}

/// Frame allocator backed by BootInfo memory map (Usable regions only, kernel range excluded).
pub struct BootInfoFrameAllocator {
    /// (start, end) physical addresses, page-aligned; end exclusive.
    ranges: [(u64, u64); MAX_USABLE_RANGES],
    len: usize,
}

impl BootInfoFrameAllocator {
    /// Builds allocator from boot_info. Excludes kernel and non-Usable regions.
    ///
    /// Assumes bootloader memory regions are non-overlapping.
    /// Ranges are consumed linearly; ordering is not guaranteed.
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

/// Opaque identifier for an address space. Stage 2A metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressSpaceId(pub u64);

/// One address space = one root page table (PML4). Wrapper around OffsetPageTable.
/// Kernel space is identity-mapped in each AS. No shared user mappings.
pub struct AddressSpace {
    pub id: AddressSpaceId,
    root_frame: PhysFrame<Size4KiB>,
}

impl AddressSpace {
    /// Returns a mapper for this address space. Caller must ensure the frame is
    /// identity-mapped (physical = virtual) so the table is accessible.
    ///
    /// # Safety
    /// Root frame must be identity-mapped in the current page tables.
    #[inline]
    pub unsafe fn mapper_mut(&mut self) -> OffsetPageTable<'_> {
        let table = &mut *(self.root_frame.start_address().as_u64() as *mut PageTable);
        OffsetPageTable::new(table, VirtAddr::new(0))
    }

    /// Physical frame of the root PML4 (for loading into CR3 on switch).
    #[inline]
    pub fn root_frame(&self) -> PhysFrame<Size4KiB> {
        self.root_frame
    }
}

/// Result of paging init: kernel address space and frame allocator.
pub struct PagingState {
    pub kernel_space: AddressSpace,
    pub frame_allocator: BootInfoFrameAllocator,
}

/// Creates a new address space with kernel range identity-mapped. Allocates a new PML4
/// and maps [kernel_start, kernel_end) as identity (virt = phys). No user mappings.
///
/// # Safety
/// Caller must ensure kernel physical range is identity-mapped in the current tables
/// so the new PML4 frame is accessible when building mappings.
pub unsafe fn create_address_space(
    id: AddressSpaceId,
    frame_allocator: &mut BootInfoFrameAllocator,
    kernel_start: u64,
    kernel_end: u64,
) -> Result<AddressSpace, PagingError> {
    let root_frame = frame_allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
    let dst = root_frame.start_address().as_u64() as *mut u8;
    unsafe { core::ptr::write_bytes(dst, 0, Size4KiB::SIZE as usize) };

    let table = unsafe { &mut *(root_frame.start_address().as_u64() as *mut PageTable) };
    let mut mapper = OffsetPageTable::new(table, VirtAddr::new(0));

    let page_size = Size4KiB::SIZE;
    let start_page = (kernel_start / page_size) * page_size;
    let end_page = ((kernel_end + page_size - 1) / page_size) * page_size;

    let flags = Flags::PRESENT | Flags::WRITABLE;
    let mut addr = start_page;
    while addr < end_page {
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(addr));
        let frame = PhysFrame::containing_address(PhysAddr::new(addr));
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| PagingError::MapFailed)?
                .flush();
        }
        addr += page_size;
    }

    Ok(AddressSpace {
        id,
        root_frame,
    })
}

/// Returns the currently active PML4 (CR3). Localizes UB; Stage 2 can replace with clone PML4.
///
/// # Safety
/// Caller must ensure: paging is enabled, CR3 points to a valid PML4, and that physical
/// frame is identity-mapped so the returned pointer is dereferenceable.
pub unsafe fn active_pml4() -> Result<&'static mut PageTable, PagingError> {
    let (frame, _) = Cr3::read();
    let pml4_phys = frame.start_address();
    // CR3 is never 0 on a live system; this error will almost never trigger.
    // Kept as an assert-like guard — harmless and documents the invariant.
    if pml4_phys.as_u64() == 0 {
        return Err(PagingError::InvalidCr3);
    }
    Ok(unsafe { &mut *(pml4_phys.as_u64() as *mut PageTable) })
}

/// Initializes paging state: allocates kernel-owned PML4, copies bootloader tables into it,
/// switches CR3 to it. Kernel space remains identity-mapped.
///
/// # Safety
/// Caller must ensure: paging is enabled, CR3 points to a valid PML4, and physical memory
/// at the PML4 frame is identity-mapped so we can read/write page tables.
pub unsafe fn init(boot_info: &'static bootloader_api::BootInfo) -> Result<PagingState, PagingError> {
    let level_4_table = active_pml4()?;

    let kernel_start = boot_info.kernel_addr / Size4KiB::SIZE * Size4KiB::SIZE;
    let kernel_end = (boot_info.kernel_addr + boot_info.kernel_len + Size4KiB::SIZE - 1)
        / Size4KiB::SIZE
        * Size4KiB::SIZE;
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::new(
            boot_info.memory_regions.as_ref(),
            kernel_start,
            kernel_end,
        )
    };

    let kernel_root_frame = frame_allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
    let src = level_4_table as *const PageTable as *const u8;
    let dst = kernel_root_frame.start_address().as_u64() as *mut u8;
    unsafe { core::ptr::copy_nonoverlapping(src, dst, Size4KiB::SIZE as usize) };

    let (_frame, flags) = Cr3::read();
    Cr3::write(kernel_root_frame, flags);

    Ok(PagingState {
        kernel_space: AddressSpace {
            id: AddressSpaceId(0),
            root_frame: kernel_root_frame,
        },
        frame_allocator,
    })
}

/// Kernel-only mapping. Never use for user space.
/// Do not use `Flags::USER_ACCESSIBLE` here. Stage 2 will provide `map_user_region` and `map_kernel_region`.
///
/// Maps a contiguous virtual range to physical frames. Caller must not overlap existing mappings.
///
/// # Safety
/// Unsafe: can create aliasing or invalid mappings if used incorrectly.
#[allow(dead_code)]
pub unsafe fn map_region<M>(
    mapper: &mut M,
    frame_allocator: &mut BootInfoFrameAllocator,
    virt_start: VirtAddr,
    size: u64,
    flags: Flags,
) -> Result<(), PagingError>
where
    M: Mapper<Size4KiB>,
{
    let page_count = (size + Size4KiB::SIZE - 1) / Size4KiB::SIZE;
    let start_page = Page::containing_address(virt_start);
    for i in 0..page_count {
        let page = start_page + i as u64;
        let frame = frame_allocator.allocate_frame().ok_or(PagingError::OutOfFrames)?;
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| PagingError::MapFailed)?
                .flush();
        }
    }
    Ok(())
}
