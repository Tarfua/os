//! Task State Segment (TSS) management
//!
//! The TSS on x86-64 is primarily used for:
//! - Stack switching on privilege level changes
//! - Interrupt Stack Table (IST) for critical exception handlers

use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use super::{DF_IST_INDEX, INTERRUPT_IST_INDEX};
use super::stack;

/// Global TSS instance
///
/// There is one TSS per CPU core. In a multi-core system,
/// each core would have its own TSS.
#[no_mangle]
static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// Initialize TSS with stack pointers
///
/// Sets up:
/// - Privilege stack table (for ring 0-3 transitions)
/// - Interrupt stack table (for critical exception handlers)
pub fn init() {
    crate::serial::write_str("Configuring TSS...\n");
    
    unsafe {
        // Get stack top addresses (stacks grow downward)
        let kernel_top = VirtAddr::new(stack::get_kernel_stack_top());
        let interrupt_top = VirtAddr::new(stack::get_interrupt_stack_top());
        let df_top = VirtAddr::new(stack::get_double_fault_stack_top());
        
        // Set privilege stack table
        // Index 0 is used for ring 3 -> ring 0 transitions
        TSS.privilege_stack_table[0] = kernel_top;
        
        // Set interrupt stack table
        // IST1: Double fault handler (critical)
        TSS.interrupt_stack_table[DF_IST_INDEX as usize] = df_top;
        
        // IST2: General interrupt handlers
        TSS.interrupt_stack_table[INTERRUPT_IST_INDEX as usize] = interrupt_top;
    }
    
    log_tss_info();
}

/// Get reference to TSS
///
/// # Safety
/// Caller must ensure TSS has been initialized
pub unsafe fn get_tss() -> &'static TaskStateSegment {
    &*(&raw const TSS)
}

/// Get mutable reference to TSS
///
/// # Safety
/// Caller must ensure:
/// - TSS has been initialized
/// - No concurrent access occurs
/// - TSS invariants are maintained
pub unsafe fn get_tss_mut() -> &'static mut TaskStateSegment {
    &mut *(&raw mut TSS)
}

/// Log TSS configuration
fn log_tss_info() {
    unsafe {
        let tss = &*(&raw const TSS);
        
        crate::serial::write_str("TSS configuration:\n");
        
        crate::serial::write_str("  Ring 0 stack:  0x");
        crate::serial::write_u64_hex(tss.privilege_stack_table[0].as_u64());
        crate::serial::write_str("\n");
        
        crate::serial::write_str("  IST1 (DF):     0x");
        crate::serial::write_u64_hex(tss.interrupt_stack_table[DF_IST_INDEX as usize].as_u64());
        crate::serial::write_str("\n");
        
        crate::serial::write_str("  IST2 (IRQ):    0x");
        crate::serial::write_u64_hex(tss.interrupt_stack_table[INTERRUPT_IST_INDEX as usize].as_u64());
        crate::serial::write_str("\n");
    }
}

/// Update kernel stack pointer
///
/// Used when switching between kernel threads/tasks.
///
/// # Safety
/// Caller must ensure the new stack is valid and properly initialized.
pub unsafe fn set_kernel_stack(stack_top: VirtAddr) {
    let tss = &mut *(&raw mut TSS);
    tss.privilege_stack_table[0] = stack_top;
}

/// Get current kernel stack pointer
pub fn get_kernel_stack() -> VirtAddr {
    unsafe { 
        let tss = &*(&raw const TSS);
        tss.privilege_stack_table[0]
    }
}
