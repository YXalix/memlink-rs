//! Unified session management for ETMEM operations
//!
//! This module provides `EtmemSession`, a unified session that combines
//! scanning and swapping capabilities with full VMA awareness. It serves as
//! the primary high-level API for ETMEM operations.
//!
//! # Example
//!
//! ```no_run
//! use etmem_rs::{EtmemSession, SessionConfig, ScanConfig};
//!
//! // Create a unified session
//! let mut session = EtmemSession::new(
//!     std::process::id() as u32,
//!     SessionConfig::default()
//! ).expect("Failed to create session");
//!
//! // Discover VMAs
//! session.discover_vmas()
//!     .expect("Failed to discover VMAs");
//!
//! // Scan all scannable VMAs
//! let results = session.scan_all_vmas(ScanConfig::default())
//!     .expect("Failed to scan VMAs");
//! println!("Found {} idle pages across {} VMAs",
//!     results.total_idle_pages(), results.per_vma.len());
//! ```

use crate::error::{EtmemError, Result};
use crate::scan::ScanSession;
use crate::swap::SwapSession;
use crate::types::{AddressRange, IdlePageInfo, ScanConfig, SwapConfig};
use crate::vma::{VmaFilter, VmaMap, VmaRegion};

/// Configuration for a unified ETMEM session
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Scan configuration
    pub scan: ScanConfig,
    /// Swap configuration
    pub swap: SwapConfig,
    /// Auto-discover VMAs on session creation
    pub auto_discover_vmas: bool,
    /// Filter for auto-discovered VMAs
    pub vma_filter: VmaFilter,
}

impl SessionConfig {
    /// Create a new session configuration with defaults
    pub const fn new() -> Self {
        Self {
            scan: ScanConfig::new(),
            swap: SwapConfig::new(),
            auto_discover_vmas: false,
            vma_filter: VmaFilter::SCANNABLE,
        }
    }

    /// Set scan configuration
    pub const fn with_scan_config(mut self, config: ScanConfig) -> Self {
        self.scan = config;
        self
    }

    /// Set swap configuration
    pub const fn with_swap_config(mut self, config: SwapConfig) -> Self {
        self.swap = config;
        self
    }

    /// Enable auto-discovery of VMAs on session creation
    pub const fn with_auto_discover_vmas(mut self, enable: bool) -> Self {
        self.auto_discover_vmas = enable;
        self
    }

