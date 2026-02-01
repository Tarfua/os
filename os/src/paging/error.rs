//! Error types for paging operations
//!
//! Provides detailed, actionable error information for debugging and recovery.

use x86_64::{
    structures::paging::{Page, Size4KiB},
    VirtAddr,
};

use super::AddressSpaceId;

/// Paging operation errors with detailed context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// CR3 contains invalid or zero physical address
    InvalidCr3,

    /// Frame allocator has no more frames available
    OutOfFrames,

    /// Page mapping operation failed (overlap or invalid parameters)
    MapFailed,

    /// Attempted to map with invalid flags
    InvalidFlags,

    /// Address range overflow or misalignment
    InvalidRange,

    /// Attempted to map kernel address in user address space
    ///
    /// User address spaces should only contain mappings below
    /// the kernel/user split (typically 0x0000_8000_0000_0000 on x86_64).
    KernelAddressInUserSpace {
        /// The invalid address that was attempted
        addr: VirtAddr,
    },

    /// Page is already mapped to a frame
    ///
    /// Indicates attempt to map a page that already has a valid mapping.
    /// Caller should either unmap first or use remap operation.
    AlreadyMapped {
        /// The page that is already mapped
        page: Page<Size4KiB>,
    },

    /// Address is not aligned to page boundary
    ///
    /// Most paging operations require page-aligned addresses.
    Misaligned {
        /// The misaligned address
        addr: VirtAddr,
        /// Required alignment (typically 4096 for 4 KiB pages)
        required: u64,
    },

    /// Size calculation would overflow
    ///
    /// Occurs when start + size overflows u64, indicating
    /// an invalid memory region specification.
    SizeOverflow {
        /// Starting address
        start: VirtAddr,
        /// Size in bytes
        size: u64,
    },

    /// Attempted to destroy the currently active address space
    ///
    /// Cannot destroy an address space while it's loaded in CR3.
    /// Switch to a different address space first.
    CannotDestroyActive {
        /// ID of the address space that cannot be destroyed
        id: AddressSpaceId,
    },

    /// Attempted to destroy the kernel address space
    ///
    /// The kernel address space (ID 0) must never be destroyed.
    CannotDestroyKernel,

    /// Attempted operation on user address with insufficient size
    ///
    /// Some operations require minimum sizes (e.g., stack must be at least one page).
    SizeTooSmall {
        /// Provided size
        provided: u64,
        /// Minimum required size
        required: u64,
    },

    /// Memory region would overlap with existing mapping
    ///
    /// Detected when trying to create a region that conflicts with
    /// existing mappings (useful when VMA tracking is implemented).
    RegionOverlap {
        /// Start of the new region
        new_start: VirtAddr,
        /// End of the new region
        new_end: VirtAddr,
    },
}

impl PagingError {
    /// Returns a human-readable description of the error
    pub fn description(&self) -> &'static str {
        match self {
            Self::InvalidCr3 => "CR3 register contains invalid page table address",
            Self::OutOfFrames => "physical memory exhausted",
            Self::MapFailed => "page mapping operation failed",
            Self::InvalidFlags => "invalid page table flags combination",
            Self::InvalidRange => "invalid address range",
            Self::KernelAddressInUserSpace { .. } => {
                "attempted to map kernel address in user space"
            }
            Self::AlreadyMapped { .. } => "page is already mapped",
            Self::Misaligned { .. } => "address is not properly aligned",
            Self::SizeOverflow { .. } => "size calculation overflow",
            Self::CannotDestroyActive { .. } => "cannot destroy active address space",
            Self::CannotDestroyKernel => "cannot destroy kernel address space",
            Self::SizeTooSmall { .. } => "size is smaller than required minimum",
            Self::RegionOverlap { .. } => "memory region overlaps with existing mapping",
        }
    }
}

/// Convenience type alias for Results with PagingError
pub type PagingResult<T> = Result<T, PagingError>;

// Implement Display for better error messages in logs
impl core::fmt::Display for PagingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::KernelAddressInUserSpace { addr } => {
                write!(
                    f,
                    "{}: address 0x{:x} is in kernel space",
                    self.description(),
                    addr.as_u64()
                )
            }
            Self::AlreadyMapped { page } => {
                write!(
                    f,
                    "{}: page at 0x{:x}",
                    self.description(),
                    page.start_address().as_u64()
                )
            }
            Self::Misaligned { addr, required } => {
                write!(
                    f,
                    "{}: address 0x{:x} must be aligned to 0x{:x}",
                    self.description(),
                    addr.as_u64(),
                    required
                )
            }
            Self::SizeOverflow { start, size } => {
                write!(
                    f,
                    "{}: start 0x{:x} + size 0x{:x} overflows",
                    self.description(),
                    start.as_u64(),
                    size
                )
            }
            Self::CannotDestroyActive { id } => {
                write!(
                    f,
                    "{}: address space {} is currently active",
                    self.description(),
                    id.0
                )
            }
            Self::SizeTooSmall { provided, required } => {
                write!(
                    f,
                    "{}: provided 0x{:x}, required at least 0x{:x}",
                    self.description(),
                    provided,
                    required
                )
            }
            Self::RegionOverlap { new_start, new_end } => {
                write!(
                    f,
                    "{}: region 0x{:x}-0x{:x}",
                    self.description(),
                    new_start.as_u64(),
                    new_end.as_u64()
                )
            }
            _ => write!(f, "{}", self.description()),
        }
    }
}
