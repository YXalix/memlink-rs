//! Low-level FFI bindings for ETMEM kernel interface
//!
//! This module contains unsafe FFI bindings to interact with the kernel's
//! ETMEM subsystem through procfs and IOCTL interfaces.
//!
//! # Safety
//!
//! All functions in this module are unsafe as they perform raw system calls.
//! They should be wrapped by safe abstractions in the higher-level modules.

use libc::{c_int, c_void, ioctl, off_t, ssize_t};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};

/// Procfs path for idle page scanning
pub fn idle_pages_path(pid: u32) -> String {
    format!("/proc/{}/idle_pages", pid)
}

/// Procfs path for page swapping
pub fn swap_pages_path(pid: u32) -> String {
    format!("/proc/{}/swap_pages", pid)
}

/// Sysfs path for kernel swap enable
pub const SYS_ETMEM_SWAP_ENABLE: &str = "/sys/kernel/mm/swap/kernel_swap_enable";

/// IOCTL commands for idle scan operations
///
/// These are constructed using the standard Linux IOCTL encoding:
/// - bits 0-7: command number
/// - bits 8-15: magic number (0x66 for idle scan)
/// - bits 16-29: size of argument
/// - bit 30: _IOW (write to kernel)
///
/// IOCTL command to add scan flags
pub const IDLE_SCAN_ADD_FLAGS: u64 =
    ((IDLE_SCAN_MAGIC as u64) << 8) | (4u64 << 16) | (1u64 << 30);
/// IOCTL command to remove scan flags
pub const IDLE_SCAN_REMOVE_FLAGS: u64 =
    ((IDLE_SCAN_MAGIC as u64) << 8) | (0x1u64) | (4u64 << 16) | (1u64 << 30);
/// IOCTL command to add VMA scan flags
pub const VMA_SCAN_ADD_FLAGS: u64 =
    ((IDLE_SCAN_MAGIC as u64) << 8) | (0x2u64) | (4u64 << 16) | (1u64 << 30);
/// IOCTL command to remove VMA scan flags
pub const VMA_SCAN_REMOVE_FLAGS: u64 =
    ((IDLE_SCAN_MAGIC as u64) << 8) | (0x3u64) | (4u64 << 16) | (1u64 << 30);

/// IOCTL commands for swapcache reclaim operations
///
/// Uses magic number 0x77 for swapcache operations.
///
/// IOCTL command to enable swapcache reclaim
pub const RECLAIM_SWAPCACHE_ON: u64 =
    ((RECLAIM_SWAPCACHE_MAGIC as u64) << 8) | (0x01u64) | (4u64 << 16) | (1u64 << 30);
/// IOCTL command to disable swapcache reclaim
pub const RECLAIM_SWAPCACHE_OFF: u64 =
    ((RECLAIM_SWAPCACHE_MAGIC as u64) << 8) | (4u64 << 16) | (1u64 << 30);
/// IOCTL command to set swapcache watermark
pub const SET_SWAPCACHE_WMARK: u64 =
    ((RECLAIM_SWAPCACHE_MAGIC as u64) << 8) | (0x02u64) | (8u64 << 16) | (1u64 << 30);

use crate::types::{IDLE_SCAN_MAGIC, RECLAIM_SWAPCACHE_MAGIC};

/// Raw procfs file handle for ETMEM operations
///
/// This is a low-level wrapper around a file descriptor for procfs files.
/// It provides basic read/write operations for interacting with the kernel.
#[derive(Debug)]
pub struct ProcfsHandle {
    fd: RawFd,
}

