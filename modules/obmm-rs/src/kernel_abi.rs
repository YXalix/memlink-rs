//! Kernel ABI definitions for OBMM (Open-source Bare-Metal Memory Manager)
//!
//! This module contains the ioctl command definitions and structures that match
//! the kernel interface at /usr/include/ub/obmm.h.

use core::mem::size_of;

use libc::c_void;

// ============================================================================
// IOCTL Command Encoding Macros
// ============================================================================

const IOC_NRBITS: u32 = 8;
const IOC_TYPEBITS: u32 = 8;
const IOC_SIZEBITS: u32 = 14;
const IOC_DIRBITS: u32 = 2;

const IOC_NRSHIFT: u32 = 0;
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

#[inline]
const fn _ioc(dir: u32, ty: u32, nr: u32, size: u32) -> u32 {
    (dir << IOC_DIRSHIFT)
        | (ty << IOC_TYPESHIFT)
        | (nr << IOC_NRSHIFT)
        | (size << IOC_SIZESHIFT)
}

#[inline]
const fn _ior<T>(ty: u32, nr: u32) -> u32 {
    _ioc(IOC_READ, ty, nr, size_of::<T>() as u32)
}

#[inline]
const fn _iow<T>(ty: u32, nr: u32) -> u32 {
    _ioc(IOC_WRITE, ty, nr, size_of::<T>() as u32)
}

#[inline]
const fn _iowr<T>(ty: u32, nr: u32) -> u32 {
    _ioc(IOC_READ | IOC_WRITE, ty, nr, size_of::<T>() as u32)
}

// ============================================================================
// OBMM Constants
// ============================================================================

/// Maximum number of local NUMA nodes
pub const OBMM_MAX_LOCAL_NUMA_NODES: usize = 16;

/// Maximum NUMA distance
pub const MAX_NUMA_DIST: u32 = 254;

/// Maximum private data length
pub const OBMM_MAX_PRIV_LEN: usize = 512;

/// Maximum vendor data length
pub const OBMM_MAX_VENDOR_LEN: usize = 128;

/// Invalid memory ID
pub const OBMM_INVALID_MEMID: u64 = 0;

/// Maximum number of NUMA nodes (for user API compatibility)
pub const MAX_NUMA_NODES: usize = 16;

// ============================================================================
// Export Flags
// ============================================================================

/// Allow memory mapping of exported memory
pub const OBMM_EXPORT_FLAG_ALLOW_MMAP: u64 = 0x1;

/// Fast export path
pub const OBMM_EXPORT_FLAG_FAST: u64 = 0x2;

/// Valid export flag mask
pub const OBMM_EXPORT_FLAG_MASK: u64 = OBMM_EXPORT_FLAG_ALLOW_MMAP | OBMM_EXPORT_FLAG_FAST;

// ============================================================================
// Unexport Flags
// ============================================================================

/// Valid unexport flag mask (no flags defined)
pub const OBMM_UNEXPORT_FLAG_MASK: u64 = 0;

// ============================================================================
// Import Flags
// ============================================================================

/// Allow memory mapping of imported memory
pub const OBMM_IMPORT_FLAG_ALLOW_MMAP: u64 = 0x1;

/// Pre-import flag
pub const OBMM_IMPORT_FLAG_PREIMPORT: u64 = 0x2;

/// Import to remote NUMA node
pub const OBMM_IMPORT_FLAG_NUMA_REMOTE: u64 = 0x4;

/// Valid import flag mask
pub const OBMM_IMPORT_FLAG_MASK: u64 = OBMM_IMPORT_FLAG_ALLOW_MMAP
    | OBMM_IMPORT_FLAG_PREIMPORT
    | OBMM_IMPORT_FLAG_NUMA_REMOTE;

// ============================================================================
// Unimport Flags
// ============================================================================

/// Valid unimport flag mask (no flags defined)
pub const OBMM_UNIMPORT_FLAG_MASK: u64 = 0;

// ============================================================================
// Preimport Flags
// ============================================================================

