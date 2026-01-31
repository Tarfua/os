//! Long mode (64-bit) check via CR0, CR4, EFER. Stage 1.2.

const CR0_PE: u64 = 1 << 0;   // Protected mode
const CR0_PG: u64 = 1 << 31;  // Paging
const CR4_PAE: u64 = 1 << 5;  // Physical address extension
const EFER_MSR: u32 = 0xC000_0080;
const EFER_LME: u64 = 1 << 8;  // Long mode enable
const EFER_LMA: u64 = 1 << 10; // Long mode active

fn read_cr0() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("mov {}, cr0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

fn read_cr4() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("mov {}, cr4", out(reg) value, options(nostack, preserves_flags)) };
    value
}

fn read_efer() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") EFER_MSR,
            out("eax") low,
            out("edx") high,
            options(nostack, preserves_flags)
        );
    }
    (high as u64) << 32 | (low as u64)
}

/// Returns true if CPU is in 64-bit long mode (PE, PG, PAE, LME, LMA set).
pub fn is_long_mode() -> bool {
    let cr0 = read_cr0();
    let cr4 = read_cr4();
    let efer = read_efer();
    (cr0 & CR0_PE) != 0
        && (cr0 & CR0_PG) != 0
        && (cr4 & CR4_PAE) != 0
        && (efer & EFER_LME) != 0
        && (efer & EFER_LMA) != 0
}
