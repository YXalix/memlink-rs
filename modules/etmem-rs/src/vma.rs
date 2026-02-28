//! Virtual Memory Area (VMA) discovery and management
//!
//! This module provides functionality to discover and inspect process memory
//! mappings by parsing `/proc/[pid]/maps`. It enables per-VMA operations for
//! scanning and swapping.
//!
//! # Example
//!
//! ```no_run
//! use etmem_rs::vma::{VmaMap, VmaFilter};
//!
//! // Discover all VMAs for a process
//! let vma_map = VmaMap::for_process(std::process::id() as u32)
//!     .expect("Failed to parse VMAs");
//!
//! // Get the heap VMA
//! if let Some(heap) = vma_map.heap() {
//!     println!("Heap: {} bytes", heap.size());
//! }
//!
//! // Filter for anonymous, writable regions
//! let writable_anon = vma_map.filter(VmaFilter::ANONYMOUS | VmaFilter::WRITABLE);
//! ```

use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::error::{EtmemError, Result};
use crate::types::AddressRange;

/// Represents a Virtual Memory Area (memory mapping)
///
/// This struct contains all the metadata for a single memory region
/// as reported by `/proc/[pid]/maps`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmaRegion {
    /// Start address of the region (inclusive)
    pub start: u64,
    /// End address of the region (exclusive)
    pub end: u64,
    /// Memory permissions for this region
    pub permissions: VmaPermissions,
    /// Offset within the mapped file (0 for anonymous mappings)
    pub offset: u64,
    /// Device number (major:minor) for file-backed mappings
    pub device: String,
    /// Inode number for file-backed mappings
    pub inode: u64,
    /// Pathname for file-backed mappings (None for anonymous)
    pub pathname: Option<String>,
    /// Parsed pathname type
    pub pathname_type: PathnameType,
}

/// Type of pathname for a VMA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathnameType {
    /// Anonymous mapping (no file)
    Anonymous,
    /// Heap segment
    Heap,
    /// Stack segment
    Stack,
    /// Program text/data (e.g., /usr/bin/ls)
    Program,
    /// Shared library
    SharedLibrary,
    /// Memory-mapped file
    MappedFile,
    /// Kernel virtual memory (e.g., [vdso], [vsyscall])
    Kernel,
    /// Other special mappings
    Other,
}

impl PathnameType {
    /// Determine pathname type from the raw pathname string
    fn from_pathname(pathname: Option<&str>) -> Self {
        match pathname {
            None => Self::Anonymous,
            Some("[heap]") => Self::Heap,
            Some("[stack]") => Self::Stack,
            Some(p) if p.starts_with("[stack:") => Self::Stack,
            Some("[vdso]") | Some("[vsyscall]") | Some("[vvar]") => Self::Kernel,
            Some(p) if p.starts_with('[') && p.ends_with(']') => Self::Other,
            Some(p) if p.contains(".so") => Self::SharedLibrary,
            Some(_) => Self::Program,
        }
    }
}

/// Memory permissions for a VMA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VmaPermissions {
    /// Read permission
    pub read: bool,
    /// Write permission
    pub write: bool,
    /// Execute permission
    pub execute: bool,
    /// Shared (vs private) mapping
    pub shared: bool,
}

impl VmaPermissions {
    /// Create permissions from a permission string (e.g., "rwxp")
    fn from_str(s: &str) -> Result<Self> {
        if s.len() != 4 {
            return Err(EtmemError::VmaParseError(format!(
                "Invalid permission string length: {}",
                s
            )));
        }

        let chars: Vec<char> = s.chars().collect();
        Ok(Self {
            read: chars[0] == 'r',
            write: chars[1] == 'w',
            execute: chars[2] == 'x',
            shared: chars[3] == 's',
        })
    }

    /// Check if this is a private mapping (copy-on-write)
    pub const fn is_private(&self) -> bool {
        !self.shared
    }

    /// Convert to a short string representation (e.g., "rwxp")
    pub fn to_string(&self) -> String {
        format!(
            "{}{}{}{}",
            if self.read { 'r' } else { '-' },
            if self.write { 'w' } else { '-' },
            if self.execute { 'x' } else { '-' },
            if self.shared { 's' } else { 'p' }
        )
    }
}

impl fmt::Display for VmaPermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