/// Valid preimport flag mask (no flags defined)
pub const OBMM_PREIMPORT_FLAG_MASK: u64 = 0;

/// Valid unpreimport flag mask (no flags defined)
pub const OBMM_UNPREIMPORT_FLAG_MASK: u64 = 0;

// ============================================================================
// Memory State Flags
// ============================================================================

/// Reserved cache type
pub const OBMM_SHM_MEM_CACHE_RESV: u8 = 0x0;

/// Normal cacheable memory
pub const OBMM_SHM_MEM_NORMAL: u8 = 0x1;

/// Normal non-cacheable memory
pub const OBMM_SHM_MEM_NORMAL_NC: u8 = 0x2;

/// Device memory
pub const OBMM_SHM_MEM_DEVICE: u8 = 0x3;

/// Cache type mask
pub const OBMM_SHM_MEM_CACHE_MASK: u8 = 0b11;

/// Read-only access
pub const OBMM_SHM_MEM_READONLY: u8 = 0x0;

/// Read-execute access
pub const OBMM_SHM_MEM_READEXEC: u8 = 0x4;

/// Read-write access
pub const OBMM_SHM_MEM_READWRITE: u8 = 0x8;

/// No access
pub const OBMM_SHM_MEM_NO_ACCESS: u8 = 0xc;

/// Access type mask
pub const OBMM_SHM_MEM_ACCESS_MASK: u8 = 0b1100;

// ============================================================================
// Cache Operations
// ============================================================================

/// No cache maintenance
pub const OBMM_SHM_CACHE_NONE: u8 = 0x0;

/// Invalidate only
pub const OBMM_SHM_CACHE_INVAL: u8 = 0x1;

/// Write back and invalidate
pub const OBMM_SHM_CACHE_WB_INVAL: u8 = 0x2;

/// Write back only
pub const OBMM_SHM_CACHE_WB_ONLY: u8 = 0x3;

/// Automatically choose cache maintenance action
pub const OBMM_SHM_CACHE_INFER: u8 = 0x4;

// ============================================================================
// MMAP Flags
// ============================================================================

/// Use huge pages for mmap
pub const OBMM_MMAP_FLAG_HUGETLB_PMD: u64 = 1 << 63;

// ============================================================================
// IOCTL Magic Numbers
// ============================================================================

/// Main OBMM ioctl magic number
pub const OBMM_MAGIC: u32 = b'x' as u32;

/// OBMM device update range ioctl magic number
pub const OBMM_SHM_MAGIC: u32 = b'X' as u32;

// ============================================================================
// IOCTL Commands
// ============================================================================

/// Export memory
pub const OBMM_CMD_EXPORT: u32 = _iowr::<ObmmCmdExport>(OBMM_MAGIC, 0);

/// Import memory
pub const OBMM_CMD_IMPORT: u32 = _iowr::<ObmmCmdImport>(OBMM_MAGIC, 1);

/// Unexport memory
pub const OBMM_CMD_UNEXPORT: u32 = _iow::<ObmmCmdUnexport>(OBMM_MAGIC, 2);

/// Unimport memory
pub const OBMM_CMD_UNIMPORT: u32 = _iow::<ObmmCmdUnimport>(OBMM_MAGIC, 3);

/// Query address by PA or memid+offset
pub const OBMM_CMD_ADDR_QUERY: u32 = _iowr::<ObmmCmdAddrQuery>(OBMM_MAGIC, 4);

/// Export user address space
pub const OBMM_CMD_EXPORT_PID: u32 = _iowr::<ObmmCmdExportPid>(OBMM_MAGIC, 5);

/// Declare preimport
pub const OBMM_CMD_DECLARE_PREIMPORT: u32 = _iowr::<ObmmCmdPreimport>(OBMM_MAGIC, 6);

/// Undeclare preimport
pub const OBMM_CMD_UNDECLARE_PREIMPORT: u32 = _iow::<ObmmCmdPreimport>(OBMM_MAGIC, 7);

