//! High-level workflow builders for ETMEM operations
//!
//! This module provides declarative workflow builders that simplify common
//! ETMEM operations like "scan then swap" workflows.
//!
//! # Example
//!
//! ```no_run
//! use etmem_rs::workflow::ScanAndSwapWorkflow;
//! use etmem_rs::vma::VmaFilter;
//!
//! // Simple scan-and-swap workflow
//! let report = ScanAndSwapWorkflow::new(std::process::id() as u32)
//!     .expect("Failed to create workflow")
//!     .target_vma_types(VmaFilter::ANONYMOUS | VmaFilter::WRITABLE)
//!     .with_idle_threshold(0.8)  // Only swap if 80%+ idle
//!     .execute()
//!     .expect("Failed to execute workflow");
//!
//! println!("Swapped {} bytes in {} pages",
//!     report.bytes_swapped, report.pages_swapped);
//! ```

use std::time::{Duration, Instant};

use crate::error::{EtmemError, Result};
use crate::session::{EtmemSession, SessionConfig};
use crate::types::{IdlePageInfo, ScanConfig};
use crate::vma::{VmaFilter, VmaRegion};

/// Criteria for selecting pages to swap
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwapCriteria {
    /// Minimum idle ratio required before swapping (0.0 - 1.0)
    pub min_idle_ratio: f64,
    /// Minimum idle duration before swapping (not yet implemented)
    pub min_idle_duration: Option<Duration>,
    /// Minimum page size to consider (0 = any size)
    pub min_page_size: u64,
    /// Only swap huge pages
    pub huge_pages_only: bool,
}

impl SwapCriteria {
    /// Create default swap criteria
    pub const fn new() -> Self {
        Self {
            min_idle_ratio: 0.0,
            min_idle_duration: None,
            min_page_size: 0,
            huge_pages_only: false,
        }
    }

    /// Set minimum idle ratio threshold
    pub const fn with_min_idle_ratio(mut self, ratio: f64) -> Self {
        self.min_idle_ratio = ratio;
        self
    }

    /// Set minimum idle duration
    pub const fn with_min_idle_duration(mut self, duration: Duration) -> Self {
        self.min_idle_duration = Some(duration);
        self
    }

    /// Set minimum page size
    pub const fn with_min_page_size(mut self, size: u64) -> Self {
        self.min_page_size = size;
        self
    }

    /// Only swap huge pages
    pub const fn with_huge_pages_only(mut self, enable: bool) -> Self {
        self.huge_pages_only = enable;
        self
    }

    /// Check if a page meets the swap criteria
    pub fn matches(&self, page: &IdlePageInfo) -> bool {
        // Check huge page requirement
        if self.huge_pages_only && !page.page_type.is_huge() {
            return false;
        }

        // Check minimum page size
        if self.min_page_size > 0 && page.total_size() < self.min_page_size {
            return false;
        }

        // Check if page is idle (basic criteria)
        if !page.is_idle() {
            return false;
        }

        true
    }

    /// Check if VMA-level statistics meet the criteria
    pub fn vma_meets_threshold(&self, idle_ratio: f64) -> bool {
        idle_ratio >= self.min_idle_ratio
    }
}

impl Default for SwapCriteria {
    fn default() -> Self {
        Self::new()
    }
}

/// Report from a scan-and-swap workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowReport {
    /// Number of VMAs scanned
    pub vmas_scanned: usize,
    /// Number of VMAs that met the swap criteria
    pub vmas_swapped: usize,
    /// Total pages scanned
    pub pages_scanned: usize,
    /// Total pages swapped
    pub pages_swapped: usize,
    /// Total bytes swapped
    pub bytes_swapped: u64,
    /// Duration of the workflow execution
    pub duration: Duration,
    /// Idle ratio across all scanned memory
    pub overall_idle_ratio: f64,
    /// Per-VMA results
    pub vma_results: Vec<VmaWorkflowResult>,
}

