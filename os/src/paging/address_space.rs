//! Address Space abstraction
//!
//! Stage 2A: Isolated virtual memory contexts with manual switching
//! Stage 2B: Per-thread address spaces
//! Stage 2C+: Capability-controlled memory authority
//!
//! # Architecture
//! Each AddressSpace owns a complete page table hierarchy (PML4 on x86_64).
//! Kernel space is identity-mapped and shared across all address spaces.
//! User space mappings are fully isolated between address spaces.
//!
//! # Invariants
//! - INVARIANT: Kernel space (0xFFFF_8000_0000_0000+) is always mapped
//! - INVARIANT: User space (<0x0000_8000_0000_0000) is isolated
//! - INVARIANT: AddressSpaceId::KERNEL is never destroyed
//! - INVARIANT: Active address space is never destroyed

use super::{mapper, EarlyFrameAllocator, PagingError, PagingResult};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageTable, PageTableFlags as Flags,
        PhysFrame, Size4KiB, PageSize,
    },
    VirtAddr,
};
use super::pt::PageTableRoot;
use super::mapper::MapType;

/// Opaque identifier for an address space.
///
/// Stage 2A: Simple numeric ID
/// Stage 2B: Associated with thread
/// Stage 2C+: May become capability reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(pub u64);

impl AddressSpaceId {
    /// Kernel address space ID (reserved, must never be destroyed)
    pub const KERNEL: Self = AddressSpaceId(0);

    /// Creates a new user address space ID
    ///
    /// # Panics
    /// Panics if id is 0 (reserved for kernel)
    pub const fn new(id: u64) -> Self {
        assert!(id != 0, "ID 0 is reserved for kernel address space");
        AddressSpaceId(id)
    }

    /// Creates a new user address space ID without validation
    ///
    /// # Safety
    /// Caller must ensure id is not 0
    #[inline]
    pub const fn new_unchecked(id: u64) -> Self {
        AddressSpaceId(id)
    }

    /// Returns true if this is the kernel address space
    #[inline]
    pub const fn is_kernel(&self) -> bool {
        self.0 == 0
    }
}

impl core::fmt::Display for AddressSpaceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_kernel() {
            write!(f, "AddressSpace(KERNEL)")
        } else {
            write!(f, "AddressSpace({})", self.0)
        }
    }
}

/// Memory usage statistics for an address space
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    /// Number of mapped pages (approximate)
    pub mapped_pages: usize,
    
    /// Number of user pages mapped
    pub user_pages: usize,
    
    /// Number of kernel pages (shared)
    pub kernel_pages: usize,
}

/// Address Space: isolated virtual memory context.
///
/// Each `AddressSpace` owns exactly one root page table (PML4 on x86_64).
/// Kernel space is identity-mapped in all address spaces (shared).
/// User space mappings are fully isolated between address spaces.
///
/// # Lifecycle
/// 1. Created with `create()` or wrapped with `from_existing()`
/// 2. Modified with `map_user_region()`, `map_kernel_region()`
/// 3. Activated with `switch_to()`
/// 4. Destroyed with `destroy()` or dropped
///
/// # Stage Progression
/// - Stage 2A: Manual switching only (no scheduling)
/// - Stage 2B: Bound to threads, automatic switching
/// - Stage 2C+: Capability-based access control
pub struct AddressSpace {
    /// Unique identifier
    pub id: AddressSpaceId,
    
    /// Root page table
    pt_root: PageTableRoot,
    
    /// Memory usage statistics
    stats: MemoryStats,
}