/// Update memory range attributes
pub const OBMM_SHMDEV_UPDATE_RANGE: u32 = _iow::<ObmmCmdUpdateRange>(OBMM_SHM_MAGIC, 0);

// ============================================================================
// Query Key Types
// ============================================================================

/// Query type: query by physical address
pub const OBMM_QUERY_BY_PA: u32 = 0;

/// Query type: query by memory ID and offset
pub const OBMM_QUERY_BY_ID_OFFSET: u32 = 1;

// ============================================================================
// Kernel Command Structures
// ============================================================================

/// Export command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ObmmCmdExport {
    /// Size per NUMA node
    pub size: [u64; OBMM_MAX_LOCAL_NUMA_NODES],
    /// Number of NUMA nodes
    pub length: u64,
    /// Export flags
    pub flags: u64,
    /// Output: base address (uba)
    pub uba: u64,
    /// Output: memory ID
    pub mem_id: u64,
    /// Token ID (input for register, output for export)
    pub tokenid: u32,
    /// Target NUMA node
    pub pxm_numa: i32,
    /// Private data length
    pub priv_len: u16,
    /// Vendor data length
    pub vendor_len: u16,
    /// Destination EID (128-bit)
    pub deid: [u8; 16],
    /// Source EID (128-bit)
    pub seid: [u8; 16],
    /// Vendor information pointer
    pub vendor_info: *const c_void,
    /// Private data pointer
    pub priv_data: *const c_void,
}

/// Export PID command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ObmmCmdExportPid {
    /// Virtual address
    pub va: *mut c_void,
    /// Length of region
    pub length: u64,
    /// Export flags
    pub flags: u64,
    /// Output: base address
    pub uba: u64,
    /// Output: memory ID
    pub mem_id: u64,
    /// Token ID
    pub tokenid: u32,
    /// Process ID
    pub pid: i32,
    /// Target NUMA node
    pub pxm_numa: i32,
    /// Private data length
    pub priv_len: u16,
    /// Vendor data length
    pub vendor_len: u16,
    /// Destination EID (128-bit)
    pub deid: [u8; 16],
    /// Source EID (128-bit)
    pub seid: [u8; 16],
    /// Private data pointer
    pub priv_data: *const c_void,
}

/// Unexport command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Default)]
pub struct ObmmCmdUnexport {
    /// Memory ID to unexport
    pub mem_id: u64,
    /// Unexport flags
    pub flags: u64,
}

/// Unimport command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Default)]
pub struct ObmmCmdUnimport {
    /// Memory ID to unimport
    pub mem_id: u64,
    /// Unimport flags
    pub flags: u64,
}

/// Address query command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ObmmCmdAddrQuery {
    /// Query key type (OBMM_QUERY_BY_PA or OBMM_QUERY_BY_ID_OFFSET)
    pub key_type: u32,
    pub _pad: u32,
    /// Memory ID (input/output)
    pub mem_id: u64,
    /// Offset (input/output)
    pub offset: u64,
    /// Physical address (input/output)
    pub pa: u64,
}

impl Default for ObmmCmdAddrQuery {
    fn default() -> Self {
        Self {
            key_type: 0,
            _pad: 0,
            mem_id: 0,
            offset: 0,
            pa: 0,
        }
    }
}

/// Import command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ObmmCmdImport {
    /// Import flags
    pub flags: u64,
    /// Output: memory ID
    pub mem_id: u64,
    /// Address
    pub addr: u64,
    /// Length
    pub length: u64,
    /// Token ID
    pub tokenid: u32,
    /// Source cluster/node address
    pub scna: u32,
    /// Destination cluster/node address
    pub dcna: u32,
    /// NUMA node ID (input/output)
    pub numa_id: i32,
    /// Private data length
    pub priv_len: u16,
    /// Base distance
    pub base_dist: u8,
    /// Destination EID (128-bit)
    pub deid: [u8; 16],
    /// Source EID (128-bit)
    pub seid: [u8; 16],
    /// Private data pointer
    pub priv_data: *const c_void,
}

