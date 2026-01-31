//! Error types for paging operations

/// Paging operation errors
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
}

/// Convenience type alias for Results with PagingError
pub type PagingResult<T> = Result<T, PagingError>;
