//! Ownership management for OBMM (Ownership-Based Memory Management)
//!
//! This module provides safe wrappers for setting memory ownership
//! permissions on OBMM memory regions.

use crate::error::{ObmmError, Result};
#[cfg(not(feature = "hook"))]
use crate::sys;

/// Memory protection constants (matching C PROT_* values)
pub mod prot {
    /// No access permission
    pub const NONE: i32 = 0x0;
    /// Read permission
    pub const READ: i32 = 0x1;
    /// Write permission (implies read)
    pub const WRITE: i32 = 0x2;
    /// Read-write permission
    pub const READWRITE: i32 = READ | WRITE;
}

/// Set ownership of a memory region
///
/// Sets the ownership (read, write, none) of a range of OBMM virtual
/// address space. Ownership is expressed using memory protection bits.
///
/// # Arguments
/// * `fd` - The file descriptor of an OBMM memory device
/// * `start` - The start virtual address of the range
/// * `end` - The end virtual address of the range
/// * `prot` - The ownership expressed as protection bits:
///   - `prot::NONE` (0) - No access
///   - `prot::READ` (1) - Read-only access
///   - `prot::WRITE` (2) - Write access (implies read)
///   - `prot::READWRITE` (3) - Read-write access
///
/// # Errors
/// Returns `ObmmError::SetOwnershipFailed` if the operation fails
///
/// # Safety
/// The address range must be valid OBMM-managed memory.
/// The file descriptor must be a valid OBMM device FD.
///
/// # Example
/// ```
/// use obmm_rs::ownership::{set_ownership, prot};
///
/// let fd = 3; // Example file descriptor
/// let start = 0xffff_fc00_0000;
/// let end = 0xffff_fd00_0000;
///
/// match set_ownership(fd, start, end, prot::READWRITE) {
///     Ok(()) => println!("Ownership set successfully"),
///     Err(e) => eprintln!("Failed to set ownership: {}", e),
/// }
/// ```
#[cfg(feature = "hook")]
#[inline]
pub fn set_ownership(_fd: i32, _start: u64, _end: u64, _prot: i32) -> Result<()> {
    // Hooked implementation for testing
    Ok(())
}

/// Set ownership of a memory region (real implementation)
///
/// See the hooked version for documentation.
#[cfg(not(feature = "hook"))]
#[inline]
pub fn set_ownership(fd: i32, start: u64, end: u64, prot: i32) -> Result<()> {
    let ret = unsafe {
        sys::obmm_set_ownership(
            fd,
            start as *mut c_void,
            end as *mut c_void,
            prot,
        )
    };
    ret.to_obmm_result(ObmmError::SetOwnershipFailed)
}

/// Builder-style API for setting ownership
///
/// Provides a more ergonomic way to set ownership on memory regions.
///
/// # Example
/// ```
/// use obmm_rs::ownership::OwnershipSetter;
/// use obmm_rs::ownership::prot;
///
/// OwnershipSetter::new(3)
///     .range(0xffff_fc00_0000, 0xffff_fd00_0000)
///     .read_write()
///     .apply()
///     .expect("Failed to set ownership");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct OwnershipSetter {
    /// File descriptor of the OBMM memory device
    fd: i32,
    /// Start virtual address of the range (None if not set)
    start: Option<u64>,
    /// End virtual address of the range (None if not set)
    end: Option<u64>,
    /// Protection bits (`PROT_NONE`, `PROT_READ`, `PROT_WRITE`)
    prot: i32,
}

impl OwnershipSetter {
    /// Create a new ownership setter for the given file descriptor
    ///
    /// # Arguments
    /// * `fd` - File descriptor of OBMM memory device
    #[inline]
    #[must_use]
    pub const fn new(fd: i32) -> Self {
        Self {
            fd,
            start: None,
            end: None,
            prot: prot::NONE,
        }
    }

    /// Set the memory range
    ///
    /// # Arguments
    /// * `start` - Start virtual address
    /// * `end` - End virtual address
    #[inline]
    #[must_use]
    pub const fn range(mut self, start: u64, end: u64) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    /// Set protection to none (no access)
    #[inline]
    #[must_use]
    pub const fn no_access(mut self) -> Self {
        self.prot = prot::NONE;
        self
    }

    /// Set protection to read-only
    #[inline]
    #[must_use]
    pub const fn read_only(mut self) -> Self {
        self.prot = prot::READ;
        self
    }

    /// Set protection to read-write
    #[inline]
    #[must_use]
    pub const fn read_write(mut self) -> Self {
        self.prot = prot::READWRITE;
        self
    }

    /// Set protection to write-only (actually read-write since write implies read)
    #[inline]
    #[must_use]
    pub const fn write_only(mut self) -> Self {
        self.prot = prot::WRITE;
        self
    }

    /// Apply the ownership settings
    ///
    /// # Errors
    /// Returns `ObmmError::InvalidInput` if start or end address is not set
    /// Returns `ObmmError::SetOwnershipFailed` if the kernel operation fails
    #[inline]
    pub fn apply(self) -> Result<()> {
        let start = self
            .start
            .ok_or(ObmmError::InvalidInput("start address not set"))?;
        let end = self
            .end
            .ok_or(ObmmError::InvalidInput("end address not set"))?;

        set_ownership(self.fd, start, end, self.prot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ownership_builder() {
        let setter = OwnershipSetter::new(3)
            .range(0x1000, 0x2000)
            .read_write();

        assert_eq!(setter.fd, 3);
        assert_eq!(setter.start, Some(0x1000));
        assert_eq!(setter.end, Some(0x2000));
        assert_eq!(setter.prot, prot::READWRITE);
    }

    #[test]
    fn test_ownership_builder_chaining() {
        let setter = OwnershipSetter::new(5)
            .range(0x1000, 0x2000)
            .read_only();

        assert_eq!(setter.prot, prot::READ);

        let setter = OwnershipSetter::new(5)
            .range(0x1000, 0x2000)
            .no_access();

        assert_eq!(setter.prot, prot::NONE);
    }

    #[test]
    fn test_prot_constants() {
        assert_eq!(prot::NONE, 0);
        assert_eq!(prot::READ, 1);
        assert_eq!(prot::WRITE, 2);
        assert_eq!(prot::READWRITE, 3);
    }
}