impl WorkflowReport {
    /// Create an empty report
    pub fn new() -> Self {
        Self {
            vmas_scanned: 0,
            vmas_swapped: 0,
            pages_scanned: 0,
            pages_swapped: 0,
            bytes_swapped: 0,
            duration: Duration::ZERO,
            overall_idle_ratio: 0.0,
            vma_results: Vec::new(),
        }
    }

    /// Check if the workflow was effective (swapped any pages)
    pub fn was_effective(&self) -> bool {
        self.pages_swapped > 0
    }

    /// Calculate swap efficiency
    pub fn efficiency(&self) -> f64 {
        if self.pages_scanned == 0 {
            0.0
        } else {
            self.pages_swapped as f64 / self.pages_scanned as f64
        }
    }

    /// Calculate throughput (bytes swapped per second)
    pub fn throughput_bytes_per_sec(&self) -> f64 {
        if self.duration.as_secs_f64() == 0.0 {
            0.0
        } else {
            self.bytes_swapped as f64 / self.duration.as_secs_f64()
        }
    }
}

impl Default for WorkflowReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-VMA workflow result
#[derive(Debug, Clone)]
pub struct VmaWorkflowResult {
    /// VMA address range
    pub range: crate::types::AddressRange,
    /// VMA name/path
    pub name: String,
    /// Pages found in this VMA
    pub pages_found: usize,
    /// Pages swapped from this VMA
    pub pages_swapped: usize,
    /// Bytes swapped from this VMA
    pub bytes_swapped: u64,
    /// Idle ratio for this VMA
    pub idle_ratio: f64,
    /// Whether this VMA met the swap criteria
    pub met_criteria: bool,
}

/// Builder for "scan then swap" workflows
///
/// This provides a declarative API for configuring and executing
/// scan-and-swap operations with various filtering and threshold options.
#[derive(Debug)]
pub struct ScanAndSwapWorkflow {
    session: EtmemSession,
    scan_config: ScanConfig,
    criteria: SwapCriteria,
    vma_filter: VmaFilter,
    dry_run: bool,
}

impl ScanAndSwapWorkflow {
    /// Create a new scan-and-swap workflow for a process
    ///
    /// # Errors
    /// Returns error if:
    /// - Process doesn't exist
    /// - Permission denied
    /// - ETMEM module not loaded
    pub fn new(pid: u32) -> Result<Self> {
        let session = EtmemSession::new(pid, SessionConfig::default())?;

        Ok(Self {
            session,
            scan_config: ScanConfig::default(),
            criteria: SwapCriteria::default(),
            vma_filter: VmaFilter::SCANNABLE,
            dry_run: false,
        })
    }

    /// Create a workflow from an existing session
    ///
    /// This allows reusing an existing session with its discovered VMAs.
    pub fn from_session(session: EtmemSession) -> Self {
        Self {
            session,
            scan_config: ScanConfig::default(),
            criteria: SwapCriteria::default(),
            vma_filter: VmaFilter::SCANNABLE,
            dry_run: false,
        }
    }

    /// Set the scan configuration
    pub fn with_scan_config(mut self, config: ScanConfig) -> Self {
        self.scan_config = config;
        self
    }

    /// Only swap pages that have been idle for at least this duration
    ///
    /// Note: This is currently a placeholder for future implementation
    /// that would track page idle time across multiple scans.
    pub fn with_min_idle_duration(mut self, duration: Duration) -> Self {
        self.criteria.min_idle_duration = Some(duration);
        self
    }

    /// Only swap pages in VMAs matching the filter
    pub fn target_vma_types(mut self, filter: VmaFilter) -> Self {
        self.vma_filter = filter;
        self
    }