    /// Set the VMA filter for auto-discovery
    pub const fn with_vma_filter(mut self, filter: VmaFilter) -> Self {
        self.vma_filter = filter;
        self
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Results from scanning multiple VMAs
#[derive(Debug, Clone, Default)]
pub struct VmaScanResults {
    /// Results per VMA (region -> page infos)
    pub per_vma: std::collections::HashMap<AddressRange, Vec<IdlePageInfo>>,
    /// Total idle bytes across all scanned VMAs
    pub total_idle_bytes: u64,
    /// Total accessed bytes across all scanned VMAs
    pub total_accessed_bytes: u64,
    /// Total scannable bytes
    pub total_scanned_bytes: u64,
}

impl VmaScanResults {
    /// Create empty results
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate idle ratio (0.0 - 1.0)
    pub fn idle_ratio(&self) -> f64 {
        if self.total_scanned_bytes == 0 {
            0.0
        } else {
            self.total_idle_bytes as f64 / self.total_scanned_bytes as f64
        }
    }

    /// Calculate accessed ratio (0.0 - 1.0)
    pub fn accessed_ratio(&self) -> f64 {
        if self.total_scanned_bytes == 0 {
            0.0
        } else {
            self.total_accessed_bytes as f64 / self.total_scanned_bytes as f64
        }
    }

    /// Get total number of idle pages
    pub fn total_idle_pages(&self) -> usize {
        self.per_vma
            .values()
            .flat_map(|pages| pages.iter().filter(|p| p.is_idle()))
            .count()
    }

    /// Get total number of accessed pages
    pub fn total_accessed_pages(&self) -> usize {
        self.per_vma
            .values()
            .flat_map(|pages| pages.iter().filter(|p| p.is_accessed()))
            .count()
    }

    /// Get all idle page addresses from all VMAs
    pub fn all_idle_addresses(&self) -> Vec<u64> {
        self.per_vma
            .values()
            .flat_map(|pages| pages.iter().filter(|p| p.is_idle()).map(|p| p.address))
            .collect()
    }

    /// Merge another set of results into this one
    pub fn merge(&mut self, other: VmaScanResults) {
        for (range, pages) in other.per_vma {
            let entry = self.per_vma.entry(range).or_default();
            entry.extend(pages);
        }
        self.total_idle_bytes += other.total_idle_bytes;
        self.total_accessed_bytes += other.total_accessed_bytes;
        self.total_scanned_bytes += other.total_scanned_bytes;
    }
}

/// Unified ETMEM session combining scan and swap operations
///
/// This is the primary high-level API for ETMEM operations. It provides:
/// - Unified management of scan and swap sessions
/// - VMA-aware operations
/// - Lifecycle management (resources cleaned up on drop)
#[derive(Debug)]
pub struct EtmemSession {
    /// Process ID
    pid: u32,
    /// Session configuration
    config: SessionConfig,
    /// Scan session (lazy-initialized)
    scan_session: Option<ScanSession>,
    /// Swap session (lazy-initialized)
    swap_session: Option<SwapSession>,
    /// Discovered VMA map
    vma_map: Option<VmaMap>,
    /// Whether the session is closed
    closed: bool,
}

impl EtmemSession {
    /// Create a new unified session for a process
    ///
    /// # Errors
    /// Returns error if:
    /// - Process doesn't exist
    /// - Permission denied (requires CAP_SYS_ADMIN)
    /// - ETMEM module not loaded
    ///
    /// # Example
    /// ```no_run
    /// use etmem_rs::EtmemSession;
    ///
    /// let session = EtmemSession::new(
    ///     std::process::id() as u32,
    ///     Default::default()
    /// ).expect("Failed to create session");
    /// ```
    pub fn new(pid: u32, config: SessionConfig) -> Result<Self> {
        if pid == 0 {
            return Err(EtmemError::InvalidPid);
        }

        let mut session = Self {
            pid,
            config,
            scan_session: None,
            swap_session: None,
            vma_map: None,
            closed: false,
        };

        // Auto-discover VMAs if configured
        if session.config.auto_discover_vmas {
            session.discover_vmas()?;
        }

        Ok(session)
    }

    /// Get the process ID
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Check if the session is closed
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Get the session configuration
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Discover VMAs for the process
    ///
    /// Parses `/proc/[pid]/maps` and stores the result for later use.
    /// This is called automatically if `auto_discover_vmas` is enabled.
    pub fn discover_vmas(&mut self) -> Result<&VmaMap> {
        self.ensure_open()?;
        let vma_map = VmaMap::for_process(self.pid)?;
        self.vma_map = Some(vma_map);
        Ok(self.vma_map.as_ref().unwrap())
    }

    /// Get the discovered VMA map (if any)
    pub fn vma_map(&self) -> Option<&VmaMap> {
        self.vma_map.as_ref()
    }

    /// Get or create the scan session
    fn get_scan_session(&mut self) -> Result<&mut ScanSession> {
        self.ensure_open()?;
        if self.scan_session.is_none() {
            self.scan_session = Some(ScanSession::new(self.pid, self.config.scan.clone())?);
        }
        Ok(self.scan_session.as_mut().unwrap())
    }

    /// Get or create the swap session
    fn get_swap_session(&mut self) -> Result<&mut SwapSession> {
        self.ensure_open()?;
        if self.swap_session.is_none() {
            self.swap_session = Some(SwapSession::new(self.pid, self.config.swap.clone())?);
        }
        Ok(self.swap_session.as_mut().unwrap())
    }

    /// Scan a specific address range
    ///
    /// # Errors
    /// Returns error if:
    /// - I/O error occurs
    /// - Invalid range
    /// - Session is closed
    pub fn scan_range(&mut self, range: AddressRange) -> Result<Vec<IdlePageInfo>> {
        self.ensure_open()?;
        let session = self.get_scan_session()?;
        session.read_range(range)
    }

    /// Scan a specific VMA
    ///
    /// # Errors
    /// Returns error if:
    /// - I/O error occurs
    /// - Session is closed
    pub fn scan_vma(&mut self, vma: &VmaRegion, _config: ScanConfig) -> Result<Vec<IdlePageInfo>> {
        self.ensure_open()?;
        let session = self.get_scan_session()?;
        session.read_range(vma.to_address_range())
    }

    /// Scan all scannable VMAs
    ///
    /// This scans all VMAs that match the `SCANNABLE` filter criteria.
    /// Automatically discovers VMAs if not already done.
    pub fn scan_all_vmas(&mut self, scan_config: ScanConfig) -> Result<VmaScanResults> {
        self.ensure_open()?;

        // Auto-discover VMAs if needed
        if self.vma_map.is_none() {
            self.discover_vmas()?;
        }

        let vma_map = self.vma_map.as_ref().unwrap().clone();
        let scannable = vma_map.scannable();

        let mut results = VmaScanResults::new();

        for vma in scannable {
            let pages = self.scan_vma(vma, scan_config.clone())?;

            // Calculate statistics
            let range = vma.to_address_range();
            let mut idle_bytes = 0u64;
            let mut accessed_bytes = 0u64;

            for page in &pages {
                if page.is_idle() {
                    idle_bytes += page.total_size();
                } else if page.is_accessed() {
                    accessed_bytes += page.total_size();
                }
            }

            results.per_vma.insert(range, pages);
            results.total_idle_bytes += idle_bytes;
            results.total_accessed_bytes += accessed_bytes;
            results.total_scanned_bytes += range.size();
        }

        Ok(results)
    }

    /// Swap a single address
    ///
    /// # Errors
    /// Returns error if:
    /// - Address not page-aligned
    /// - I/O error occurs
    /// - Session is closed
    pub fn swap_address(&mut self, addr: u64) -> Result<()> {
        self.ensure_open()?;
        let session = self.get_swap_session()?;
        session.swap_address(addr)
    }

    /// Swap multiple addresses
    ///
    /// # Errors
    /// Returns error if:
    /// - Any address not page-aligned
    /// - I/O error occurs
    /// - Session is closed
    pub fn swap_addresses(&mut self, addrs: &[u64]) -> Result<usize> {
        self.ensure_open()?;
        let session = self.get_swap_session()?;
        session.add_addresses(addrs)?;
        session.flush()
    }

    /// Swap pages in a specific VMA
    ///
    /// This swaps all idle pages discovered in the given VMA.
    /// Note: This does NOT automatically scan first - use `scan_and_swap_vma` for that.
    pub fn swap_in_vma(&mut self, vma: &VmaRegion, pages: &[u64]) -> Result<usize> {
        self.ensure_open()?;
        // Validate pages are within the VMA
        for &addr in pages {
            if !vma.contains(addr) {
                return Err(EtmemError::InvalidAddress);
            }
        }
        self.swap_addresses(pages)
    }

    /// Scan and swap a specific VMA
    ///
    /// This is a convenience method that:
    /// 1. Scans the VMA for idle pages
    /// 2. Swaps all idle pages found
    ///
    /// Returns the number of pages swapped.
    pub fn scan_and_swap_vma(&mut self, vma: &VmaRegion, config: ScanConfig) -> Result<usize> {
        self.ensure_open()?;
        let pages = self.scan_vma(vma, config)?;
        let idle_addrs: Vec<u64> = pages
            .into_iter()
            .filter(|p| p.is_idle())
            .map(|p| p.address)
            .collect();

        if idle_addrs.is_empty() {
            return Ok(0);
        }

        self.swap_in_vma(vma, &idle_addrs)
    }

    /// Scan and swap all scannable VMAs
    ///
    /// This is a comprehensive operation that:
    /// 1. Discovers VMAs (if not already done)
    /// 2. Scans all scannable VMAs
    /// 3. Swaps idle pages in each VMA
    ///
    /// Returns a report of the operation.
    pub fn scan_and_swap_all(&mut self, scan_config: ScanConfig) -> Result<ScanAndSwapReport> {
        self.ensure_open()?;

        // Ensure VMAs are discovered
        if self.vma_map.is_none() {
            self.discover_vmas()?;
        }

        let scan_results = self.scan_all_vmas(scan_config)?;
        let idle_addrs = scan_results.all_idle_addresses();
        let pages_swapped = self.swap_addresses(&idle_addrs)?;

        Ok(ScanAndSwapReport {
            vmas_scanned: scan_results.per_vma.len(),
            pages_scanned: scan_results.total_idle_pages() + scan_results.total_accessed_pages(),
            pages_swapped,
            bytes_swapped: scan_results.total_idle_bytes,
            idle_ratio: scan_results.idle_ratio(),
        })
    }

    /// Ensure the session is open
    fn ensure_open(&self) -> Result<()> {
        if self.closed {
            Err(EtmemError::InvalidVma("Session is closed".to_string()))
        } else {
            Ok(())
        }
    }

    /// Close the session and release all resources
    ///
    /// This is called automatically when the session is dropped, but
    /// can be called explicitly for error handling.
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }

        // Flush any pending swap operations
        if let Some(ref mut session) = self.swap_session {
            session.flush()?;
        }

        // Clear sessions
        self.scan_session = None;
        self.swap_session = None;
        self.closed = true;

        Ok(())
    }
}

