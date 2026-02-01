use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};
use x86_64::registers::control::Cr2;
use crate::kernel::idt::storage::*;
use core::sync::atomic::Ordering;
use crate::kernel::arch::x86::pic;

// === Exception handlers ===
pub extern "x86-interrupt" fn divide_error_handler(_frame: InterruptStackFrame) {
    DIV_COUNT.fetch_add(1, Ordering::SeqCst);
    crate::serial::write_str("=== DIVIDE ERROR ===\n");
}

pub extern "x86-interrupt" fn double_fault_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    DF_COUNT.fetch_add(1, Ordering::SeqCst);

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

pub extern "x86-interrupt" fn invalid_tss_handler(_frame: InterruptStackFrame, _error_code: u64) {
    crate::serial::write_str("=== INVALID TSS ===\n");
}

pub extern "x86-interrupt" fn segment_not_present_handler(_frame: InterruptStackFrame, _error_code: u64) {
    crate::serial::write_str("=== SEGMENT NOT PRESENT ===\n");
}

pub extern "x86-interrupt" fn stack_segment_fault_handler(_frame: InterruptStackFrame, _error_code: u64) {
    crate::serial::write_str("=== STACK SEGMENT FAULT ===\n");
}

pub extern "x86-interrupt" fn general_protection_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) {
    GP_COUNT.fetch_add(1, Ordering::SeqCst);

    crate::serial::write_str("\n=== GENERAL PROTECTION FAULT ===\n");
    crate::serial::write_str("RIP="); crate::serial::write_u64_hex(frame.instruction_pointer.as_u64());
    crate::serial::write_str("ERR="); crate::serial::write_u64_hex(error_code);

    loop { x86_64::instructions::hlt(); }
}

pub extern "x86-interrupt" fn breakpoint_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("=== BREAKPOINT ===\n");
}

// === Page fault handler ===
pub extern "x86-interrupt" fn page_fault_handler(
    frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    PF_COUNT.fetch_add(1, Ordering::SeqCst);

    let fault_addr = Cr2::read().expect("CR2 read failed");

    crate::serial::write_str("\n=== PAGE FAULT ===\n");
    crate::serial::write_str("Fault addr="); crate::serial::write_u64_hex(fault_addr.as_u64());
    crate::serial::write_str("RIP="); crate::serial::write_u64_hex(frame.instruction_pointer.as_u64());
    crate::serial::write_str("ERR="); crate::serial::write_u64_hex(error_code.bits());

    crate::serial::write_str("\nFlags: ");
    if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE) { crate::serial::write_str("WRITE "); } else { crate::serial::write_str("READ "); }
    if error_code.contains(PageFaultErrorCode::USER_MODE) { crate::serial::write_str("USER "); } else { crate::serial::write_str("SUPERVISOR "); }
    if error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) { crate::serial::write_str("PROTECTION_VIOLATION "); } else { crate::serial::write_str("NOT_PRESENT "); }

    loop { x86_64::instructions::hlt(); }
}

// === Timer handler ===
pub extern "x86-interrupt" fn timer_handler(_frame: InterruptStackFrame) {
    on_timer_tick();
    pic::notify_end_of_interrupt(pic::IRQ_TIMER);
}

fn on_timer_tick() {
    let n = TICK_COUNT.fetch_add(1, Ordering::SeqCst);
    if (n + 1) % TICKS_PER_DOT == 0 {
        crate::serial::write_byte(b'.');
    }
}

// === Keyboard IRQ ===
pub extern "x86-interrupt" fn keyboard_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("=== KEYBOARD IRQ ===\n");
    pic::notify_end_of_interrupt(pic::IRQ_KEYBOARD);
}

// === Generic Exception Stub for unused exceptions ===
macro_rules! stub {
    ($name:ident) => {
        pub extern "x86-interrupt" fn $name(_frame: InterruptStackFrame) {
            crate::serial::write_str(concat!("=== ", stringify!($name), " ===\n"));
        }
    };
}

// === Define stubs for all unimplemented exceptions ===
stub!(debug_handler);
stub!(nmi_handler);
stub!(overflow_handler);
stub!(bound_range_handler);
stub!(invalid_opcode_handler);
stub!(device_not_available_handler);

// === Generic unexpected handler ===
pub extern "x86-interrupt" fn unexpected_interrupt_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("=== UNEXPECTED INTERRUPT ===\n");
    pic::notify_end_of_interrupt(pic::IRQ_UNKNOWN);
}
