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
//! # Example: Scanning for Idle Pages
//!
//! ```no_run
//! use etmem_rs::{IdlePageScanner, ScanConfig, ScanFlags};
//!
//! // Configure scan to only report huge pages
//! let config = ScanConfig::default()
//!     .with_flags(ScanFlags::SCAN_HUGE_PAGE);
//!
//! // Scan current process for idle pages
//! let pages = IdlePageScanner::scan_process(std::process::id() as u32, config)
//!     .expect("Failed to scan process");
//!
//! for page in pages {
//!     if page.is_idle() {
//!         println!("Idle page at {:x} (size: {} bytes)",
//!             page.address, page.total_size());
//!     }
//! }
//! ```
//!
//! # Example: Swapping Cold Pages
//!
//! ```no_run
//! use etmem_rs::{SwapSession, SwapConfig};
//!
//! let mut session = SwapSession::new(
//!     std::process::id() as u32,
//!     SwapConfig::default()
//! ).expect("Failed to create swap session");
//!
//! // Swap a specific address
//! session.swap_address(0x7fff0000)
//!     .expect("Failed to swap page");
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
pub mod error;
pub mod scan;
pub mod swap;
pub mod sys;
pub mod types;
pub mod util;

// Public API exports
pub use error::{EtmemError, Result, ToEtmemResult};
pub use scan::{IdlePageScanner, PageIdleCtrl, ScanSession};
pub use swap::{PageSwapper, SwapSession, SwapcacheConfig};
pub use types::{
    AddressRange, BufferStatus, IdlePageInfo, PipEncoding, ProcIdlePageType, ScanConfig,
    ScanFlags, SwapConfig, SwapcacheWatermark, WatermarkConfig, IDLE_SCAN_MAGIC, INVALID_PAGE,
    PAGE_IDLE_BUF_MIN, PAGE_IDLE_KBUF_SIZE, RECLAIM_SWAPCACHE_MAGIC, RET_RESCAN_FLAG,
    SWAP_SCAN_NUM_MAX, WATERMARK_MAX,
};
// PageIdleCtrl is re-exported from scan module above
pub use util::{
    bytes_to_pages, filter_accessed_pages, filter_huge_pages, filter_idle_pages, format_bytes,
    group_by_type, huge_page_align_down, is_etmem_available, is_huge_page_aligned, is_page_aligned,
    is_root, page_align_down, page_align_up, pages_to_bytes, suggest_page_size, IdlePageStats,
};

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