bitflags! {
    /// Filter criteria for VMA queries
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct VmaFilter: u32 {
        /// Anonymous mappings (no backing file)
        const ANONYMOUS = 0x0001;
        /// File-backed mappings
        const FILE_BACKED = 0x0002;
        /// Readable regions
        const READABLE = 0x0004;
        /// Writable regions
        const WRITABLE = 0x0008;
        /// Executable regions
        const EXECUTABLE = 0x0010;
        /// Heap region
        const HEAP = 0x0020;
        /// Stack region(s)
        const STACK = 0x0040;
        /// Shared libraries
        const SHARED_LIB = 0x0080;
        /// Private mappings (copy-on-write)
        const PRIVATE = 0x0100;
        /// Shared mappings
        const SHARED = 0x0200;
        /// Scannable regions (readable, not kernel, not vsyscall)
        const SCANNABLE = 0x0400;
        /// Swappable regions (writable anonymous)
        const SWAPPABLE = 0x0800;
    }
}

impl VmaRegion {
    /// Create a new VMA region
    pub const fn new(start: u64, end: u64, permissions: VmaPermissions) -> Self {
        Self {
            start,
            end,
            permissions,
            offset: 0,
            device: String::new(),
            inode: 0,
            pathname: None,
            pathname_type: PathnameType::Anonymous,
        }
    }

    /// Get the size of this region in bytes
    pub const fn size(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Check if an address is within this region
    pub const fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }

    /// Convert to an AddressRange for scanning operations
    pub const fn to_address_range(&self) -> AddressRange {
        AddressRange::new(self.start, self.end)
    }

    /// Check if this VMA is anonymous (no backing file)
    pub fn is_anonymous(&self) -> bool {
        self.pathname_type == PathnameType::Anonymous
    }

    /// Check if this is the heap VMA
    pub fn is_heap(&self) -> bool {
        self.pathname_type == PathnameType::Heap
    }

    /// Check if this is a stack VMA
    pub fn is_stack(&self) -> bool {
        self.pathname_type == PathnameType::Stack
    }

    /// Check if this VMA is suitable for scanning
    ///
    /// A scannable region must be:
    /// - Readable
    /// - Not a kernel special region (vdso, vsyscall, etc.)
    pub fn is_scannable(&self) -> bool {
        self.permissions.read
            && !matches!(
                self.pathname_type,
                PathnameType::Kernel | PathnameType::Other
            )
    }

    /// Check if this VMA is suitable for swapping
    ///
    /// A swappable region should be:
    /// - Writable (to avoid swapping read-only data)
    /// - Anonymous or private (file-backed shared pages have different semantics)
    /// - Not the stack (stack pages shouldn't be swapped mid-execution)
    pub fn is_swappable(&self) -> bool {
        self.permissions.write
            && (self.is_anonymous() || self.permissions.is_private())
            && !self.is_stack()
    }

    /// Check if this region matches the given filter criteria
    pub fn matches_filter(&self, filter: VmaFilter) -> bool {
        if filter.contains(VmaFilter::ANONYMOUS) && !self.is_anonymous() {
            return false;
        }
        if filter.contains(VmaFilter::FILE_BACKED) && self.is_anonymous() {
            return false;
        }
        if filter.contains(VmaFilter::READABLE) && !self.permissions.read {
            return false;
        }
        if filter.contains(VmaFilter::WRITABLE) && !self.permissions.write {
            return false;
        }
        if filter.contains(VmaFilter::EXECUTABLE) && !self.permissions.execute {
            return false;
        }
        if filter.contains(VmaFilter::HEAP) && !self.is_heap() {
            return false;
        }
        if filter.contains(VmaFilter::STACK) && !self.is_stack() {
            return false;
        }
        if filter.contains(VmaFilter::SHARED_LIB)
            && self.pathname_type != PathnameType::SharedLibrary
        {
            return false;
        }
        if filter.contains(VmaFilter::PRIVATE) && !self.permissions.is_private() {
            return false;
        }
        if filter.contains(VmaFilter::SHARED) && !self.permissions.shared {
            return false;
        }
        if filter.contains(VmaFilter::SCANNABLE) && !self.is_scannable() {
            return false;
        }
        if filter.contains(VmaFilter::SWAPPABLE) && !self.is_swappable() {
            return false;
        }
        true
    }

    /// Get a descriptive name for this region
    pub fn name(&self) -> &str {
        match &self.pathname {
            Some(p) => p.as_str(),
            None => match self.pathname_type {
                PathnameType::Anonymous => "[anonymous]",
                PathnameType::Heap => "[heap]",
                PathnameType::Stack => "[stack]",
                PathnameType::Kernel => "[kernel]",
                PathnameType::Other => "[other]",
                _ => "[unknown]",
            },
        }
    }
}

