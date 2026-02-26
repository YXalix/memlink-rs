//! Type definitions for ETMEM operations
//!
//! This module contains data structures, constants, and type definitions
//! for the ETMEM (Enhanced Tiered Memory) subsystem.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

/// Maximum buffer size for idle page kernel buffer
pub const PAGE_IDLE_KBUF_SIZE: usize = 8000;

/// Minimum buffer size for page scan operations
pub const PAGE_IDLE_BUF_MIN: usize = std::mem::size_of::<u64>() * 2 + 3;

/// Invalid page constant (used when address is not found)
pub const INVALID_PAGE: u64 = !0u64;

/// Watermark maximum percentage
pub const WATERMARK_MAX: u32 = 100;

/// IOCTL magic numbers for ETMEM operations
pub const IDLE_SCAN_MAGIC: u8 = 0x66;
pub const RECLAIM_SWAPCACHE_MAGIC: u8 = 0x77;

/// Maximum number of pages to scan per iteration
pub const SWAP_SCAN_NUM_MAX: u32 = 32;

/// Flag to trigger rescan
pub const RET_RESCAN_FLAG: u32 = 0x10000;

/// Default walk step (number of pages)
pub const DEFAULT_WALK_STEP: u32 = 512;

/// Page type enumeration for idle page detection
///
/// These types correspond to the hardware page table entry states
/// and indicate the size and access status of memory pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ProcIdlePageType {
    /// 4KB page was accessed (A bit set in PTE)
    PteAccessed = 0,
    /// 2MB page was accessed (A bit set in PMD)
    PmdAccessed = 1,
    /// 1GB page is present (PUD present bit)
    PudPresent = 2,
    /// 4KB page is dirty (D bit set in PTE)
    PteDirty = 3,
    /// 2MB page is dirty (D bit set in PMD)
    PmdDirty = 4,
    /// 4KB page is idle (A bit not set in PTE)
    PteIdle = 5,
    /// 2MB page is idle (A bit not set in PMD)
    PmdIdle = 6,
    /// All PTEs within a PMD are idle
    PmdIdlePtes = 7,
    /// 4KB page table entry is a hole (not present)
    PteHole = 8,
    /// 2MB PMD entry is a hole (not present)
    PmdHole = 9,
    /// Command marker for PIP protocol
    PipCmd = 10,
    /// Maximum valid type value
    Max = 11,
}

impl ProcIdlePageType {
    /// Convert from raw u8 value
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::PteAccessed),
            1 => Some(Self::PmdAccessed),
            2 => Some(Self::PudPresent),
            3 => Some(Self::PteDirty),
            4 => Some(Self::PmdDirty),
            5 => Some(Self::PteIdle),
            6 => Some(Self::PmdIdle),
            7 => Some(Self::PmdIdlePtes),
            8 => Some(Self::PteHole),
            9 => Some(Self::PmdHole),
            10 => Some(Self::PipCmd),
            _ => None,
        }
    }

    /// Check if this page type represents a huge page (2MB or 1GB)
    pub const fn is_huge(&self) -> bool {
        matches!(
            self,
            Self::PmdAccessed
                | Self::PmdDirty
                | Self::PmdIdle
                | Self::PmdIdlePtes
                | Self::PmdHole
                | Self::PudPresent
        )
    }

    /// Check if this page type represents an idle (cold) page
    pub const fn is_idle(&self) -> bool {
        matches!(
            self,
            Self::PteIdle | Self::PmdIdle | Self::PmdIdlePtes
        )
    }

    /// Check if this page type represents an accessed (hot) page
    pub const fn is_accessed(&self) -> bool {
        matches!(self, Self::PteAccessed | Self::PmdAccessed)
    }

    /// Check if this page type represents a hole (not mapped)
    pub const fn is_hole(&self) -> bool {
        matches!(self, Self::PteHole | Self::PmdHole)
    }

    /// Get the page size in bytes for this type
    pub const fn page_size(&self) -> u64 {
        match self {
            Self::PteAccessed | Self::PteDirty | Self::PteIdle | Self::PteHole => 4096, // 4KB
            Self::PmdAccessed
            | Self::PmdDirty
            | Self::PmdIdle
            | Self::PmdIdlePtes
            | Self::PmdHole => 2 * 1024 * 1024, // 2MB
            Self::PudPresent => 1024 * 1024 * 1024, // 1GB
            _ => 4096, // Default to 4KB for command types
        }
    }
}

