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
    kernel::early_init(boot_info);
    kernel::kernel_loop()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
