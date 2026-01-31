//! Address Space abstraction
//!
//! Stage 2A: Isolated virtual memory contexts with manual switching
//! Stage 2B: Per-thread address spaces
//! Stage 2C+: Capability-controlled memory authority

use super::{mapper, BootInfoFrameAllocator, PagingResult};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PageTableFlags as Flags, PhysFrame, Size4KiB},
    VirtAddr,
};

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
    /// Physical frame of root PML4
    root_frame: PhysFrame<Size4KiB>,
    /// Virtual offset for physical memory access (0 = identity)
    kernel_offset: VirtAddr,
}

impl AddressSpace {
        /// Creates an AddressSpace from existing PML4 frame.
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
            root_frame,
            kernel_offset,
        }
    }
    
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
            true, // identity
        )?;

        Ok(AddressSpace {
            id,
            root_frame,
            kernel_offset,
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
        &mut self,
        frame_allocator: &mut BootInfoFrameAllocator,
        virt_start: VirtAddr,
        size: u64,
    ) -> PagingResult<()> {
        let mut mapper = unsafe { self.mapper_mut() };
        mapper::map_region(
            &mut mapper,
            frame_allocator,
            virt_start,
            size,
            Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
            false, // allocate new frames
        )
    }
}

// Stage 2B+: Will add Drop implementation to deallocate page tables
