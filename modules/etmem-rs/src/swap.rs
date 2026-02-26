//! Page swapping operations for cold page reclamation
//!
//! This module provides safe wrappers around the kernel's page swapping
//! functionality. It allows reclaiming "cold" memory pages by swapping
//! them out to secondary storage.

use std::fmt::Write as _;

use crate::error::{EtmemError, Result};
use crate::sys::ProcfsHandle;
use crate::types::{SwapConfig, SwapcacheWatermark, WatermarkConfig};

/// Safe wrapper for page swapping session
///
/// This provides a safe interface to the kernel's page swapping
/// functionality. Pages are swapped out to free up physical memory.
#[derive(Debug)]
pub struct SwapSession {
    /// Underlying procfs file handle
    handle: ProcfsHandle,
    /// Swap configuration
    config: SwapConfig,
    /// Process ID
    pid: u32,
    /// List of virtual addresses to swap (accumulated)
    pending_addrs: Vec<u64>,
}

impl SwapSession {
    /// Create a new swap session for a process
    ///
    /// # Errors
    /// Returns error if:
    /// - Process doesn't exist
    /// - Permission denied (requires CAP_SYS_ADMIN)
    /// - ETMEM module not loaded
    ///
    /// # Example
    /// ```no_run
    /// use etmem_rs::{SwapSession, SwapConfig};
    ///
    /// let config = SwapConfig::default();
    /// let session = SwapSession::new(std::process::id() as u32, config)
    ///     .expect("Failed to create swap session");
    /// ```
    pub fn new(pid: u32, config: SwapConfig) -> Result<Self> {
        if pid == 0 {
            return Err(EtmemError::InvalidPid);
        }

        // Validate watermark configuration
        config.watermark.validate()?;

        // Safe: handle construction is encapsulated
        let handle = unsafe { ProcfsHandle::open_swap_pages(pid)? };

        Ok(Self {
            handle,
            config,
            pid,
            pending_addrs: Vec::new(),
        })
    }

    /// Add a virtual address to the swap list
    ///
    /// The address will be buffered and swapped when `flush()` is called
    /// or when the buffer reaches capacity.
    ///
    /// # Errors
    /// Returns error if the address is not page-aligned.
    pub fn add_address(&mut self, addr: u64) -> Result<()> {
        // Validate address alignment (must be page-aligned)
        if addr % 4096 != 0 {
            return Err(EtmemError::InvalidAddress);
        }

        self.pending_addrs.push(addr);

        // Auto-flush if we reach the max
        if self.pending_addrs.len() >= self.config.max_pages as usize {
            self.flush()?;
        }

        Ok(())
    }

    /// Add multiple virtual addresses to the swap list
    ///
    /// Convenience method for adding multiple addresses at once.
    pub fn add_addresses(&mut self, addrs: &[u64]) -> Result<()> {
        for &addr in addrs {
            self.add_address(addr)?;
        }
        Ok(())
    }

    /// Flush pending addresses to the kernel
    ///
    /// This writes the buffered addresses to `/proc/[pid]/swap_pages`
    /// and clears the internal buffer.
    ///
    /// # Errors
    /// Returns error if:
    /// - I/O error occurs
    /// - Kernel rejects the addresses
    pub fn flush(&mut self) -> Result<usize> {
        if self.pending_addrs.is_empty() {
            return Ok(0);
        }

        // Format addresses as newline-separated hex strings
        let mut buf = String::new();
        for addr in &self.pending_addrs {
            writeln!(buf, "{:x}", addr).map_err(|e| EtmemError::IoError(e.to_string()))?;
        }

        // Write to procfs
        let bytes_written = unsafe {
            self.handle
                .write(buf.as_bytes())
                .map_err(|e| EtmemError::IoError(e.to_string()))?
        };

        if bytes_written < 0 {
            return Err(EtmemError::SwapFailed(
                "Kernel rejected swap request".to_string(),
            ));
        }

        let count = self.pending_addrs.len();
        self.pending_addrs.clear();

        Ok(count)
    }

    /// Swap a single address immediately
    ///
    /// Convenience method that adds an address and flushes immediately.
    pub fn swap_address(&mut self, addr: u64) -> Result<()> {
        self.add_address(addr)?;
        self.flush()?;
        Ok(())
    }

    /// Get the number of pending addresses
    pub fn pending_count(&self) -> usize {
        self.pending_addrs.len()
    }

    /// Check if there are pending addresses to swap
    pub fn has_pending(&self) -> bool {
        !self.pending_addrs.is_empty()
    }

    /// Clear pending addresses without swapping
    pub fn clear_pending(&mut self) {
        self.pending_addrs.clear();
    }

    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get current swap configuration
    pub fn config(&self) -> &SwapConfig {
        &self.config
    }

    /// Set proactive reclaim watermark
    ///
    /// Configures the kernel's proactive swapcache reclaim watermarks
    /// via IOCTL.
    ///
    /// # Errors
    /// Returns error if the watermark is invalid or IOCTL fails.
    pub fn set_watermark(&mut self, watermark: WatermarkConfig) -> Result<()> {
        watermark.validate()?;

        // Set low watermark
        unsafe {
            crate::sys::set_swapcache_watermark(
                &self.handle,
                SwapcacheWatermark::Low as u32,
                watermark.low_percent as u32,
            )?;

            // Set high watermark
            crate::sys::set_swapcache_watermark(
                &self.handle,
                SwapcacheWatermark::High as u32,
                watermark.high_percent as u32,
            )?;
        }

        self.config.watermark = watermark;
        Ok(())
    }

