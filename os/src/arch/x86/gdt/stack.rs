//! Kernel stack management
//!
//! This module defines and manages the kernel's execution stacks.
//! Each stack is 16-byte aligned and placed in the .bss section.

use super::STACK_SIZE;

/// Aligned stack structure
///
/// Stacks must be 16-byte aligned for proper x86-64 operation.
/// They grow downward from high addresses to low addresses.
#[repr(align(16))]
pub struct Stack(pub [u8; STACK_SIZE]);

impl Stack {
    /// Get pointer to stack base (lowest address)
    pub const fn base_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
    
    /// Get pointer to stack top (highest address)
    ///
    /// This is where the stack pointer should be initialized,
    /// as stacks grow downward.
    pub fn top_ptr(&self) -> *const u8 {
        unsafe { self.0.as_ptr().add(STACK_SIZE) }
    }
}

// === Kernel Stacks ===
//
// These are placed in .bss section which is automatically mapped
// by the bootloader as part of the kernel image.

/// Main kernel stack
///
/// Used for normal kernel execution and system calls.
#[no_mangle]
pub static mut KERNEL_STACK: Stack = Stack([0; STACK_SIZE]);

/// Interrupt handler stack (IST2)
///
/// Provides isolation for interrupt handlers to prevent
/// stack corruption in case of nested interrupts.
#[no_mangle]
pub static mut INTERRUPT_STACK: Stack = Stack([0; STACK_SIZE]);

/// Double fault handler stack (IST1)
///
/// Critical for handling stack overflow and other catastrophic
/// failures. This stack must never be used for normal execution.
#[no_mangle]
pub static mut DOUBLE_FAULT_STACK: Stack = Stack([0; STACK_SIZE]);

/// Get kernel stack top address
pub fn get_kernel_stack_top() -> u64 {
    unsafe { 
        let ptr = &raw const KERNEL_STACK;
        (*ptr).top_ptr() as u64 
    }
}

/// Get interrupt stack top address
pub fn get_interrupt_stack_top() -> u64 {
    unsafe { 
        let ptr = &raw const INTERRUPT_STACK;
        (*ptr).top_ptr() as u64 
    }
}

/// Get double fault stack top address
pub fn get_double_fault_stack_top() -> u64 {
    unsafe { 
        let ptr = &raw const DOUBLE_FAULT_STACK;
        (*ptr).top_ptr() as u64 
    }
}

/// Log stack configuration
pub fn log_stack_info() {
    crate::serial::write_str("Stack layout:\n");
    
    unsafe {
        let kernel_ptr = &raw const KERNEL_STACK;
        crate::serial::write_str("  Kernel:    0x");
        crate::serial::write_u64_hex((*kernel_ptr).base_ptr() as u64);
        crate::serial::write_str(" - 0x");
        crate::serial::write_u64_hex((*kernel_ptr).top_ptr() as u64);
        crate::serial::write_str("\n");
        
        let interrupt_ptr = &raw const INTERRUPT_STACK;
        crate::serial::write_str("  Interrupt: 0x");
        crate::serial::write_u64_hex((*interrupt_ptr).base_ptr() as u64);
        crate::serial::write_str(" - 0x");
        crate::serial::write_u64_hex((*interrupt_ptr).top_ptr() as u64);
        crate::serial::write_str("\n");
        
        let df_ptr = &raw const DOUBLE_FAULT_STACK;
        crate::serial::write_str("  DF:        0x");
        crate::serial::write_u64_hex((*df_ptr).base_ptr() as u64);
        crate::serial::write_str(" - 0x");
        crate::serial::write_u64_hex((*df_ptr).top_ptr() as u64);
        crate::serial::write_str("\n");
    }
}
