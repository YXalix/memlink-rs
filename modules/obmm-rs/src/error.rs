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
    /// Memory export failed with error code
    ExportFailed(i32),
    /// Memory unexport failed with error code
    UnexportFailed(i32),
    /// Memory import failed with error code
    ImportFailed(i32),
    /// Memory unimport failed with error code
    UnimportFailed(i32),
    /// Memory preimport failed with error code
    PreimportFailed(i32),
    /// Memory unpreimport failed with error code
    UnpreimportFailed(i32),
    /// Export user address failed with error code
    ExportUseraddrFailed(i32),
    /// Set ownership failed with error code
    SetOwnershipFailed(i32),
    /// Query operation failed with error code
    QueryFailed(i32),
    /// Invalid input provided
    InvalidInput(&'static str),
    /// I/O error occurred
    IoError(String),
    /// Serialization/deserialization error
    SerializationError(String),
}

impl fmt::Display for ObmmError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ObmmError::InvalidMemId => write!(f, "Invalid memory ID"),
            ObmmError::ExportFailed(code) => write!(f, "Memory export failed with code: {code}"),
            ObmmError::UnexportFailed(code) => {
                write!(f, "Memory unexport failed with code: {code}")
            }
            ObmmError::ImportFailed(code) => write!(f, "Memory import failed with code: {code}"),
            ObmmError::UnimportFailed(code) => {
                write!(f, "Memory unimport failed with code: {code}")
            }
            ObmmError::PreimportFailed(code) => {
                write!(f, "Memory preimport failed with code: {code}")
            }
            ObmmError::UnpreimportFailed(code) => {
                write!(f, "Memory unpreimport failed with code: {code}")
            }
            ObmmError::ExportUseraddrFailed(code) => {
                write!(f, "Export user address failed with code: {code}")
            }
            ObmmError::SetOwnershipFailed(code) => {
                write!(f, "Set ownership failed with code: {code}")
            }
            ObmmError::QueryFailed(code) => write!(f, "Query operation failed with code: {code}"),
            ObmmError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            ObmmError::IoError(ref msg) => write!(f, "I/O error: {msg}"),
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
