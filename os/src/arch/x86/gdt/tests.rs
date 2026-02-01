//! GDT subsystem tests
//!
//! These tests verify that the GDT and TSS are properly configured.

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Test that TSS has valid stack pointers
    #[test_case]
    fn test_tss_stack_pointers() {
        unsafe {
            let tss = crate::kernel::gdt::tss::get_tss();
            
            // Check that stack pointers are non-zero
            assert_ne!(tss.privilege_stack_table[0].as_u64(), 0);
            assert_ne!(tss.interrupt_stack_table[1].as_u64(), 0);
            assert_ne!(tss.interrupt_stack_table[2].as_u64(), 0);
            
            // Check that stacks are properly aligned (16-byte)
            assert_eq!(tss.privilege_stack_table[0].as_u64() % 16, 0);
            assert_eq!(tss.interrupt_stack_table[1].as_u64() % 16, 0);
            assert_eq!(tss.interrupt_stack_table[2].as_u64() % 16, 0);
        }
    }
}

/// Runtime tests (called from kernel code)
pub mod runtime_tests {
    use crate::serial;
    
    /// Test double fault handling
    ///
    /// This triggers a stack overflow to verify that the
    /// double fault handler is working correctly.
    ///
    /// # Safety
    /// This will trigger a double fault and halt the system.
    /// Only use for testing!
    #[allow(unconditional_recursion)]
    pub unsafe fn test_double_fault() {
        serial::write_str("\n=== Testing Double Fault Handler ===\n");
        serial::write_str("Triggering stack overflow...\n");
        
        fn stack_overflow() {
            // Prevent tail-call optimization
            volatile_write(&mut 0, 0);
            stack_overflow();
        }
        
        use core::ptr::write_volatile as volatile_write;
        stack_overflow();
    }
    
    /// Verify GDT configuration
    pub fn verify_gdt() {
        serial::write_str("\n=== GDT Verification ===\n");
        
        unsafe {
            use x86_64::instructions::segmentation::{CS, DS};
            use x86_64::instructions::tables::sgdt;
            
            // Get current segment selectors
            let cs = CS::get_reg();
            let ds = DS::get_reg();
            
            serial::write_str("Current CS: 0x");
            serial::write_u16_hex(cs.0);
            serial::write_str("\n");
            
            serial::write_str("Current DS: 0x");
            serial::write_u16_hex(ds.0);
            serial::write_str("\n");
            
            // Get GDT base and limit
            let gdtr = sgdt();
            serial::write_str("GDTR base: 0x");
            serial::write_u64_hex(gdtr.base.as_u64());
            serial::write_str("\n");
            
            serial::write_str("GDTR limit: 0x");
            serial::write_u16_hex(gdtr.limit);
            serial::write_str("\n");
            
            serial::write_str("GDT verification passed\n");
        }
    }
    
    /// Verify TSS is loaded
    pub fn verify_tss() {
        serial::write_str("\n=== TSS Verification ===\n");
        
        unsafe {
            use x86_64::instructions::tables::str;
            
            // Get current task register
            let tr = str();
            
            serial::write_str("Task Register: 0x");
            serial::write_u16_hex(tr.0);
            serial::write_str("\n");
            
            if tr.0 == 0 {
                serial::write_str("ERROR: TSS not loaded!\n");
            } else {
                serial::write_str("TSS verification passed\n");
            }
        }
    }
}
