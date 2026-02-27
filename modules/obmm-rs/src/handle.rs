//! Memory handle types for RAII-based memory management
//!
//! This module provides handle types that automatically manage memory lifecycle,
//! ensuring proper cleanup when the handle goes out of scope.
//!
//! # Example
//!
//! ```
//! use obmm_rs::handle::ExportedMemory;
//! use obmm_rs::types::{ObmmExportFlags, UbPrivData};
//!
//! // Memory is automatically unexported when `memory` goes out of scope
//! let lengths = vec![1024 * 1024 * 64]; // 64MB on NUMA node 0
//! let memory = ExportedMemory::<UbPrivData>::export(
//!     &lengths,
//!     ObmmExportFlags::ALLOWMMAP,
//! ).expect("Export failed");
//!
//! println!("Memory ID: {}", memory.mem_id());
//! ```

use crate::error::{ObmmError, Result};
use crate::export::{export_useraddr, mem_export, mem_unexport};
use crate::import::{mem_import, mem_unimport};
use crate::types::{
    ImportResult, MemId, ObmmExportFlags, ObmmMemDesc, ObmmUnexportFlags, UbPrivData,
    OBMM_INVALID_MEMID,
};

/// Handle for exported memory regions
///
/// Automatically unexports the memory when dropped, unless explicitly released.
#[derive(Debug)]
pub struct ExportedMemory<T = UbPrivData> {
    /// Memory ID of the exported region
    mem_id: MemId,
    /// Memory descriptor containing metadata
    desc: ObmmMemDesc<T>,
    /// Whether the memory has been released from automatic cleanup
    released: bool,
}

impl<T: Default> ExportedMemory<T> {
    /// Export memory regions
    ///
    /// # Arguments
    /// * `lengths` - Vector of lengths for each NUMA node (index 0 = NUMA node 0, etc.)
    /// * `flags` - Export flags
    ///
    /// # Returns
    /// An `ExportedMemory` handle on success
    ///
    /// # Errors
    /// Returns `ObmmError::ExportFailed` if the export operation fails
    ///
    /// # Example
    /// ```
    /// use obmm_rs::handle::ExportedMemory;
    /// use obmm_rs::types::{ObmmExportFlags, UbPrivData};
    ///
    /// let mut lengths = vec![0; 16];
    /// lengths[0] = 1024 * 1024 * 64; // 64MB on NUMA node 0
    ///
    /// let memory = ExportedMemory::<UbPrivData>::export(&lengths, ObmmExportFlags::ALLOWMMAP).expect("Export failed");
    /// ```
    #[inline]
    pub fn export(lengths: &[usize], flags: ObmmExportFlags) -> Result<Self> {
        let (mem_id, desc) = mem_export::<T>(lengths, flags)
            .map_err(|_e| ObmmError::ExportFailed(-1))?;

        if mem_id == OBMM_INVALID_MEMID {
            return Err(ObmmError::ExportFailed(-1));
        }

        Ok(Self {
            mem_id,
            desc,
            released: false,
        })
    }

    /// Export user address space
    ///
    /// # Arguments
    /// * `pid` - Process ID (0 for current process)
    /// * `va` - Virtual address to export
    /// * `length` - Length of the region in bytes
    /// * `flags` - Export flags
    ///
    /// # Returns
    /// An `ExportedMemory` handle on success
    ///
    /// # Errors
    /// Returns `ObmmError::ExportFailed` if the export operation fails
    #[inline]
    pub fn export_useraddr(pid: i32, va: u64, length: usize, flags: ObmmExportFlags) -> Result<Self> {
        let (mem_id, desc) = export_useraddr::<T>(pid, va, length, flags)?;

        if mem_id == OBMM_INVALID_MEMID {
            return Err(ObmmError::InvalidMemId);
        }

        Ok(Self {
            mem_id,
            desc,
            released: false,
        })
    }

    /// Get the memory ID
    #[inline]
    #[must_use]
    pub const fn mem_id(&self) -> MemId {
        self.mem_id
    }

    /// Get a reference to the memory descriptor
    #[inline]
    #[must_use]
    pub const fn descriptor(&self) -> &ObmmMemDesc<T> {
        &self.desc
    }

    /// Release ownership without unexporting
    ///
    /// After calling this, the caller is responsible for unexporting the memory.
    ///
    /// # Returns
    /// The memory ID and descriptor
    #[inline]
    pub fn release(mut self) -> (MemId, ObmmMemDesc<T>) {
        self.released = true;
        (self.mem_id, std::mem::take(&mut self.desc))
    }

    /// Manually unexport the memory
    ///
    /// This is called automatically when the handle is dropped, but can be
    /// called explicitly for early cleanup.
    ///
    /// # Errors
    /// Returns `ObmmError::UnexportFailed` if the unexport operation fails
    #[inline]
    pub fn unexport(&mut self) -> Result<()> {
        if !self.released {
            mem_unexport(self.mem_id, ObmmUnexportFlags::empty())?;
            self.released = true;
        }
        Ok(())
    }
}

