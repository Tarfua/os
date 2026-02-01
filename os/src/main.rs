#![no_std]
#![no_main]
#![allow(dead_code)]
#![feature(abi_x86_interrupt)]

mod gdt;
mod idt;
mod kernel;
mod long_mode;
mod paging;
mod pic;
mod pit;
mod serial;

use core::panic::PanicInfo;

bootloader_api::entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    match kernel::early_init(&*boot_info) {
        Ok(state) => kernel::kernel_loop(state),
        Err(_) => {
            serial::write_str("paging: init failed\n");
            loop {}
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    x86_64::instructions::interrupts::disable();

    crate::serial::write_str("KERNEL PANIC: ");
    if let Some(location) = info.location() {
        crate::serial::write_fmt(format_args!(
            "file={} line={}\n",
            location.file(),
            location.line()
        ));
    }

    loop {
        x86_64::instructions::hlt();
    }
}
