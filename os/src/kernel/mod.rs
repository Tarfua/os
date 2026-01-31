use x86_64::instructions::interrupts;

pub fn early_init(boot_info: &'static mut bootloader_api::BootInfo) {
    crate::serial::init();
    crate::serial::write_str("Stage 1: kernel running\n");

    if crate::long_mode::is_long_mode() {
        crate::serial::write_str("64-bit long mode\n");
    } else {
        crate::serial::write_str("NOT in long mode\n");
    }

    let _paging_state = unsafe { crate::paging::init(boot_info) };
    if _paging_state.is_some() {
        crate::serial::write_str("paging: init OK (bootloader tables)\n");
    } else {
        crate::serial::write_str("paging: init failed\n");
    }

    // Order: GDT (TSS) -> IDT -> PIC remap -> PIT rate -> enable interrupts.
    crate::gdt::init();
    crate::idt::init();
    crate::pic::init();
    crate::pit::init();
    interrupts::enable();

    crate::serial::write_str("IDT loaded; PIT 100 Hz; timer enabled\n");
}

pub fn kernel_loop() -> ! {
    loop {}
}
