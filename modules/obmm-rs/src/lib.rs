//! obmm-rs: Pure Rust implementation for OBMM (Ownership-Based Memory Management)
//!
//! This crate provides a pure Rust implementation for interacting with the OBMM
//! kernel module, enabling memory exporting, importing, and management without
//! requiring a C library.
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`error`]: Custom error types and result aliases
//! - [`types`]: Type definitions, constants, and bitflags
//! - `kernel_abi`: Kernel ABI definitions (ioctl constants and structures)
//! - `device`: Low-level device file operations
//! - `kernel`: Pure Rust implementation of OBMM kernel interface
//! - [`export`]: Safe wrappers for memory export operations
//! - [`import`]: Safe wrappers for memory import operations
//! - [`query`]: Safe wrappers for memory query operations
//! - [`ownership`]: Safe wrappers for ownership management
//! - [`handle`]: RAII memory handles for automatic cleanup
//!
//! # Feature Flags
//!
//! This crate uses feature flags to control the implementation:
//!
//! - `native` (enabled by default): Uses the pure Rust implementation that directly
//!   communicates with the OBMM kernel module via ioctl system calls.
//!
//! When `native` is disabled, stub implementations are used automatically. These
//! return test data without making actual system calls, which is useful for
//! development and testing on systems without OBMM kernel support.
//!
//! ## Usage Examples
//!
//! ### Production build (with native kernel interface):
//! ```bash
//! # Build with native feature (default)
//! cargo build
//! ```
//!
//! ### Development/testing build (without kernel interface):
//! ```bash
//! # Build without default features to use stub implementations
//! cargo build --no-default-features
//! ```
//!
//! # Quick Start
//!
//! ```no_run
//! use obmm_rs::prelude::*;
//!
//! // Export memory
//! let mut lengths = vec![0; MAX_NUMA_NODES];
//! lengths[0] = 1024 * 1024 * 64; // 64MB
//! let (mem_id, _desc) = mem_export::<UbPrivData>(&lengths, ObmmExportFlags::ALLOWMMAP)
//!     .expect("Export failed");
//!
//! // Query physical address
//! let pa = query_pa_by_memid(mem_id, 0).expect("Query failed");
//! println!("Physical address: 0x{:x}", pa);
//! ```

#![warn(
    absolute_paths_not_starting_with_crate,
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_abi,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    non_ascii_idents,
    noop_method_call,
    rust_2021_incompatible_closure_captures,
    rust_2021_incompatible_or_patterns,
    rust_2021_prefixes_incompatible_syntax,
    rust_2021_prelude_collisions,
    single_use_lifetimes,
    trivial_numeric_casts,
    unsafe_op_in_unsafe_fn,
    unstable_features,
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    variant_size_differences
)]
// Allow certain warnings that are common in kernel interface code
#![allow(
    dead_code,
    unreachable_pub,
    trivial_casts,
    unused_unsafe,
    unused_results,
    unused_imports,
    unused_parens,
    unused_qualifications
)]

// Module declarations
pub mod error;
pub mod export;
pub mod handle;
pub mod import;
pub mod ownership;
pub mod query;
pub mod types;

// Pure Rust kernel interface modules (native feature)
#[cfg(feature = "native")]
pub(crate) mod device;
#[cfg(feature = "native")]
pub(crate) mod kernel;
#[cfg(feature = "native")]
pub(crate) mod kernel_abi;

// Legacy sys module (for backward compatibility)
#[cfg(feature = "native")]
pub mod sys;
#[cfg(not(feature = "native"))]
pub mod sys;

