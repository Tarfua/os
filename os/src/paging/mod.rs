//! Paging: Address Space Abstraction for Stage 2A
//!
//! Stage 2A Goal: Establish fundamental execution units and memory isolation.
//!
//! This module provides:
//! - `AddressSpace` as a first-class kernel object
//! - Each address space owns exactly one root page table (PML4)
//! - Kernel space is identity-mapped in all address spaces
//! - User space mappings are fully isolated per address space
//!
//! Stage 2A Constraints:
//! - No shared user mappings
//! - No lazy mapping or copy-on-write
//! - No demand paging
//!
//! Stage 2A Result:
//! - Kernel can create, destroy, and switch address spaces safely

use bootloader_api::info::MemoryRegionKind;
use x86_64::addr::{align_down, align_up};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable,
        PageTableFlags as Flags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};
use x86_64::registers::control::Cr3;

const MAX_USABLE_RANGES: usize = 32;

/// Paging operation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// CR3 contains invalid or zero physical address
    InvalidCr3,
    /// Frame allocator has no more frames available
    OutOfFrames,
    /// Page mapping operation failed (overlap or invalid parameters)
    MapFailed,
}

/// Physical frame allocator backed by bootloader memory map.
///
/// Only uses regions marked as `Usable` by the bootloader. Automatically excludes:
/// - First 1 MiB minimum (BIOS data, VGA memory, NULL detection)
/// - Bootloader code and data
/// - Kernel code and data
///
/// Frame allocation is sequential within each range for simplicity.
pub struct BootInfoFrameAllocator {
    /// Array of available physical memory ranges (start, end), page-aligned.
    /// End is exclusive.
    ranges: [(u64, u64); MAX_USABLE_RANGES],
    /// Number of valid ranges
    len: usize,
    /// Optimization: index to try first on next allocation
    next: usize,
}

impl BootInfoFrameAllocator {
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
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
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

/// Opaque identifier for an address space.
///
/// Stage 2A: Simple numeric ID. Later stages may extend this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(pub u64);

/// Address Space: isolated virtual memory context.
///
/// Each `AddressSpace` owns exactly one root page table (PML4).
/// Kernel space is identity-mapped in all address spaces.
/// User space mappings are fully isolated.
///
/// Stage 2A: Manual switching only (no scheduling).
pub struct AddressSpace {
    /// Unique identifier
    pub id: AddressSpaceId,
    /// Physical frame of root PML4
    root_frame: PhysFrame<Size4KiB>,
    /// Virtual offset for physical memory access (0 = identity)
    kernel_offset: VirtAddr,
}

impl AddressSpace {
    /// Returns a mutable page table mapper for this address space.
    ///
    /// # Safety
    /// Caller must ensure the root PML4 frame is currently accessible
    /// (identity-mapped or via kernel_offset).
    #[inline]
    pub unsafe fn mapper_mut(&mut self) -> OffsetPageTable<'_> {
        let virt_addr = self.kernel_offset.as_u64() + self.root_frame.start_address().as_u64();
        let table = unsafe { &mut *(virt_addr as *mut PageTable) };
        OffsetPageTable::new(table, self.kernel_offset)
    }

    /// Returns the physical frame of the root PML4.
    ///
    /// Used for loading into CR3 during context switches.
    #[inline]
    pub fn root_frame(&self) -> PhysFrame<Size4KiB> {
        self.root_frame
    }

    /// Returns the kernel offset for this address space.
    #[inline]
    pub fn kernel_offset(&self) -> VirtAddr {
        self.kernel_offset
    }

    /// Switches to this address space by loading its PML4 into CR3.
    ///
    /// # Safety
    /// - Caller must ensure kernel code/data remains mapped after switch
    /// - This operation invalidates TLB entries
    /// - Must be called from kernel context
    #[inline]
    pub unsafe fn switch_to(&self) {
        let (_old_frame, flags) = Cr3::read();
        Cr3::write(self.root_frame, flags);
    }

