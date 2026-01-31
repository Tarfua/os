//! GDT and TSS for double-fault IST. Bootloader already set up a GDT; we load our own
//! so we can add a TSS with an interrupt stack for the double-fault handler.

use core::ptr::{addr_of, addr_of_mut};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use x86_64::instructions::tables::load_tss;

const DOUBLE_FAULT_STACK_SIZE: usize = 4096;

static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];
static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::empty();

/// Initializes GDT with kernel code, kernel data, and TSS. Sets TSS IST[0] to the
/// double-fault stack and loads GDT and TSS. Call once at boot before loading IDT.
pub fn init() {
    unsafe {
        let stack_top = VirtAddr::from_ptr(
            addr_of!(DOUBLE_FAULT_STACK).cast::<u8>().add(DOUBLE_FAULT_STACK_SIZE),
        );
        (*addr_of_mut!(TSS)).interrupt_stack_table[0] = stack_top;

        let gdt = addr_of!(GDT).cast_mut().as_mut().unwrap();
        gdt.append(Descriptor::kernel_code_segment());
        gdt.append(Descriptor::kernel_data_segment());
        let tss_sel = gdt.append(Descriptor::tss_segment(&*addr_of!(TSS)));

        let gdt_ref: &'static GlobalDescriptorTable = &*addr_of!(GDT);
        gdt_ref.load();
        load_tss(tss_sel);
    }
}
