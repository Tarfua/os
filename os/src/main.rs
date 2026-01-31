#![no_std]
#![no_main]
#![allow(dead_code)]
#![feature(abi_x86_interrupt)]

mod gdt;
mod idt;
mod long_mode;
mod paging;
mod pic;
mod pit;
mod serial;

use core::panic::PanicInfo;
use x86_64::instructions::interrupts;

bootloader_api::entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    serial::init();
    serial::write_str("Stage 1: kernel running\n");

    if long_mode::is_long_mode() {
        serial::write_str("64-bit long mode\n");
    } else {
        serial::write_str("NOT in long mode\n");
    }

    let _paging_state = unsafe { paging::init(boot_info) };
    if _paging_state.is_some() {
        serial::write_str("paging: init OK (bootloader tables)\n");
    } else {
        serial::write_str("paging: init failed\n");
    }

    // Order: GDT (TSS) -> IDT -> PIC remap -> PIT rate -> enable interrupts.
    gdt::init();
    idt::init();
    pic::init();
    pit::init();
    interrupts::enable();

    serial::write_str("IDT loaded; PIT 100 Hz; timer enabled\n");

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