    /// Set minimum idle ratio before swapping (0.0 - 1.0)
    ///
    /// Only VMAs with at least this ratio of idle pages will be swapped.
    pub fn with_idle_threshold(mut self, ratio: f64) -> Self {
        self.criteria.min_idle_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Only swap huge pages
    pub fn huge_pages_only(mut self) -> Self {
        self.criteria.huge_pages_only = true;
        self
    }

    /// Enable dry-run mode
    ///
    /// In dry-run mode, the workflow will scan and analyze but not
    /// actually swap any pages. Useful for testing and analysis.
    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    /// Get the underlying session (for advanced use)
    pub fn session(&self) -> &EtmemSession {
        &self.session
    }

    /// Get mutable access to the underlying session
    pub fn session_mut(&mut self) -> &mut EtmemSession {
        &mut self.session
    }

    /// Execute the scan-and-swap workflow
    ///
    /// This performs the following steps:
    /// 1. Discover VMAs (if not already done)
    /// 2. Scan each VMA matching the filter
    /// 3. Apply swap criteria to select pages
    /// 4. Swap the selected pages (unless dry_run)
    ///
    /// Returns a detailed report of the operation.
    pub fn execute(mut self) -> Result<WorkflowReport> {
        let start_time = Instant::now();
        let mut report = WorkflowReport::new();

        // Ensure VMAs are discovered
        if self.session.vma_map().is_none() {
            self.session.discover_vmas()?;
        }

        let vma_map = self
            .session
            .vma_map()
            .ok_or_else(|| EtmemError::InvalidVma("VMA map not available".to_string()))?
            .clone();

        // Filter VMAs based on criteria
        let target_vmas: Vec<VmaRegion> = vma_map
            .filter(self.vma_filter)
            .into_iter()
            .cloned()
            .collect();

        if target_vmas.is_empty() {
            return Ok(report);
        }

        // Process each VMA
        let mut total_idle_bytes = 0u64;
        let mut total_scanned_bytes = 0u64;

        for vma in target_vmas {
            let result = self.process_vma(&vma)?;

            if result.pages_found > 0 {
                report.vmas_scanned += 1;
                if result.met_criteria && result.pages_swapped > 0 {
                    report.vmas_swapped += 1;
                }
                report.pages_scanned += result.pages_found;
                report.pages_swapped += result.pages_swapped;
                report.bytes_swapped += result.bytes_swapped;
                total_idle_bytes += (result.idle_ratio * (vma.end - vma.start) as f64) as u64;
                total_scanned_bytes += vma.end - vma.start;
            }

            report.vma_results.push(result);
        }

        // Calculate overall statistics
        report.duration = start_time.elapsed();
        if total_scanned_bytes > 0 {
            report.overall_idle_ratio = total_idle_bytes as f64 / total_scanned_bytes as f64;
        }

        Ok(report)
    }

    /// Process a single VMA
    fn process_vma(&mut self, vma: &VmaRegion) -> Result<VmaWorkflowResult> {
        let mut result = VmaWorkflowResult {
            range: vma.to_address_range(),
            name: vma.name().to_string(),
            pages_found: 0,
            pages_swapped: 0,
            bytes_swapped: 0,
            idle_ratio: 0.0,
            met_criteria: false,
        };

        // Scan the VMA
        let pages = match self.session.scan_vma(vma, self.scan_config.clone()) {
            Ok(p) => p,
            Err(_) => return Ok(result), // Skip VMAs that can't be scanned
        };

        if pages.is_empty() {
            return Ok(result);
        }

        result.pages_found = pages.len();

        // Calculate idle ratio for this VMA
        let idle_bytes: u64 = pages
            .iter()
            .filter(|p| p.is_idle())
            .map(|p| p.total_size())
            .sum();
        let total_bytes: u64 = pages.iter().map(|p| p.total_size()).sum();

        if total_bytes > 0 {
            result.idle_ratio = idle_bytes as f64 / total_bytes as f64;
        }

        // Check if VMA meets the threshold criteria
        result.met_criteria = self.criteria.vma_meets_threshold(result.idle_ratio);

        if !result.met_criteria {
            return Ok(result);
        }

        // Select pages matching criteria
        let swap_addrs: Vec<u64> = pages
            .iter()
            .filter(|p| self.criteria.matches(p))
            .map(|p| p.address)
            .collect();

        if swap_addrs.is_empty() {
            return Ok(result);
        }

        result.pages_swapped = swap_addrs.len();
        result.bytes_swapped = swap_addrs.len() as u64 * 4096; // Approximate

        // Perform the swap (unless dry-run)
        if !self.dry_run {
            match self.session.swap_in_vma(vma, &swap_addrs) {
                Ok(actual_swapped) => {
                    result.pages_swapped = actual_swapped;
                    result.bytes_swapped = actual_swapped as u64 * 4096;
                }
                Err(_) => {
                    result.pages_swapped = 0;
                    result.bytes_swapped = 0;
                }
            }
        }

        Ok(result)
    }
}

/// Convenience function for quick scan-and-swap
///
/// Performs a simple scan-and-swap operation on a process with default settings.
///
/// # Example
/// ```no_run
/// use etmem_rs::workflow::quick_scan_and_swap;
///
/// let report = quick_scan_and_swap(std::process::id() as u32)
///     .expect("Failed to scan and swap");
///
/// println!("Swapped {} pages", report.pages_swapped);
/// ```
pub fn quick_scan_and_swap(pid: u32) -> Result<WorkflowReport> {
    ScanAndSwapWorkflow::new(pid)?.execute()
}

/// Convenience function for scanning only (no swap)
///
/// Scans a process and returns statistics without swapping any pages.
///
/// # Example
/// ```no_run
/// use etmem_rs::workflow::analyze_memory;
///
/// let report = analyze_memory(std::process::id() as u32)
///     .expect("Failed to analyze memory");
///
/// println!("Overall idle ratio: {:.2}%", report.overall_idle_ratio * 100.0);
/// ```
pub fn analyze_memory(pid: u32) -> Result<WorkflowReport> {
    ScanAndSwapWorkflow::new(pid)?.dry_run().execute()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_criteria_default() {
        let criteria = SwapCriteria::default();
        assert_eq!(criteria.min_idle_ratio, 0.0);
        assert!(!criteria.huge_pages_only);
    }

    #[test]
    fn test_swap_criteria_builder() {
        let criteria = SwapCriteria::new()
            .with_min_idle_ratio(0.8)
            .with_huge_pages_only(true)
            .with_min_page_size(2 * 1024 * 1024);

        assert_eq!(criteria.min_idle_ratio, 0.8);
        assert!(criteria.huge_pages_only);
        assert_eq!(criteria.min_page_size, 2 * 1024 * 1024);
    }

    #[test]
    fn test_swap_criteria_matches() {
        use crate::types::{IdlePageInfo, ProcIdlePageType};

        let criteria = SwapCriteria::new().with_huge_pages_only(true);

        let huge_page = IdlePageInfo::new(0x1000, ProcIdlePageType::PmdIdle, 1);
        let regular_page = IdlePageInfo::new(0x1000, ProcIdlePageType::PteIdle, 1);
        let accessed_page = IdlePageInfo::new(0x1000, ProcIdlePageType::PteAccessed, 1);

        assert!(criteria.matches(&huge_page));
        assert!(!criteria.matches(&regular_page));
        assert!(!criteria.matches(&accessed_page));
    }

    #[test]
    fn test_swap_criteria_threshold() {
        let criteria = SwapCriteria::new().with_min_idle_ratio(0.5);

        assert!(criteria.vma_meets_threshold(0.6));
        assert!(criteria.vma_meets_threshold(0.5));
        assert!(!criteria.vma_meets_threshold(0.4));
    }

    #[test]
    fn test_workflow_report() {
        let report = WorkflowReport {
            pages_scanned: 100,
            pages_swapped: 30,
            ..Default::default()
        };

        assert!(report.was_effective());
        assert!((report.efficiency() - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_scan_and_swap_workflow_builder() {
        // This test just verifies the builder compiles correctly
        // We can't actually run it without a real process with ETMEM

        // Test that the builder pattern works
        fn _test_builder_compiles(pid: u32) -> Result<ScanAndSwapWorkflow> {
            Ok(ScanAndSwapWorkflow::new(pid)?
                .target_vma_types(VmaFilter::ANONYMOUS)
                .with_idle_threshold(0.8)
                .huge_pages_only()
                .dry_run())
        }
    }
}
