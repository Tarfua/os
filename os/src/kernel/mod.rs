use x86_64::instructions::interrupts;
use crate::paging::PagingState;
use bootloader_api::BootInfo;
use crate::serial;

mod idt;

pub enum KernelInitError {
    PagingInitFailed,
}

pub struct KernelState {
    pub paging: PagingState,
    pub boot_info: &'static BootInfo,
}

pub fn early_init(
    boot_info: &'static BootInfo,
) -> Result<KernelState, KernelInitError> {
    serial::init();
    serial::write_str("Stage 1: kernel running\n");

    if crate::long_mode::is_long_mode() {
        serial::write_str("64-bit long mode\n");
    } else {
        serial::write_str("NOT in long mode\n");
    }

    // Boot type detection
    match &boot_info.framebuffer {
        bootloader_api::info::Optional::Some(_) => {
            serial::write_str("Boot type: UEFI\n");
        }
        bootloader_api::info::Optional::None => {
            serial::write_str("Boot type: BIOS\n");
        }
    }

    // GDT / IDT initialization
    crate::gdt::init();
    serial::write_str("GDT loaded\n");

    // Paging initialization

    let paging = unsafe { crate::paging::init(boot_info) }
    .map_err(|_| KernelInitError::PagingInitFailed)?;
    serial::write_str("paging: init OK (bootloader tables)\n");

    // IDT initialization
    crate::kernel::idt::init();
    serial::write_str("IDT loaded\n");

    // PIC / PIT initialization
    crate::pic::init();
    crate::pit::init();
    interrupts::enable();
    serial::write_str("PIC / PIT initialized; PIT 100 Hz; timer enabled\n");

    Ok(KernelState {
        paging,
        boot_info,
    })
}

pub fn kernel_loop(_state: KernelState) -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
