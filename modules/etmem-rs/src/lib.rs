//! ETMEM Rust Bindings
//!
//! This crate provides Rust bindings and safe wrappers for the Linux kernel's
//! ETMEM (Enhanced Tiered Memory) subsystem. ETMEM enables proactive page
//! scanning and swapping for tiered memory management.
//!
//! # Features
//!
//! - **Page Scanning**: Detect idle (cold) and accessed (hot) pages via
//!   hardware page table access/dirty bits
//! - **Page Swapping**: Reclaim cold pages to swap to free up DRAM
//! - **VMA Awareness**: Full support for Virtual Memory Area discovery and
//!   per-VMA operations
//! - **VM Support**: Scan and swap VM guest memory via EPT/stage-2 page tables
//! - **Proactive Reclaim**: Configure kernel background thread for automatic
//!   swapcache reclaim based on watermarks
//!
//! # Architecture
//!
//! The crate is organized into layers:
//!
//! - **`sys`**: Low-level FFI bindings to kernel procfs/IOCTL (unsafe)
//! - **`types`**: Data structures and constants
//! - **`error`**: Error types and handling
//! - **`vma`**: Virtual Memory Area discovery and management
//! - **`session`**: Unified `EtmemSession` for combined operations
//! - **`builder`**: Fluent builder APIs for ergonomic operations
//! - **`workflow`**: High-level workflow builders for complex operations
//! - **`scan`**: Safe wrappers for page scanning operations
//! - **`swap`**: Safe wrappers for page swapping operations
//! - **`util`**: Utility functions and helpers
//!
//! # Requirements
//!
//! - Linux kernel with ETMEM support (CONFIG_ETMEM=y)
//! - CAP_SYS_ADMIN capability (root access)
//! - Kernel modules: etmem_scan.ko, etmem_swap.ko (if built as modules)
//!
//! # Quick Start
//!
//! ## Using the Unified Session API (Recommended)
//!
//! ```no_run
//! use etmem_rs::{EtmemSession, SessionConfig, ScanConfig};
//!
//! // Create a session and discover VMAs
//! let mut session = EtmemSession::new(
//!     std::process::id() as u32,
//!     SessionConfig::default()
//! ).expect("Failed to create session");
//!
//! // Scan all scannable VMAs and swap idle pages
//! let report = session.scan_and_swap_all(ScanConfig::default())
//!     .expect("Failed to scan and swap");
//!
//! println!("Swapped {} pages ({} bytes)",
//!     report.pages_swapped, report.bytes_swapped);
//! ```
//!
//! ## Using the Builder Pattern
//!
//! ```no_run
//! use etmem_rs::builder::ScanBuilder;
//!
//! // Scan heap for idle pages
//! let pages = ScanBuilder::for_process(std::process::id() as u32)
//!     .expect("Failed to create builder")
//!     .for_heap()
//!     .idle_only()
//!     .scan()
//!     .expect("Failed to scan");
//!
//! println!("Found {} idle pages in heap", pages.len());
//! ```
//!
//! ## Using the Workflow API
//!
//! ```no_run
//! use etmem_rs::workflow::ScanAndSwapWorkflow;
//! use etmem_rs::vma::VmaFilter;
//!
//! // Declarative workflow with filtering
//! let report = ScanAndSwapWorkflow::new(std::process::id() as u32)
//!     .expect("Failed to create workflow")
//!     .target_vma_types(VmaFilter::ANONYMOUS | VmaFilter::WRITABLE)
//!     .with_idle_threshold(0.8)
//!     .execute()
//!     .expect("Failed to execute workflow");
//!
//! println!("Scanned {} VMAs, swapped {} pages",
//!     report.vmas_scanned, report.pages_swapped);
//! ```
//!
//! ## Quick One-Liners
//!
//! ```no_run
//! use etmem_rs::builder::{quick_scan_heap, quick_swap};
//!
//! // Quick scan for idle heap pages
//! let pages = quick_scan_heap(std::process::id() as u32)
//!     .expect("Failed to scan");
//!
//! // Quick swap specific addresses
//! let swapped = quick_swap(std::process::id() as u32, &[0x7fff0000])
//!     .expect("Failed to swap");
//! ```
//!
//! # VMA Awareness
//!
//! The crate provides comprehensive VMA (Virtual Memory Area) support:
//!
//! ```no_run
//! use etmem_rs::vma::VmaMap;
//!
//! // Discover all VMAs for a process
//! let vma_map = VmaMap::for_process(std::process::id() as u32)
//!     .expect("Failed to parse VMAs");
//!
//! // Access specific regions
//! if let Some(heap) = vma_map.heap() {
//!     println!("Heap: {} bytes", heap.size());
//! }
//!
//! // Filter by criteria
//! let scannable = vma_map.scannable();
//! let swappable = vma_map.swappable();
//! ```
//!
//! # Safety
//!
//! This crate uses `unsafe` blocks only in the `sys` module for FFI calls.
//! All public APIs are safe Rust. However, improper use (e.g., swapping
//! wrong pages) can still cause application crashes.

