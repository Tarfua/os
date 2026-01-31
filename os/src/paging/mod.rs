//! Paging subsystem for Stage 2+ microkernel
//!
//! This module provides memory management primitives:
//! - Address space abstraction and isolation
//! - Physical frame allocation
//! - Page table mapping utilities
//!
//! Stage progression:
//! - Stage 2A: Address space abstraction, manual switching
//! - Stage 2B: Per-thread address spaces
//! - Stage 2C: Capability-based memory authority
//! - Stage 3+: Advanced memory management policies

mod address_space;
mod error;
mod frame_allocator;
mod init;
mod mapper;
mod pt;

// Public exports
pub use address_space::{AddressSpace, AddressSpaceId};
pub use error::{PagingError, PagingResult};
pub use frame_allocator::EarlyFrameAllocator;
pub use init::{init, PagingState};

// Internal utilities (not exported publicly)
// pub use mapper::{map_region, zero_frame};
