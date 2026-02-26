//! Query operations for OBMM (Ownership-Based Memory Management)
//!
//! This module provides safe wrappers for querying memory information
//! including memory ID to physical address translation and vice versa.

use crate::error::{ObmmError, Result};
#[cfg(not(feature = "hook"))]
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
#[cfg(feature = "hook")]
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
/// See the hooked version for documentation.
#[cfg(not(feature = "hook"))]
#[inline]
pub fn query_memid_by_pa(pa: u64) -> Result<QueryResult> {
    let mut mem_id: MemId = 0;
    let mut offset: u64 = 0;
    let ret = unsafe { sys::obmm_query_memid_by_pa(pa, &mut mem_id, &mut offset) };

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
#[cfg(feature = "hook")]
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
/// See the hooked version for documentation.
#[cfg(not(feature = "hook"))]
#[inline]
pub fn query_pa_by_memid(mem_id: MemId, offset: u64) -> Result<u64> {
    let mut pa: u64 = 0;
    let ret = unsafe { sys::obmm_query_pa_by_memid(mem_id, offset, &mut pa) };

    if ret == 0 {
        Ok(pa)
    } else {
        Err(ObmmError::QueryFailed(ret))
    }
}
