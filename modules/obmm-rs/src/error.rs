//! Error handling for OBMM (Ownership-Based Memory Management)
//!
//! This module provides custom error types and result aliases for OBMM operations.

use std::fmt;
use std::result;

/// Result type alias for OBMM operations
pub type Result<T> = result::Result<T, ObmmError>;

/// Custom error type for OBMM operations
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ObmmError {
    /// Invalid memory ID encountered
    InvalidMemId,
    /// Memory export failed
    ExportFailed(String),
    /// Memory unexport failed
    UnexportFailed(String),
    /// Memory import failed
    ImportFailed(String),
    /// Memory unimport failed
    UnimportFailed(String),
    /// Memory preimport failed
    PreimportFailed(String),
    /// Memory unpreimport failed
    UnpreimportFailed(String),
    /// Export user address failed
    ExportUseraddrFailed(String),
    /// Set ownership failed
    SetOwnershipFailed(String),
    /// Query operation failed
    QueryFailed(String),
    /// Invalid input provided
    InvalidInput(&'static str),
    /// I/O error occurred
    IoError(String),
    /// Device error (e.g., failed to open /dev/obmm)
    DeviceError(String),
    /// Ownership operation failed
    OwnershipFailed(String),
    /// Serialization/deserialization error
    SerializationError(String),
}

impl fmt::Display for ObmmError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ObmmError::InvalidMemId => write!(f, "Invalid memory ID"),
            ObmmError::ExportFailed(ref msg) => write!(f, "Memory export failed: {msg}"),
            ObmmError::UnexportFailed(ref msg) => {
                write!(f, "Memory unexport failed: {msg}")
            }
            ObmmError::ImportFailed(ref msg) => write!(f, "Memory import failed: {msg}"),
            ObmmError::UnimportFailed(ref msg) => {
                write!(f, "Memory unimport failed: {msg}")
            }
            ObmmError::PreimportFailed(ref msg) => {
                write!(f, "Memory preimport failed: {msg}")
            }
            ObmmError::UnpreimportFailed(ref msg) => {
                write!(f, "Memory unpreimport failed: {msg}")
            }
            ObmmError::ExportUseraddrFailed(ref msg) => {
                write!(f, "Export user address failed: {msg}")
            }
            ObmmError::SetOwnershipFailed(ref msg) => {
                write!(f, "Set ownership failed: {msg}")
            }
            ObmmError::QueryFailed(ref msg) => write!(f, "Query operation failed: {msg}"),
            ObmmError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            ObmmError::IoError(ref msg) => write!(f, "I/O error: {msg}"),
            ObmmError::DeviceError(ref msg) => write!(f, "Device error: {msg}"),
            ObmmError::OwnershipFailed(ref msg) => write!(f, "Ownership operation failed: {msg}"),
            ObmmError::SerializationError(ref msg) => write!(f, "Serialization error: {msg}"),
        }
    }
}

impl std::error::Error for ObmmError {
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// Trait for converting raw error codes to `ObmmResult`
///
/// This trait allows easy conversion of C-style error codes (where 0 is success
/// and non-zero is failure) into the `ObmmResult` type.
pub trait ToObmmResult<T> {
    /// Convert the value to an `ObmmResult` using the provided error mapper
    ///
    /// # Arguments
    /// * `error_mapper` - Function that converts an error code to an `ObmmError`
    ///
    /// # Returns
    /// `Ok(T)` on success, `Err(ObmmError)` on failure
    ///
    /// # Errors
    /// Returns an error if the conversion fails, using the provided `error_mapper`
    /// to create the specific error variant.
    fn to_obmm_result(self, error_mapper: fn(i32) -> ObmmError) -> Result<T>;
}

impl ToObmmResult<()> for i32 {
    #[inline]
    fn to_obmm_result(self, error_mapper: fn(i32) -> ObmmError) -> Result<()> {
        if self == 0 {
            Ok(())
        } else {
            Err(error_mapper(self))
        }
    }
}

impl ToObmmResult<u64> for u64 {
    #[inline]
    fn to_obmm_result(self, error_mapper: fn(i32) -> ObmmError) -> Result<u64> {
        // For MemId returns, we check if it's invalid (0)
        if self == 0 {
            Err(error_mapper(0))
        } else {
            Ok(self)
        }
    }
}
