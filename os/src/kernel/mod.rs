// kernel module
pub mod arch;   // architecture-specific modules
pub mod gdt;    // global descriptor table
pub mod idt;    // interrupt descriptor table
pub mod init;   // kernel initialization

pub use init::{early_init, kernel_loop};
