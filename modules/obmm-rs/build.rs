//! Build script for obmm-rs
//!
//! This build script is now minimal since the pure Rust implementation
//! no longer requires building a C library. It only emits a warning
//! to inform the user about the build mode.

use std::env;

fn main() {
    // Check if native feature is enabled
    let is_native = env::var("CARGO_FEATURE_NATIVE").is_ok();

    if is_native {
        println!("cargo:warning=Building with pure Rust OBMM implementation (direct kernel interface)");
    } else {
        println!("cargo:warning=Building with stub implementations (no real OBMM calls)");
    }

    // Re-run if source files change
    println!("cargo:rerun-if-changed=src/");
}
