use x86_64::instructions::interrupts;

pub enum KernelInitError {
    PagingInitFailed,
}

pub struct KernelState {
    pub paging: crate::paging::PagingState,
}

pub fn early_init(boot_info: &'static bootloader_api::BootInfo) -> Result<KernelState, KernelInitError> {
    crate::serial::init();
    crate::serial::write_str("Stage 1: kernel running\n");

    if crate::long_mode::is_long_mode() {
        crate::serial::write_str("64-bit long mode\n");
    } else {
        crate::serial::write_str("NOT in long mode\n");
    }

    let paging = unsafe { crate::paging::init(boot_info) }.ok_or(KernelInitError::PagingInitFailed)?;
    crate::serial::write_str("paging: init OK (bootloader tables)\n");

    // Order: GDT (TSS) -> IDT -> PIC remap -> PIT rate -> enable interrupts.
    crate::gdt::init();
    crate::idt::init();
    crate::pic::init();
    crate::pit::init();
    interrupts::enable();

    crate::serial::write_str("IDT loaded; PIT 100 Hz; timer enabled\n");

    Ok(KernelState { paging })
}

pub fn kernel_loop(state: KernelState) -> ! {
    let _ = state;
    loop {}
}