/// Preimport command structure
///
/// Note: The kernel expects 16-byte alignment for this structure.
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct ObmmCmdPreimport {
    /// Physical address
    pub pa: u64,
    /// Length
    pub length: u64,
    /// Flags
    pub flags: u64,
    /// Source cluster/node address
    pub scna: u32,
    /// Destination cluster/node address
    pub dcna: u32,
    /// NUMA node ID (input/output)
    pub numa_id: i32,
    /// Private data length
    pub priv_len: u16,
    /// Base distance
    pub base_dist: u8,
    /// Destination EID (128-bit)
    pub deid: [u8; 16],
    /// Source EID (128-bit)
    pub seid: [u8; 16],
    /// Private data pointer
    pub priv_data: *const c_void,
}

/// Update range command structure
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Default)]
pub struct ObmmCmdUpdateRange {
    /// Start address (inclusive)
    pub start: u64,
    /// End address (exclusive)
    pub end: u64,
    /// Memory state
    pub mem_state: u8,
    /// Cache operations
    pub cache_ops: u8,
    pub _pad: [u8; 6],
}

// ============================================================================
// User API Structures (for public interface compatibility)
// ============================================================================

/// Memory descriptor for user API
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ObmmMemDesc {
    /// Address
    pub addr: u64,
    /// Length
    pub length: u64,
    /// Source EID (128-bit, little-endian)
    pub seid: [u8; 16],
    /// Destination EID (128-bit, little-endian)
    pub deid: [u8; 16],
    /// Token ID
    pub tokenid: u32,
    /// Source cluster/node address
    pub scna: u32,
    /// Destination cluster/node address
    pub dcna: u32,
    /// Private data length
    pub priv_len: u16,
    // Note: Private data (flexible array member) is handled separately
    // as the kernel API uses trailing variable-length data
}

/// Preimport information structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ObmmPreimportInfo {
    /// Physical address
    pub pa: u64,
    /// Length
    pub length: u64,
    /// Base distance
    pub base_dist: i32,
    /// NUMA node ID
    pub numa_id: i32,
    /// Source EID (128-bit)
    pub seid: [u8; 16],
    /// Destination EID (128-bit)
    pub deid: [u8; 16],
    /// Source cluster/node address
    pub scna: u32,
    /// Destination cluster/node address
    pub dcna: u32,
    /// Private data length
    pub priv_len: u16,
    // Note: Private data (flexible array member) is handled separately
}

// ============================================================================
// Type Aliases
// ============================================================================

/// Memory ID type
pub type MemId = u64;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_ioctl_constants() {
        // Verify ioctl numbers match expected values
        // These can be computed with: echo '#include <ub/obmm.h>' | gcc -E -dM - | grep OBMM_CMD
        assert_eq!(OBMM_MAGIC, b'x' as u32);
        assert_eq!(OBMM_SHM_MAGIC, b'X' as u32);
    }

    #[test]
    fn test_structure_sizes() {
        // Verify structure sizes are correct for 64-bit systems
        // Note: actual sizes may vary - these are sanity checks
        assert!(size_of::<ObmmCmdExport>() >= 200);
        assert!(size_of::<ObmmCmdExportPid>() >= 96);
        assert!(size_of::<ObmmCmdImport>() >= 88);
        assert!(size_of::<ObmmCmdAddrQuery>() >= 32);
        assert!(size_of::<ObmmCmdUnexport>() >= 16);
        assert!(size_of::<ObmmCmdUnimport>() >= 16);
        assert!(size_of::<ObmmCmdPreimport>() >= 80);
        assert!(size_of::<ObmmCmdUpdateRange>() >= 24);
    }

    #[test]
    fn test_structure_alignments() {
        // Verify 8-byte alignment for most structures
        assert!(align_of::<ObmmCmdExport>() >= 8);
        assert!(align_of::<ObmmCmdExportPid>() >= 8);
        assert!(align_of::<ObmmCmdImport>() >= 8);
        assert!(align_of::<ObmmCmdAddrQuery>() >= 8);
    }
}
