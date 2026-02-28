//! Pure Rust implementation of OBMM kernel interface
//!
//! This module provides the same functionality as the C library (`libobmm.so`)
//! but implemented directly in Rust using ioctl system calls to the OBMM kernel module.

use std::ffi::c_void;

use crate::device::with_device;
use crate::error::ObmmError;
use crate::kernel_abi::*;
use crate::types::{MemId, ObmmPreimportInfo};

/// Export memory regions for remote access
///
/// # Arguments
/// * `length` - Array of lengths for each NUMA node
/// * `flags` - Export flags
/// * `desc` - Output memory descriptor (ObmmMemDesc)
///
/// # Returns
/// Memory ID on success, `OBMM_INVALID_MEMID` on failure
///
/// # Safety
///
/// The caller must ensure that:
/// - `length` points to a valid array of at least `OBMM_MAX_LOCAL_NUMA_NODES` elements
/// - `desc` points to a valid, writable `ObmmMemDesc` structure
pub unsafe fn obmm_export(length: *const usize, flags: u64, desc: *mut c_void) -> MemId {
    if length.is_null() || desc.is_null() {
        return OBMM_INVALID_MEMID;
    }

    // Convert desc pointer to ObmmMemDesc for field access
    let mem_desc = unsafe { &mut *(desc as *mut crate::types::ObmmMemDesc<()>) };

    // Build kernel command structure
    let mut cmd = ObmmCmdExport {
        size: [0; OBMM_MAX_LOCAL_NUMA_NODES],
        length: 0,
        flags: flags & OBMM_EXPORT_FLAG_MASK,
        uba: 0,
        mem_id: 0,
        tokenid: mem_desc.tokenid,
        pxm_numa: -1,
        priv_len: mem_desc.priv_len,
        vendor_len: 0,
        deid: mem_desc.deid,
        seid: mem_desc.seid,
        vendor_info: std::ptr::null(),
        priv_data: std::ptr::null(),
    };

    // Copy lengths from input array
    unsafe {
        for i in 0..OBMM_MAX_LOCAL_NUMA_NODES {
            cmd.size[i] = *length.add(i) as u64;
            if cmd.size[i] > 0 {
                cmd.length += 1;
            }
        }
    }

    // Execute ioctl
    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_EXPORT as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::ExportFailed(e.to_string()))
    });

    match result {
        Ok(_) => {
            // Copy results back to desc
            mem_desc.addr = cmd.uba;
            mem_desc.length = cmd.size.iter().sum::<u64>();
            mem_desc.tokenid = cmd.tokenid;
            cmd.mem_id
        }
        Err(_) => OBMM_INVALID_MEMID,
    }
}

/// Unexport previously exported memory region
///
/// # Arguments
/// * `id` - Memory ID to unexport
/// * `flags` - Unexport flags
///
/// # Returns
/// 0 on success, -1 on failure
pub fn obmm_unexport(id: MemId, flags: u64) -> i32 {
    if id == OBMM_INVALID_MEMID {
        return -1;
    }

    let mut cmd = ObmmCmdUnexport {
        mem_id: id,
        flags: flags & OBMM_UNEXPORT_FLAG_MASK,
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_UNEXPORT as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::UnexportFailed(e.to_string()))
    });

    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Import remote memory region
///
/// # Arguments
/// * `desc` - Memory descriptor from remote
/// * `flags` - Import flags
/// * `base_dist` - Base distribution hint
/// * `numa` - Output NUMA node ID (can be null)
///
/// # Returns
/// Memory ID on success, `OBMM_INVALID_MEMID` on failure
///
/// # Safety
///
/// The caller must ensure that:
/// - `desc` points to a valid `ObmmMemDesc` structure
/// - `numa` is either null or points to a writable `i32`
pub unsafe fn obmm_import(
    desc: *const c_void,
    flags: u64,
    base_dist: i32,
    numa: *mut i32,
) -> MemId {
    if desc.is_null() {
        return OBMM_INVALID_MEMID;
    }

    // Convert desc pointer to ObmmMemDesc for field access
    let mem_desc = unsafe { &*(desc as *const crate::types::ObmmMemDesc<()>) };

    // Validate base_dist if NUMA_REMOTE flag is set without PREIMPORT
    if (flags & OBMM_IMPORT_FLAG_NUMA_REMOTE) != 0
        && (flags & OBMM_IMPORT_FLAG_PREIMPORT) == 0
        && !(0..=255).contains(&base_dist)
    {
        return OBMM_INVALID_MEMID;
    }

    let numa_id = if numa.is_null() { -1 } else { unsafe { *numa } };

    let mut cmd = ObmmCmdImport {
        flags: flags & OBMM_IMPORT_FLAG_MASK,
        mem_id: 0,
        addr: mem_desc.addr,
        length: mem_desc.length,
        tokenid: mem_desc.tokenid,
        scna: mem_desc.scna,
        dcna: mem_desc.dcna,
        numa_id,
        priv_len: mem_desc.priv_len,
        base_dist: base_dist as u8,
        deid: mem_desc.deid,
        seid: mem_desc.seid,
        priv_data: std::ptr::null(),
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_IMPORT as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::ImportFailed(e.to_string()))
    });

    match result {
        Ok(_) => {
            if !numa.is_null() {
                unsafe {
                    std::ptr::write(numa, cmd.numa_id);
                }
            }
            cmd.mem_id
        }
        Err(_) => OBMM_INVALID_MEMID,
    }
}

