//! Interrupt Descriptor Table: exception and IRQ handlers. Requires GDT with TSS (IST) for double fault.

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::PageFaultErrorCode;

// === Exception counters ===
static DIV_COUNT: AtomicU64 = AtomicU64::new(0);
static DF_COUNT: AtomicU64 = AtomicU64::new(0);
static PF_COUNT: AtomicU64 = AtomicU64::new(0);
static GP_COUNT: AtomicU64 = AtomicU64::new(0);

// === Timer tick counter ===
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
const TICKS_PER_DOT: u64 = 10;

// === Global IDT Storage ===
// We allocate raw bytes for IDT to ensure stable location.
// Will be initialized once and never moved.
static mut IDT_STORAGE: InterruptDescriptorTable = InterruptDescriptorTable::new();

// === Handlers ===

extern "x86-interrupt" fn divide_error_handler(_frame: InterruptStackFrame) {
    DIV_COUNT.fetch_add(1, Ordering::Relaxed);
    crate::serial::write_str("=== DIVIDE ERROR ===\n");
}

extern "x86-interrupt" fn double_fault_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    use x86_64::instructions::interrupts;
    interrupts::disable();

    DF_COUNT.fetch_add(1, Ordering::Relaxed);

    crate::serial::write_str("\n=== DOUBLE FAULT ===\n");
    crate::serial::write_str("System halted\n");
    crate::serial::write_str("RIP="); crate::serial::write_u64_hex(frame.instruction_pointer.as_u64());
    crate::serial::write_str("RSP="); crate::serial::write_u64_hex(frame.stack_pointer.as_u64());
    crate::serial::write_str("RFLAGS="); crate::serial::write_u64_hex(frame.cpu_flags.bits());
    crate::serial::write_str("CS="); crate::serial::write_u16_hex(frame.code_segment.0);
    crate::serial::write_str("SS="); crate::serial::write_u16_hex(frame.stack_segment.0);
    crate::serial::write_str("ERR="); crate::serial::write_u64_hex(error_code);

    loop { x86_64::instructions::hlt(); }
}

extern "x86-interrupt" fn general_protection_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) {
    use x86_64::instructions::interrupts;
    interrupts::disable();

    GP_COUNT.fetch_add(1, Ordering::Relaxed);

    crate::serial::write_str("\n=== GENERAL PROTECTION FAULT ===\n");
    crate::serial::write_str("RIP="); crate::serial::write_u64_hex(frame.instruction_pointer.as_u64());
    crate::serial::write_str("ERR="); crate::serial::write_u64_hex(error_code);

    loop { x86_64::instructions::hlt(); }
}

extern "x86-interrupt" fn breakpoint_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("=== BREAKPOINT ===\n");
}

fn on_timer_tick() {
    let n = TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    if (n + 1) % TICKS_PER_DOT == 0 {
        crate::serial::write_byte(b'.');
    }
}

extern "x86-interrupt" fn timer_handler(_frame: InterruptStackFrame) {
    crate::pic::notify_end_of_interrupt();
    on_timer_tick();
}

extern "x86-interrupt" fn page_fault_handler(
    frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::instructions::interrupts;
    interrupts::disable();

    PF_COUNT.fetch_add(1, Ordering::Relaxed);

    let fault_addr = match Cr2::read() {
        Ok(addr) => addr.as_u64(),
        Err(_) => 0,
    };

    crate::serial::write_str("\n=== PAGE FAULT ===\n");
    crate::serial::write_str("Fault addr="); crate::serial::write_u64_hex(fault_addr);
    crate::serial::write_str("RIP="); crate::serial::write_u64_hex(frame.instruction_pointer.as_u64());
    crate::serial::write_str("ERR="); crate::serial::write_u64_hex(error_code.bits());

    loop { x86_64::instructions::hlt(); }
}

// === Init IDT ===

pub fn init() {
    unsafe {
        // — Raw pointer to global IDT —
        let idt_ptr: *mut InterruptDescriptorTable = core::ptr::addr_of_mut!(IDT_STORAGE);

        // — Explicit deref + &mut for each operation —
        (&mut *idt_ptr).divide_error.set_handler_fn(divide_error_handler);
        (&mut *idt_ptr).double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::gdt::DF_IST_INDEX);
        (&mut *idt_ptr).breakpoint.set_handler_fn(breakpoint_handler);
        (&mut *idt_ptr).general_protection_fault.set_handler_fn(general_protection_handler);
        (&mut *idt_ptr).page_fault.set_handler_fn(page_fault_handler);

        // === Explicit conversion for external IRQ index ===
        {
            let table = &mut *idt_ptr;
            table[32].set_handler_fn(timer_handler);
        }

        // Load IDT
        (&*idt_ptr).load();
    }
}
