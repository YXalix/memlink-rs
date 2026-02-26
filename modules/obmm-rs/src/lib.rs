//! obmm-rs: Rust bindings for OBMM (Ownership-Based Memory Management)
//!
//! This crate provides safe Rust bindings and utilities for interacting with OBMM,
//! enabling memory exporting, importing, and management in a safe and ergonomic way.
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`error`](crate::error): Custom error types and result aliases
//! - [`types`](crate::types): Type definitions, constants, and bitflags
//! - [`sys`](crate::sys): Low-level FFI bindings to the C library
//! - [`export`](crate::export): Safe wrappers for memory export operations
//! - [`import`](crate::export): Safe wrappers for memory import operations
//! - [`query`](crate::query): Safe wrappers for memory query operations
//! - [`ownership`](crate::ownership): Safe wrappers for ownership management
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
#![deny(
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
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_op_in_unsafe_fn,
    unstable_features,
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences,
    warnings,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::as_conversions,
    clippy::clone_on_ref_ptr,
    clippy::create_dir,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::disallowed_script_idents,
    clippy::else_if_without_else,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::exit,
    clippy::expect_used,
    clippy::filetype_is_file,
    clippy::float_arithmetic,
    clippy::float_cmp_const,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    clippy::indexing_slicing,
    clippy::inline_asm_x86_intel_syntax,
    clippy::arithmetic_side_effects,
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::missing_docs_in_private_items,
    clippy::missing_enforced_import_renames,
    clippy::missing_inline_in_public_items,
    clippy::modulo_arithmetic,
    clippy::multiple_inherent_impl,
    clippy::pattern_type_mismatch,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::string_add,
    clippy::todo,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    clippy::verbose_file_reads,
    clippy::wildcard_enum_match_arm,
)]

// Module declarations
pub mod error;
pub mod export;
pub mod import;
pub mod ownership;
pub mod query;
pub mod sys;
pub mod types;

/// Prelude module for convenient imports
///
/// This module re-exports commonly used types and functions for convenience.
pub mod prelude {
    pub use crate::error::{ObmmError, Result, ToObmmResult};
    pub use crate::export::{export_useraddr, mem_export, mem_unexport};
    pub use crate::import::{mem_import, mem_unimport, preimport, unpreimport};
    pub use crate::ownership::{
        prot::{self},
        set_ownership, OwnershipSetter,
    };
    pub use crate::query::{query_memid_by_pa, query_pa_by_memid};
    pub use crate::sys;
    pub use crate::types::{
        ImportResult, MemId, ObmmExportFlags, ObmmMemDesc, ObmmPreimportFlags,
        ObmmPreimportInfo, ObmmUnexportFlags, QueryResult, UbPrivData, MAX_NUMA_NODES,
        OBMM_INVALID_MEMID, OBMM_MAX_LOCAL_NUMA_NODES,
    };
}

// Backward compatibility: re-export common items at crate root
pub use error::{ObmmError, Result, ToObmmResult};
pub use export::{export_useraddr, mem_export, mem_unexport};
pub use import::{mem_import, mem_unimport, preimport, unpreimport};
pub use ownership::{
    set_ownership,
    prot::{self},
    OwnershipSetter,
};
pub use query::{query_memid_by_pa, query_pa_by_memid};
pub use types::{
    ImportResult, MemId, ObmmExportFlags, ObmmMemDesc, ObmmPreimportFlags, ObmmPreimportInfo,
    ObmmUnexportFlags, QueryResult, UbPrivData, MAX_NUMA_NODES, OBMM_INVALID_MEMID,
    OBMM_MAX_LOCAL_NUMA_NODES,
};

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_export_unexport_roundtrip() {
        let mut lengths = vec![0; MAX_NUMA_NODES];
        lengths[0] = 1024 * 1024 * 64; // 64MB on NUMA node 0
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
                println!("Imported MemID: {}, NUMA: {}", result.mem_id, result.numa_node);
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

        let json_str = desc.to_json().expect("Failed to serialize");
        println!("Serialized JSON: {json_str}");

        let deserialized: ObmmMemDesc<UbPrivData> =
            ObmmMemDesc::from_json(&json_str).expect("Failed to deserialize");

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
        let mut preimport_info = ObmmPreimportInfo::default();
        preimport_info.length = 1024 * 1024 * 64;
        preimport_info.base_dist = 0;
        preimport_info.numa_id = 0;

        match preimport(&mut preimport_info,
            ObmmPreimportFlags::empty(),
        ) {
            Ok(()) => println!("Preimport succeeded"),
            Err(e) => println!("Preimport failed (expected on non-OBMM system): {e}"),
        }

        match unpreimport(&preimport_info,
            Default::default(),
        ) {
            Ok(()) => println!("Unpreimport succeeded"),
            Err(e) => println!("Unpreimport failed (expected on non-OBMM system): {e}"),
        }

        // export_useraddr
        match export_useraddr::<UbPrivData>(0, 0x7fff_0000_0000, 1024 * 1024 * 2, ObmmExportFlags::ALLOWMMAP) {
            Ok((mem_id, _desc)) => {
                println!("Export useraddr succeeded: {mem_id}");
                let _ = mem_unexport(mem_id, ObmmUnexportFlags::empty());
            }
            Err(e) => println!("Export useraddr failed (expected on non-OBMM system): {e}"),
        }

        // query operations
        match query_memid_by_pa(0x10000000) {
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