/// Unimport previously imported memory region
///
/// # Arguments
/// * `id` - Memory ID to unimport
/// * `flags` - Unimport flags
///
/// # Returns
/// 0 on success, -1 on failure
pub fn obmm_unimport(id: MemId, flags: u64) -> i32 {
    if id == OBMM_INVALID_MEMID {
        return -1;
    }

    let mut cmd = ObmmCmdUnimport {
        mem_id: id,
        flags: flags & OBMM_UNIMPORT_FLAG_MASK,
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_UNIMPORT as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::UnimportFailed(e.to_string()))
    });

    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Preimport memory region
///
/// # Arguments
/// * `info` - Preimport information structure
/// * `flags` - Preimport flags
///
/// # Returns
/// 0 on success, -1 on failure
///
/// # Safety
///
/// The caller must ensure that `info` points to a valid, writable
/// `ObmmPreimportInfo` structure.
pub unsafe fn obmm_preimport(info: *mut ObmmPreimportInfo, flags: u64) -> i32 {
    if info.is_null() {
        return -1;
    }

    let pre_info = unsafe { &*info };

    // Validate base_dist
    if !(0..=255).contains(&pre_info.base_dist) {
        return -1;
    }

    let mut cmd = ObmmCmdPreimport {
        pa: pre_info.pa,
        length: pre_info.length,
        flags: flags & OBMM_PREIMPORT_FLAG_MASK,
        scna: pre_info.scna,
        dcna: pre_info.dcna,
        numa_id: pre_info.numa_id,
        priv_len: pre_info.priv_len,
        base_dist: pre_info.base_dist as u8,
        deid: pre_info.deid,
        seid: pre_info.seid,
        priv_data: std::ptr::null(),
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_DECLARE_PREIMPORT as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::ImportFailed(e.to_string()))
    });

    match result {
        Ok(_) => {
            // Update numa_id in the original struct
            unsafe {
                (*info).numa_id = cmd.numa_id;
            }
            0
        }
        Err(_) => -1,
    }
}

/// Unpreimport previously preimported memory region
///
/// # Arguments
/// * `info` - Preimport information structure
/// * `flags` - Unpreimport flags
///
/// # Returns
/// 0 on success, -1 on failure
///
/// # Safety
///
/// The caller must ensure that `info` points to a valid `ObmmPreimportInfo` structure.
pub unsafe fn obmm_unpreimport(info: *const ObmmPreimportInfo, flags: u64) -> i32 {
    if info.is_null() {
        return -1;
    }

    let pre_info = unsafe { &*info };

    let cmd = ObmmCmdPreimport {
        pa: pre_info.pa,
        length: pre_info.length,
        flags: flags & OBMM_UNPREIMPORT_FLAG_MASK,
        scna: pre_info.scna,
        dcna: pre_info.dcna,
        numa_id: pre_info.numa_id,
        priv_len: pre_info.priv_len,
        base_dist: pre_info.base_dist as u8,
        deid: pre_info.deid,
        seid: pre_info.seid,
        priv_data: std::ptr::null(),
    };

    // Note: OBMM_CMD_UNDECLARE_PREIMPORT uses _IOW (write only), so we pass
    // a const pointer by casting to a mutable pointer (the kernel won't modify it)
    let cmd_ptr: *mut ObmmCmdPreimport = &cmd as *const _ as *mut _;
    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_UNDECLARE_PREIMPORT as libc::c_ulong, cmd_ptr) }
            .map_err(|e| ObmmError::UnimportFailed(e.to_string()))
    });

    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Export user address space
