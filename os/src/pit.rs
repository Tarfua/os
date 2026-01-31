//! 8253/8254 PIT channel 0: programmable rate, square wave. Drives IRQ0 for system tick.

const CH0_DATA: u16 = 0x40;
const CMD: u16 = 0x43;

/// PIT input clock (Hz). Divisor = PIT_BASE_HZ / desired_freq.
const PIT_BASE_HZ: u32 = 1_193_182;

/// Target tick rate (Hz). ~100 Hz gives one tick every 10 ms; good for timer and future scheduler.
pub const TICK_HZ: u32 = 100;

/// Command: channel 0, lo/hi bytes, mode 3 (square wave), binary.
const CMD_CH0_SQUARE: u8 = 0x36;

fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
    }
}

/// Programs PIT channel 0 to generate IRQ0 at TICK_HZ (~100 Hz), mode 3 (square wave).
/// Call after PIC remap, before enabling interrupts.
pub fn init() {
    let divisor = (PIT_BASE_HZ / TICK_HZ) as u16;
    outb(CMD, CMD_CH0_SQUARE);
    outb(CH0_DATA, (divisor & 0xFF) as u8);
    outb(CH0_DATA, (divisor >> 8) as u8);
}
