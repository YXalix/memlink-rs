//! Export operations for OBMM (Ownership-Based Memory Management)
//!
//! This module provides safe wrappers for memory export operations including
//! standard memory export and user address space export.

#[cfg(feature = "native")]
use std::ffi::c_void;

use crate::error::{ObmmError, Result};
#[cfg(feature = "native")]
use crate::error::ToObmmResult;
#[cfg(feature = "native")]
use crate::sys;
use crate::types::{MemId, ObmmExportFlags, ObmmMemDesc, ObmmUnexportFlags, OBMM_INVALID_MEMID};

/// Export memory region
///
/// Exports memory regions for remote access across NUMA nodes.
///
/// # Arguments
/// * `length` - Array of lengths for each NUMA node (index 0 = NUMA node 0, etc.)
/// * `flags` - Export flags controlling the export behavior
///
/// # Returns
/// A tuple containing:
/// - The memory ID assigned to the exported region
/// - The memory descriptor containing metadata about the export
///
/// # Errors
/// Returns `ObmmError::ExportFailed` if the export operation fails
///
/// # Example
/// ```
/// use obmm_rs::{export::mem_export, types::{ObmmExportFlags, UbPrivData}};
///
/// let mut lengths = vec![0; 16];
/// lengths[0] = 1024 * 1024 * 64; // 64MB on NUMA node 0
/// let flags = ObmmExportFlags::ALLOWMMAP;
///
/// match mem_export::<UbPrivData>(&lengths, flags) {
///     Ok((mem_id, desc)) => println!("Exported memory ID: {}", mem_id),
///     Err(e) => eprintln!("Export failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn mem_export<T: Default>(
    length: &[usize],
    _: ObmmExportFlags,
) -> anyhow::Result<(MemId, ObmmMemDesc<T>)> {
    let mut desc = ObmmMemDesc::<T>::default();
    // Hooked implementation for testing
    let memid = 1;
    desc.addr = 0xffff_fc00_0000;
    desc.length = length.iter().sum::<usize>().try_into()?;
    if memid == OBMM_INVALID_MEMID {
        Err(anyhow::anyhow!("Failed to export memory"))
    } else {
        Ok((memid, desc))
    }
}

/// Export memory region (real implementation)
///
/// Exports memory regions for remote access across NUMA nodes.
/// Uses the actual OBMM C library to communicate with the kernel.
///
/// # Arguments
/// * `length` - Array of lengths for each NUMA node (index 0 = NUMA node 0, etc.)
/// * `flags` - Export flags controlling the export behavior
///
/// # Returns
/// A tuple containing:
/// - The memory ID assigned to the exported region
/// - The memory descriptor containing metadata about the export
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The export operation fails (e.g., insufficient memory)
/// - The flags are invalid
#[cfg(feature = "native")]
#[inline]
pub fn mem_export<T: Default>(
    length: &[usize],
    flags: ObmmExportFlags,
) -> anyhow::Result<(MemId, ObmmMemDesc<T>)> {
    let mut desc = ObmmMemDesc::<T>::default();
    let desc_ptr = std::ptr::addr_of_mut!(desc);
    let memid = unsafe {
        sys::obmm_export(
            length.as_ptr(),
            flags.bits(),
            desc_ptr.cast::<c_void>(),
        )
    };
    if memid == OBMM_INVALID_MEMID {
        Err(anyhow::anyhow!("Failed to export memory"))
    } else {
        Ok((memid, desc))
    }
}

/// Unexport memory region
///
/// Unexports a previously exported memory region, making it unavailable
/// for remote access.
///
/// # Arguments
/// * `mem_id` - Memory ID to unexport
/// * `flags` - Unexport flags (e.g., `ObmmUnexportFlags::FORCE`)
///
/// # Errors
/// Returns `ObmmError::UnexportFailed` if the unexport operation fails
///
/// # Example
/// ```
/// use obmm_rs::{export::mem_unexport, types::{ObmmUnexportFlags}};
///
/// let mem_id = 12345;
/// match mem_unexport(mem_id, ObmmUnexportFlags::empty()) {
///     Ok(()) => println!("Successfully unexported"),
///     Err(e) => eprintln!("Unexport failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn mem_unexport(_: MemId, _: ObmmUnexportFlags) -> Result<()> {
    // Hooked implementation for testing
    Ok(())
}

