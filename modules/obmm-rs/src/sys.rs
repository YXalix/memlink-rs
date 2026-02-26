//! FFI bindings for OBMM (Ownership-Based Memory Management)
//!
//! This module provides unsafe FFI bindings to the OBMM C library.
//! These functions directly interface with the kernel OBMM subsystem.

use std::ffi::c_void;

use crate::types::{MemId, ObmmPreimportInfo};

// FFI bindings to OBMM C library
unsafe extern "C" {
    /// Export memory regions for remote access
    ///
    /// # Arguments
    /// * `length` - Array of lengths for each NUMA node
    /// * `flags` - Export flags
    /// * `desc` - Output memory descriptor
    ///
    /// # Returns
    /// Memory ID on success, `OBMM_INVALID_MEMID` on failure
    pub fn obmm_export(
        length: *const usize,
        flags: u64,
        desc: *mut c_void,
    ) -> MemId;

    /// Unexport previously exported memory region
    ///
    /// # Arguments
    /// * `id` - Memory ID to unexport
    /// * `flags` - Unexport flags
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_unexport(id: MemId, flags: u64) -> i32;

    /// Import remote memory region
    ///
    /// # Arguments
    /// * `desc` - Memory descriptor from remote
    /// * `flags` - Import flags
    /// * `base_dist` - Base distribution hint
    /// * `numa` - Output NUMA node ID
    ///
    /// # Returns
    /// Memory ID on success, `OBMM_INVALID_MEMID` on failure
    pub fn obmm_import(
        desc: *const c_void,
        flags: u64,
        base_dist: i32,
        numa: *mut i32,
    ) -> MemId;

    /// Unimport previously imported memory region
    ///
    /// # Arguments
    /// * `id` - Memory ID to unimport
    /// * `flags` - Unimport flags
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_unimport(id: MemId, flags: u64) -> i32;

    /// Preimport memory region
    ///
    /// Preimports a memory region before actual use, allowing for
    /// pre-allocation and setup of memory resources.
    ///
    /// # Arguments
    /// * `info` - Preimport information structure
    /// * `flags` - Preimport flags
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_preimport(
        info: *mut ObmmPreimportInfo,
        flags: u64,
    ) -> i32;

    /// Unpreimport previously preimported memory region
    ///
    /// # Arguments
    /// * `info` - Preimport information structure
    /// * `flags` - Unpreimport flags
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_unpreimport(
        info: *const ObmmPreimportInfo,
        flags: u64,
    ) -> i32;

    /// Export user address space
    ///
    /// Exports a user-space virtual memory address range for
    /// OBMM management and remote access.
    ///
    /// # Arguments
    /// * `pid` - Process ID (0 for current process)
    /// * `va` - Virtual address to export
    /// * `length` - Length of the region
    /// * `flags` - Export flags
    /// * `desc` - Output memory descriptor
    ///
    /// # Returns
    /// Memory ID on success, `OBMM_INVALID_MEMID` on failure
    pub fn obmm_export_useraddr(
        pid: i32,
        va: *mut c_void,
        length: usize,
        flags: u64,
        desc: *mut c_void,
    ) -> MemId;

    /// Set ownership of a memory region
    ///
    /// Sets the ownership (read, write, none) of a range of OBMM virtual
    /// address space using memory protection bits (`PROT_NONE`, `PROT_READ`, `PROT_WRITE`).
    ///
    /// # Arguments
    /// * `fd` - File descriptor of OBMM memory device
    /// * `start` - Start virtual address
    /// * `end` - End virtual address
    /// * `prot` - Protection bits (`PROT_NONE=0`, `PROT_READ=1`, `PROT_WRITE=2`)
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_set_ownership(
        fd: i32,
        start: *mut c_void,
        end: *mut c_void,
        prot: i32,
    ) -> i32;

    /* debug interface */

    /// Query memory ID by physical address
    ///
    /// # Arguments
    /// * `pa` - Physical address
    /// * `id` - Output memory ID
    /// * `offset` - Output offset within memory region
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_query_memid_by_pa(
        pa: u64,
        id: *mut MemId,
        offset: *mut u64,
    ) -> i32;

    /// Query physical address by memory ID and offset
    ///
    /// # Arguments
    /// * `id` - Memory ID
    /// * `offset` - Offset within memory region
    /// * `pa` - Output physical address
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn obmm_query_pa_by_memid(
        id: MemId,
        offset: u64,
        pa: *mut u64,
    ) -> i32;
}