impl fmt::Display for VmaRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:016x}-{:016x} {} {:08x} {} {} {}",
            self.start,
            self.end,
            self.permissions,
            self.offset,
            self.device,
            self.inode,
            self.name()
        )
    }
}

/// Collection of VMAs for a process
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VmaMap {
    /// All VMA regions for the process
    regions: Vec<VmaRegion>,
    /// Process ID
    pid: u32,
}

impl VmaMap {
    /// Parse VMAs for a given process from `/proc/[pid]/maps`
    ///
    /// # Errors
    /// Returns error if:
    /// - Process doesn't exist
    /// - Permission denied
    /// - Parse error
    pub fn for_process(pid: u32) -> Result<Self> {
        let path = format!("/proc/{}/maps", pid);
        Self::from_file(&path, pid)
    }

    /// Parse VMAs from a file (useful for testing)
    pub fn from_file<P: AsRef<Path>>(path: P, pid: u32) -> Result<Self> {
        let file = File::open(path).map_err(|e| {
            if e.raw_os_error() == Some(libc::ESRCH) {
                EtmemError::ProcessNotFound
            } else {
                EtmemError::IoError(e.to_string())
            }
        })?;

        let reader = BufReader::new(file);
        let mut regions = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| EtmemError::IoError(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }

            match Self::parse_line(&line) {
                Ok(region) => regions.push(region),
                Err(e) => {
                    // Log parse errors but continue processing other lines
                    log::debug!("Failed to parse VMA line '{}': {}", line, e);
                }
            }
        }

        // Sort by start address for consistent ordering
        regions.sort_by_key(|r| r.start);

