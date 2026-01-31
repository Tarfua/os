//! 8259 PIC: remap IRQ 0–7 to vectors 32–39, mask all except IRQ0, EOI.

const MASTER_CMD: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_CMD: u16 = 0xA0;
const SLAVE_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;
const MASTER_VECTOR: u8 = 32;
const SLAVE_VECTOR: u8 = 40;
const MASTER_CASCADE: u8 = 0x04;
const SLAVE_CASCADE: u8 = 0x02;
const EOI: u8 = 0x20;

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack, preserves_flags));
    value
}

/// Initializes both PICs: remap to vectors 32–39, mask all IRQs, then unmask IRQ0.
pub fn init() {
    unsafe {
        let _mask_master = inb(MASTER_DATA);
        let _mask_slave = inb(SLAVE_DATA);

        outb(MASTER_CMD, ICW1_INIT);
        outb(SLAVE_CMD, ICW1_INIT);
        outb(MASTER_DATA, MASTER_VECTOR);
        outb(SLAVE_DATA, SLAVE_VECTOR);
        outb(MASTER_DATA, MASTER_CASCADE);
        outb(SLAVE_DATA, SLAVE_CASCADE);
        outb(MASTER_DATA, ICW4_8086);
        outb(SLAVE_DATA, ICW4_8086);

        outb(MASTER_DATA, 0xFE);
        outb(SLAVE_DATA, 0xFF);
    }
}

/// Send EOI to master PIC. Call at end of IRQ handler (e.g. timer).
pub fn notify_end_of_interrupt() {
    unsafe {
        outb(MASTER_CMD, EOI);
    }
}
