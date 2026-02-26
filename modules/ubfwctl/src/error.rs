//! Error types for ubfwctl operations

use thiserror::Error;

/// Errors that can occur during ubfwctl operations
#[derive(Error, Debug)]
pub enum UbfwctlError {
    /// Invalid time value (must be between 1 and 3600 ms)
    #[error("Invalid time value: {0}. Must be between {MIN_TIME_MS} and {MAX_TIME_MS} ms")]
    InvalidTime(u32),

    /// Invalid port number
    #[error("Invalid port number: {0}")]
    InvalidPort(u32),

    /// Ioctl operation failed
    #[error("Ioctl failed: {0}")]
    IoctlFailed(String),

    /// Device not found
    #[error("Fwctl device not found for chip {chip_id}, die {die_id}")]
    DeviceNotFound {
        /// Chip ID
        chip_id: u32,
        /// Die ID
        die_id: u32,
    },

    /// Invalid response from kernel
    #[error("Invalid response from kernel: {0}")]
    InvalidResponse(String),

    /// Shared memory lock failed
    #[error("Shared memory lock failed: {0}")]
    ShmLockFailed(String),

    /// Command not supported
    #[error("Command not supported: {0}")]
    CommandNotSupported(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Null pointer encountered
    #[error("Null pointer: {0}")]
    NullPointer(String),
}

/// Minimum measurement time in milliseconds
pub const MIN_TIME_MS: u32 = 1;
/// Maximum measurement time in milliseconds
pub const MAX_TIME_MS: u32 = 3600;
/// Conversion factor from milliseconds to microseconds
pub const MS_TO_US: u32 = 1000;
/// Conversion factor from milliseconds to seconds
pub const MS_TO_S: f64 = 1e-3;
/// Conversion factor from Hz to GHz
pub const HZ_TO_GHZ: f64 = 1e9;

impl UbfwctlError {
    /// Check if a time value is valid
    ///
    /// # Arguments
    /// * `time_ms` - Time in milliseconds
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err(UbfwctlError::InvalidTime)` otherwise
    ///
    /// # Errors
    /// Returns `UbfwctlError::InvalidTime` if the time is not within the valid range
    pub fn validate_time(time_ms: u32) -> Result<(), Self> {
        if (MIN_TIME_MS..=MAX_TIME_MS).contains(&time_ms) {
            Ok(())
        } else {
            Err(Self::InvalidTime(time_ms))
        }
    }
}
