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

/// Write formatted string to serial (via Writer)
pub fn write_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write; // import trait to make write_fmt available
    let mut w = Writer;
    let _ = w.write_fmt(args);
}

/// Write u64 as hex (0x1234abcd) without using format_args
pub fn write_u64_hex(n: u64) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    crate::serial::write_str("0x");
    let mut started = false;
    for i in (0..16).rev() {
        let digit = ((n >> (i * 4)) & 0xF) as u8;
        if digit != 0 || started || i == 0 {
            crate::serial::write_byte(HEX_CHARS[digit as usize]);
            started = true;
        }
    }
    crate::serial::write_str("\n");
}

/// Write u16 as hex
pub fn write_u16_hex(n: u16) {
    write_u64_hex(n as u64);
}