/// PIP (Proc Idle Page) encoding helpers
///
/// The kernel encodes idle page information in a compact byte format:
/// - Upper 4 bits: page type
/// - Lower 4 bits: count of consecutive pages (0 means 1 page)
pub struct PipEncoding;

impl PipEncoding {
    /// Extract type from encoded byte
    #[inline]
    pub const fn extract_type(encoded: u8) -> u8 {
        (encoded >> 4) & 0xf
    }

    /// Extract size/count from encoded byte
    /// Returns count of consecutive pages minus 1 (so 0 means 1 page)
    #[inline]
    pub const fn extract_size(encoded: u8) -> u8 {
        encoded & 0xf
    }

    /// Compose type and size into encoded byte
    /// count is the number of consecutive pages minus 1 (0-15, representing 1-16 pages)
    #[inline]
    pub const fn compose(page_type: u8, count: u8) -> u8 {
        ((page_type & 0xf) << 4) | (count & 0xf)
    }

    /// PIP command to set HVA (Host Virtual Address)
    pub const SET_HVA: u8 = Self::compose(ProcIdlePageType::PipCmd as u8, 0);

    /// Decode an encoded byte into (type, count)
    pub const fn decode(encoded: u8) -> (u8, u8) {
        (Self::extract_type(encoded), Self::extract_size(encoded))
    }
}

/// Idle page scan flags
///
/// These flags control the behavior of page scanning operations.
/// They can be combined using bitwise OR.
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct ScanFlags: u32 {
        /// Only scan huge pages (maps to O_NONBLOCK)
        const SCAN_HUGE_PAGE = libc::O_NONBLOCK as u32;
        /// Stop on PMD_IDLE_PTES (maps to O_NOFOLLOW)
        const SCAN_SKIM_IDLE = libc::O_NOFOLLOW as u32;
        /// Report PTE/PMD dirty bit (maps to O_NOATIME)
        const SCAN_DIRTY_PAGE = libc::O_NOATIME as u32;
        /// Treat normal pages as huge in VM context
        const SCAN_AS_HUGE = 0o100000000;
        /// Ignore host access when scanning VM
        const SCAN_IGN_HOST = 0o200000000;
        /// Internal: scanning host for VM hole detection
        const VM_SCAN_HOST = 0o400000000;
        /// Scan specific VMA with flag
        const VMA_SCAN_FLAG = 0x1000;
    }
}

impl ScanFlags {
    /// Check if flags are valid (no reserved bits set)
    pub fn is_valid(&self) -> bool {
        let valid_mask = Self::SCAN_HUGE_PAGE.bits()
            | Self::SCAN_SKIM_IDLE.bits()
            | Self::SCAN_DIRTY_PAGE.bits()
            | Self::SCAN_AS_HUGE.bits()
            | Self::SCAN_IGN_HOST.bits()
            | Self::VM_SCAN_HOST.bits()
            | Self::VMA_SCAN_FLAG.bits();
        self.bits() & !valid_mask == 0
    }
}

/// Swapcache watermark levels
///
/// Watermarks control when proactive swapcache reclaim starts and stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SwapcacheWatermark {
    /// Low watermark - start reclaiming when swapcache exceeds this
    Low = 0,
    /// High watermark - stop reclaiming when swapcache drops to this
    High = 1,
    /// Number of watermark levels
    NrWatermark = 2,
}

impl SwapcacheWatermark {
    /// Convert from raw u8 value
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Low),
            1 => Some(Self::High),
            _ => None,
        }
    }
}

/// Page idle information entry
///
/// Represents a single idle (or accessed) page detected during scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdlePageInfo {
    /// Virtual address of the page
    pub address: u64,
    /// Page type (accessed, idle, dirty, etc.)
    pub page_type: ProcIdlePageType,
    /// Number of consecutive pages of this type (1-16)
    pub count: u8,
}

impl IdlePageInfo {
    /// Create a new IdlePageInfo
    pub fn new(address: u64, page_type: ProcIdlePageType, count: u8) -> Self {
        Self {
            address,
            page_type,
            count: if count < 1 { 1 } else { count },
        }
    }

