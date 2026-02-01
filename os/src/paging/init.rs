//! Paging subsystem initialization (DF-safe, modular, Linux-like)

use super::{AddressSpace, AddressSpaceId, EarlyFrameAllocator, PagingResult};
use bootloader_api::BootInfo;
use crate::serial;
use x86_64::{registers::control::Cr3, VirtAddr};

/// Result of paging initialization
pub struct PagingState {
    /// Kernel address space (ID 0)
    pub kernel_space: AddressSpace,
    /// Physical frame allocator
    pub frame_allocator: EarlyFrameAllocator,
}

/// Initializes paging subsystem using bootloader's page tables.
///
/// # Safety
/// - Paging must be enabled
/// - CR3 must point to valid PML4
/// - Physical memory must be identity-mapped
pub unsafe fn init(boot_info: &'static BootInfo) -> PagingResult<PagingState> {
    let kernel_start = boot_info.kernel_addr;
    let kernel_end = boot_info.kernel_addr + boot_info.kernel_len;

    let kernel_offset = get_kernel_offset(boot_info);

    log_framebuffer_info(boot_info);
    check_memory_regions(boot_info);

    let frame_allocator =
        EarlyFrameAllocator::new(&boot_info.memory_regions, kernel_start, kernel_end);

    let (current_pml4_frame, _) = Cr3::read();

    let pml4_virt_addr: u64 = (kernel_offset + current_pml4_frame.start_address().as_u64()).as_u64();
    log_paging_info(kernel_start, kernel_end, kernel_offset, pml4_virt_addr);

    let kernel_space = AddressSpace::from_existing(
        AddressSpaceId::KERNEL,
        current_pml4_frame,
        kernel_offset,
    );

    Ok(PagingState {
        kernel_space,
        frame_allocator,
    })
}

/// Get physical memory offset safely
fn get_kernel_offset(boot_info: &BootInfo) -> VirtAddr {
    match boot_info.physical_memory_offset {
        bootloader_api::info::Optional::Some(addr) => VirtAddr::new(addr),
        bootloader_api::info::Optional::None => {
            serial::write_str("WARNING: No physical_memory_offset, defaulting to 0\n");
            VirtAddr::new(0)
        }
    }
}

/// Log framebuffer info if available
fn log_framebuffer_info(boot_info: &BootInfo) {
    if let bootloader_api::info::Optional::Some(fb) = &boot_info.framebuffer {
        let info = fb.info();
        serial::write_str("Framebuffer: ");
        serial::write_u64_hex(info.width as u64);
        serial::write_str("x");
        serial::write_u64_hex(info.height as u64);
        serial::write_str(", bytes/pixel=");
        serial::write_u64_hex(info.bytes_per_pixel as u64);
        serial::write_str("\n");
    }
}

/// Sanity check memory regions
fn check_memory_regions(boot_info: &BootInfo) {
    if boot_info.memory_regions.is_empty() {
        serial::write_str("WARNING: memory_regions empty!\n");
    }
}

/// Log kernel and paging info (safe for DF)
fn log_paging_info(kernel_start: u64, kernel_end: u64, kernel_offset: VirtAddr, pml4_base: u64) {
    serial::write_str("=== Paging Init ===\n");
    serial::write_str("kernel_start=");
    serial::write_u64_hex(kernel_start);
    serial::write_str("\nkernel_end=");
    serial::write_u64_hex(kernel_end);
    serial::write_str("\nkernel_offset=");
    serial::write_u64_hex(kernel_offset.as_u64());
    serial::write_str("\nbootloader PML4=");
    serial::write_u64_hex(pml4_base);
    serial::write_str("\n");
}