        Ok(Self { regions, pid })
    }

    /// Parse a single line from /proc/[pid]/maps
    ///
    /// Format: start-end perms offset dev inode pathname
    /// Example: 55c3e5a6c000-55c3e5a6d000 r-xp 00000000 08:01 1310734 /usr/bin/ls
    fn parse_line(line: &str) -> Result<VmaRegion> {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 5 {
            return Err(EtmemError::VmaParseError(format!(
                "Invalid line format (need at least 5 fields): {}",
                line
            )));
        }

        // Parse address range (first field)
        let addr_part = parts[0];
        let addrs: Vec<&str> = addr_part.split('-').collect();
        if addrs.len() != 2 {
            return Err(EtmemError::VmaParseError(format!(
                "Invalid address range: {}",
                addr_part
            )));
        }

        let start = u64::from_str_radix(addrs[0], 16).map_err(|_| {
            EtmemError::VmaParseError(format!("Invalid start address: {}", addrs[0]))
        })?;

        let end = u64::from_str_radix(addrs[1], 16).map_err(|_| {
            EtmemError::VmaParseError(format!("Invalid end address: {}", addrs[1]))
        })?;

        // Parse permissions (second field)
        let permissions = VmaPermissions::from_str(parts[1])?;

        // Parse offset (third field)
        let offset = u64::from_str_radix(parts[2], 16).map_err(|_| {
            EtmemError::VmaParseError(format!("Invalid offset: {}", parts[2]))
        })?;

        // Device (fourth field)
        let device = parts[3].to_string();

        // Inode (fifth field)
        let inode = parts[4].parse::<u64>().map_err(|_| {
            EtmemError::VmaParseError(format!("Invalid inode: {}", parts[4]))
        })?;

        // Pathname (optional, remainder of line)
        let pathname = if parts.len() > 5 {
            // Reconstruct pathname from remaining parts (may contain spaces)
            let pathname_start = line
                .find(parts[5])
                .ok_or_else(|| EtmemError::VmaParseError("Cannot find pathname".to_string()))?;
            let pathname_str = &line[pathname_start..];
            Some(pathname_str.trim().to_string())
        } else {
            None
        };

        let pathname_type = PathnameType::from_pathname(pathname.as_deref());

        Ok(VmaRegion {
            start,
            end,
            permissions,
            offset,
            device,
            inode,
            pathname,
            pathname_type,
        })
    }

    /// Get all regions
    pub fn regions(&self) -> &[VmaRegion] {
        &self.regions
    }

    /// Get the process ID
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the number of regions
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Check if there are no regions
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Find a region containing the given address
    pub fn find_region(&self, addr: u64) -> Option<&VmaRegion> {
        self.regions.binary_search_by_key(&addr, |r| r.start)
            .ok()
            .map(|idx| &self.regions[idx])
            .or_else(|| {
                // Binary search gives us insertion point, check previous region
                let idx = self.regions.binary_search_by_key(&addr, |r| r.start)
                    .unwrap_err();
                if idx > 0 && self.regions[idx - 1].contains(addr) {
                    Some(&self.regions[idx - 1])
                } else {
                    None
                }
            })
    }

    /// Get the heap VMA
    pub fn heap(&self) -> Option<&VmaRegion> {
        self.regions.iter().find(|r| r.is_heap())
    }

    /// Get the stack VMA(s)
    pub fn stack(&self) -> Option<&VmaRegion> {
        self.regions.iter().find(|r| r.is_stack())
    }

    /// Get all stack VMAs (there may be multiple threads)
    pub fn stacks(&self) -> Vec<&VmaRegion> {
        self.regions.iter().filter(|r| r.is_stack()).collect()
    }

    /// Get all anonymous mappings
    pub fn anonymous(&self) -> Vec<&VmaRegion> {
        self.regions.iter().filter(|r| r.is_anonymous()).collect()
    }

    /// Get all file-backed mappings
    pub fn file_backed(&self) -> Vec<&VmaRegion> {
        self.regions.iter().filter(|r| !r.is_anonymous()).collect()
    }

    /// Filter VMAs by criteria
    pub fn filter(&self, filter: VmaFilter) -> Vec<&VmaRegion> {
        self.regions
            .iter()
            .filter(|r| r.matches_filter(filter))
            .collect()
    }

    /// Get scannable regions
    pub fn scannable(&self) -> Vec<&VmaRegion> {
        self.filter(VmaFilter::SCANNABLE)
    }

    /// Get swappable regions
    pub fn swappable(&self) -> Vec<&VmaRegion> {
        self.filter(VmaFilter::SWAPPABLE)
    }

    /// Calculate total virtual memory size
    pub fn total_size(&self) -> u64 {
        self.regions.iter().map(|r| r.size()).sum()
    }

    /// Calculate total anonymous memory size
    pub fn anonymous_size(&self) -> u64 {
        self.regions
            .iter()
            .filter(|r| r.is_anonymous())
            .map(|r| r.size())
            .sum()
    }

    /// Get combined address ranges for all regions matching the filter
    ///
    /// Adjacent or overlapping regions are merged into single ranges.
    pub fn merged_ranges(&self, filter: VmaFilter) -> Vec<AddressRange> {
        let mut ranges: Vec<AddressRange> = self
            .regions
            .iter()
            .filter(|r| r.matches_filter(filter))
            .map(|r| r.to_address_range())
            .collect();

        if ranges.is_empty() {
            return ranges;
        }

        // Sort by start address
        ranges.sort_by_key(|r| r.start);

        // Merge overlapping or adjacent ranges
        let mut merged = vec![ranges[0]];
        for range in ranges.iter().skip(1) {
            let last = merged.last_mut().unwrap();
            if range.start <= last.end {
                // Overlapping or adjacent, merge
                last.end = last.end.max(range.end);
            } else {
                merged.push(*range);
            }
        }

        merged
    }
}