/// Unexport memory region (real implementation)
///
/// Unexports a previously exported memory region using the actual OBMM C library.
///
/// # Arguments
/// * `mem_id` - Memory ID to unexport
/// * `flags` - Unexport flags (e.g., `ObmmUnexportFlags::FORCE`)
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The memory ID is invalid or not exported
/// - The unexport operation fails (e.g., memory still in use)
#[cfg(feature = "native")]
#[inline]
pub fn mem_unexport(mem_id: MemId, flags: ObmmUnexportFlags) -> Result<()> {
    let ret = unsafe { sys::obmm_unexport(mem_id, flags.bits()) };
    ret.to_obmm_result(ObmmError::UnexportFailed)
}

/// Export user address space
///
/// Exports a specific virtual address range of a process for remote access.
/// Due to hardware limitations, this allocates and pins physical memory
/// for the VA range and verifies 2M page alignment.
///
/// # Arguments
/// * `pid` - Process ID (0 for current process)
/// * `va` - Virtual address to export
/// * `length` - Length of the region in bytes
/// * `flags` - Export flags
///
/// # Returns
/// A tuple containing:
/// - The memory ID assigned to the exported region
/// - The memory descriptor containing metadata
///
/// # Errors
/// Returns `ObmmError::InvalidMemId` if the export operation fails
///
/// # Safety
/// The virtual address range must be valid and accessible in the target process.
///
/// # Example
/// ```
/// use obmm_rs::export::export_useraddr;
/// use obmm_rs::types::{ObmmExportFlags, UbPrivData};
///
/// let va = 0x7fff_0000_0000;
/// let length = 1024 * 1024 * 2; // 2MB
/// let flags = ObmmExportFlags::ALLOWMMAP;
///
/// match export_useraddr::<UbPrivData>(0, va, length, flags) {
///     Ok((mem_id, desc)) => println!("Exported user address as ID: {}", mem_id),
///     Err(e) => eprintln!("Export failed: {}", e),
/// }
#[cfg(not(feature = "native"))]
#[inline]
pub fn export_useraddr<T: Default>(
    _pid: i32,
    _va: u64,
    length: usize,
    _: ObmmExportFlags,
) -> Result<(MemId, ObmmMemDesc<T>)> {
    // Hooked implementation for testing
    let mut desc = ObmmMemDesc::<T>::default();
    let memid = 1;
    desc.addr = 0x7fff_fc00_0000;
    desc.length = length.try_into().map_err(|_e| ObmmError::InvalidInput("length too large"))?;
    if memid == OBMM_INVALID_MEMID {
        Err(ObmmError::InvalidMemId)
    } else {
        Ok((memid, desc))
    }
}

/// Export user address space (real implementation)
///
/// Exports a specific virtual address range using the actual OBMM C library.
///
/// # Arguments
/// * `pid` - Process ID (0 for current process)
/// * `va` - Virtual address to export
/// * `length` - Length of the region in bytes
/// * `flags` - Export flags
///
/// # Returns
/// A tuple containing:
/// - The memory ID assigned to the exported region
/// - The memory descriptor containing metadata
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The virtual address is invalid or not accessible
/// - The export operation fails (e.g., insufficient memory)
#[cfg(feature = "native")]
#[inline]
#[allow(clippy::as_conversions)]
pub fn export_useraddr<T: Default>(
    pid: i32,
    va: u64,
    length: usize,
    flags: ObmmExportFlags,
) -> Result<(MemId, ObmmMemDesc<T>)> {
    let mut desc = ObmmMemDesc::<T>::default();
    let desc_ptr = std::ptr::addr_of_mut!(desc);
    let memid = unsafe {
        sys::obmm_export_useraddr(
            pid,
            va as *mut c_void,
            length,
            flags.bits(),
            desc_ptr.cast::<c_void>(),
        )
    };
    if memid == OBMM_INVALID_MEMID {
        Err(ObmmError::InvalidMemId)
    } else {
        Ok((memid, desc))
    }
}
