//! Paging subsystem initialization

use super::{AddressSpace, AddressSpaceId, BootInfoFrameAllocator, PagingResult};
use bootloader_api::BootInfo;
use crate::serial;
use core::fmt::Write;
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
    let kernel_start = boot_info.kernel_addr;
    let kernel_end = boot_info.kernel_addr + boot_info.kernel_len;

    let kernel_offset = match boot_info.physical_memory_offset {
        bootloader_api::info::Optional::Some(addr) => VirtAddr::new(addr),
        bootloader_api::info::Optional::None => {
            let _ = writeln!(crate::serial::Writer, "No physical_memory_offset, defaulting to 0");
            VirtAddr::new(0)
        },
    };

    // Boot type detection
    serial::write_fmt(format_args!(
        "Boot type: {}\n",
        if let bootloader_api::info::Optional::Some(_) = &boot_info.framebuffer {
            "UEFI"
        } else {
            "BIOS"
        }
    ));

    // Boot type
    match &boot_info.framebuffer {
        bootloader_api::info::Optional::Some(_) => {
            let _ = writeln!(crate::serial::Writer, "Boot type: UEFI");
        }
        bootloader_api::info::Optional::None => {
            let _ = writeln!(crate::serial::Writer, "Boot type: BIOS");
        }
    }

    // Framebuffer log
    if let bootloader_api::info::Optional::Some(fb) = &boot_info.framebuffer {
        let info = fb.info(); // виклик методу, а не прямий доступ
        serial::write_fmt(format_args!(
            "Framebuffer: {}x{}, {} bytes/pixel\n",
            info.width,
            info.height,
            info.bytes_per_pixel
        ));
    }

    // memory_regions sanity check
    if boot_info.memory_regions.is_empty() {
        let _ = writeln!(crate::serial::Writer, "WARNING: memory_regions empty!");
    }

    let frame_allocator =
        BootInfoFrameAllocator::new(&boot_info.memory_regions, kernel_start, kernel_end);

    let (current_pml4_frame, _) = Cr3::read();

    let _ = writeln!(
        crate::serial::Writer,
        "=== Paging Init ===\nkernel: 0x{:x}-0x{:x}\nkernel_offset: 0x{:x}\nUsing bootloader PML4: 0x{:x}",
        kernel_start,
        kernel_end,
        kernel_offset.as_u64(),
        current_pml4_frame.start_address().as_u64()
    );

    Ok(PagingState {
        kernel_space: AddressSpace::from_existing(
            AddressSpaceId::KERNEL,
            current_pml4_frame,
            kernel_offset,
        ),
        frame_allocator,
    })
}
