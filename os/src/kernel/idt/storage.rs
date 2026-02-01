//! Global storage for IDT and counters

use core::sync::atomic::{AtomicU64};
use x86_64::structures::idt::InterruptDescriptorTable;

// === Exception counters ===
pub static DIV_COUNT: AtomicU64 = AtomicU64::new(0);
pub static DF_COUNT: AtomicU64 = AtomicU64::new(0);
pub static PF_COUNT: AtomicU64 = AtomicU64::new(0);
pub static GP_COUNT: AtomicU64 = AtomicU64::new(0);

// === Timer tick counter ===
pub static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
pub const TICKS_PER_DOT: u64 = 10;

// === Global IDT Storage ===
pub static mut IDT_STORAGE: InterruptDescriptorTable = InterruptDescriptorTable::new();
