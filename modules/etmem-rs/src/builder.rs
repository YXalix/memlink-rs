//! Fluent builder patterns for ETMEM operations
//!
//! This module provides ergonomic builder APIs for common ETMEM operations.
//! These builders offer a more intuitive, chainable interface compared to
//! the lower-level session APIs.
//!
//! # Example
//!
//! ```no_run
//! use etmem_rs::builder::ScanBuilder;
//! use etmem_rs::vma::VmaFilter;
//!
//! // Scan heap only
//! let pages = ScanBuilder::for_process(std::process::id() as u32)
//!     .expect("Failed to create builder")
//!     .for_heap()
//!     .idle_only()
//!     .scan()
//!     .expect("Failed to scan");
//!
//! println!("Found {} idle pages in heap", pages.len());
//! ```

use crate::error::Result;
use crate::scan::ScanSession;
use crate::swap::SwapSession;
use crate::types::{AddressRange, IdlePageInfo, ScanConfig, ScanFlags, SwapConfig};
use crate::vma::{VmaFilter, VmaMap, VmaRegion};

/// Target selection for scan operations
#[derive(Debug, Clone)]
pub enum ScanTarget {
    /// Scan all scannable memory
    AllMemory,
    /// Scan a specific address range
    SpecificRange(AddressRange),
    /// Scan a specific VMA
    SpecificVma(VmaRegion),
    /// Scan VMAs matching filter criteria
    VmaFilter(VmaFilter),
    /// Scan only the heap
    Heap,
    /// Scan only the stack
    Stack,
    /// Scan only anonymous mappings
    Anonymous,
}

/// Fluent builder for scan operations
///
/// Provides a chainable API for configuring and executing scan operations.
#[derive(Debug)]
pub struct ScanBuilder {
    pid: u32,
    config: ScanConfig,
    target: ScanTarget,
    idle_only: bool,
    accessed_only: bool,
    huge_only: bool,
}

impl ScanBuilder {
    /// Create a new scan builder for a process
    ///
    /// # Errors
    /// Returns error if:
    /// - Process doesn't exist
    /// - Permission denied
    /// - ETMEM module not loaded
    pub fn for_process(pid: u32) -> Result<Self> {
        Ok(Self {
            pid,
            config: ScanConfig::default(),
            target: ScanTarget::AllMemory,
            idle_only: false,
            accessed_only: false,
            huge_only: false,
        })
    }

    /// Set scan flags
    pub fn with_flags(mut self, flags: ScanFlags) -> Self {
        self.config.flags = flags;
        self
    }

    /// Set buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set walk step
    pub fn with_walk_step(mut self, step: u32) -> Self {
        self.config.walk_step = step;
        self
    }

    /// Target a specific address range
    pub fn for_range(mut self, range: AddressRange) -> Self {
        self.target = ScanTarget::SpecificRange(range);
        self
    }

    /// Target a specific VMA
    pub fn for_vma(mut self, vma: VmaRegion) -> Self {
        self.target = ScanTarget::SpecificVma(vma);
        self
    }

    /// Target only the heap
    pub fn for_heap(mut self) -> Self {
        self.target = ScanTarget::Heap;
        self
    }

    /// Target only the stack
    pub fn for_stack(mut self) -> Self {
        self.target = ScanTarget::Stack;
        self
    }

    /// Target only anonymous mappings
    pub fn for_anonymous(mut self) -> Self {
        self.target = ScanTarget::Anonymous;
        self
    }

    /// Target VMAs matching a filter
    pub fn for_vma_filter(mut self, filter: VmaFilter) -> Self {
        self.target = ScanTarget::VmaFilter(filter);
        self
    }

    /// Only return idle pages
    pub fn idle_only(mut self) -> Self {
        self.idle_only = true;
        self.accessed_only = false;
        self
    }

    /// Only return accessed (hot) pages
    pub fn accessed_only(mut self) -> Self {
        self.accessed_only = true;
        self.idle_only = false;
        self
    }

