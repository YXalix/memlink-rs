//! Utility functions and helpers for ETMEM operations
//!
//! This module provides common utilities used across the ETMEM crate,
//! including address manipulation, page size calculations, and
//! statistics helpers.

use crate::types::{IdlePageInfo, ProcIdlePageType};

/// Check if an address is page-aligned (4KB)
#[inline]
pub const fn is_page_aligned(addr: u64) -> bool {
    addr % 4096 == 0
}

/// Check if an address is huge page aligned (2MB)
#[inline]
pub const fn is_huge_page_aligned(addr: u64) -> bool {
    addr % (2 * 1024 * 1024) == 0
}

/// Align an address down to page boundary (4KB)
#[inline]
pub const fn page_align_down(addr: u64) -> u64 {
    addr & !(4096 - 1)
}

/// Align an address up to page boundary (4KB)
#[inline]
pub const fn page_align_up(addr: u64) -> u64 {
    ((addr + 4096 - 1) / 4096) * 4096
}

/// Align an address down to huge page boundary (2MB)
#[inline]
pub const fn huge_page_align_down(addr: u64) -> u64 {
    addr & !((2 * 1024 * 1024) - 1)
}

/// Get the page size for a given address range
///
/// Attempts to determine the optimal page size based on alignment.
pub fn suggest_page_size(start: u64, size: u64) -> u64 {
    if size >= 1024 * 1024 * 1024 && is_huge_page_aligned(start) {
        1024 * 1024 * 1024 // 1GB
    } else if size >= 2 * 1024 * 1024 && is_huge_page_aligned(start) {
        2 * 1024 * 1024 // 2MB
    } else {
        4096 // 4KB
    }
}

/// Calculate total memory size from a list of page infos
pub fn total_memory_size(pages: &[IdlePageInfo]) -> u64 {
    pages.iter().map(|p| p.total_size()).sum()
}

/// Calculate idle memory size from a list of page infos
pub fn idle_memory_size(pages: &[IdlePageInfo]) -> u64 {
    pages
        .iter()
        .filter(|p| p.is_idle())
        .map(|p| p.total_size())
        .sum()
}

/// Calculate accessed (hot) memory size from a list of page infos
pub fn accessed_memory_size(pages: &[IdlePageInfo]) -> u64 {
    pages
        .iter()
        .filter(|p| p.is_accessed())
        .map(|p| p.total_size())
        .sum()
}

/// Statistics for idle page analysis
#[derive(Debug, Clone, Copy, Default)]
pub struct IdlePageStats {
    /// Total number of pages
    pub total_pages: usize,
    /// Number of idle pages
    pub idle_pages: usize,
    /// Number of accessed pages
    pub accessed_pages: usize,
    /// Number of huge pages
    pub huge_pages: usize,
    /// Total memory size in bytes
    pub total_bytes: u64,
    /// Idle memory size in bytes
    pub idle_bytes: u64,
    /// Accessed memory size in bytes
    pub accessed_bytes: u64,
}

impl IdlePageStats {
    /// Calculate statistics from a list of page infos
    pub fn from_pages(pages: &[IdlePageInfo]) -> Self {
        let mut stats = Self::default();

        for page in pages {
            stats.total_pages += page.count as usize;
            stats.total_bytes += page.total_size();

            if page.is_idle() {
                stats.idle_pages += page.count as usize;
                stats.idle_bytes += page.total_size();
            } else if page.is_accessed() {
                stats.accessed_pages += page.count as usize;
                stats.accessed_bytes += page.total_size();
            }

            if page.page_type.is_huge() {
                stats.huge_pages += 1;
            }
        }

        stats
    }

    /// Calculate idle ratio (0.0 - 1.0)
    pub fn idle_ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.idle_bytes as f64 / self.total_bytes as f64
        }
    }

    /// Calculate accessed ratio (0.0 - 1.0)
    pub fn accessed_ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.accessed_bytes as f64 / self.total_bytes as f64
        }
    }

    /// Check if the workload has significant idle memory
    pub fn has_idle_memory(&self, threshold: f64) -> bool {
        self.idle_ratio() > threshold
    }
}

/// Group pages by their type
pub fn group_by_type(pages: &[IdlePageInfo]) -> std::collections::HashMap<ProcIdlePageType, Vec<IdlePageInfo>> {
    let mut groups: std::collections::HashMap<ProcIdlePageType, Vec<IdlePageInfo>> =
        std::collections::HashMap::new();

    for page in pages {
        groups
            .entry(page.page_type)
            .or_default()
            .push(*page);
    }

    groups
}

