//! Interrupt Descriptor Table (IDT)
//!
//! This module configures the IDT with handlers for CPU exceptions,
//! hardware interrupts (IRQs), and user-defined interrupts.

pub mod handlers;
pub mod storage;

use crate::kernel::idt::handlers::*;
use crate::kernel::idt::storage::*;
use x86_64::structures::idt::InterruptDescriptorTable;
use crate::serial;

/// Initialize Interrupt Descriptor Table
pub fn init() {
    serial::write_str("=== IDT Initialization ===\n");
    
    unsafe {
        let idt = &mut *(&raw mut IDT_STORAGE.0);
        
        serial::write_str("IDT at: 0x");
        serial::write_u64_hex(idt as *const _ as u64);
        serial::write_str("\n");
        
        install_exception_handlers(idt);
        install_irq_handlers(idt);
        install_default_handlers(idt);
        
        serial::write_str("Loading IDT...\n");
        idt.load();
        serial::write_str("IDT loaded\n");
    }
}

/// Install CPU exception handlers (vectors 0-31)
unsafe fn install_exception_handlers(idt: &mut InterruptDescriptorTable) {
    serial::write_str("Installing exception handlers...\n");
    
    // CPU exceptions with named handlers
    idt.divide_error.set_handler_fn(divide_error_handler);                    // 0: #DE
    idt.debug.set_handler_fn(debug_handler);                                  // 1: #DB
    idt.non_maskable_interrupt.set_handler_fn(nmi_handler);                   // 2: NMI
    idt.breakpoint.set_handler_fn(breakpoint_handler);                        // 3: #BP
    idt.overflow.set_handler_fn(overflow_handler);                            // 4: #OF
    idt.bound_range_exceeded.set_handler_fn(bound_range_handler);             // 5: #BR
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);                // 6: #UD
    idt.device_not_available.set_handler_fn(device_not_available_handler);    // 7: #NM
    
    // Double fault with dedicated stack
    idt.double_fault                                                          // 8: #DF
        .set_handler_fn(double_fault_handler)
        .set_stack_index(crate::gdt::DF_IST_INDEX);
    
    // More exceptions
    idt.invalid_tss.set_handler_fn(invalid_tss_handler);                      // 10: #TS
    idt.segment_not_present.set_handler_fn(segment_not_present_handler);      // 11: #NP
    idt.stack_segment_fault.set_handler_fn(stack_segment_fault_handler);      // 12: #SS
    idt.general_protection_fault.set_handler_fn(general_protection_handler);  // 13: #GP
    idt.page_fault.set_handler_fn(page_fault_handler);                        // 14: #PF
}

/// Install hardware IRQ handlers (vectors 32-47)
unsafe fn install_irq_handlers(idt: &mut InterruptDescriptorTable) {
    serial::write_str("Installing IRQ handlers...\n");
    
    idt[32].set_handler_fn(timer_handler);       // IRQ0: PIT Timer
    idt[33].set_handler_fn(keyboard_handler);    // IRQ1: PS/2 Keyboard
}

/// Install default handler for remaining vectors
unsafe fn install_default_handlers(idt: &mut InterruptDescriptorTable) {
    serial::write_str("Installing default handlers...\n");
    
    // Remaining IRQs (34-47 = IRQ2-IRQ15)
    for vector in 34u8..=47 {
        idt[vector].set_handler_fn(unexpected_interrupt_handler);
    }
    
    // User-defined interrupts (48-255)
    for vector in 48u8..=255 {
        idt[vector].set_handler_fn(unexpected_interrupt_handler);
    }
}