impl Drop for EtmemSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Report from a scan-and-swap operation
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanAndSwapReport {
    /// Number of VMAs scanned
    pub vmas_scanned: usize,
    /// Total pages scanned
    pub pages_scanned: usize,
    /// Number of pages swapped
    pub pages_swapped: usize,
    /// Bytes swapped (idle memory)
    pub bytes_swapped: u64,
    /// Idle ratio (0.0 - 1.0)
    pub idle_ratio: f64,
}

impl ScanAndSwapReport {
    /// Check if the swap was effective
    pub fn was_effective(&self) -> bool {
        self.pages_swapped > 0
    }

    /// Calculate swap efficiency (pages swapped / pages scanned)
    pub fn efficiency(&self) -> f64 {
        if self.pages_scanned == 0 {
            0.0
        } else {
            self.pages_swapped as f64 / self.pages_scanned as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert!(!config.auto_discover_vmas);
        assert!(config.vma_filter.contains(VmaFilter::SCANNABLE));
    }

    #[test]
    fn test_session_config_builder() {
        let config = SessionConfig::new()
            .with_auto_discover_vmas(true)
            .with_vma_filter(VmaFilter::ANONYMOUS);

        assert!(config.auto_discover_vmas);
        assert!(config.vma_filter.contains(VmaFilter::ANONYMOUS));
    }

    #[test]
    fn test_vma_scan_results() {
        let mut results = VmaScanResults::new();
        results.total_idle_bytes = 1000;
        results.total_accessed_bytes = 2000;
        results.total_scanned_bytes = 3000;

        assert!((results.idle_ratio() - 0.333).abs() < 0.01);
        assert!((results.accessed_ratio() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_scan_and_swap_report() {
        let report = ScanAndSwapReport {
            vmas_scanned: 5,
            pages_scanned: 100,
            pages_swapped: 30,
            bytes_swapped: 123_456,
            idle_ratio: 0.3,
        };

        assert!(report.was_effective());
        assert!((report.efficiency() - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_etmem_session_new_invalid_pid() {
        let result = EtmemSession::new(0, SessionConfig::default());
        assert!(matches!(result.unwrap_err(), EtmemError::InvalidPid));
    }

    #[test]
    fn test_vma_scan_results_merge() {
        let mut results1 = VmaScanResults::new();
        results1.total_idle_bytes = 1000;
        results1.total_scanned_bytes = 3000;

        let mut results2 = VmaScanResults::new();
        results2.total_idle_bytes = 500;
        results2.total_scanned_bytes = 1500;

        results1.merge(results2);

        assert_eq!(results1.total_idle_bytes, 1500);
        assert_eq!(results1.total_scanned_bytes, 4500);
    }
}
