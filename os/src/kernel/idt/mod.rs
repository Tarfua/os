//! Public API for IDT

pub mod handlers;
pub mod storage;

use crate::kernel::idt::handlers::*;
use crate::kernel::idt::storage::*;
use x86_64::structures::idt::InterruptDescriptorTable;

pub fn init() {
    unsafe {
        let idt_ptr: *mut InterruptDescriptorTable = core::ptr::addr_of_mut!(IDT_STORAGE);

        // Exception handlers
        (&mut *idt_ptr).divide_error.set_handler_fn(divide_error_handler);
        (&mut *idt_ptr).double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::gdt::DF_IST_INDEX);
        (&mut *idt_ptr).breakpoint.set_handler_fn(breakpoint_handler);
        (&mut *idt_ptr).general_protection_fault.set_handler_fn(general_protection_handler);
        (&mut *idt_ptr).page_fault.set_handler_fn(page_fault_handler);

        // Timer IRQ
        (&mut *idt_ptr)[32].set_handler_fn(timer_handler);

        // Load IDT
        (&*idt_ptr).load();
    }
}