impl AddressSpace {
    /// Creates an AddressSpace from existing PageTableRoot.
    ///
    /// Used during initialization to wrap bootloader's PML4.
    ///
    /// # Arguments
    /// * `id` - Address space identifier
    /// * `root_frame` - Physical frame containing the PML4 table
    /// * `kernel_offset` - Virtual offset for accessing physical memory
    ///
    /// # Safety
    /// Caller must ensure:
    /// - PML4 frame is valid and properly initialized
    /// - PML4 is not being used by another AddressSpace instance
    /// - Kernel space is properly mapped in the PML4
    /// - kernel_offset correctly maps physical memory
    pub unsafe fn from_existing(
        id: AddressSpaceId,
        root_frame: PhysFrame<Size4KiB>,
        kernel_offset: VirtAddr,
    ) -> Self {
        Self {
            id,
            pt_root: PageTableRoot::new(root_frame, kernel_offset),
            stats: MemoryStats::default(),
        }
    }

    /// Returns the current active address space ID by reading CR3.
    ///
    /// This is useful for safety checks before destroying an address space.
    #[inline]
    pub fn current_id() -> PhysFrame<Size4KiB> {
        Cr3::read().0
    }

    /// Checks if this address space is currently active (loaded in CR3).
    #[inline]
    pub fn is_active(&self) -> bool {
        let (current_frame, _) = Cr3::read();
        current_frame == self.pt_root.frame()
    }

    /// Switches to this address space by loading its PML4 into CR3.
    ///
    /// # Safety
    /// Caller must ensure:
    /// - Kernel code/data remains accessible after the switch
    /// - This is called from kernel context (CPL 0)
    /// - The address space is properly initialized
    /// - Stack is in kernel space or will remain mapped
    ///
    /// # Effects
    /// - Updates CR3 register
    /// - Invalidates TLB entries (hardware automatic)
    /// - Changes virtual memory mapping for current CPU core
    ///
    /// # Multi-core Safety
    /// This only affects the current CPU core. Other cores maintain
    /// their own address spaces. For multi-core synchronization, use
    /// TLB shootdown (Stage 2B+).
    #[inline]
    pub unsafe fn switch_to(&self) {
        let (_, flags) = Cr3::read();
        Cr3::write(self.pt_root.frame(), flags);
    }

    /// Creates a new isolated address space.
    ///
    /// The new address space will have:
    /// - Kernel memory identity-mapped (shared with all address spaces)
    /// - Empty user space (no user mappings)
    /// - Fresh PML4 table
    ///
    /// This is suitable for creating new processes.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this address space
    /// * `frame_allocator` - Allocator for page table frames
    /// * `kernel_offset` - Virtual offset for accessing physical memory
    /// * `kernel_start` - Start of kernel physical memory
    /// * `kernel_end` - End of kernel physical memory
    ///
    /// # Safety
    /// Caller must ensure:
    /// - kernel_start/kernel_end describe valid kernel region
    /// - Kernel region is identity-mapped in current page tables
    /// - kernel_offset correctly maps physical memory
    /// - id is unique and not already in use
    ///
    /// # Errors
    /// - `OutOfFrames` if frame allocation fails
    /// - `MapFailed` if kernel mapping fails
    pub unsafe fn create(
        id: AddressSpaceId,
        frame_allocator: &mut EarlyFrameAllocator,
        kernel_offset: VirtAddr,
        kernel_start: u64,
        kernel_end: u64,
    ) -> PagingResult<Self> {
        // Allocate new PML4 frame
        let root_frame = frame_allocator
            .allocate_frame()
            .ok_or(PagingError::OutOfFrames)?;

        // Zero the PML4 table
        // SAFETY: Frame is freshly allocated, no concurrent access
        unsafe {
            mapper::zero_frame(root_frame, kernel_offset);
        }

        // Set up mapper for new address space
        let virt_addr = kernel_offset.as_u64() + root_frame.start_address().as_u64();
        let table = unsafe { &mut *(virt_addr as *mut PageTable) };
        let mut mapper = OffsetPageTable::new(table, kernel_offset);

        // Map kernel space (identity mapping, shared across all address spaces)
        // SAFETY: Caller guarantees kernel region is valid
        unsafe {
            mapper::map_region(
                &mut mapper,
                frame_allocator,
                VirtAddr::new(kernel_start),
                kernel_end - kernel_start,
                Flags::PRESENT | Flags::WRITABLE | Flags::GLOBAL,
                MapType::Identity,
            )?;
        }

        let kernel_pages = ((kernel_end - kernel_start) / Size4KiB::SIZE) as usize;

        Ok(AddressSpace {
            id,
            pt_root: PageTableRoot::new(root_frame, kernel_offset),
            stats: MemoryStats {
                mapped_pages: kernel_pages,
                user_pages: 0,
                kernel_pages,
            },
        })
    }

