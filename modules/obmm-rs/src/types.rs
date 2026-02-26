//! Type definitions for OBMM (Ownership-Based Memory Management)
//!
//! This module provides constants, type aliases, bitflags, and structures
//! used throughout the OBMM library.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

/// Maximum number of NUMA nodes supported
pub const MAX_NUMA_NODES: usize = 16;

/// Invalid memory ID constant
pub const OBMM_INVALID_MEMID: u64 = 0;

/// Maximum number of local NUMA nodes supported
pub const OBMM_MAX_LOCAL_NUMA_NODES: usize = 16;

/// Memory ID type
pub type MemId = u64;

bitflags! {
    /// Privilege data for UB memory regions
    #[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
    #[serde(transparent)]
    pub struct UbPrivData: u16 {
        /// Owner Chip ID
        const OCHIP = 1 << 5;
        /// Cacheable flag
        const CACHEABLE = 1 << 6;
    }
}

bitflags! {
    /// Export flags for memory exporting
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObmmExportFlags: u64 {
        /// Allow memory mapping
        const ALLOWMMAP = 1 << 0;
        /// Export to remote NUMA nodes
        const REMOTENUMA = 1 << 1;
    }
}

bitflags! {
    /// Unexport flags for memory unexporting
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObmmUnexportFlags: u64 {
        /// Force unexport
        const FORCE = 1 << 0;
    }
}

bitflags! {
    /// Preimport flags for memory preimporting
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObmmPreimportFlags: u64 {
        /// Allow memory mapping for preimported region
        const ALLOWMMAP = 1 << 0;
    }
}

/// Memory descriptor structure
///
/// This structure describes a memory region for OBMM operations including
/// export, import, and management of memory.
#[repr(C)]
#[derive(Default, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ObmmMemDesc<T> {
    /// Base address of the memory region
    pub addr: u64,
    /// Length of the memory region
    pub length: u64,
    /// 128bit eid, ordered by little-endian
    pub seid: [u8; 16],
    /// 128bit deid, ordered by little-endian
    pub deid: [u8; 16],
    /// Token ID
    pub tokenid: u32,
    /// Source CNA
    pub scna: u32,
    /// Destination CNA
    pub dcna: u32,
    /// Length of privilege data
    pub priv_len: u16,
    /// Privilege data
    pub priv_data: T,
}

impl<T> ObmmMemDesc<T>
where
    T: Default + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new `ObmmMemDesc` with default values
    ///
    /// # Returns
    /// A new `ObmmMemDesc` instance with all fields set to their default values
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        ObmmMemDesc::<T>::default()
    }

    /// Deserialize the `ObmmMemDesc` from json format
    ///
    /// # Arguments
    /// * `json_str` - JSON string representation
    ///
    /// # Returns
    /// `ObmmMemDesc` on success, `anyhow::Error` on failure
    ///
    /// # Errors
    /// Returns an error if the JSON string is invalid or cannot be deserialized
    #[inline]
    pub fn from_json(json_str: &str) -> anyhow::Result<Self> {
        let desc: ObmmMemDesc<T> = serde_json::from_str(json_str)?;
        Ok(desc)
    }

    /// Serialize the `ObmmMemDesc` to json format
    ///
    /// # Returns
    /// JSON string on success, `anyhow::Error` on failure
    ///
    /// # Errors
    /// Returns an error if serialization fails
    #[inline]
    pub fn to_json(&self) -> anyhow::Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }

    /// Read the `ObmmMemDesc` from a json file
    ///
    /// # Arguments
    /// * `mem_id` - Memory ID used to construct the filename
    ///
    /// # Returns
    /// `ObmmMemDesc` on success, `anyhow::Error` on failure
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or the JSON is invalid
    #[inline]
    pub fn from_json_file(mem_id: MemId) -> anyhow::Result<Self> {
        let file_path = format!("/tmp/memlink/memdesc_{mem_id}.json");
        let json_str = std::fs::read_to_string(file_path)?;
        let desc: ObmmMemDesc<T> = serde_json::from_str(&json_str)?;
        Ok(desc)
    }

    /// Write the `ObmmMemDesc` to a json file
    ///
    /// # Arguments
    /// * `mem_id` - Memory ID used to construct the filename
    ///
    /// # Returns
    /// `Ok(())` on success, `anyhow::Error` on failure
    ///
    /// # Errors
    /// Returns an error if the file cannot be written or serialization fails
    #[inline]
    pub fn to_json_file(&self, mem_id: MemId) -> anyhow::Result<()> {
        let file_path = format!("/tmp/memlink/memdesc_{mem_id}.json");
        let json_str = serde_json::to_string_pretty(self)?;
        std::fs::write(file_path, json_str)?;
        Ok(())
    }
}

/// Preimport information structure
///
/// This structure contains information needed for memory preimport operations.
/// It matches the C struct layout used by the OBMM kernel interface.
///
/// # Note
/// The C struct has a flexible array member `priv[]` at the end. In Rust,
/// we omit this field and use `std::mem::size_of::<ObmmPreimportInfo>()` to
/// get the base size. For operations requiring priv data, use manual allocation.
#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ObmmPreimportInfo {
    /// Physical address for preimport
    pub pa: u64,
    /// Length of the preimport region
    pub length: u64,
    /// Base distribution hint
    pub base_dist: i32,
    /// NUMA node ID for the preimport
    pub numa_id: i32,
    /// Source EID (128-bit, little-endian)
    pub seid: [u8; 16],
    /// Destination EID (128-bit, little-endian)
    pub deid: [u8; 16],
    /// Source CNA
    pub scna: u32,
    /// Destination CNA
    pub dcna: u32,
    /// Length of privilege data (flexible array in C)
    pub priv_len: u16,
}

/// Import result structure
///
/// Contains the result of a memory import operation including the
/// assigned memory ID and the NUMA node where the memory was placed.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImportResult {
    /// The memory ID assigned to the imported region
    pub mem_id: MemId,
    /// The NUMA node where the memory was placed
    pub numa_node: i32,
}

/// Query result structure
///
/// Contains the result of a memory query operation.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct QueryResult {
    /// Memory ID (for query by physical address)
    pub mem_id: MemId,
    /// Offset within the memory region (for query by physical address)
    pub offset: u64,
    /// Physical address (for query by memory ID)
    pub phys_addr: u64,
}
