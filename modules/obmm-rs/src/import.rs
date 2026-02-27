//! Import operations for OBMM (Ownership-Based Memory Management)
//!
//! This module provides safe wrappers for memory import operations including
//! standard memory import, preimport, and their unimport counterparts.

#[cfg(feature = "native")]
use std::ffi::c_void;

use crate::error::{ObmmError, Result};
#[cfg(feature = "native")]
use crate::error::ToObmmResult;
#[cfg(feature = "native")]
use crate::sys;
use crate::types::{
    ImportResult, MemId, ObmmExportFlags, ObmmMemDesc, ObmmPreimportFlags, ObmmPreimportInfo,
    UbPrivData, OBMM_INVALID_MEMID,
};

/// Import memory region
///
/// Imports a remote memory region described by a memory descriptor.
///
/// # Arguments
/// * `desc` - Memory descriptor from the remote export
/// * `flags` - Import flags
/// * `base_dist` - Base distribution hint for NUMA placement
///
/// # Returns
/// An `ImportResult` containing:
/// - The memory ID assigned to the imported region
/// - The NUMA node where the memory was placed
///
/// # Errors
/// Returns `ObmmError::ImportFailed` if the import operation fails
///
/// # Example
/// ```
/// use obmm_rs::import::mem_import;
/// use obmm_rs::types::{ObmmMemDesc, ObmmExportFlags, UbPrivData};
///
/// let desc = ObmmMemDesc::<UbPrivData>::default();
/// let flags = ObmmExportFlags::ALLOWMMAP;
///
/// match mem_import(&desc, flags, 0) {
///     Ok(result) => println!("Imported to NUMA node {}", result.numa_node),
///     Err(e) => eprintln!("Import failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn mem_import(
    _: &ObmmMemDesc<UbPrivData>,
    _: ObmmExportFlags,
    _: i32,
) -> Result<ImportResult> {
    // Hooked implementation for testing
    let memid = 1;
    let numa = 0;
    if memid == OBMM_INVALID_MEMID {
        Err(ObmmError::ImportFailed(-1))
    } else {
        Ok(ImportResult {
            mem_id: memid,
            numa_node: numa,
        })
    }
}

/// Import memory region (real implementation)
///
/// Imports a remote memory region using the actual OBMM C library.
///
/// # Arguments
/// * `desc` - Memory descriptor from the remote export
/// * `flags` - Import flags
/// * `base_dist` - Base distribution hint for NUMA placement
///
/// # Returns
/// An `ImportResult` containing:
/// - The memory ID assigned to the imported region
/// - The NUMA node where the memory was placed
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The memory descriptor is invalid
/// - The import operation fails (e.g., insufficient memory)
#[cfg(feature = "native")]
#[inline]
pub fn mem_import(
    desc: &ObmmMemDesc<UbPrivData>,
    flags: ObmmExportFlags,
    base_dist: i32,
) -> Result<ImportResult> {
    let mut numa: i32 = -1;
    let desc_ptr = std::ptr::addr_of!(*desc);
    let numa_ptr = std::ptr::addr_of_mut!(numa);
    let memid = unsafe {
        sys::obmm_import(
            desc_ptr.cast::<c_void>(),
            flags.bits(),
            base_dist,
            numa_ptr,
        )
    };
    if memid == OBMM_INVALID_MEMID {
        Err(ObmmError::ImportFailed(-1))
    } else {
        Ok(ImportResult {
            mem_id: memid,
            numa_node: numa,
        })
    }
}

/// Unimport memory region
///
/// Unimports a previously imported memory region.
///
/// # Arguments
/// * `mem_id` - Memory ID to unimport
/// * `flags` - Unimport flags
///
/// # Errors
/// Returns `ObmmError::UnimportFailed` if the unimport operation fails
///
/// # Example
/// ```
/// use obmm_rs::import::mem_unimport;
/// use obmm_rs::types::ObmmExportFlags;
///
/// let mem_id = 12345;
/// match mem_unimport(mem_id, ObmmExportFlags::empty()) {
///     Ok(()) => println!("Successfully unimported"),
///     Err(e) => eprintln!("Unimport failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn mem_unimport(_: MemId, _: ObmmExportFlags) -> Result<()> {
    // Hooked implementation for testing
    Ok(())
}