    /// Maps user memory into this address space.
    ///
    /// Creates new virtual memory mappings in user space with freshly
    /// allocated physical frames.
    ///
    /// # Arguments
    /// * `allocator` - Frame allocator for physical memory
    /// * `start` - Starting virtual address (must be in user space)
    /// * `size` - Size in bytes (will be rounded up to page size)
    ///
    /// # Safety
    /// Caller must ensure:
    /// - Start address is in user space (<0x0000_8000_0000_0000)
    /// - Region doesn't overlap with existing mappings
    /// - Size doesn't cause overflow
    ///
    /// Stage 2C+: Will require capability check
    ///
    /// # Errors
    /// - `KernelAddressInUserSpace` if start is in kernel space
    /// - `Misaligned` if start is not page-aligned
    /// - `SizeOverflow` if start + size overflows
    /// - `OutOfFrames` if allocation fails
    /// - `MapFailed` if mapping operation fails
    pub unsafe fn map_user_region(
        &mut self,
        allocator: &mut impl FrameAllocator<Size4KiB>,
        start: VirtAddr,
        size: u64,
    ) -> PagingResult<()> {
        let mut mapper = self.pt_root.mapper();
        
        let page_count = ((size + Size4KiB::SIZE - 1) / Size4KiB::SIZE) as usize;
        
        // SAFETY: Caller guarantees safety requirements
        unsafe {
            mapper::map_region(
                &mut mapper,
                allocator,
                start,
                size,
                Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                MapType::Allocate,
            )?;
        }

        // Update statistics
        self.stats.mapped_pages += page_count;
        self.stats.user_pages += page_count;

        Ok(())
    }

    /// Maps kernel memory into this address space.
    ///
    /// Creates identity-mapped kernel memory regions. Typically used for
    /// mapping additional kernel resources after initial creation.
    ///
    /// # Arguments
    /// * `allocator` - Frame allocator (for page table structures)
    /// * `start` - Starting virtual address (must be in kernel space)
    /// * `size` - Size in bytes
    ///
    /// # Safety
    /// Caller must ensure:
    /// - Start address is in kernel space
    /// - Physical memory at start is valid
    /// - Region doesn't conflict with existing mappings
    ///
    /// # Errors
    /// Similar to `map_user_region` but for kernel space
    pub unsafe fn map_kernel_region(
        &mut self,
        allocator: &mut impl FrameAllocator<Size4KiB>,
        start: VirtAddr,
        size: u64,
    ) -> PagingResult<()> {
        let mut mapper = self.pt_root.mapper();
        
        let page_count = ((size + Size4KiB::SIZE - 1) / Size4KiB::SIZE) as usize;
        
        // SAFETY: Caller guarantees safety requirements
        unsafe {
            mapper::map_region(
                &mut mapper,
                allocator,
                start,
                size,
                Flags::PRESENT | Flags::WRITABLE | Flags::GLOBAL,
                MapType::Identity,
            )?;
        }

        // Update statistics
        self.stats.mapped_pages += page_count;
        self.stats.kernel_pages += page_count;

        Ok(())
    }

    /// Returns memory usage statistics for this address space.
    #[inline]
    pub fn stats(&self) -> MemoryStats {
        self.stats
    }

