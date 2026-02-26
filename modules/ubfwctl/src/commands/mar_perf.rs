//! `mar_perf` command implementation for bandwidth and latency measurement

use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

use libc::{c_int, flock, LOCK_EX, LOCK_NB};

use crate::error::UbfwctlError;
use crate::ioctl::FwctlDevice;
use crate::types::{MarPerfQuery, MarPerfResult};

/// `mar_perf` command implementation
#[derive(Debug, Clone, Copy)]
pub struct MarPerfCommand;

impl MarPerfCommand {
    /// Create a new `mar_perf` command
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Execute the `mar_perf` measurement
    ///
    /// # Arguments
    /// * `chip_id` - Chip ID
    /// * `die_id` - Die ID
    /// * `port` - Port ID
    /// * `time_ms` - Measurement time in milliseconds
    ///
    /// # Returns
    /// `Ok(MarPerfResult)` on success, `Err(UbfwctlError)` on failure
    ///
    /// # Errors
    /// Returns an error if:
    /// - The time parameter is invalid
    /// - The device cannot be opened
    /// - The ioctl call fails
    /// - Shared memory locking fails
    pub fn execute(
        &self,
        chip_id: u32,
        die_id: u32,
        port: u32,
        time_ms: u32,
    ) -> Result<MarPerfResult, UbfwctlError> {
        // Validate time parameter
        UbfwctlError::validate_time(time_ms)?;

        // Acquire shared memory lock for concurrent access safety
        let _lock = Self::acquire_shm_lock(chip_id, die_id, port)?;

        // Open device
        let device = FwctlDevice::open(chip_id, die_id)?;

        // Configuration phase - starts the measurement
        device.mar_perf_config(port, time_ms)?;

        // Query phase - get the results
        let raw_data = device.mar_perf_query(port)?;

        // Parse query data
        let query = MarPerfQuery::from_raw_data(&raw_data);

        // Get clock frequency from raw data
        let clock_freq_hz = raw_data.get(1).copied().unwrap_or(0);

        // Calculate results
        let result = MarPerfResult::calculate(&query, time_ms, clock_freq_hz);

        Ok(result)
    }

    /// Acquire shared memory lock for concurrent access safety
    ///
    /// # Arguments
    /// * `chip_id` - Chip ID
    /// * `die_id` - Die ID
    /// * `port` - Port ID
    ///
    /// # Returns
    /// `Ok(LockGuard)` on success, `Err(UbfwctlError)` on failure
    fn acquire_shm_lock(
        chip_id: u32,
        die_id: u32,
        port: u32,
    ) -> Result<ShmLockGuard, UbfwctlError> {
        // Calculate port pair (mar_perf measures pairs of ports)
        let port_pair = port / 2;
        let shm_name = format!("/ubctl_{chip_id}_{die_id}_nl_{port_pair}");

        ShmLockGuard::new(&shm_name)
    }
}

impl Default for MarPerfCommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard for shared memory lock
#[derive(Debug)]
pub struct ShmLockGuard {
    /// File descriptor for the lock file
    fd: c_int,
}

impl ShmLockGuard {
    /// Create a new shared memory lock guard
    ///
    /// # Arguments
    /// * `name` - Name of the shared memory segment
    ///
    /// # Returns
    /// `Ok(ShmLockGuard)` on success, `Err(UbfwctlError)` on failure
    fn new(name: &str) -> Result<Self, UbfwctlError> {
        let shm_path = format!("/dev/shm{name}");

        // Create/open the lock file
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&shm_path)
            .map_err(|e| UbfwctlError::ShmLockFailed(format!("Failed to open shm: {e}")))?;

        let fd = file.as_raw_fd();

        // Acquire exclusive lock (non-blocking)
        // SAFETY: flock is safe to call with a valid fd and valid flags
        let ret = unsafe { flock(fd, LOCK_EX | LOCK_NB) };

        if ret != 0 {
            return Err(UbfwctlError::ShmLockFailed(format!(
                "Failed to acquire lock on {shm_path}: errno {ret}"
            )));
        }

        // Keep the file open by forgetting it - we'll close it in Drop
        std::mem::forget(file);

        Ok(Self { fd })
    }
}

impl Drop for ShmLockGuard {
    fn drop(&mut self) {
        // SAFETY: flock and close are safe to call with a valid fd
        unsafe {
            // Release the lock and close the file
            let _ = flock(self.fd, libc::LOCK_UN);
            let _ = libc::close(self.fd);
        }
    }
}

// Safety: ShmLockGuard contains a c_int which is Send + Sync
unsafe impl Send for ShmLockGuard {}
unsafe impl Sync for ShmLockGuard {}

/// High-level convenience function for `mar_perf` measurement
///
/// # Arguments
/// * `chip_id` - Chip ID
/// * `die_id` - Die ID
/// * `port` - Port ID
/// * `time_ms` - Measurement time in milliseconds
///
/// # Returns
/// `Ok(MarPerfResult)` on success, `Err(UbfwctlError)` on failure
///
/// # Errors
/// Returns an error if:
/// - The time parameter is invalid
/// - The device cannot be opened
/// - The ioctl call fails
/// - Shared memory locking fails
///
/// # Example
/// ```no_run
/// use ubfwctl::commands::mar_perf::mar_perf_measure;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = mar_perf_measure(0, 0, 0, 1000)?;
///     println!("Write traffic: {} bytes/s", result.wr_traffic);
///     println!("Read traffic: {} bytes/s", result.rd_traffic);
///     Ok(())
/// }
/// ```
pub fn mar_perf_measure(
    chip_id: u32,
    die_id: u32,
    port: u32,
    time_ms: u32,
) -> Result<MarPerfResult, UbfwctlError> {
    let cmd = MarPerfCommand::new();
    cmd.execute(chip_id, die_id, port, time_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mar_perf_command_new() {
        let _cmd = MarPerfCommand::new();
        // Just verify it can be created
    }
}
