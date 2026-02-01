// //! GDT and TSS initialization with kernel and interrupt stacks
// //! Linux-style, safe для ядра з Rust 2024 raw pointers

// use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
// use x86_64::structures::tss::TaskStateSegment;
// use x86_64::VirtAddr;
// use x86_64::instructions::tables::load_tss;

// pub const STACK_SIZE: usize = 32 * 1024;

// pub const DF_IST_INDEX: u16 = 1;
// pub const INTERRUPT_IST_INDEX: u16 = 2;

// #[repr(align(16))]
// pub struct Stack(pub [u8; STACK_SIZE]);

// impl Stack {
//     pub fn as_ptr(&self) -> *const u8 {
//         self.0.as_ptr()
//     }
// }

// // === Stacks ===
// #[no_mangle]
// #[link_section = ".bss.kernel_stack"]
// pub static mut KERNEL_STACK: Stack = Stack([0; STACK_SIZE]);

// #[no_mangle]
// #[link_section = ".bss.interrupt_stack"]
// pub static mut INTERRUPT_STACK: Stack = Stack([0; STACK_SIZE]);

// #[no_mangle]
// #[link_section = ".bss.df_stack"]
// pub static mut DOUBLE_FAULT_STACK: Stack = Stack([0; STACK_SIZE]);

// // === TSS and GDT ===
// #[no_mangle]
// static mut TSS: TaskStateSegment = TaskStateSegment::new();
// #[no_mangle]
// static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::empty();

// pub fn setup_tss() {
//     unsafe {
//         let kernel_top = VirtAddr::new((&raw const KERNEL_STACK.0 as *const u8).add(STACK_SIZE) as u64);
//         let interrupt_top = VirtAddr::new((&raw const INTERRUPT_STACK.0 as *const u8).add(STACK_SIZE) as u64);
//         let df_top = VirtAddr::new((&raw const DOUBLE_FAULT_STACK.0 as *const u8).add(STACK_SIZE) as u64);

//         TSS.interrupt_stack_table[DF_IST_INDEX as usize] = df_top;
//         TSS.interrupt_stack_table[INTERRUPT_IST_INDEX as usize] = interrupt_top;
//         TSS.privilege_stack_table[0] = kernel_top;
//     }
// }

// pub fn load_gdt_tss() {
//     unsafe {
//         let gdt_ptr: *mut GlobalDescriptorTable = &raw mut GDT;
//         let gdt = &mut *gdt_ptr;

//         gdt.append(Descriptor::kernel_code_segment());
//         gdt.append(Descriptor::kernel_data_segment());

//         let tss_ref: &'static TaskStateSegment = &*(&raw const TSS);
//         let tss_sel = gdt.append(Descriptor::tss_segment(tss_ref));

//         gdt.load();
//         load_tss(tss_sel);
//     }
// }

// pub fn dump_gdt_info() {
//     unsafe {
//         let tss_ptr: *const TaskStateSegment = &raw const TSS;
//         crate::serial::write_str("=== GDT/TSS Info ===\n");
//         crate::serial::write_str("TSS stacks:\n");
//         crate::serial::write_str("  Kernel: ");
//         crate::serial::write_u64_hex((*tss_ptr).privilege_stack_table[0].as_u64());
//         crate::serial::write_str("\n  DF: ");
//         crate::serial::write_u64_hex((*tss_ptr).interrupt_stack_table[DF_IST_INDEX as usize].as_u64());
//         crate::serial::write_str("\n  IRQ: ");
//         crate::serial::write_u64_hex((*tss_ptr).interrupt_stack_table[INTERRUPT_IST_INDEX as usize].as_u64());
//         crate::serial::write_str("\n");
//     }
// }

// pub fn init() {
//     setup_tss();
//     load_gdt_tss();
//     dump_gdt_info();
// }

//! Global Descriptor Table (GDT) and Task State Segment (TSS)
//!
//! This module sets up the GDT with kernel code/data segments and a TSS
//! with separate stacks for different exception contexts.

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use x86_64::instructions::tables::load_tss;

/// Stack size for all kernel stacks (32 KiB)
pub const STACK_SIZE: usize = 32 * 1024;

/// IST index for double fault handler
pub const DF_IST_INDEX: u16 = 1;

/// IST index for interrupt handlers
pub const INTERRUPT_IST_INDEX: u16 = 2;

/// Aligned stack structure
#[repr(align(16))]
pub struct Stack(pub [u8; STACK_SIZE]);

impl Stack {
    /// Get pointer to stack base
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
}

// === Kernel Stacks ===
// These are placed in .bss section which is mapped by the bootloader

/// Main kernel stack
#[no_mangle]
pub static mut KERNEL_STACK: Stack = Stack([0; STACK_SIZE]);

/// Stack for interrupt handlers
#[no_mangle]
pub static mut INTERRUPT_STACK: Stack = Stack([0; STACK_SIZE]);

/// Stack for double fault handler
#[no_mangle]
pub static mut DOUBLE_FAULT_STACK: Stack = Stack([0; STACK_SIZE]);

// === TSS and GDT ===

/// Task State Segment
#[no_mangle]
static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// Global Descriptor Table
#[no_mangle]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::empty();

/// Initialize GDT and TSS
pub fn init() {
    setup_tss();
    load_gdt_and_tss();
    log_gdt_info();
}

/// Configure TSS with stack pointers
fn setup_tss() {
    unsafe {
        // Calculate top of each stack (stacks grow downward)
        let kernel_top = VirtAddr::new(
            (&raw const KERNEL_STACK.0 as *const u8).add(STACK_SIZE) as u64
        );
        let interrupt_top = VirtAddr::new(
            (&raw const INTERRUPT_STACK.0 as *const u8).add(STACK_SIZE) as u64
        );
        let df_top = VirtAddr::new(
            (&raw const DOUBLE_FAULT_STACK.0 as *const u8).add(STACK_SIZE) as u64
        );
        
        // Set IST entries
        TSS.interrupt_stack_table[DF_IST_INDEX as usize] = df_top;
        TSS.interrupt_stack_table[INTERRUPT_IST_INDEX as usize] = interrupt_top;
        
        // Set privilege stack
        TSS.privilege_stack_table[0] = kernel_top;
    }
}

/// Load GDT and TSS into CPU
fn load_gdt_and_tss() {
    unsafe {
        let gdt = &mut *(&raw mut GDT);
        
        // Add segments
        gdt.append(Descriptor::kernel_code_segment());
        gdt.append(Descriptor::kernel_data_segment());
        
        // Add TSS
        let tss = &*(&raw const TSS);
        let tss_selector = gdt.append(Descriptor::tss_segment(tss));
        
        // Load GDT and TSS
        gdt.load();
        load_tss(tss_selector);
    }
}

/// Log GDT/TSS configuration
fn log_gdt_info() {
    unsafe {
        let tss = &*(&raw const TSS);
        
        crate::serial::write_str("=== GDT/TSS Initialization ===\n");
        crate::serial::write_str("Kernel stack:    0x");
        crate::serial::write_u64_hex(tss.privilege_stack_table[0].as_u64());
        crate::serial::write_str("\n");
        
        crate::serial::write_str("DF stack (IST1): 0x");
        crate::serial::write_u64_hex(tss.interrupt_stack_table[DF_IST_INDEX as usize].as_u64());
        crate::serial::write_str("\n");
        
        crate::serial::write_str("IRQ stack (IST2): 0x");
        crate::serial::write_u64_hex(tss.interrupt_stack_table[INTERRUPT_IST_INDEX as usize].as_u64());
        crate::serial::write_str("\n");
    }
}
