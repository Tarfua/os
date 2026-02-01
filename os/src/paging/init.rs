use super::{AddressSpace, AddressSpaceId, EarlyFrameAllocator, PagingResult};
use bootloader_api::BootInfo;
use crate::serial;
use x86_64::{registers::control::Cr3, VirtAddr};

/// Paging subsystem state
pub struct PagingState {
    /// Kernel address space (ID 0)
    pub kernel_space: AddressSpace,
    /// Physical frame allocator
    pub frame_allocator: EarlyFrameAllocator,
}

/// Initialize paging subsystem using bootloader's page tables
///
/// # Safety
/// - Paging must be enabled
/// - CR3 must point to valid PML4
/// - Kernel must be properly loaded by bootloader
pub unsafe fn init(boot_info: &'static BootInfo) -> PagingResult<PagingState> {
    let kernel_start = boot_info.kernel_addr;
    let kernel_end = boot_info.kernel_addr + boot_info.kernel_len;

    let kernel_offset = get_physical_memory_offset(boot_info);
    
    log_boot_info(boot_info, kernel_start, kernel_end, kernel_offset);
    check_memory_regions(boot_info);

    let frame_allocator = EarlyFrameAllocator::new(
        &boot_info.memory_regions,
        kernel_start,
        kernel_end,
    );

    let (current_pml4_frame, _) = Cr3::read();

    let kernel_space = AddressSpace::from_existing(
        AddressSpaceId::KERNEL,
        current_pml4_frame,
        kernel_offset,
    );

    serial::write_str("Paging subsystem initialized\n");
    
    Ok(PagingState {
        kernel_space,
        frame_allocator,
    })    
}

/// Get physical memory offset from bootloader
fn get_physical_memory_offset(boot_info: &BootInfo) -> VirtAddr {
    match boot_info.physical_memory_offset {
        bootloader_api::info::Optional::Some(addr) => {
            serial::write_str("Physical memory offset: 0x");
            serial::write_u64_hex(addr);
            serial::write_str("\n");
            VirtAddr::new(addr)
        }
        bootloader_api::info::Optional::None => {
            serial::write_str("No physical memory offset (using identity mapping)\n");
            VirtAddr::new(0)
        }
    }
}

/// Log bootloader information
fn log_boot_info(boot_info: &BootInfo, kernel_start: u64, kernel_end: u64, kernel_offset: VirtAddr) {
    serial::write_str("=== Paging Initialization ===\n");
    
    // Kernel info
    serial::write_str("Kernel: 0x");
    serial::write_u64_hex(kernel_start);
    serial::write_str(" - 0x");
    serial::write_u64_hex(kernel_end);
    serial::write_str("\n");
    
    // Physical memory offset
    serial::write_str("Physical offset: 0x");
    serial::write_u64_hex(kernel_offset.as_u64());
    serial::write_str("\n");
    
    // Current page table
    let (pml4_frame, _) = Cr3::read();
    serial::write_str("Page table (CR3): 0x");
    serial::write_u64_hex(pml4_frame.start_address().as_u64());
    serial::write_str("\n");
    
    // Framebuffer if available
    if let bootloader_api::info::Optional::Some(fb) = &boot_info.framebuffer {
        let info = fb.info();
        serial::write_str("Framebuffer: ");
        serial::write_u64_hex(info.width as u64);
        serial::write_str("x");
        serial::write_u64_hex(info.height as u64);
        serial::write_str(" (");
        serial::write_u64_hex(info.bytes_per_pixel as u64);
        serial::write_str(" bpp)\n");
    }
}

/// Sanity check memory regions
fn check_memory_regions(boot_info: &BootInfo) {
    if boot_info.memory_regions.is_empty() {
        serial::write_str("WARNING: No memory regions provided by bootloader\n");
    }
}