    /// Get the total size covered by this entry in bytes
    pub fn total_size(&self) -> u64 {
        self.page_type.page_size() * self.count as u64
    }

    /// Get the end address (exclusive) of this entry
    pub fn end_address(&self) -> u64 {
        self.address + self.total_size()
    }

    /// Check if this entry represents an idle page
    pub fn is_idle(&self) -> bool {
        self.page_type.is_idle()
    }

    /// Check if this entry represents an accessed (hot) page
    pub fn is_accessed(&self) -> bool {
        self.page_type.is_accessed()
    }
}

/// Virtual address range for scanning
///
/// Defines a range of virtual addresses to scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressRange {
    /// Start address (inclusive)
    pub start: u64,
    /// End address (exclusive)
    pub end: u64,
}

impl AddressRange {
    /// Create a new address range
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    /// Create a range from start with given size
    pub const fn with_size(start: u64, size: u64) -> Self {
        Self {
            start,
            end: start + size,
        }
    }

    /// Check if an address is within this range
    pub const fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }

    /// Get the size of this range in bytes
    pub const fn size(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Check if the range is valid (start < end)
    pub const fn is_valid(&self) -> bool {
        self.start < self.end
    }

    /// Check if this range overlaps with another
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

impl Default for AddressRange {
    fn default() -> Self {
        Self { start: 0, end: 0 }
    }
}

/// Watermark configuration for swapcache reclaim
///
/// Controls when the kernel proactively reclaims swapcache pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatermarkConfig {
    /// Low watermark percentage (0-100)
    pub low_percent: u8,
    /// High watermark percentage (0-100)
    pub high_percent: u8,
}

impl WatermarkConfig {
    /// Create a new watermark configuration
    pub const fn new(low_percent: u8, high_percent: u8) -> Self {
        Self {
            low_percent,
            high_percent,
        }
    }

    /// Validate the watermark configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        use crate::error::EtmemError;

        if self.low_percent > 100 || self.high_percent > 100 {
            return Err(EtmemError::WatermarkOutOfRange);
        }
        if self.low_percent >= self.high_percent {
            return Err(EtmemError::InvalidWatermarkOrder);
        }
        Ok(())
    }

    /// Get default watermark configuration (30% low, 70% high)
    pub const fn default() -> Self {
        Self {
            low_percent: 30,
            high_percent: 70,
        }
    }
}

impl Default for WatermarkConfig {
    fn default() -> Self {
        Self::default()
    }
}

/// Kernel buffer status after scan operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BufferStatus {
    /// Operation completed successfully
    Success = 0,
    /// Kernel buffer full, more data available
    KbufFull = 1,
    /// User buffer full
    BufFull = 2,
}

impl BufferStatus {
    /// Convert from raw u8 value
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Success),
            1 => Some(Self::KbufFull),
            2 => Some(Self::BufFull),
            _ => None,
        }
    }

    /// Check if more data is available
    pub const fn has_more(&self) -> bool {
        matches!(self, Self::KbufFull | Self::BufFull)
    }
}

/// ETMEM scan session configuration
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Scan flags controlling scan behavior
    pub flags: ScanFlags,
    /// Buffer size for reading idle page data
    pub buffer_size: usize,
    /// Walk step in pages (how many pages to skip between samples)
    pub walk_step: u32,
}

impl ScanConfig {
    /// Create a new scan configuration with default values
    pub const fn new() -> Self {
        Self {
            flags: ScanFlags::empty(),
            buffer_size: PAGE_IDLE_KBUF_SIZE,
            walk_step: DEFAULT_WALK_STEP,
        }
    }

