//! Paging subsystem initialization

use super::{AddressSpace, AddressSpaceId, BootInfoFrameAllocator, PagingResult};
use bootloader_api::BootInfo;
use x86_64::{registers::control::Cr3, VirtAddr};

/// Result of paging initialization
pub struct PagingState {
    /// Kernel address space (ID 0)
    pub kernel_space: AddressSpace,
    /// Physical frame allocator
    pub frame_allocator: BootInfoFrameAllocator,
}

/// Initializes paging subsystem using bootloader's page tables.
///
/// Stage 2A: Reuse bootloader's PML4 as kernel address space
/// Stage 2B+: May create fresh kernel PML4
///
/// # Safety
/// - Paging must be enabled
/// - CR3 must point to valid PML4
/// - Physical memory must be identity-mapped
pub unsafe fn init(boot_info: &'static BootInfo) -> PagingResult<PagingState> {
    use core::fmt::Write;

    let kernel_start = boot_info.kernel_addr;
    let kernel_end = boot_info.kernel_addr + boot_info.kernel_len;

    let kernel_offset = match boot_info.physical_memory_offset {
        bootloader_api::info::Optional::Some(addr) => VirtAddr::new(addr),
        bootloader_api::info::Optional::None => VirtAddr::new(0),
    };

    // Debug logging (Stage 2A development)
    #[cfg(debug_assertions)]
    {
        let _ = writeln!(crate::serial::Writer, "=== Paging Init ===");
        let _ = writeln!(
            crate::serial::Writer,
            "kernel: 0x{:x}-0x{:x}",
            kernel_start,
            kernel_end
        );
        let _ = writeln!(
            crate::serial::Writer,
            "kernel_offset: 0x{:x}",
            kernel_offset.as_u64()
        );
    }

    let frame_allocator =
        BootInfoFrameAllocator::new(boot_info.memory_regions.as_ref(), kernel_start, kernel_end);

    #[cfg(debug_assertions)]
    {
        let _ = writeln!(
            crate::serial::Writer,
            "Frame allocator: {} ranges",
            frame_allocator.range_count()
        );
    }

    let (current_pml4_frame, _) = Cr3::read();

    #[cfg(debug_assertions)]
    {
        let _ = writeln!(
            crate::serial::Writer,
            "Using bootloader PML4: 0x{:x}",
            current_pml4_frame.start_address().as_u64()
        );
    }

    Ok(PagingState {
        kernel_space: AddressSpace::from_existing(
            AddressSpaceId::KERNEL,
            current_pml4_frame,
            kernel_offset,
        ),
        frame_allocator,
    })
}
