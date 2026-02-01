//! 8253/8254 PIT (Programmable Interval Timer) channel 0.
//!
//! Generates IRQ0 at a programmable frequency. Drives system tick.
//! Default: 100 Hz (~10 ms per tick).

const CH0_DATA: u16 = 0x40;
const CMD: u16 = 0x43;

/// PIT input clock in Hz
const PIT_BASE_HZ: u32 = 1_193_182;

/// Target tick rate
pub const TICK_HZ: u32 = 100;

/// Command: channel 0, lo/hi bytes, mode 3 (square wave), binary
const CMD_CH0_SQUARE: u8 = 0x36;

#[inline(always)]
fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
    }
}

/// Initialize PIT channel 0 to generate IRQ0 at `TICK_HZ`.
pub fn init() {
    let divisor = PIT_BASE_HZ / TICK_HZ;
    assert!(divisor > 0, "PIT divisor must be > 0");

    let divisor_lo = (divisor & 0xFF) as u8;
    let divisor_hi = (divisor >> 8) as u8;

    // Program PIT
    outb(CMD, CMD_CH0_SQUARE);
    outb(CH0_DATA, divisor_lo);
    outb(CH0_DATA, divisor_hi);
}