    /// Only scan huge pages
    pub fn huge_pages_only(mut self) -> Self {
        self.huge_only = true;
        self.config.flags |= ScanFlags::SCAN_HUGE_PAGE;
        self
    }

    /// Execute the scan and return results
    ///
    /// This performs the scan based on the configured target and filters.
    pub fn scan(self) -> Result<Vec<IdlePageInfo>> {
        let mut session = ScanSession::new(self.pid, self.config)?;

        let pages = match &self.target {
            ScanTarget::AllMemory => {
                // Scan all memory starting from 0
                let mut all_pages = Vec::new();
                let mut current_addr: u64 = 0;

                loop {
                    let (pages, next) = session.read(current_addr)?;
                    all_pages.extend(pages);

                    match next {
                        Some(addr) => current_addr = addr,
                        None => break,
                    }
                }

                all_pages
            }
            ScanTarget::SpecificRange(range) => session.read_range(*range)?,
            ScanTarget::SpecificVma(vma) => session.read_range(vma.to_address_range())?,
            ScanTarget::VmaFilter(filter) => {
                // Discover VMAs and scan matching ones
                let vma_map = VmaMap::for_process(self.pid)?;
                let mut all_pages = Vec::new();

                for vma in vma_map.filter(*filter) {
                    let pages = session.read_range(vma.to_address_range())?;
                    all_pages.extend(pages);
                }

                all_pages
            }
            ScanTarget::Heap | ScanTarget::Stack | ScanTarget::Anonymous => {
                // Discover VMAs and scan specific type
                let vma_map = VmaMap::for_process(self.pid)?;
                let filter = match &self.target {
                    ScanTarget::Heap => VmaFilter::HEAP,
                    ScanTarget::Stack => VmaFilter::STACK,
                    ScanTarget::Anonymous => VmaFilter::ANONYMOUS,
                    _ => unreachable!(),
                };

                let mut all_pages = Vec::new();
                for vma in vma_map.filter(filter) {
                    let pages = session.read_range(vma.to_address_range())?;
                    all_pages.extend(pages);
                }

                all_pages
            }
        };

        // Apply post-scan filters
        let filtered: Vec<IdlePageInfo> = pages
            .into_iter()
            .filter(|p| {
                if self.idle_only && !p.is_idle() {
                    return false;
                }
                if self.accessed_only && !p.is_accessed() {
                    return false;
                }
                if self.huge_only && !p.page_type.is_huge() {
                    return false;
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    /// Execute the scan and return only idle pages
    ///
    /// Convenience method equivalent to `.idle_only().scan()`
    pub fn scan_idle(self) -> Result<Vec<IdlePageInfo>> {
        self.idle_only().scan()
    }

    /// Execute the scan and return only accessed pages
    ///
    /// Convenience method equivalent to `.accessed_only().scan()`
    pub fn scan_accessed(self) -> Result<Vec<IdlePageInfo>> {
        self.accessed_only().scan()
    }
}

/// Fluent builder for swap operations
#[derive(Debug)]
pub struct SwapBuilder {
    pid: u32,
    config: SwapConfig,
    addresses: Vec<u64>,
}

impl SwapBuilder {
    /// Create a new swap builder for a process
    pub fn for_process(pid: u32) -> Result<Self> {
        Ok(Self {
            pid,
            config: SwapConfig::default(),
            addresses: Vec::new(),
        })
    }

    /// Add a single address to swap
    pub fn add_address(mut self, addr: u64) -> Self {
        self.addresses.push(addr);
        self
    }

    /// Add multiple addresses to swap
    pub fn add_addresses(mut self, addrs: &[u64]) -> Self {
        self.addresses.extend_from_slice(addrs);
        self
    }

    /// Set swap configuration
    pub fn with_config(mut self, config: SwapConfig) -> Self {
        self.config = config;
        self
    }

    /// Execute the swap operation
    ///
    /// Returns the number of pages swapped.
    pub fn swap(self) -> Result<usize> {
        if self.addresses.is_empty() {
            return Ok(0);
        }

        let mut session = SwapSession::new(self.pid, self.config)?;
        session.add_addresses(&self.addresses)?;
        session.flush()
    }
}

/// Quick scan function - one-liner for simple scans
///
/// # Example
/// ```no_run
/// use etmem_rs::builder::quick_scan;
///
/// let pages = quick_scan(std::process::id() as u32)
///     .expect("Failed to scan");
/// println!("Found {} pages", pages.len());
/// ```
pub fn quick_scan(pid: u32) -> Result<Vec<IdlePageInfo>> {
    ScanBuilder::for_process(pid)?.scan()
}

/// Quick scan for idle pages only
///
/// # Example
/// ```no_run
/// use etmem_rs::builder::quick_scan_idle;
///
/// let pages = quick_scan_idle(std::process::id() as u32)
///     .expect("Failed to scan");
/// println!("Found {} idle pages", pages.len());
/// ```
pub fn quick_scan_idle(pid: u32) -> Result<Vec<IdlePageInfo>> {
    ScanBuilder::for_process(pid)?.idle_only().scan()
}

/// Quick scan for idle pages in the heap
///
/// # Example
/// ```no_run
/// use etmem_rs::builder::quick_scan_heap;
///
/// let pages = quick_scan_heap(std::process::id() as u32)
///     .expect("Failed to scan heap");
/// println!("Found {} idle pages in heap", pages.len());
/// ```
pub fn quick_scan_heap(pid: u32) -> Result<Vec<IdlePageInfo>> {
    ScanBuilder::for_process(pid)?.for_heap().idle_only().scan()
}

/// Quick swap function - one-liner for swapping specific addresses
///
/// # Example
/// ```no_run
/// use etmem_rs::builder::quick_swap;
///
/// let swapped = quick_swap(std::process::id() as u32, &[0x7fff0000, 0x7fff1000])
///     .expect("Failed to swap");
/// println!("Swapped {} pages", swapped);
/// ```
pub fn quick_swap(pid: u32, addrs: &[u64]) -> Result<usize> {
    SwapBuilder::for_process(pid)?.add_addresses(addrs).swap()
}

/// Combined scan-and-swap using the builder pattern
///
/// Scans for idle pages and swaps them in one operation.
///
/// # Example
/// ```no_run
/// use etmem_rs::builder::scan_and_swap;
///
/// let swapped = scan_and_swap(std::process::id() as u32)
///     .expect("Failed to scan and swap");
/// println!("Swapped {} pages", swapped);
/// ```
pub fn scan_and_swap(pid: u32) -> Result<usize> {
    let pages = quick_scan_idle(pid)?;
    let addrs: Vec<u64> = pages.iter().map(|p| p.address).collect();
    quick_swap(pid, &addrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_builder_default() {
        let builder = ScanBuilder::for_process(1234).unwrap();
        assert_eq!(builder.pid, 1234);
        assert!(!builder.idle_only);
        assert!(!builder.huge_only);
    }

    #[test]
    fn test_scan_builder_chain() {
        let builder = ScanBuilder::for_process(1234)
            .unwrap()
            .for_heap()
            .idle_only()
            .huge_pages_only()
            .with_buffer_size(4096);

        assert!(builder.idle_only);
        assert!(builder.huge_only);
        assert_eq!(builder.config.buffer_size, 4096);
    }

    #[test]
    fn test_swap_builder_default() {
        let builder = SwapBuilder::for_process(1234).unwrap();
        assert!(builder.addresses.is_empty());
    }

    #[test]
    fn test_swap_builder_chain() {
        let builder = SwapBuilder::for_process(1234)
            .unwrap()
            .add_address(0x1000)
            .add_addresses(&[0x2000, 0x3000]);

        assert_eq!(builder.addresses.len(), 3);
        assert_eq!(builder.addresses[0], 0x1000);
    }

    #[test]
    fn test_scan_target_variants() {
        let range = AddressRange::new(0x1000, 0x5000);

        let _ = ScanBuilder::for_process(1234)
            .unwrap()
            .for_range(range)
            .for_heap()
            .for_stack()
            .for_anonymous()
            .for_vma_filter(VmaFilter::SCANNABLE);
    }
}