    /// Creates a new isolated address space.
    ///
    /// The new address space will have kernel memory identity-mapped but
    /// no user mappings. Suitable for creating new processes.
    ///
    /// # Safety
    /// - Caller must ensure kernel_start/kernel_end describe valid kernel region
    /// - Kernel region must be identity-mapped in current page tables
    pub unsafe fn create(
        id: AddressSpaceId,
        frame_allocator: &mut BootInfoFrameAllocator,
        kernel_offset: VirtAddr,
        kernel_start: u64,
        kernel_end: u64,
    ) -> Result<Self, PagingError> {
        // Allocate and zero new PML4
        let root_frame = frame_allocator
            .allocate_frame()
            .ok_or(PagingError::OutOfFrames)?;
        zero_frame(root_frame);

        // Set up mapper for new address space
        let virt_addr = kernel_offset.as_u64() + root_frame.start_address().as_u64();
        let table = unsafe { &mut *(virt_addr as *mut PageTable) };
        let mut mapper = OffsetPageTable::new(table, kernel_offset);

        // Map kernel space (identity mapping)
        map_region(
            &mut mapper,
            frame_allocator,
            VirtAddr::new(kernel_start),
            kernel_end - kernel_start,
            Flags::PRESENT | Flags::WRITABLE,
            true, // identity
        )?;

        Ok(AddressSpace {
            id,
            root_frame,
            kernel_offset,
        })
    }
}

/// Result of paging initialization: kernel address space and frame allocator.
pub struct PagingState {
    pub kernel_space: AddressSpace,
    pub frame_allocator: BootInfoFrameAllocator,
}

/// Zeros a physical frame.
///
/// # Safety
/// Frame must be valid and not currently in use.
#[inline]
pub unsafe fn zero_frame(frame: PhysFrame<Size4KiB>) {
    let ptr = frame.start_address().as_u64() as *mut u8;
    core::ptr::write_bytes(ptr, 0, Size4KiB::SIZE as usize);
}

/// Initializes paging subsystem using bootloader's page tables.
///
/// Stage 2A: We reuse bootloader's PML4 as the kernel address space.
/// Later stages may create a fresh kernel PML4.
///
/// # Safety
/// - Paging must be enabled
/// - CR3 must point to valid PML4
/// - Physical memory must be identity-mapped
pub unsafe fn init(
    boot_info: &'static bootloader_api::BootInfo,
) -> Result<PagingState, PagingError> {
    let kernel_start = boot_info.kernel_addr;
    let kernel_end = boot_info.kernel_addr + boot_info.kernel_len;

    let kernel_offset = match boot_info.physical_memory_offset {
        bootloader_api::info::Optional::Some(addr) => VirtAddr::new(addr),
        bootloader_api::info::Optional::None => VirtAddr::new(0),
    };

    let frame_allocator = BootInfoFrameAllocator::new(
        boot_info.memory_regions.as_ref(),
        kernel_start,
        kernel_end,
    );

    let (current_pml4_frame, _) = Cr3::read();

    Ok(PagingState {
        kernel_space: AddressSpace {
            id: AddressSpaceId(0),
            root_frame: current_pml4_frame,
            kernel_offset,
        },
        frame_allocator,
    })
}

/// Maps a contiguous virtual range (kernel-only).
///
/// If `identity` is true, each virtual page maps to the same physical address.
/// Otherwise, allocates new physical frames.
///
/// # Safety
/// - Can create invalid/aliasing mappings if misused
/// - Caller must not overlap existing mappings
/// - Never use with USER_ACCESSIBLE flag (Stage 2A)
#[allow(dead_code)]
pub unsafe fn map_region<M>(
    mapper: &mut M,
    frame_allocator: &mut BootInfoFrameAllocator,
    virt_start: VirtAddr,
    size: u64,
    flags: Flags,
    identity: bool,
) -> Result<(), PagingError>
where
    M: Mapper<Size4KiB>,
{
    let page_count = (size + Size4KiB::SIZE - 1) / Size4KiB::SIZE;
    let start_page = Page::containing_address(virt_start);

    for i in 0..page_count {
        let page = start_page + i;
        let frame = if identity {
            PhysFrame::containing_address(PhysAddr::new(page.start_address().as_u64()))
        } else {
            frame_allocator
                .allocate_frame()
                .ok_or(PagingError::OutOfFrames)?
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