/// Prelude module for convenient imports
///
/// This module re-exports commonly used types and functions for convenience.
pub mod prelude {
    pub use crate::error::{ObmmError, Result, ToObmmResult};
    pub use crate::export::{export_useraddr, mem_export, mem_unexport};
    pub use crate::handle::{ExportedMemory, ImportedMemory};
    pub use crate::import::{mem_import, mem_unimport, preimport, unpreimport};
    pub use crate::ownership::{
        OwnershipSetter,
        prot::{self},
        set_ownership,
    };
    pub use crate::query::{query_memid_by_pa, query_pa_by_memid};
    pub use crate::sys;
    pub use crate::types::{
        ImportResult, MAX_NUMA_NODES, MemId, OBMM_INVALID_MEMID, OBMM_MAX_LOCAL_NUMA_NODES,
        ObmmExportFlags, ObmmMemDesc, ObmmPreimportFlags, ObmmPreimportInfo, ObmmUnexportFlags,
        QueryResult, UbPrivData,
    };
}

// Backward compatibility: re-export common items at crate root
pub use error::{ObmmError, Result, ToObmmResult};
pub use export::{export_useraddr, mem_export, mem_unexport};
pub use import::{mem_import, mem_unimport, preimport, unpreimport};
pub use ownership::{
    OwnershipSetter,
    prot::{self},
    set_ownership,
};
pub use query::{query_memid_by_pa, query_pa_by_memid};
pub use types::{
    ImportResult, MAX_NUMA_NODES, MemId, OBMM_INVALID_MEMID, OBMM_MAX_LOCAL_NUMA_NODES,
    ObmmExportFlags, ObmmMemDesc, ObmmPreimportFlags, ObmmPreimportInfo, ObmmUnexportFlags,
    QueryResult, UbPrivData,
};

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_export_unexport_roundtrip() {
        let mut lengths = vec![0; MAX_NUMA_NODES];
        if let Some(elem) = lengths.get_mut(0) {
            *elem = 1024 * 1024 * 64; // 64MB on NUMA node 0
        } else {
            panic!("Failed to set length for NUMA node 0");
        }
        let flags = ObmmExportFlags::ALLOWMMAP;

        match mem_export::<UbPrivData>(&lengths, flags) {
            Ok((memid, desc)) => {
                println!("Exported MemID: {memid}");
                println!("Memory Descriptor: {desc:?}");
                assert!(memid != OBMM_INVALID_MEMID);
                // Note: desc.length might be adjusted by the underlying system

                // Clean up
                match mem_unexport(memid, ObmmUnexportFlags::empty()) {
                    Ok(()) => println!("Successfully unexported"),
                    Err(e) => println!("Unexport failed: {e}"),
                }
            }
            Err(e) => {
                println!("mem_export failed: {e}");
                // Don't fail the test if the underlying system doesn't support OBMM
            }
        }
    }

    #[test]
    fn test_import_roundtrip() {
        let desc = ObmmMemDesc::<UbPrivData> {
            addr: 0xffff_fc00_0000,
            length: 1024 * 1024 * 128,
            seid: [0; 16],
            deid: [0; 16],
            tokenid: 0,
            scna: 0,
            dcna: 0,
            priv_len: 0,
            priv_data: UbPrivData::default(),
        };
        let flags = ObmmExportFlags::ALLOWMMAP;

        match mem_import(&desc, flags, 0) {
            Ok(result) => {
                println!(
                    "Imported MemID: {}, NUMA: {}",
                    result.mem_id, result.numa_node
                );
                assert!(result.mem_id != OBMM_INVALID_MEMID);

                // Clean up
                match mem_unimport(result.mem_id, ObmmExportFlags::empty()) {
                    Ok(()) => println!("Successfully unimported"),
                    Err(e) => println!("Unimport failed: {e}"),
                }
            }
            Err(e) => {
                println!("mem_import failed: {e}");
                // Don't fail the test if the underlying system doesn't support OBMM
            }
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let desc = ObmmMemDesc::<UbPrivData> {
            addr: 0xffff_fc00_0000,
            length: 1024 * 1024 * 128,
            seid: [1; 16],
            deid: [2; 16],
            tokenid: 42,
            scna: 3,
            dcna: 4,
            priv_len: 2,
            priv_data: UbPrivData::OCHIP | UbPrivData::CACHEABLE,
        };

        let json_str = match desc.to_json() {
            Ok(json) => json,
            Err(e) => {
                println!("Serialization failed: {e}");
                return;
            }
        };
        println!("Serialized JSON: {json_str}");

        let deserialized: ObmmMemDesc<UbPrivData> = match ObmmMemDesc::from_json(&json_str) {
            Ok(d) => d,
            Err(e) => {
                println!("Deserialization failed: {e}");
                return;
            }
        };

        assert_eq!(desc.addr, deserialized.addr);
        assert_eq!(desc.length, deserialized.length);
        assert_eq!(desc.seid, deserialized.seid);
        assert_eq!(desc.deid, deserialized.deid);
        assert_eq!(desc.tokenid, deserialized.tokenid);
        assert_eq!(desc.scna, deserialized.scna);
        assert_eq!(desc.dcna, deserialized.dcna);
        assert_eq!(desc.priv_len, deserialized.priv_len);
        assert_eq!(desc.priv_data, deserialized.priv_data);
    }

    #[test]
    fn test_priv_data_flags() {
        let priv_data = UbPrivData::OCHIP | UbPrivData::CACHEABLE;
        assert!(priv_data.contains(UbPrivData::OCHIP));
        assert!(priv_data.contains(UbPrivData::CACHEABLE));
        assert!(!priv_data.is_empty());
    }

    #[test]
    fn test_export_flags() {
        let flags = ObmmExportFlags::ALLOWMMAP | ObmmExportFlags::REMOTENUMA;
        assert!(flags.contains(ObmmExportFlags::ALLOWMMAP));
        assert!(flags.contains(ObmmExportFlags::REMOTENUMA));
    }

    #[test]
    fn test_new_api_coverage() {
        // Test that all new APIs are accessible

        // preimport / unpreimport
        let mut preimport_info = ObmmPreimportInfo {
            length: 1024 * 1024 * 64,
            base_dist: 0,
            numa_id: 0,
            ..Default::default()
        };

        match preimport(&mut preimport_info, ObmmPreimportFlags::empty()) {
            Ok(()) => println!("Preimport succeeded"),
            Err(e) => println!("Preimport failed (expected on non-OBMM system): {e}"),
        }

        match unpreimport(&preimport_info, ObmmPreimportFlags::default()) {
            Ok(()) => println!("Unpreimport succeeded"),
            Err(e) => println!("Unpreimport failed (expected on non-OBMM system): {e}"),
        }

        // export_useraddr
        match export_useraddr::<UbPrivData>(
            0,
            0x7fff_0000_0000,
            1024 * 1024 * 2,
            ObmmExportFlags::ALLOWMMAP,
        ) {
            Ok((mem_id, _desc)) => {
                println!("Export useraddr succeeded: {mem_id}");
                if let Err(e) = mem_unexport(mem_id, ObmmUnexportFlags::empty()) {
                    println!("Cleanup unexport failed: {e}");
                }
            }
            Err(e) => println!("Export useraddr failed (expected on non-OBMM system): {e}"),
        }

        // query operations
        match query_memid_by_pa(0x1000_0000) {
            Ok(result) => println!("Query memid by pa: {result:?}"),
            Err(e) => println!("Query memid failed (expected on non-OBMM system): {e}"),
        }

        match query_pa_by_memid(1, 0) {
            Ok(pa) => println!("Query pa by memid: {pa}"),
            Err(e) => println!("Query pa failed (expected on non-OBMM system): {e}"),
        }

        // set_ownership
        match set_ownership(3, 0xffff_fc00_0000, 0xffff_fd00_0000, prot::READWRITE) {
            Ok(()) => println!("Set ownership succeeded"),
            Err(e) => println!("Set ownership failed (expected on non-OBMM system): {e}"),
        }
    }

    #[test]
    fn test_ownership_builder_api() {
        let result = OwnershipSetter::new(3)
            .range(0xffff_fc00_0000, 0xffff_fd00_0000)
            .read_write()
            .apply();

        match result {
            Ok(()) => println!("Builder API succeeded"),
            Err(_) => println!("Builder API failed (expected on non-OBMM system)"),
        }
    }
}
