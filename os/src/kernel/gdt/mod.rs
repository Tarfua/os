//! Global Descriptor Table (GDT) subsystem
//!
//! This module manages the x86-64 GDT and TSS, providing separate stacks
//! for different execution contexts (kernel, interrupts, double faults).
//!
//! # Architecture
//!
//! The GDT contains:
//! - Kernel code segment (ring 0)
//! - Kernel data segment (ring 0)
//! - User code segment (ring 3) - reserved for future use
//! - User data segment (ring 3) - reserved for future use
//! - Task State Segment (TSS)
//!
//! The TSS provides:
//! - Privilege stack table (for ring transitions)
//! - Interrupt stack table (IST) for critical handlers
//!
//! # Safety
//!
//! This module uses unsafe code to interact with CPU structures.
//! All public APIs ensure that invariants are maintained.

pub mod stack;
pub mod tss;
pub mod descriptor;

use crate::serial;

/// Initialize GDT subsystem
///
/// This must be called early in kernel initialization, before
/// enabling interrupts or setting up exception handlers.
pub fn init() {
    serial::write_str("=== GDT Initialization ===\n");
    
    // Initialize TSS with stack pointers
    tss::init();
    
    // Build and load GDT
    descriptor::init();
    
    serial::write_str("GDT initialized\n");
}

/// IST index for double fault handler
///
/// This handler gets a dedicated stack to handle stack overflow
/// situations that would otherwise cause a triple fault.
pub const DF_IST_INDEX: u16 = 1;

/// IST index for general interrupt handlers
///
/// This provides isolation between interrupt context and
/// normal kernel execution.
pub const INTERRUPT_IST_INDEX: u16 = 2;

/// Stack size for all kernel stacks (32 KiB)
///
/// This is sufficient for most kernel operations. Deep call
/// chains or large stack allocations should be avoided.
pub const STACK_SIZE: usize = 32 * 1024;
