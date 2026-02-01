// kernel module
pub mod init;   // kernel initialization

pub use init::{early_init, kernel_loop};
