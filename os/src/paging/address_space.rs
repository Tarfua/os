//! Address Space abstraction
//!
//! Stage 2A: Isolated virtual memory contexts with manual switching
//! Stage 2B: Per-thread address spaces
//! Stage 2C+: Capability-controlled memory authority

use super::{mapper, EarlyFrameAllocator, PagingResult};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PageTableFlags as Flags, PhysFrame, Size4KiB},
    VirtAddr,
};
use super::pt::PageTableRoot;
use super::mapper::MapType;

/// Opaque identifier for an address space.
///
/// Stage 2A: Simple numeric ID
/// Stage 2C+: May become capability reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(pub u64);

impl AddressSpaceId {
    /// Kernel address space ID (reserved)
    pub const KERNEL: Self = AddressSpaceId(0);

    /// Creates a new user address space ID
    pub const fn new(id: u64) -> Self {
        AddressSpaceId(id)
    }
}

/// Address Space: isolated virtual memory context.
///
/// Each `AddressSpace` owns exactly one root page table (PML4).
/// Kernel space is identity-mapped in all address spaces.
/// User space mappings are fully isolated.
///
/// Stage 2A: Manual switching only (no scheduling)
/// Stage 2B: Bound to threads
/// Stage 2C+: Capability-based access control
pub struct AddressSpace {
    /// Unique identifier
    pub id: AddressSpaceId,
    /// Root page table
    pt_root: PageTableRoot,
}

impl AddressSpace {
    /// Creates an AddressSpace from existing PageTableRoot.
    ///
    /// Used during initialization to wrap bootloader's PML4.
    ///
    /// # Safety
    /// Caller must ensure the PML4 frame is valid and properly set up.
    pub unsafe fn from_existing(
        id: AddressSpaceId,
        root_frame: PhysFrame<Size4KiB>,
        kernel_offset: VirtAddr,
    ) -> Self {
        Self {
            id,
            pt_root: PageTableRoot::new(root_frame, kernel_offset),
        }
    }

    /// Switches to this address space by loading its PML4 into CR3.
    ///
    /// # Safety
    /// - Caller must ensure kernel code/data remains mapped after switch
    /// - This operation invalidates TLB entries
    /// - Must be called from kernel context
    #[inline]
    pub unsafe fn switch_to(&self) {
        let (_, flags) = Cr3::read();
        Cr3::write(self.pt_root.frame(), flags);
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
        frame_allocator: &mut EarlyFrameAllocator,
        kernel_offset: VirtAddr,
        kernel_start: u64,
        kernel_end: u64,
    ) -> PagingResult<Self> {
        use super::PagingError;
        
        // Allocate and zero new PML4
        let root_frame = frame_allocator
            .allocate_frame()
            .ok_or(PagingError::OutOfFrames)?;
        mapper::zero_frame(root_frame);

        // Set up mapper for new address space
        let virt_addr = kernel_offset.as_u64() + root_frame.start_address().as_u64();
        let table = unsafe { &mut *(virt_addr as *mut PageTable) };
        let mut mapper = OffsetPageTable::new(table, kernel_offset);

        // Map kernel space (identity mapping)
        mapper::map_region(
            &mut mapper,
            frame_allocator,
            VirtAddr::new(kernel_start),
            kernel_end - kernel_start,
            Flags::PRESENT | Flags::WRITABLE,
            MapType::Identity,
        )?;

        Ok(AddressSpace {
            id,
            pt_root: PageTableRoot::new(root_frame, kernel_offset),
        })
    }

    /// Maps user memory into this address space.
    ///
    /// Stage 2A: Basic implementation
    /// Stage 2C+: Will require capability check
    ///
    /// # Safety
    /// - Must not overlap with kernel space
    /// - Must not create aliasing issues
    pub unsafe fn map_user_region(
        &self,
        allocator: &mut impl FrameAllocator<Size4KiB>,
        start: VirtAddr,
        size: u64,
    ) -> PagingResult<()> {
        let mut mapper = self.pt_root.mapper();
        mapper::map_region(
            &mut mapper,
            allocator,
            start,
            size,
            Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
            MapType::Allocate,
        )
    }

    /// Destroys this address space and deallocates its page tables.
    ///
    /// Stage 2A: Basic cleanup - deallocates PML4 frame only
    /// Stage 2B+: Will recursively deallocate all page table levels
    ///
    /// # Safety
    /// - Must not be called on the currently active address space
    /// - Must not be called on kernel address space (ID 0)
    /// - Caller must ensure no references to this AS remain
    /// Destroys this address space and deallocates its page tables.
    ///
    /// Stage 2A: Basic cleanup - deallocates PML4 frame only
    /// Stage 2B+: Will recursively deallocate all page table levels
    ///
    /// # Safety
    /// - Must not be called on the currently active address space
    /// - Must not be called on kernel address space (ID 0)
    /// - Caller must ensure no references to this AS remain
    pub unsafe fn destroy(self, _frame_allocator: &mut EarlyFrameAllocator) {
        // Safety check: never destroy kernel address space
        if self.id == AddressSpaceId::KERNEL {
            return;
        }

        // Stage 2A: Just deallocate the PML4 frame
        // Stage 2B+ will add recursive deallocation of all page tables
        // For now, we just let the frame leak (will fix in Stage 2B)
        
        // TODO(Stage 2B): Recursively walk and deallocate all page table levels
        // This requires implementing a page table walker that:
        // 1. Walks all levels (PML4 -> PDPT -> PD -> PT)
        // 2. Deallocates user-space page tables (not kernel mappings)
        // 3. Deallocates the frames themselves
        
        core::mem::drop(self);
    }

    pub unsafe fn map_kernel_region(
        &self,
        allocator: &mut impl FrameAllocator<Size4KiB>,
        start: VirtAddr,
        size: u64,
    ) -> PagingResult<()> {
        let mut mapper = self.pt_root.mapper();
        mapper::map_region(
            &mut mapper,
            allocator,
            start,
            size,
            Flags::PRESENT | Flags::WRITABLE | Flags::GLOBAL,
            MapType::Identity,
        )
    }
}

// Stage 2B+: Will add Drop implementation to deallocate page tables
