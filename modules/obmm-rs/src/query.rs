//! Query operations for OBMM (Ownership-Based Memory Management)
//!
//! This module provides safe wrappers for querying memory information
//! including memory ID to physical address translation and vice versa.

use crate::error::{ObmmError, Result};
#[cfg(feature = "native")]
use crate::sys;
use crate::types::{MemId, QueryResult};

/// Query memory ID by physical address
///
/// Looks up the memory region that contains a given physical address
/// and returns its memory ID and offset within the region.
///
/// # Arguments
/// * `pa` - Physical address to query
///
/// # Returns
/// A `QueryResult` containing:
/// - The memory ID of the region containing the address
/// - The offset within that region
///
/// # Errors
/// Returns `ObmmError::QueryFailed` if the address is not found
/// or the query operation fails
///
/// # Example
/// ```
/// use obmm_rs::query::query_memid_by_pa;
///
/// let pa = 0x10000000; // Example physical address
/// match query_memid_by_pa(pa) {
///     Ok(result) => {
///         println!("Memory ID: {}", result.mem_id);
///         println!("Offset: {}", result.offset);
///     }
///     Err(e) => eprintln!("Query failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn query_memid_by_pa(pa: u64) -> Result<QueryResult> {
    // Hooked implementation for testing
    if pa == 0 {
        Err(ObmmError::QueryFailed(-1))
    } else {
        Ok(QueryResult {
            mem_id: 1,
            offset: pa & 0xFFF, // Simulate page offset
            phys_addr: 0,
        })
    }
}

/// Query memory ID by physical address (real implementation)
///
/// Looks up the memory region using the actual OBMM C library.
///
/// # Arguments
/// * `pa` - Physical address to query
///
/// # Returns
/// A `QueryResult` containing:
/// - The memory ID of the region containing the address
/// - The offset within that region
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The physical address is not found in any OBMM memory region
#[cfg(feature = "native")]
#[inline]
pub fn query_memid_by_pa(pa: u64) -> Result<QueryResult> {
    let mut mem_id: MemId = 0;
    let mut offset: u64 = 0;
    let mem_id_ptr = std::ptr::addr_of_mut!(mem_id);
    let offset_ptr = std::ptr::addr_of_mut!(offset);
    let ret = unsafe { sys::obmm_query_memid_by_pa(pa, mem_id_ptr, offset_ptr) };

    if ret == 0 {
        Ok(QueryResult {
            mem_id,
            offset,
            phys_addr: 0,
        })
    } else {
        Err(ObmmError::QueryFailed(ret))
    }
}

/// Query physical address by memory ID
///
/// Converts a memory ID and offset to a physical address.
///
/// # Arguments
/// * `mem_id` - Memory ID to query
/// * `offset` - Offset within the memory region
///
/// # Returns
/// The physical address corresponding to the memory ID and offset
///
/// # Errors
/// Returns `ObmmError::QueryFailed` if the memory ID is invalid
/// or the query operation fails
///
/// # Example
/// ```
/// use obmm_rs::query::query_pa_by_memid;
///
/// let mem_id = 12345;
/// let offset = 0x1000;
///
/// match query_pa_by_memid(mem_id, offset) {
///     Ok(pa) => println!("Physical address: 0x{:x}", pa),
///     Err(e) => eprintln!("Query failed: {}", e),
/// }
/// ```
#[cfg(not(feature = "native"))]
#[inline]
pub fn query_pa_by_memid(mem_id: MemId, offset: u64) -> Result<u64> {
    // Hooked implementation for testing
    if mem_id == 0 {
        Err(ObmmError::QueryFailed(-1))
    } else {
        // Using wrapping_add to avoid potential overflow panics in debug mode
        let base: u64 = 0x1000_0000;
        let shifted = mem_id.checked_shl(12).unwrap_or(0);
        Ok(base.wrapping_add(shifted).wrapping_add(offset))
    }
}

/// Query physical address by memory ID (real implementation)
///
/// Converts a memory ID to physical address using the actual OBMM C library.
///
/// # Arguments
/// * `mem_id` - Memory ID to query
/// * `offset` - Offset within the memory region
///
/// # Returns
/// The physical address corresponding to the memory ID and offset
///
/// # Errors
/// Returns an error if:
/// - The kernel OBMM subsystem is not available
/// - The memory ID is invalid
/// - The offset is out of bounds for the memory region
#[cfg(feature = "native")]
#[inline]
pub fn query_pa_by_memid(mem_id: MemId, offset: u64) -> Result<u64> {
    let mut pa: u64 = 0;
    let pa_ptr = std::ptr::addr_of_mut!(pa);
    let ret = unsafe { sys::obmm_query_pa_by_memid(mem_id, offset, pa_ptr) };

    if ret == 0 {
        Ok(pa)
    } else {
        Err(ObmmError::QueryFailed(ret))
    }
}