#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]

// Re-export modules
pub mod builder;
pub mod error;
pub mod scan;
pub mod session;
pub mod swap;
pub mod sys;
pub mod types;
pub mod util;
pub mod vma;
pub mod workflow;

// Public API exports
pub use error::{EtmemError, Result, ToEtmemResult};
pub use scan::{IdlePageScanner, PageIdleCtrl, ScanSession};
pub use session::{EtmemSession, ScanAndSwapReport, SessionConfig, VmaScanResults};
pub use swap::{PageSwapper, SwapSession, SwapcacheConfig};
pub use types::{
    AddressRange, BufferStatus, IDLE_SCAN_MAGIC, INVALID_PAGE, IdlePageInfo, PAGE_IDLE_BUF_MIN,
    PAGE_IDLE_KBUF_SIZE, PipEncoding, ProcIdlePageType, RECLAIM_SWAPCACHE_MAGIC, RET_RESCAN_FLAG,
    SWAP_SCAN_NUM_MAX, ScanConfig, ScanFlags, SwapConfig, SwapcacheWatermark, WATERMARK_MAX,
    WatermarkConfig,
};
pub use vma::{PathnameType, VmaFilter, VmaMap, VmaPermissions, VmaRegion};
// PageIdleCtrl is re-exported from scan module above
pub use util::{
    IdlePageStats, bytes_to_pages, filter_accessed_pages, filter_huge_pages, filter_idle_pages,
    format_bytes, group_by_type, huge_page_align_down, is_etmem_available, is_huge_page_aligned,
    is_page_aligned, is_root, page_align_down, page_align_up, pages_to_bytes, suggest_page_size,
};

/// Convenience prelude module for common imports
///
/// # Example
/// ```
/// use etmem_rs::prelude::*;
/// ```
pub mod prelude {
    pub use crate::builder::{
        ScanBuilder, SwapBuilder, quick_scan_heap, quick_scan_idle, quick_swap,
    };
    pub use crate::error::{EtmemError, Result};
    pub use crate::session::{EtmemSession, SessionConfig};
    pub use crate::types::{AddressRange, IdlePageInfo, ScanConfig, SwapConfig};
    pub use crate::vma::{VmaFilter, VmaMap, VmaRegion};
    pub use crate::workflow::ScanAndSwapWorkflow;
}

// Version information
/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if the ETMEM subsystem is available on this system
///
/// This checks if the required kernel interfaces are present.
///
/// # Example
/// ```
/// use etmem_rs;
///
/// if etmem_rs::is_available() {
///     println!("ETMEM is available");
/// } else {
///     println!("ETMEM is not available - check kernel config");
/// }
/// ```
pub fn is_available() -> bool {
    util::is_etmem_available()
}

/// Check if the current process has required permissions
///
/// ETMEM operations require CAP_SYS_ADMIN (root) capability.
///
/// # Example
/// ```
/// use etmem_rs;
///
/// if !etmem_rs::has_permission() {
///     eprintln!("ETMEM requires root privileges");
/// }
/// ```
pub fn has_permission() -> bool {
    util::is_root()
}

/// Initialize the ETMEM subsystem
///
/// This checks for availability and permissions, returning an error
/// if ETMEM cannot be used.
///
/// # Errors
/// Returns error if:
/// - ETMEM is not available (kernel not configured)
/// - Permission denied (not root)
pub fn init() -> Result<()> {
    if !is_available() {
        return Err(EtmemError::ModuleNotLoaded);
    }
    if !has_permission() {
        return Err(EtmemError::PermissionDenied);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available() {
        // This will be false in test environment without ETMEM
        let available = is_available();
        println!("ETMEM available: {}", available);
    }

    #[test]
    fn test_re_exports() {
        // Verify all re-exports compile correctly
        let _: AddressRange = AddressRange::default();
        let _: ScanFlags = ScanFlags::empty();
        let _: ProcIdlePageType = ProcIdlePageType::PteIdle;
    }
}
