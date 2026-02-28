//! System interface for OBMM (Ownership-Based Memory Management)
//!
//! This module provides the low-level interface to the OBMM kernel module.
//!
//! # Implementation Note
//!
//! This module is now a thin wrapper around the pure Rust [`kernel`] module
//! which directly communicates with the OBMM kernel module via ioctl system calls.
//! The C library (`libobmm.so`) is no longer required.
//!
//! For backward compatibility, this module re-exports the same functions with
//! the same signatures as the previous FFI-based implementation.

// Re-export all kernel functions when native feature is enabled
#[cfg(feature = "native")]
pub use crate::kernel::{
    obmm_export, obmm_export_useraddr, obmm_import, obmm_preimport, obmm_query_memid_by_pa,
    obmm_query_pa_by_memid, obmm_set_ownership, obmm_unexport, obmm_unimport, obmm_unpreimport,
};

// Stub implementations when native feature is disabled
#[cfg(not(feature = "native"))]
mod stubs {
    use std::ffi::c_void;

    use crate::types::{MemId, ObmmPreimportInfo};

    /// Stub implementation of obmm_export
    pub fn obmm_export(_length: *const usize, _flags: u64, _desc: *mut c_void) -> MemId {
        1 // Return a dummy valid mem_id
    }

    /// Stub implementation of obmm_unexport
    pub fn obmm_unexport(_id: MemId, _flags: u64) -> i32 {
        0 // Success
    }

    /// Stub implementation of obmm_import
    pub fn obmm_import(_desc: *const c_void, _flags: u64, _base_dist: i32, _numa: *mut i32) -> MemId {
        1 // Return a dummy valid mem_id
    }

    /// Stub implementation of obmm_unimport
    pub fn obmm_unimport(_id: MemId, _flags: u64) -> i32 {
        0 // Success
    }

    /// Stub implementation of obmm_preimport
    pub fn obmm_preimport(_info: *mut ObmmPreimportInfo, _flags: u64) -> i32 {
        0 // Success
    }

    /// Stub implementation of obmm_unpreimport
    pub fn obmm_unpreimport(_info: *const ObmmPreimportInfo, _flags: u64) -> i32 {
        0 // Success
    }

    /// Stub implementation of obmm_export_useraddr
    pub fn obmm_export_useraddr(
        _pid: i32,
        _va: *mut c_void,
        _length: usize,
        _flags: u64,
        _desc: *mut c_void,
    ) -> MemId {
        1 // Return a dummy valid mem_id
    }

    /// Stub implementation of obmm_set_ownership
    pub fn obmm_set_ownership(_fd: i32, _start: *mut c_void, _end: *mut c_void, _prot: i32) -> i32 {
        0 // Success
    }

    /// Stub implementation of obmm_query_memid_by_pa
    pub fn obmm_query_memid_by_pa(_pa: u64, id: *mut MemId, offset: *mut u64) -> i32 {
        if !id.is_null() {
            unsafe { *id = 1 };
        }
        if !offset.is_null() {
            unsafe { *offset = 0 };
        }
        0 // Success
    }

    /// Stub implementation of obmm_query_pa_by_memid
    pub fn obmm_query_pa_by_memid(_id: MemId, _offset: u64, pa: *mut u64) -> i32 {
        if !pa.is_null() {
            unsafe { *pa = 0x1000_0000 };
        }
        0 // Success
    }
}

#[cfg(not(feature = "native"))]
pub use stubs::*;