impl ProcfsHandle {
    /// Open `/proc/[pid]/idle_pages` for reading
    ///
    /// # Safety
    /// This function uses unsafe FFI calls to open files.
    /// The caller must ensure the PID is valid and the ETMEM module is loaded.
    pub unsafe fn open_idle_pages(pid: u32) -> std::io::Result<Self> {
        let path = idle_pages_path(pid);
        let c_path = std::ffi::CString::new(path)?;
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { fd })
    }

    /// Open `/proc/[pid]/swap_pages` for writing
    ///
    /// # Safety
    /// This function uses unsafe FFI calls to open files.
    /// The caller must ensure the PID is valid and the ETMEM module is loaded.
    pub unsafe fn open_swap_pages(pid: u32) -> std::io::Result<Self> {
        let path = swap_pages_path(pid);
        let c_path = std::ffi::CString::new(path)?;
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { fd })
    }

    /// Read from procfs file at a specific offset
    ///
    /// # Safety
    /// This function performs raw system calls.
    /// The buffer must be valid and have the correct size.
    pub unsafe fn read_at(&self, buf: &mut [u8], offset: off_t) -> std::io::Result<ssize_t> {
        let result = unsafe {
            libc::pread(
                self.fd,
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
                offset,
            )
        };
        if result < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    /// Read from procfs file (current offset)
    ///
    /// # Safety
    /// This function performs raw system calls.
    /// The buffer must be valid and have the correct size.
    pub unsafe fn read(&self, buf: &mut [u8]) -> std::io::Result<ssize_t> {
        let result = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut c_void, buf.len()) };
        if result < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    /// Write to procfs file
    ///
    /// # Safety
    /// This function performs raw system calls.
    /// The buffer must be valid and have the correct size.
    pub unsafe fn write(&self, buf: &[u8]) -> std::io::Result<ssize_t> {
        let result = unsafe { libc::write(self.fd, buf.as_ptr() as *const c_void, buf.len()) };
        if result < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    /// Perform IOCTL on procfs file
    ///
    /// # Safety
    /// This function performs raw IOCTL system calls.
    /// The argument pointer must be valid for the specific IOCTL command.
    pub unsafe fn ioctl(&self, request: u64, arg: *mut c_void) -> std::io::Result<c_int> {
        let result = unsafe { ioctl(self.fd, request, arg) };
        if result < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    /// Get the underlying file descriptor
    pub fn raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl AsRawFd for ProcfsHandle {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl FromRawFd for ProcfsHandle {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl Drop for ProcfsHandle {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

/// Check if kernel swap is enabled
///
/// Reads from `/sys/kernel/mm/etmem/kernel_swap_enable` to determine
/// if the kernel's proactive swap reclaim is enabled.
pub fn kernel_swap_enabled() -> std::io::Result<bool> {
    let content = std::fs::read_to_string(SYS_ETMEM_SWAP_ENABLE)?;
    let trimmed = content.trim();
    Ok(trimmed == "true" || trimmed == "1" || trimmed == "enabled")
}

/// Enable or disable kernel swap
///
/// Writes to `/sys/kernel/mm/etmem/kernel_swap_enable` to control
/// the kernel's proactive swap reclaim.
pub fn set_kernel_swap_enable(enable: bool) -> std::io::Result<()> {
    let value = if enable { "true" } else { "false" };
    std::fs::write(SYS_ETMEM_SWAP_ENABLE, value)
}

/// Structure for swapcache watermark IOCTL argument
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SwapcacheWmarkArg {
    /// Watermark level (0 = low, 1 = high)
    pub level: u32,
    /// Watermark percentage (0-100)
    pub percent: u32,
}

/// Set swapcache watermark via IOCTL
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn set_swapcache_watermark(
    handle: &ProcfsHandle,
    level: u32,
    percent: u32,
) -> std::io::Result<()> {
    let mut arg = SwapcacheWmarkArg { level, percent };
    unsafe { handle.ioctl(SET_SWAPCACHE_WMARK, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

/// Enable proactive swapcache reclaim
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn enable_swapcache_reclaim(handle: &ProcfsHandle) -> std::io::Result<()> {
    let mut arg: u32 = 1;
    unsafe { handle.ioctl(RECLAIM_SWAPCACHE_ON, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

/// Disable proactive swapcache reclaim
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn disable_swapcache_reclaim(handle: &ProcfsHandle) -> std::io::Result<()> {
    let mut arg: u32 = 0;
    unsafe { handle.ioctl(RECLAIM_SWAPCACHE_OFF, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

/// Add scan flags via IOCTL
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn add_scan_flags(handle: &ProcfsHandle, flags: u32) -> std::io::Result<()> {
    let mut arg = flags;
    unsafe { handle.ioctl(IDLE_SCAN_ADD_FLAGS, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

/// Remove scan flags via IOCTL
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn remove_scan_flags(handle: &ProcfsHandle, flags: u32) -> std::io::Result<()> {
    let mut arg = flags;
    unsafe { handle.ioctl(IDLE_SCAN_REMOVE_FLAGS, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

/// Add VMA scan flags via IOCTL
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn add_vma_scan_flags(handle: &ProcfsHandle, flags: u32) -> std::io::Result<()> {
    let mut arg = flags;
    unsafe { handle.ioctl(VMA_SCAN_ADD_FLAGS, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

/// Remove VMA scan flags via IOCTL
///
/// # Safety
/// This function performs an IOCTL system call.
pub unsafe fn remove_vma_scan_flags(handle: &ProcfsHandle, flags: u32) -> std::io::Result<()> {
    let mut arg = flags;
    unsafe { handle.ioctl(VMA_SCAN_REMOVE_FLAGS, &mut arg as *mut _ as *mut c_void) }?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths() {
        assert_eq!(idle_pages_path(1234), "/proc/1234/idle_pages");
        assert_eq!(swap_pages_path(5678), "/proc/5678/swap_pages");
    }

    #[test]
    fn test_ioctl_encoding() {
        // Verify IOCTL command encoding matches kernel expectations
        // _IOW(0x66, 0, u32) for IDLE_SCAN_ADD_FLAGS
        let expected = ((0x66u64) << 8) | (4u64 << 16) | (1u64 << 30);
        assert_eq!(IDLE_SCAN_ADD_FLAGS, expected);
    }
}