/// Filter pages to only include idle pages
pub fn filter_idle_pages(pages: &[IdlePageInfo]) -> Vec<IdlePageInfo> {
    pages.iter().filter(|p| p.is_idle()).copied().collect()
}

/// Filter pages to only include accessed pages
pub fn filter_accessed_pages(pages: &[IdlePageInfo]) -> Vec<IdlePageInfo> {
    pages.iter().filter(|p| p.is_accessed()).copied().collect()
}

/// Filter pages by size (huge pages only)
pub fn filter_huge_pages(pages: &[IdlePageInfo]) -> Vec<IdlePageInfo> {
    pages
        .iter()
        .filter(|p| p.page_type.is_huge())
        .copied()
        .collect()
}

/// Convert bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Check if running as root (required for ETMEM operations)
pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Check if ETMEM module is available
///
/// This checks if the procfs entries exist
pub fn is_etmem_available() -> bool {
    std::path::Path::new("/proc/self/idle_pages").exists()
        && std::path::Path::new("/proc/self/swap_pages").exists()
}

/// Get page shift for a given page size
pub const fn page_shift(page_size: u64) -> u32 {
    // Use if-else chain for const compatibility
    if page_size == 4096 {
        12
    } else if page_size == 2 * 1024 * 1024 {
        21
    } else if page_size == 1024 * 1024 * 1024 {
        30
    } else {
        12 // Default to 4KB
    }
}

/// Convert page count to bytes
#[inline]
pub const fn pages_to_bytes(pages: u64, page_size: u64) -> u64 {
    pages * page_size
}

/// Convert bytes to page count
#[inline]
pub const fn bytes_to_pages(bytes: u64, page_size: u64) -> u64 {
    (bytes + page_size - 1) / page_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_alignment() {
        assert!(is_page_aligned(4096));
        assert!(!is_page_aligned(4097));
        assert!(is_page_aligned(0));

        assert!(is_huge_page_aligned(2 * 1024 * 1024));
        assert!(!is_huge_page_aligned(4096));
    }

    #[test]
    fn test_page_align() {
        assert_eq!(page_align_down(4097), 4096);
        assert_eq!(page_align_up(4097), 8192);
        assert_eq!(page_align_down(8192), 8192);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_stats_calculation() {
        let pages = vec![
            IdlePageInfo::new(0x1000, ProcIdlePageType::PteIdle, 1),
            IdlePageInfo::new(0x2000, ProcIdlePageType::PteAccessed, 1),
            IdlePageInfo::new(0x200000, ProcIdlePageType::PmdIdle, 1),
        ];

        let stats = IdlePageStats::from_pages(&pages);
        assert_eq!(stats.total_pages, 3);
        assert_eq!(stats.idle_pages, 2); // PTE_IDLE + PMD_IDLE
        assert_eq!(stats.accessed_pages, 1);
        assert_eq!(stats.huge_pages, 1);
        assert_eq!(stats.total_bytes, 4096 + 4096 + 2 * 1024 * 1024);
    }

    #[test]
    fn test_idle_ratio() {
        let stats = IdlePageStats {
            total_bytes: 1000,
            idle_bytes: 300,
            accessed_bytes: 700,
            ..Default::default()
        };

        assert!((stats.idle_ratio() - 0.3).abs() < 0.001);
        assert!((stats.accessed_ratio() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_filter_functions() {
        let pages = vec![
            IdlePageInfo::new(0x1000, ProcIdlePageType::PteIdle, 1),
            IdlePageInfo::new(0x2000, ProcIdlePageType::PteAccessed, 1),
            IdlePageInfo::new(0x200000, ProcIdlePageType::PmdIdle, 1),
        ];

        let idle = filter_idle_pages(&pages);
        assert_eq!(idle.len(), 2);

        let accessed = filter_accessed_pages(&pages);
        assert_eq!(accessed.len(), 1);

        let huge = filter_huge_pages(&pages);
        assert_eq!(huge.len(), 1);
    }

    #[test]
    fn test_page_conversions() {
        assert_eq!(pages_to_bytes(10, 4096), 40960);
        assert_eq!(bytes_to_pages(4097, 4096), 2);
    }
}
