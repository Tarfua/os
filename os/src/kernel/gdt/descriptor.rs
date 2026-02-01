//! GDT descriptor table management
//!
//! This module builds and loads the Global Descriptor Table.

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::instructions::tables::load_tss;
use super::tss;

/// Global GDT instance
#[no_mangle]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::empty();

/// GDT selectors
///
/// These are set during initialization and can be used
/// to manually load segment registers if needed.
static mut SELECTORS: Selectors = Selectors {
    code_selector: SegmentSelector(0),
    data_selector: SegmentSelector(0),
    tss_selector: SegmentSelector(0),
};

/// Segment selectors returned by GDT
#[derive(Debug, Clone, Copy)]
pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

/// Initialize and load GDT
///
/// Builds the GDT with kernel segments and TSS, then loads it
/// into the CPU's GDTR register.
pub fn init() {
    crate::serial::write_str("Building GDT...\n");
    
    unsafe {
        let gdt = &mut *(&raw mut GDT);
        
        // Add null descriptor (required by x86-64)
        // Index 0 is implicitly null in x86_64 crate
        
        // Add kernel code segment (ring 0)
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        
        // Add kernel data segment (ring 0)
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        
        // Add TSS descriptor
        let tss = tss::get_tss();
        let tss_selector = gdt.append(Descriptor::tss_segment(tss));
        
        // Save selectors
        SELECTORS = Selectors {
            code_selector,
            data_selector,
            tss_selector,
        };
        
        crate::serial::write_str("Loading GDT...\n");
        
        // Load GDT into GDTR
        gdt.load();
        
        crate::serial::write_str("Loading TSS...\n");
        
        // Load TSS into TR (Task Register)
        load_tss(tss_selector);
    }
    
    log_gdt_info();
}

/// Get GDT selectors
///
/// Returns the segment selectors that were set during initialization.
pub fn get_selectors() -> Selectors {
    unsafe { SELECTORS }
}

/// Log GDT configuration
fn log_gdt_info() {
    unsafe {
        crate::serial::write_str("GDT selectors:\n");
        crate::serial::write_str("  Code: 0x");
        crate::serial::write_u16_hex(SELECTORS.code_selector.0);
        crate::serial::write_str("\n");
        
        crate::serial::write_str("  Data: 0x");
        crate::serial::write_u16_hex(SELECTORS.data_selector.0);
        crate::serial::write_str("\n");
        
        crate::serial::write_str("  TSS:  0x");
        crate::serial::write_u16_hex(SELECTORS.tss_selector.0);
        crate::serial::write_str("\n");
    }
}

/// Add user-mode segments to GDT (for future multitasking)
///
/// This should be called before switching to user mode.
///
/// # Safety
/// Must only be called once during initialization.
#[allow(dead_code)]
pub unsafe fn add_user_segments() {
    let gdt = &mut *(&raw mut GDT);
    
    // Add user code segment (ring 3)
    let _user_code = gdt.append(Descriptor::user_code_segment());
    
    // Add user data segment (ring 3)
    let _user_data = gdt.append(Descriptor::user_data_segment());
    
    // Reload GDT
    gdt.load();
    
    crate::serial::write_str("User segments added to GDT\n");
}