///
/// Exports a user-space virtual memory address range for OBMM management
/// and remote access.
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
///
/// # Safety
///
/// The caller must ensure that:
/// - `va` is a valid virtual address in the target process
/// - `desc` points to a valid, writable `ObmmMemDesc` structure
pub unsafe fn obmm_export_useraddr(
    pid: i32,
    va: *mut c_void,
    length: usize,
    flags: u64,
    desc: *mut c_void,
) -> MemId {
    if desc.is_null() {
        return OBMM_INVALID_MEMID;
    }

    // Convert desc pointer to ObmmMemDesc for field access
    let mem_desc = unsafe { &mut *(desc as *mut crate::types::ObmmMemDesc<()>) };

    let mut cmd = ObmmCmdExportPid {
        va,
        length: length as u64,
        flags: flags & OBMM_EXPORT_FLAG_MASK,
        uba: 0,
        mem_id: 0,
        tokenid: mem_desc.tokenid,
        pid,
        pxm_numa: -1,
        priv_len: mem_desc.priv_len,
        vendor_len: 0,
        deid: mem_desc.deid,
        seid: mem_desc.seid,
        priv_data: std::ptr::null(),
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_EXPORT_PID as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::ExportFailed(e.to_string()))
    });

    match result {
        Ok(_) => {
            mem_desc.addr = cmd.uba;
            mem_desc.length = length as u64;
            mem_desc.tokenid = cmd.tokenid;
            cmd.mem_id
        }
        Err(_) => OBMM_INVALID_MEMID,
    }
}

/// Set ownership of a memory region
///
/// Sets the ownership (read, write, none) of a range of OBMM virtual
/// address space using memory protection bits.
///
/// # Arguments
/// * `_fd` - File descriptor of OBMM memory device (unused in pure Rust impl)
/// * `start` - Start virtual address
/// * `end` - End virtual address
/// * `prot` - Protection bits (PROT_NONE=0, PROT_READ=1, PROT_WRITE=2)
///
/// # Returns
/// 0 on success, -1 on failure
///
/// # Safety
///
/// The caller must ensure that `start` and `end` are valid virtual addresses.
pub unsafe fn obmm_set_ownership(_fd: i32, start: *mut c_void, end: *mut c_void, prot: i32) -> i32 {
    if start.is_null() || end.is_null() {
        return -1;
    }

    // Convert protection bits to memory state
    let mem_state = match prot {
        0 => OBMM_SHM_MEM_NORMAL_NC | OBMM_SHM_MEM_NO_ACCESS,
        1 => OBMM_SHM_MEM_NORMAL | OBMM_SHM_MEM_READONLY,
        2 | 3 => OBMM_SHM_MEM_NORMAL | OBMM_SHM_MEM_READWRITE,
        _ => return -1,
    };

    let mut cmd = ObmmCmdUpdateRange {
        start: start as u64,
        end: end as u64,
        mem_state,
        cache_ops: OBMM_SHM_CACHE_INFER,
        _pad: [0; 6],
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_SHMDEV_UPDATE_RANGE as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::OwnershipFailed(e.to_string()))
    });

    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Query memory ID by physical address
///
/// # Arguments
/// * `pa` - Physical address
/// * `id` - Output memory ID
/// * `offset` - Output offset within memory region
///
/// # Returns
/// 0 on success, -1 on failure
///
/// # Safety
///
/// The caller must ensure that:
/// - `id` is either null or points to a writable `MemId`
/// - `offset` is either null or points to a writable `u64`
pub unsafe fn obmm_query_memid_by_pa(pa: u64, id: *mut MemId, offset: *mut u64) -> i32 {
    if id.is_null() && offset.is_null() {
        return -1;
    }

    let mut cmd = ObmmCmdAddrQuery {
        key_type: OBMM_QUERY_BY_PA,
        _pad: 0,
        mem_id: 0,
        offset: 0,
        pa,
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_ADDR_QUERY as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::QueryFailed(e.to_string()))
    });

    match result {
        Ok(_) => {
            if !id.is_null() {
                unsafe {
                    std::ptr::write(id, cmd.mem_id);
                }
            }
            if !offset.is_null() {
                unsafe {
                    std::ptr::write(offset, cmd.offset);
                }
            }
            0
        }
        Err(_) => -1,
    }
}

/// Query physical address by memory ID and offset
///
/// # Arguments
/// * `id` - Memory ID
/// * `offset` - Offset within memory region
/// * `pa` - Output physical address
///
/// # Returns
/// 0 on success, -1 on failure
///
/// # Safety
///
/// The caller must ensure that `pa` is either null or points to a writable `u64`.
pub unsafe fn obmm_query_pa_by_memid(id: MemId, offset: u64, pa: *mut u64) -> i32 {
    if pa.is_null() {
        return -1;
    }

    let mut cmd = ObmmCmdAddrQuery {
        key_type: OBMM_QUERY_BY_ID_OFFSET,
        _pad: 0,
        mem_id: id,
        offset,
        pa: 0,
    };

    let result = with_device(|dev| {
        unsafe { dev.ioctl(OBMM_CMD_ADDR_QUERY as libc::c_ulong, &mut cmd) }
            .map_err(|e| ObmmError::QueryFailed(e.to_string()))
    });

    match result {
        Ok(_) => {
            unsafe {
                std::ptr::write(pa, cmd.pa);
            }
            0
        }
        Err(_) => -1,
    }
}