impl<T> Drop for ExportedMemory<T> {
    #[inline]
    fn drop(&mut self) {
        if !self.released {
            // Ignore errors during drop - best effort cleanup
            let _result = mem_unexport(self.mem_id, ObmmUnexportFlags::empty());
        }
    }
}

/// Handle for imported memory regions
///
/// Automatically unimports the memory when dropped, unless explicitly released.
#[derive(Debug)]
pub struct ImportedMemory {
    /// Memory ID of the imported region
    mem_id: MemId,
    /// NUMA node where the memory was placed
    numa_node: i32,
    /// Whether the memory has been released from automatic cleanup
    released: bool,
}

impl ImportedMemory {
    /// Import a memory region
    ///
    /// # Arguments
    /// * `desc` - Memory descriptor from the remote export
    /// * `flags` - Import flags
    /// * `base_dist` - Base distribution hint for NUMA placement
    ///
    /// # Returns
    /// An `ImportedMemory` handle on success
    ///
    /// # Errors
    /// Returns `ObmmError::ImportFailed` if the import operation fails
    #[inline]
    pub fn import(
        desc: &ObmmMemDesc<UbPrivData>,
        flags: ObmmExportFlags,
        base_dist: i32,
    ) -> Result<Self> {
        let ImportResult { mem_id, numa_node } = mem_import(desc, flags, base_dist)?;

        if mem_id == OBMM_INVALID_MEMID {
            return Err(ObmmError::ImportFailed(-1));
        }

        Ok(Self {
            mem_id,
            numa_node,
            released: false,
        })
    }

    /// Get the memory ID
    #[inline]
    #[must_use]
    pub const fn mem_id(&self) -> MemId {
        self.mem_id
    }

    /// Get the NUMA node where the memory was placed
    #[inline]
    #[must_use]
    pub const fn numa_node(&self) -> i32 {
        self.numa_node
    }

    /// Release ownership without unimporting
    ///
    /// After calling this, the caller is responsible for unimporting the memory.
    ///
    /// # Returns
    /// The memory ID
    #[inline]
    #[must_use]
    pub fn release(mut self) -> MemId {
        self.released = true;
        self.mem_id
    }

    /// Manually unimport the memory
    ///
    /// This is called automatically when the handle is dropped, but can be
    /// called explicitly for early cleanup.
    ///
    /// # Errors
    /// Returns `ObmmError::UnimportFailed` if the unimport operation fails
    #[inline]
    pub fn unimport(&mut self) -> Result<()> {
        if !self.released {
            mem_unimport(self.mem_id, ObmmExportFlags::empty())?;
            self.released = true;
        }
        Ok(())
    }
}

impl Drop for ImportedMemory {
    #[inline]
    fn drop(&mut self) {
        if !self.released {
            // Ignore errors during drop - best effort cleanup
            let _result = mem_unimport(self.mem_id, ObmmExportFlags::empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exported_memory_handle() {
        let mut lengths = vec![0; 16];

        if let Some(elem) = lengths.get_mut(0) {
            *elem = 1024 * 1024;
        } else {
            panic!("Failed to set length for NUMA node 0");
        }

        let memory: Result<ExportedMemory<UbPrivData>> = ExportedMemory::export(&lengths, ObmmExportFlags::ALLOWMMAP);

        match memory {
            Ok(mem) => {
                assert!(mem.mem_id() != OBMM_INVALID_MEMID);
                assert!(mem.descriptor().length > 0);
                // Memory will be automatically unexported on drop
            }
            Err(e) => {
                println!("Export failed (expected on non-OBMM system): {e}");
            }
        }
    }

    #[test]
    fn test_imported_memory_handle() {
        let desc = ObmmMemDesc::<UbPrivData>::default();

        let memory = ImportedMemory::import(&desc, ObmmExportFlags::ALLOWMMAP, 0);

        match memory {
            Ok(mem) => {
                assert!(mem.mem_id() != OBMM_INVALID_MEMID);
                assert_eq!(mem.numa_node(), 0);
                // Memory will be automatically unimported on drop
            }
            Err(e) => {
                println!("Import failed (expected on non-OBMM system): {e}");
            }
        }
    }

    #[test]
    fn test_release_prevents_cleanup() {
        let mut lengths = vec![0; 16];

        if let Some(elem) = lengths.get_mut(0) {
            *elem = 1024 * 1024; // 1MB on NUMA node 0
        } else {
            panic!("Failed to set length for NUMA node 0");
        }

        if let Ok(mem) = ExportedMemory::<UbPrivData>::export(&lengths, ObmmExportFlags::ALLOWMMAP) {
            let (mem_id, _desc) = mem.release();
            assert!(mem_id != OBMM_INVALID_MEMID);
            // Caller is now responsible for unexporting
        }
    }
}
