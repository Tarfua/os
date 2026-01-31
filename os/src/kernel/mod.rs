use x86_64::instructions::interrupts;

pub enum KernelInitError {
    PagingInitFailed,
}

pub struct KernelState {
    pub paging: crate::paging::PagingState,
}

pub fn early_init(
    boot_info: &'static bootloader_api::BootInfo,
) -> Result<KernelState, KernelInitError> {
    crate::serial::init();
    crate::serial::write_str("Stage 1: kernel running\n");

    if crate::long_mode::is_long_mode() {
        crate::serial::write_str("64-bit long mode\n");
    } else {
        crate::serial::write_str("NOT in long mode\n");
    }

    let paging = unsafe { crate::paging::init(boot_info) }
        .map_err(|_| KernelInitError::PagingInitFailed)?;

    crate::serial::write_str("paging: init OK (bootloader tables)\n");

    // Initialize GDT, IDT, PIC, PIT
    crate::gdt::init();
    crate::idt::init();
    crate::pic::init();
    crate::pit::init();
    interrupts::enable();

    crate::serial::write_str("IDT loaded; PIT 100 Hz; timer enabled\n");

    Ok(KernelState { paging })
}

pub fn kernel_loop(_state: KernelState) -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
