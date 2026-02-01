//! GDT and TSS initialization with IST for double fault

use core::ptr::{addr_of, addr_of_mut};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use x86_64::instructions::tables::load_tss;

const DOUBLE_FAULT_STACK_SIZE: usize = 32 * 1024; // 32 KiB

/// IST index for double fault
/// x86 convention:
/// IST = 1 -> TSS.interrupt_stack_table[0]
pub const DF_IST_INDEX: u16 = 1;

static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];
static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::empty();

pub fn init() {
    unsafe {
        // 1. Calculate stack top (stack grows down)
        let stack_top = VirtAddr::from_ptr(
            addr_of!(DOUBLE_FAULT_STACK).cast::<u8>().add(DOUBLE_FAULT_STACK_SIZE),
        );

        // 2. Bind it to IST[0]
        // (DF_IST_INDEX = 1 → interrupt_stack_table[0])
        (*addr_of_mut!(TSS)).interrupt_stack_table[0] = stack_top;

        // 3. Form GDT
        let gdt = addr_of_mut!(GDT).as_mut().unwrap();
        gdt.append(Descriptor::kernel_code_segment());
        gdt.append(Descriptor::kernel_data_segment());

        // 4. Add TSS descriptor
        let tss_selector = gdt.append(Descriptor::tss_segment(&*addr_of!(TSS)));

        // 5. Load GDT
        let gdt_ref: &'static GlobalDescriptorTable = &*addr_of!(GDT);
        gdt_ref.load();

        // 6. Load TSS (ENABLE IST)
        load_tss(tss_selector);
    }
}
