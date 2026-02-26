//! Error handling for ETMEM operations

use std::fmt;
use std::result;

/// Result type alias for ETMEM operations
pub type Result<T> = result::Result<T, EtmemError>;

/// Custom error type for ETMEM operations
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EtmemError {
    /// Invalid process ID
    InvalidPid,
    /// Invalid virtual address
    InvalidAddress,
    /// Invalid watermark configuration
    InvalidWatermark,
    /// Invalid scan flags
    InvalidFlags,
    /// Procfs operation failed
    ProcfsError(String),
    /// IOCTL operation failed
    IoctlError(i32),
    /// Buffer too small
    BufferTooSmall,
    /// Kernel buffer full (more data available)
    KernelBufferFull,
    /// User buffer full
    UserBufferFull,
    /// Permission denied (requires CAP_SYS_ADMIN)
    PermissionDenied,
    /// Module not loaded
    ModuleNotLoaded,
    /// Process not found
    ProcessNotFound,
    /// Invalid page type in response
    InvalidPageType(u8),
    /// Scan operation failed
    ScanFailed(String),
    /// Swap operation failed
    SwapFailed(String),
    /// Watermark out of range (0-100)
    WatermarkOutOfRange,
    /// Low watermark >= high watermark
    InvalidWatermarkOrder,
    /// I/O error
    IoError(String),
    /// Operation not supported (e.g., VM scan without KVM)
    NotSupported,
    /// Address range invalid
    InvalidRange,
}

impl fmt::Display for EtmemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EtmemError::InvalidPid => write!(f, "Invalid process ID"),
            EtmemError::InvalidAddress => write!(f, "Invalid virtual address"),
            EtmemError::InvalidWatermark => write!(f, "Invalid watermark configuration"),
            EtmemError::InvalidFlags => write!(f, "Invalid scan flags"),
            EtmemError::ProcfsError(msg) => write!(f, "Procfs error: {}", msg),
            EtmemError::IoctlError(code) => write!(f, "IOCTL failed with code: {}", code),
            EtmemError::BufferTooSmall => write!(
                f,
                "Buffer too small (minimum {} bytes)",
                crate::types::PAGE_IDLE_BUF_MIN
            ),
            EtmemError::KernelBufferFull => write!(f, "Kernel buffer full, more data available"),
            EtmemError::UserBufferFull => write!(f, "User buffer full"),
            EtmemError::PermissionDenied => {
                write!(f, "Permission denied (requires CAP_SYS_ADMIN capability)")
            }
            EtmemError::ModuleNotLoaded => write!(f, "ETMEM kernel module not loaded"),
            EtmemError::ProcessNotFound => write!(f, "Process not found"),
            EtmemError::InvalidPageType(t) => write!(f, "Invalid page type: {}", t),
            EtmemError::ScanFailed(msg) => write!(f, "Scan failed: {}", msg),
            EtmemError::SwapFailed(msg) => write!(f, "Swap failed: {}", msg),
            EtmemError::WatermarkOutOfRange => write!(f, "Watermark must be 0-100"),
            EtmemError::InvalidWatermarkOrder => {
                write!(f, "Low watermark must be less than high watermark")
            }
            EtmemError::IoError(msg) => write!(f, "I/O error: {}", msg),
            EtmemError::NotSupported => write!(f, "Operation not supported"),
            EtmemError::InvalidRange => write!(f, "Invalid address range"),
        }
    }
}

impl std::error::Error for EtmemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<std::io::Error> for EtmemError {
    fn from(err: std::io::Error) -> Self {
        match err.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => EtmemError::PermissionDenied,
            Some(libc::ESRCH) => EtmemError::ProcessNotFound,
            Some(libc::ENODEV) => EtmemError::ModuleNotLoaded,
            Some(libc::EINVAL) => EtmemError::InvalidFlags,
            _ => EtmemError::IoError(err.to_string()),
        }
    }
}

/// Trait for converting raw error codes to EtmemResult
pub trait ToEtmemResult<T> {
    /// Convert to Result, mapping error codes via the provided function
    fn to_etmem_result(self, error_mapper: fn(i32) -> EtmemError) -> Result<T>;
}

impl ToEtmemResult<()> for i32 {
    fn to_etmem_result(self, error_mapper: fn(i32) -> EtmemError) -> Result<()> {
        if self == 0 {
            Ok(())
        } else {
            Err(error_mapper(self))
        }
    }
}

impl ToEtmemResult<i32> for i32 {
    fn to_etmem_result(self, error_mapper: fn(i32) -> EtmemError) -> Result<i32> {
        if self >= 0 {
            Ok(self)
        } else {
            Err(error_mapper(self))
        }
    }
}
