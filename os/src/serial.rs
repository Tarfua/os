//! Serial port (COM1 @ 0x3F8) for debug output. Stage 1 primary debug channel.

const COM1: u16 = 0x3F8;

const LCR_OFF: u16 = 3;
const LCR_8N1: u8 = 0x03;
const MCR_OFF: u16 = 4;
const MCR_DTR_RTS: u8 = 0x03;
const LSR_OFF: u16 = 5;
const LSR_THRE: u8 = 0x20;

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack, preserves_flags));
    value
}

/// Initialize COM1 (8n1, no interrupts). Safe to call once at boot.
pub fn init() {
    unsafe {
        outb(COM1 + LCR_OFF, LCR_8N1);
        outb(COM1 + MCR_OFF, MCR_DTR_RTS);
    }
}

fn is_transmit_empty() -> bool {
    unsafe { (inb(COM1 + LSR_OFF) & LSR_THRE) != 0 }
}

/// Write one byte to serial. Blocks until THR empty. Call `init()` first.
pub fn write_byte(b: u8) {
    while !is_transmit_empty() {}
    unsafe { outb(COM1, b) }
}

/// Write a string to serial. Newlines not translated.
pub fn write_str(s: &str) {
    for b in s.bytes() {
        write_byte(b);
    }
}

/// Writer struct for use with core::fmt::Write
pub struct Writer;

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            write_byte(b);
        }
        Ok(())
    }
}