    /// Set scan flags
    pub const fn with_flags(mut self, flags: ScanFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set buffer size
    pub const fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Set walk step
    pub const fn with_walk_step(mut self, step: u32) -> Self {
        self.walk_step = step;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        use crate::error::EtmemError;

        if !self.flags.is_valid() {
            return Err(EtmemError::InvalidFlags);
        }
        if self.buffer_size < PAGE_IDLE_BUF_MIN {
            return Err(EtmemError::BufferTooSmall);
        }
        if self.buffer_size > PAGE_IDLE_KBUF_SIZE {
            // Clamp to max size
            return Err(EtmemError::BufferTooSmall);
        }
        Ok(())
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Page swap configuration
#[derive(Debug, Clone)]
pub struct SwapConfig {
    /// Enable proactive swapcache reclaim
    pub proactive_reclaim: bool,
    /// Watermark configuration for reclaim
    pub watermark: WatermarkConfig,
    /// Maximum number of pages to swap per operation
    pub max_pages: u32,
}

impl SwapConfig {
    /// Create a new swap configuration with defaults
    pub const fn new() -> Self {
        Self {
            proactive_reclaim: false,
            watermark: WatermarkConfig::new(30, 70),
            max_pages: SWAP_SCAN_NUM_MAX,
        }
    }

    /// Enable proactive reclaim
    pub const fn with_proactive_reclaim(mut self, enable: bool) -> Self {
        self.proactive_reclaim = enable;
        self
    }

    /// Set watermark configuration
    pub const fn with_watermark(mut self, watermark: WatermarkConfig) -> Self {
        self.watermark = watermark;
        self
    }

    /// Set maximum pages per operation
    pub const fn with_max_pages(mut self, max: u32) -> Self {
        self.max_pages = max;
        self
    }
}

impl Default for SwapConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pip_encoding() {
        let encoded = PipEncoding::compose(ProcIdlePageType::PteIdle as u8, 5);
        assert_eq!(PipEncoding::extract_type(encoded), ProcIdlePageType::PteIdle as u8);
        assert_eq!(PipEncoding::extract_size(encoded), 5);

        let (t, s) = PipEncoding::decode(encoded);
        assert_eq!(t, ProcIdlePageType::PteIdle as u8);
        assert_eq!(s, 5);
    }

    #[test]
    fn test_proc_idle_page_type() {
        assert!(ProcIdlePageType::PmdIdle.is_huge());
        assert!(!ProcIdlePageType::PteIdle.is_huge());
        assert!(ProcIdlePageType::PteIdle.is_idle());
        assert!(ProcIdlePageType::PteAccessed.is_accessed());
        assert!(ProcIdlePageType::PteHole.is_hole());
        assert_eq!(ProcIdlePageType::PteAccessed.page_size(), 4096);
        assert_eq!(ProcIdlePageType::PmdAccessed.page_size(), 2 * 1024 * 1024);
        assert_eq!(ProcIdlePageType::PudPresent.page_size(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_scan_flags() {
        let flags = ScanFlags::SCAN_HUGE_PAGE | ScanFlags::SCAN_DIRTY_PAGE;
        assert!(flags.is_valid());
        assert!(flags.contains(ScanFlags::SCAN_HUGE_PAGE));
        assert!(flags.contains(ScanFlags::SCAN_DIRTY_PAGE));
    }

    #[test]
    fn test_address_range() {
        let range = AddressRange::new(0x1000, 0x5000);
        assert!(range.contains(0x2000));
        assert!(!range.contains(0x5000));
        assert_eq!(range.size(), 0x4000);
        assert!(range.is_valid());

        let with_size = AddressRange::with_size(0x1000, 0x4000);
        assert_eq!(with_size, range);
    }

    #[test]
    fn test_watermark_config() {
        let config = WatermarkConfig::new(30, 70);
        assert!(config.validate().is_ok());

        let invalid = WatermarkConfig::new(70, 30);
        assert!(invalid.validate().is_err());

        let out_of_range = WatermarkConfig::new(0, 101);
        assert!(out_of_range.validate().is_err());
    }

    #[test]
    fn test_idle_page_info() {
        let info = IdlePageInfo::new(0x1000, ProcIdlePageType::PteIdle, 2);
        assert_eq!(info.address, 0x1000);
        assert!(info.is_idle());
        assert_eq!(info.total_size(), 4096 * 2);
        assert_eq!(info.end_address(), 0x1000 + 4096 * 2);
    }

    #[test]
    fn test_scan_config_validation() {
        let config = ScanConfig::default();
        assert!(config.validate().is_ok());

        let invalid = ScanConfig::default().with_buffer_size(10);
        assert!(invalid.validate().is_err());
    }
}
