//! 8259 PIC (Programmable Interrupt Controller).
//!
//! Remap IRQ 0–15 to IDT vectors 32–47 (0x20–0x2F).
//! Initially masks all IRQs except timer (IRQ0).

const MASTER_CMD: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_CMD: u16 = 0xA0;
const SLAVE_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;
const MASTER_VECTOR: u8 = 0x20;
const SLAVE_VECTOR: u8 = 0x28;
const MASTER_CASCADE: u8 = 0x04; // IR2 has slave
const SLAVE_CASCADE: u8 = 0x02;  // connected to master's IR2
const EOI: u8 = 0x20;

/// IRQs handled by PIC
pub const IRQ_TIMER: u8 = 0;
pub const IRQ_KEYBOARD: u8 = 1;
pub const IRQ_UNKNOWN: u8 = 0xFF; // for unexpected interrupts

/// Write byte to port
#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
}

/// Read byte from port
#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack, preserves_flags));
    value
}

/// Initialize PICs: remap IRQs, mask all except IRQ0 (timer).
/// Safe to call only once at kernel startup.
pub fn init() {
    unsafe {
        let mask_master = inb(MASTER_DATA);
        let mask_slave = inb(SLAVE_DATA);

        // Start initialization
        outb(MASTER_CMD, ICW1_INIT);
        outb(SLAVE_CMD, ICW1_INIT);

        // Remap vectors
        outb(MASTER_DATA, MASTER_VECTOR);
        outb(SLAVE_DATA, SLAVE_VECTOR);

        // Setup cascade
        outb(MASTER_DATA, MASTER_CASCADE);
        outb(SLAVE_DATA, SLAVE_CASCADE);

        // 8086 mode
        outb(MASTER_DATA, ICW4_8086);
        outb(SLAVE_DATA, ICW4_8086);

        // Mask all IRQs except timer (IRQ0)
        outb(MASTER_DATA, mask_master & !0x01);
        outb(SLAVE_DATA, mask_slave);
    }
}

/// Notify PIC that IRQ has been handled.
/// Should be called at end of each IRQ handler.
pub fn notify_end_of_interrupt(irq: u8) {
    unsafe {
        if irq >= 8 {
            outb(SLAVE_CMD, EOI);
        }
        outb(MASTER_CMD, EOI);
    }
}