/// Unimport memory region (real implementation)
///
/// Unimports a previously imported memory region using the actual OBMM C library.
///
/// # Arguments
/// * `mem_id` - Memory ID to unimport
/// * `flags` - Unimport flags
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The memory ID is invalid or not imported
/// - The unimport operation fails (e.g., memory still in use)
#[cfg(feature = "native")]
#[inline]
pub fn mem_unimport(mem_id: MemId, flags: ObmmExportFlags) -> Result<()> {
    let ret = unsafe { sys::obmm_unimport(mem_id, flags.bits()) };
    ret.to_obmm_result(ObmmError::UnimportFailed)
}

/// Preimport memory region
///
/// Preimports a memory region before actual use, allowing for
/// pre-allocation and setup of memory resources. This can improve
/// performance when the actual import is performed later.
///
/// # Arguments
/// * `info` - Preimport information structure containing region details
/// * `flags` - Preimport flags
///
/// # Errors
/// Returns `ObmmError::PreimportFailed` if the preimport operation fails
///
/// # Example
/// ```
/// use obmm_rs::import::preimport;
/// use obmm_rs::types::{ObmmPreimportInfo, ObmmPreimportFlags};
///
/// let mut info = ObmmPreimportInfo::default();
/// info.length = 1024 * 1024 * 64; // 64MB
/// info.base_dist = 0;
/// info.numa_id = 0;
///
/// match preimport(&mut info, ObmmPreimportFlags::empty()) {
///     Ok(()) => println!("Successfully preimported"),
///     Err(e) => eprintln!("Preimport failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn preimport(_: &mut ObmmPreimportInfo, _: ObmmPreimportFlags) -> Result<()> {
    // Hooked implementation for testing
    Ok(())
}

/// Preimport memory region (real implementation)
///
/// Preimports a memory region using the actual OBMM C library.
///
/// # Arguments
/// * `info` - Preimport information structure containing region details
/// * `flags` - Preimport flags
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The preimport information is invalid
/// - The preimport operation fails (e.g., insufficient memory)
#[cfg(feature = "native")]
#[inline]
pub fn preimport(info: &mut ObmmPreimportInfo, flags: ObmmPreimportFlags) -> Result<()> {
    let ret = unsafe { sys::obmm_preimport(info, flags.bits()) };
    ret.to_obmm_result(ObmmError::PreimportFailed)
}

/// Unpreimport memory region
///
/// Removes a preimported memory region, releasing the pre-allocated resources.
///
/// # Arguments
/// * `info` - Preimport information structure (must match the one used for preimport)
/// * `flags` - Unpreimport flags
///
/// # Errors
/// Returns `ObmmError::UnpreimportFailed` if the unpreimport operation fails
///
/// # Example
/// ```
/// use obmm_rs::import::unpreimport;
/// use obmm_rs::types::ObmmPreimportInfo;
///
/// let info = ObmmPreimportInfo::default();
/// match unpreimport(&info, Default::default()) {
///     Ok(()) => println!("Successfully unpreimported"),
///     Err(e) => eprintln!("Unpreimport failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn unpreimport(_: &ObmmPreimportInfo, _: ObmmPreimportFlags) -> Result<()> {
    // Hooked implementation for testing
    Ok(())
}

/// Unpreimport memory region (real implementation)
///
/// Removes a preimported memory region using the actual OBMM C library.
///
/// # Arguments
/// * `info` - Preimport information structure (must match the one used for preimport)
/// * `flags` - Unpreimport flags
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The preimport information does not match an existing preimport
/// - The unpreimport operation fails
#[cfg(feature = "native")]
#[inline]
pub fn unpreimport(info: &ObmmPreimportInfo, flags: ObmmPreimportFlags) -> Result<()> {
    let ret = unsafe { sys::obmm_unpreimport(info, flags.bits()) };
    ret.to_obmm_result(ObmmError::UnpreimportFailed)
}
