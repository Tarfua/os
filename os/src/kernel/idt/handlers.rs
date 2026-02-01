//! Interrupt and exception handlers


use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};
use x86_64::registers::control::Cr2;
use x86_64::instructions::interrupts;

use crate::kernel::idt::storage::*;
use core::sync::atomic::Ordering;

// === Exception handlers ===

pub extern "x86-interrupt" fn divide_error_handler(_frame: InterruptStackFrame) {
    DIV_COUNT.fetch_add(1, Ordering::Relaxed);
    crate::serial::write_str("=== DIVIDE ERROR ===\n");
}

pub extern "x86-interrupt" fn double_fault_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
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

pub extern "x86-interrupt" fn general_protection_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) {
    interrupts::disable();
    GP_COUNT.fetch_add(1, Ordering::Relaxed);

    crate::serial::write_str("\n=== GENERAL PROTECTION FAULT ===\n");
    crate::serial::write_str("RIP="); crate::serial::write_u64_hex(frame.instruction_pointer.as_u64());
    crate::serial::write_str("ERR="); crate::serial::write_u64_hex(error_code);

    loop { x86_64::instructions::hlt(); }
}

pub extern "x86-interrupt" fn breakpoint_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("=== BREAKPOINT ===\n");
}

// === Timer handler ===

fn on_timer_tick() {
    let n = TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    if (n + 1) % TICKS_PER_DOT == 0 {
        crate::serial::write_byte(b'.');
    }
}

pub extern "x86-interrupt" fn timer_handler(_frame: InterruptStackFrame) {
    crate::pic::notify_end_of_interrupt();
    on_timer_tick();
}

// === Page fault handler ===

pub extern "x86-interrupt" fn page_fault_handler(
    frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
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

// === Unexpected interrupt handler ===

pub extern "x86-interrupt" fn unexpected_interrupt_handler(_frame: InterruptStackFrame) {
    crate::serial::write_str("=== UNEXPECTED INTERRUPT ===\n");
    loop { x86_64::instructions::hlt(); }
}