    /// Enable proactive swapcache reclaim
    ///
    /// This starts a kernel thread that proactively reclaims swapcache
    /// pages when they exceed the configured watermarks.
    pub fn enable_proactive_reclaim(&mut self) -> Result<()> {
        unsafe {
            crate::sys::enable_swapcache_reclaim(&self.handle)?;
        }
        self.config.proactive_reclaim = true;
        Ok(())
    }

    /// Disable proactive swapcache reclaim
    pub fn disable_proactive_reclaim(&mut self) -> Result<()> {
        unsafe {
            crate::sys::disable_swapcache_reclaim(&self.handle)?;
        }
        self.config.proactive_reclaim = false;
        Ok(())
    }
}

impl Drop for SwapSession {
    fn drop(&mut self) {
        // Try to flush any remaining addresses
        let _ = self.flush();
        // File handle is closed automatically by ProcfsHandle Drop
    }
}

/// High-level page swapper
///
/// This provides a convenient API for swapping pages without managing
/// the session lifecycle manually.
#[derive(Debug)]
pub struct PageSwapper;

impl PageSwapper {
    /// Swap a single page in a process
    ///
    /// Creates a temporary swap session and swaps a single address.
    ///
    /// # Example
    /// ```no_run
    /// use etmem_rs::PageSwapper;
    ///
    /// PageSwapper::swap_page(std::process::id() as u32, 0x7fff0000)
    ///     .expect("Failed to swap page");
    /// ```
    pub fn swap_page(pid: u32, addr: u64) -> Result<()> {
        let mut session = SwapSession::new(pid, SwapConfig::default())?;
        session.swap_address(addr)
    }

    /// Swap multiple pages in a process
    ///
    /// Creates a temporary swap session and swaps multiple addresses.
    pub fn swap_pages(pid: u32, addrs: &[u64]) -> Result<usize> {
        let mut session = SwapSession::new(pid, SwapConfig::default())?;
        session.add_addresses(addrs)?;
        session.flush()
    }

    /// Configure proactive reclaim for a process
    ///
    /// Sets up proactive swapcache reclaim with the specified watermarks.
    ///
    /// # Example
    /// ```no_run
    /// use etmem_rs::{PageSwapper, WatermarkConfig};
    ///
    /// let watermark = WatermarkConfig::new(30, 70);
    /// PageSwapper::configure_proactive_reclaim(std::process::id() as u32, watermark, true)
    ///     .expect("Failed to configure reclaim");
    /// ```
    pub fn configure_proactive_reclaim(
        pid: u32,
        watermark: WatermarkConfig,
        enable: bool,
    ) -> Result<()> {
        let mut session = SwapSession::new(pid, SwapConfig::default().with_watermark(watermark))?;
        session.set_watermark(watermark)?;

        if enable {
            session.enable_proactive_reclaim()?;
        } else {
            session.disable_proactive_reclaim()?;
        }

        Ok(())
    }
}

/// Global swapcache configuration
///
/// These functions operate on the system-wide swapcache settings
/// via sysfs.
pub struct SwapcacheConfig;

impl SwapcacheConfig {
    /// Check if kernel swap is enabled
    ///
    /// Reads from `/sys/kernel/mm/etmem/kernel_swap_enable`
    pub fn is_enabled() -> Result<bool> {
        crate::sys::kernel_swap_enabled()
            .map_err(|e| EtmemError::IoError(e.to_string()))
    }

    /// Enable or disable kernel swap
    ///
    /// Writes to `/sys/kernel/mm/etmem/kernel_swap_enable`
    pub fn set_enabled(enable: bool) -> Result<()> {
        crate::sys::set_kernel_swap_enable(enable)
            .map_err(|e| EtmemError::IoError(e.to_string()))
    }

    /// Enable kernel swap (convenience method)
    pub fn enable() -> Result<()> {
        Self::set_enabled(true)
    }

    /// Disable kernel swap (convenience method)
    pub fn disable() -> Result<()> {
        Self::set_enabled(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SWAP_SCAN_NUM_MAX;

    #[test]
    fn test_swap_config_default() {
        let config = SwapConfig::default();
        assert!(!config.proactive_reclaim);
        assert_eq!(config.max_pages, SWAP_SCAN_NUM_MAX);
        assert_eq!(config.watermark.low_percent, 30);
        assert_eq!(config.watermark.high_percent, 70);
    }

    #[test]
    fn test_watermark_validation() {
        let watermark = WatermarkConfig::new(30, 70);
        assert!(watermark.validate().is_ok());

        let invalid = WatermarkConfig::new(70, 30);
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_address_formatting() {
        let mut buf = String::new();
        writeln!(buf, "{:x}", 0x7fff0000u64).unwrap();
        assert_eq!(buf, "7fff0000\n");
    }
}
