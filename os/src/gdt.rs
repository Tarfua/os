//! GDT and TSS initialization with kernel and interrupt stacks
//! Linux-style, safe для ядра з Rust 2024 raw pointers

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use x86_64::instructions::tables::load_tss;

const STACK_SIZE: usize = 32 * 1024;

pub const DF_IST_INDEX: u16 = 1;
pub const INTERRUPT_IST_INDEX: u16 = 2;

#[repr(align(16))]
struct Stack([u8; STACK_SIZE]);

// === Stacks ===
#[no_mangle]
static mut KERNEL_STACK: Stack = Stack([0; STACK_SIZE]);
#[no_mangle]
static mut INTERRUPT_STACK: Stack = Stack([0; STACK_SIZE]);
#[no_mangle]
static mut DOUBLE_FAULT_STACK: Stack = Stack([0; STACK_SIZE]);

// === TSS and GDT ===
#[no_mangle]
static mut TSS: TaskStateSegment = TaskStateSegment::new();
#[no_mangle]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::empty();

pub fn setup_tss() {
    unsafe {
        let kernel_top = VirtAddr::new(((*(&raw const KERNEL_STACK)).0.as_ptr()) as u64 + STACK_SIZE as u64);
        let interrupt_top = VirtAddr::new(((*(&raw const INTERRUPT_STACK)).0.as_ptr()) as u64 + STACK_SIZE as u64);
        let df_top = VirtAddr::new(((*(&raw const DOUBLE_FAULT_STACK)).0.as_ptr()) as u64 + STACK_SIZE as u64);

        let tss_ptr: *mut TaskStateSegment = &raw mut TSS;
        (*tss_ptr).privilege_stack_table[0] = kernel_top;
        (*tss_ptr).interrupt_stack_table[0] = df_top;
        (*tss_ptr).interrupt_stack_table[1] = interrupt_top;
    }
}

pub fn load_gdt_tss() {
    unsafe {
        let gdt_ptr: *mut GlobalDescriptorTable = &raw mut GDT;
        let gdt = &mut *gdt_ptr;

        gdt.append(Descriptor::kernel_code_segment());
        gdt.append(Descriptor::kernel_data_segment());

        let tss_ref: &'static TaskStateSegment = &*(&raw const TSS);
        let tss_sel = gdt.append(Descriptor::tss_segment(tss_ref));

        gdt.load();
        load_tss(tss_sel);
    }
}

pub fn dump_gdt_info() {
    unsafe {
        let tss_ptr: *const TaskStateSegment = &raw const TSS;
        crate::serial::write_str("=== GDT/TSS Info ===\n");
        crate::serial::write_str("TSS stacks:\n");
        crate::serial::write_str("  Kernel: ");
        crate::serial::write_u64_hex((*tss_ptr).privilege_stack_table[0].as_u64());
        crate::serial::write_str("\n  DF: ");
        crate::serial::write_u64_hex((*tss_ptr).interrupt_stack_table[0].as_u64());
        crate::serial::write_str("\n  IRQ: ");
        crate::serial::write_u64_hex((*tss_ptr).interrupt_stack_table[1].as_u64());
        crate::serial::write_str("\n");
    }
}

pub fn init() {
    setup_tss();
    load_gdt_tss();
    dump_gdt_info();
}
