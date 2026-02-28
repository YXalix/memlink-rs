//! Low-level device communication for OBMM
//!
//! This module provides thread-safe access to the /dev/obmm device file
//! using ioctl system calls.

use crate::error::ObmmError;
use libc::{O_CLOEXEC, O_RDWR, c_char, c_int, c_ulong, c_void, close, ioctl, open};
use std::os::fd::RawFd;
use std::sync::{Mutex, MutexGuard};

/// OBMM device path
const OBMM_DEV_PATH: &str = "/dev/obmm";

/// Thread-safe singleton device handle
static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

/// OBMM device handle
pub struct Device {
    /// Raw file descriptor
    fd: RawFd,
}

impl Device {
    /// Open the OBMM device file
    ///
    /// # Errors
    ///
    /// Returns an error if the device file cannot be opened.
    pub fn open() -> Result<Self, ObmmError> {
        let path = b"/dev/obmm\0";
        let fd = unsafe { open(path.as_ptr().cast::<c_char>(), O_RDWR | O_CLOEXEC) };

        if fd < 0 {
            let err = std::io::Error::last_os_error();
            return Err(ObmmError::DeviceError(format!(
                "Failed to open /dev/obmm: {} (is the OBMM kernel module loaded?)",
                err
            )));
        }

        Ok(Self { fd })
    }

    /// Execute an ioctl command on the device
    ///
    /// # Safety
    ///
    /// The caller must ensure that `arg` points to a valid structure of the
    /// correct type for the given ioctl command.
    ///
    /// # Arguments
    ///
    /// * `request` - The ioctl request code
    /// * `arg` - Pointer to the argument structure
    ///
    /// # Errors
    ///
    /// Returns an error if the ioctl call fails.
    pub unsafe fn ioctl<T>(&self, request: c_ulong, arg: *mut T) -> Result<c_int, ObmmError> {
        unsafe {
            let ret = ioctl(self.fd, request, arg as *mut c_void);

            if ret < 0 {
                let err = std::io::Error::last_os_error();
                Err(ObmmError::IoError(format!("ioctl failed: {}", err)))
            } else {
                Ok(ret)
            }
        }
    }

    /// Get the raw file descriptor
    #[inline]
    pub fn fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            close(self.fd);
        }
    }
}

/// Get or initialize the global device instance
///
/// This function provides thread-safe access to a singleton Device instance.
/// The device is opened on first access and reused for subsequent calls.
fn get_device() -> Result<MutexGuard<'static, Option<Device>>, ObmmError> {
    let mut guard = DEVICE
        .lock()
        .map_err(|_| ObmmError::DeviceError("Failed to lock device mutex".to_string()))?;

    if guard.is_none() {
        *guard = Some(Device::open()?);
    }

    Ok(guard)
}

/// Execute a closure with access to the device
///
/// This function handles device initialization and provides the closure
/// with a reference to the device.
///
/// # Type Parameters
///
/// * `F` - Closure type
/// * `R` - Return type
///
/// # Arguments
///
/// * `f` - Closure that takes a `&Device` and returns `Result<R, ObmmError>`
///
/// # Example
///
/// ```rust,ignore
/// with_device(|dev| {
///     let mut cmd = ObmmCmdExport::default();
///     // ... populate cmd ...
///     unsafe { dev.ioctl(OBMM_CMD_EXPORT as c_ulong, &mut cmd) }?;
///     Ok(cmd.mem_id)
/// })
/// ```
pub fn with_device<F, R>(f: F) -> Result<R, ObmmError>
where
    F: FnOnce(&Device) -> Result<R, ObmmError>,
{
    let guard = get_device()?;
    f(guard.as_ref().unwrap())
}

/// Execute an ioctl with the global device instance
///
/// This is a convenience function that combines `with_device` and `ioctl`.
///
/// # Safety
///
/// The caller must ensure that `arg` points to a valid structure of the
/// correct type for the given ioctl command.
///
/// # Example
///
/// ```rust,ignore
/// let mut cmd = ObmmCmdExport::default();
/// // ... populate cmd ...
/// unsafe { device_ioctl(OBMM_CMD_EXPORT as c_ulong, &mut cmd) }?;
/// ```
pub unsafe fn device_ioctl<T>(request: u32, arg: *mut T) -> Result<c_int, ObmmError> {
    with_device(|dev| unsafe { dev.ioctl(request as c_ulong, arg) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_open() {
        // This test requires the OBMM kernel module to be loaded
        // Skip if not available
        match Device::open() {
            Ok(_) => {}
            Err(ObmmError::DeviceError(_)) => {
                // Expected if kernel module is not loaded
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
