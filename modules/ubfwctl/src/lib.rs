//! ubfwctl: Rust interface for UB fwctl kernel framework
//!
//! This crate provides a Rust interface for communicating with the UB (Unified Bus)
//! kernel modules via the fwctl framework using ioctl calls.
//!
//! # Features
//!
//! - **`mar_perf`**: Bandwidth and latency measurement for UB ports
//! - **`list`**: List all fwctl devices with their port information
//!
//! # Examples
//!
//! ## Measure bandwidth and latency
//!
//! ```no_run
//! use ubfwctl::mar_perf_measure;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Measure port 0 for 1000ms
//!     let result = mar_perf_measure(0, 0, 0, 1000)?;
//!     println!("{}", result);
//!     Ok(())
//! }
//! ```
//!
//! ## List all devices
//!
//! ```no_run
//! use ubfwctl::list_devices;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let output = list_devices()?;
//!     println!("{}", output);
//!     Ok(())
//! }
//! ```
#![deny(
    absolute_paths_not_starting_with_crate,
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_abi,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    non_ascii_idents,
    noop_method_call,
    rust_2021_incompatible_closure_captures,
    rust_2021_incompatible_or_patterns,
    rust_2021_prefixes_incompatible_syntax,
    rust_2021_prelude_collisions,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_op_in_unsafe_fn,
    unstable_features,
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences,
    warnings,
    clippy::all,
    clippy::pedantic,
    clippy::cargo
)]

pub mod commands;
pub mod device;
pub mod error;
pub mod ioctl;
pub mod types;

pub use commands::list::{format_device_list, list_devices, list_devices_raw};
pub use commands::mar_perf::{mar_perf_measure, MarPerfCommand};
pub use device::{scan_devices, device_count, list_device_paths, DiscoveredDevice};
pub use error::UbfwctlError;
pub use ioctl::FwctlDevice;
pub use types::{FwctlDeviceInfo, IoDieInfo, MarPerfConfig, MarPerfQuery, MarPerfResult, PortInfo};

/// Convenience re-export for error handling
pub use anyhow;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Measure `mar_perf` for a specific port
///
/// This is a high-level convenience function that performs bandwidth and
/// latency measurement on a UB port.
///
/// # Arguments
/// * `chip_id` - Chip ID
/// * `die_id` - Die ID
/// * `port` - Port ID to measure
/// * `time_ms` - Measurement duration in milliseconds (1-3600)
///
/// # Returns
/// `Ok(MarPerfResult)` containing the measurement results, or `Err(UbfwctlError)`
/// if an error occurs.
///
/// # Errors
/// - `InvalidTime` if `time_ms` is not between 1 and 3600
/// - `DeviceNotFound` if the fwctl device doesn't exist
/// - `IoctlFailed` if communication with the kernel fails
/// - `ShmLockFailed` if shared memory locking fails
///
/// # Example
/// ```no_run
/// use ubfwctl::measure_mar_perf;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = measure_mar_perf(0, 0, 0, 1000)?;
///     println!("Write traffic: {} bytes/s", result.wr_traffic);
///     println!("Read traffic: {} bytes/s", result.rd_traffic);
///     println!("Write latency: {} ns", result.wr_delayed);
///     println!("Read latency: {} ns", result.rd_delayed);
///     Ok(())
/// }
/// ```
pub fn measure_mar_perf(
    chip_id: u32,
    die_id: u32,
    port: u32,
    time_ms: u32,
) -> Result<MarPerfResult, UbfwctlError> {
    mar_perf_measure(chip_id, die_id, port, time_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(NAME, "ubfwctl");
    }

    #[test]
    fn test_error_display() {
        let err = UbfwctlError::InvalidTime(0);
        let msg = format!("{err}");
        assert!(msg.contains("Invalid time"));
    }
}