impl fmt::Display for VmaMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "VMA Map for PID {} ({} regions):", self.pid, self.len())?;
        for region in &self.regions {
            writeln!(f, "  {}", region)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vma_permissions() {
        let perms = VmaPermissions::from_str("rwxp").unwrap();
        assert!(perms.read);
        assert!(perms.write);
        assert!(perms.execute);
        assert!(!perms.shared);
        assert!(perms.is_private());

        let perms = VmaPermissions::from_str("r-xs").unwrap();
        assert!(perms.read);
        assert!(!perms.write);
        assert!(perms.execute);
        assert!(perms.shared);
        assert!(!perms.is_private());
    }

    #[test]
    fn test_parse_line() {
        let line = "55c3e5a6c000-55c3e5a6d000 r-xp 00000000 08:01 1310734 /usr/bin/ls";
        let vma = VmaMap::parse_line(line).unwrap();

        assert_eq!(vma.start, 0x55c3e5a6c000);
        assert_eq!(vma.end, 0x55c3e5a6d000);
        assert!(vma.permissions.read);
        assert!(vma.permissions.execute);
        assert!(!vma.permissions.write);
        assert_eq!(vma.offset, 0);
        assert_eq!(vma.device, "08:01");
        assert_eq!(vma.inode, 1310734);
        assert_eq!(vma.pathname, Some("/usr/bin/ls".to_string()));
        assert_eq!(vma.pathname_type, PathnameType::Program);
    }

    #[test]
    fn test_parse_line_anonymous() {
        let line = "7f8b3c000000-7f8b3c021000 rw-p 00000000 00:00 0";
        let vma = VmaMap::parse_line(line).unwrap();

        assert!(vma.is_anonymous());
        assert_eq!(vma.pathname_type, PathnameType::Anonymous);
    }

    #[test]
    fn test_parse_line_heap() {
        let line = "55c3e616e000-55c3e618f000 rw-p 00000000 00:00 0 [heap]";
        let vma = VmaMap::parse_line(line).unwrap();

        assert!(vma.is_heap());
        assert_eq!(vma.pathname_type, PathnameType::Heap);
    }

    #[test]
    fn test_parse_line_stack() {
        let line = "7ffd5d8a5000-7ffd5d8c6000 rw-p 00000000 00:00 0 [stack]";
        let vma = VmaMap::parse_line(line).unwrap();

        assert!(vma.is_stack());
        assert_eq!(vma.pathname_type, PathnameType::Stack);
    }

    #[test]
    fn test_parse_line_shared_lib() {
        let line = "7f8b3c400000-7f8b3c428000 r-xp 00000000 08:01 1310735 /lib/x86_64-linux-gnu/libc.so.6";
        let vma = VmaMap::parse_line(line).unwrap();

        assert_eq!(vma.pathname_type, PathnameType::SharedLibrary);
        assert!(!vma.is_anonymous());
    }

    #[test]
    fn test_vma_region_methods() {
        let vma = VmaRegion {
            start: 0x1000,
            end: 0x5000,
            permissions: VmaPermissions::from_str("rw-p").unwrap(),
            offset: 0,
            device: String::new(),
            inode: 0,
            pathname: None,
            pathname_type: PathnameType::Anonymous,
        };

        assert_eq!(vma.size(), 0x4000);
        assert!(vma.contains(0x2000));
        assert!(!vma.contains(0x6000));
        assert!(vma.is_scannable());
        assert!(vma.is_swappable());
    }

    #[test]
    fn test_vma_filter() {
        let vma = VmaRegion {
            start: 0x1000,
            end: 0x5000,
            permissions: VmaPermissions::from_str("rw-p").unwrap(),
            offset: 0,
            device: String::new(),
            inode: 0,
            pathname: None,
            pathname_type: PathnameType::Anonymous,
        };

        assert!(vma.matches_filter(VmaFilter::ANONYMOUS));
        assert!(vma.matches_filter(VmaFilter::READABLE));
        assert!(vma.matches_filter(VmaFilter::WRITABLE));
        assert!(vma.matches_filter(VmaFilter::SCANNABLE));
        assert!(vma.matches_filter(VmaFilter::SWAPPABLE));
        assert!(!vma.matches_filter(VmaFilter::FILE_BACKED));
        assert!(!vma.matches_filter(VmaFilter::EXECUTABLE));
        assert!(!vma.matches_filter(VmaFilter::HEAP));
    }

    #[test]
    fn test_pathname_type() {
        assert_eq!(PathnameType::from_pathname(None), PathnameType::Anonymous);
        assert_eq!(PathnameType::from_pathname(Some("[heap]")), PathnameType::Heap);
        assert_eq!(PathnameType::from_pathname(Some("[stack]")), PathnameType::Stack);
        assert_eq!(
            PathnameType::from_pathname(Some("[stack:1234]")),
            PathnameType::Stack
        );
        assert_eq!(
            PathnameType::from_pathname(Some("[vdso]")),
            PathnameType::Kernel
        );
        assert_eq!(
            PathnameType::from_pathname(Some("/lib/libc.so.6")),
            PathnameType::SharedLibrary
        );
        assert_eq!(
            PathnameType::from_pathname(Some("/usr/bin/ls")),
            PathnameType::Program
        );
    }

    #[test]
    fn test_vma_permissions_display() {
        let perms = VmaPermissions::from_str("rwxp").unwrap();
        assert_eq!(perms.to_string(), "rwxp");

        let perms = VmaPermissions::from_str("r--s").unwrap();
        assert_eq!(perms.to_string(), "r--s");
    }
}
