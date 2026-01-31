//! Interrupt Descriptor Table: exception and IRQ handlers. Requires GDT with TSS (IST) for double fault.

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// IDT is global and immutable after init.
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// One dot every 100 ms at 100 Hz = 10 ticks.
const TICKS_PER_DOT: u64 = 10;

extern "x86-interrupt" fn divide_error_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("divide error\n");
}

extern "x86-interrupt" fn double_fault_handler(_frame: InterruptStackFrame, _code: u64) -> ! {
    crate::serial::write_str("double fault\n");
    loop {}
}

extern "x86-interrupt" fn breakpoint_handler(frame: InterruptStackFrame) {
    crate::serial::write_str("breakpoint\n");
    let _ = frame;
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

/// Fills IDT with handlers and loads it. Call after gdt::init().
pub fn init() {
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT).as_mut().unwrap();
        idt.divide_error
            .set_handler_fn(divide_error_handler);
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::gdt::DF_IST_INDEX);
        idt.breakpoint
            .set_handler_fn(breakpoint_handler);
        idt.slice_mut(32..33)[0].set_handler_fn(timer_handler);

        let idt_ref: &'static InterruptDescriptorTable = &*core::ptr::addr_of!(IDT);
        idt_ref.load();
    }
}