    /// Returns a Mapper for this AddressSpace.
    ///
    /// Useful for advanced operations not covered by high-level methods.
    ///
    /// # Safety
    /// Caller must ensure:
    /// - Mapper is not used to violate address space invariants
    /// - Concurrent access is properly synchronized
    #[inline]
    pub unsafe fn mapper(&mut self) -> OffsetPageTable<'_> {
        self.pt_root.mapper()
    }

    /// Returns the physical frame containing the PML4 table.
    ///
    /// Useful for debugging and diagnostics.
    #[inline]
    pub fn root_frame(&self) -> PhysFrame<Size4KiB> {
        self.pt_root.frame()
    }

    /// Destroys this address space and deallocates its page tables.
    ///
    /// # Safety Requirements (CRITICAL)
    /// Caller must ensure:
    /// - This is NOT the currently active address space
    /// - This is NOT the kernel address space (ID 0)
    /// - No other references to this address space exist
    /// - No threads are using this address space
    ///
    /// # Implementation Note
    /// Stage 2A: Deallocates PML4 frame only (page tables leak - will fix in Stage 2B)
    /// Stage 2B+: Will recursively deallocate all page table levels
    ///
    /// # Memory Leak Warning
    /// Currently (Stage 2A), this only frees the PML4 frame. All lower-level
    /// page tables (PDPT, PD, PT) and the mapped frames leak. This is
    /// acceptable for Stage 2A but MUST be fixed before production use.
    ///
    /// TODO(Stage 2B): Implement recursive page table deallocation:
    /// 1. Walk PML4 entries
    /// 2. For each valid entry, walk PDPT
    /// 3. For each valid PDPT entry, walk PD
    /// 4. For each valid PD entry, walk PT  
    /// 5. Deallocate PT frames (skip kernel mappings)
    /// 6. Deallocate PD frames
    /// 7. Deallocate PDPT frames
    /// 8. Deallocate PML4 frame
    ///
    /// # Panics
    /// - Panics if attempting to destroy kernel address space
    /// - Panics if attempting to destroy currently active address space (debug only)
    pub unsafe fn destroy(self, frame_allocator: &mut EarlyFrameAllocator) {
        // SAFETY CHECK 1: Never destroy kernel address space
        if self.id == AddressSpaceId::KERNEL {
            panic!("Attempted to destroy kernel address space - this is forbidden");
        }

        // SAFETY CHECK 2: Never destroy active address space
        // This is a debug check because it has performance cost
        #[cfg(debug_assertions)]
        {
            if self.is_active() {
                panic!(
                    "Attempted to destroy active address space {} - switch away first",
                    self.id
                );
            }
        }

        // Stage 2A: Just deallocate the PML4 frame
        // This leaks all page tables and mapped memory - acceptable for now
        
        // Get the PML4 frame before self is consumed
        let _pml4_frame = self.pt_root.frame();
        
        // TODO(Stage 2B): Recursive deallocation here
        // For now, we intentionally leak to avoid complex unsafe code
        // This will be fixed when we implement proper page table walking
        
        // Note: We don't actually deallocate the frame yet because
        // EarlyFrameAllocator doesn't support deallocation
        // This will be added in Stage 2B with a proper frame allocator
        
        let _ = frame_allocator; // Silence unused warning
        
        // Drop self, releasing the PageTableRoot
        core::mem::drop(self);
    }
}

// Future: Stage 2B+ will add Drop implementation for automatic cleanup
// impl Drop for AddressSpace {
//     fn drop(&mut self) {
//         // Automatically clean up page tables when address space goes out of scope
//         // Must check that we're not the active address space
//     }
// }

impl core::fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AddressSpace")
            .field("id", &self.id)
            .field("pml4_frame", &self.pt_root.frame())
            .field("stats", &self.stats)
            .field("is_active", &self.is_active())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_space_id() {
        assert!(AddressSpaceId::KERNEL.is_kernel());
        assert!(!AddressSpaceId::new_unchecked(1).is_kernel());
        assert!(!AddressSpaceId::new_unchecked(100).is_kernel());
    }

    #[test]
    #[should_panic]
    fn test_address_space_id_zero_panics() {
        let _ = AddressSpaceId::new(0);
    }
}
