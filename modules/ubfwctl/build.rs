//! Build script for generating FFI bindings

use std::env;
use std::path::PathBuf;

fn main() {
    // Get the workspace root from CARGO_MANIFEST_DIR
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let ubctl_headers = workspace_root.join("../ubctl/kernel_headers");

    let fwctl_h = ubctl_headers.join("fwctl.h");
    let ub_fwctl_h = ubctl_headers.join("ub_fwctl.h");
    let header_include = format!("-I{}", ubctl_headers.display());

    // Tell cargo to re-run if headers change
    println!("cargo:rerun-if-changed={}", fwctl_h.display());
    println!("cargo:rerun-if-changed={}", ub_fwctl_h.display());

    // Generate bindings for fwctl.h
    let fwctl_bindings = bindgen::Builder::default()
        .header(fwctl_h.to_string_lossy())
        .clang_arg(&header_include)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate fwctl bindings");

    // Generate bindings for ub_fwctl.h
    let ub_fwctl_bindings = bindgen::Builder::default()
        .header(ub_fwctl_h.to_string_lossy())
        .clang_arg(&header_include)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate ub_fwctl bindings");

    // Write bindings to OUT_DIR
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    fwctl_bindings
        .write_to_file(out_path.join("fwctl_bindings.rs"))
        .expect("Couldn't write fwctl bindings");

    ub_fwctl_bindings
        .write_to_file(out_path.join("ub_fwctl_bindings.rs"))
        .expect("Couldn't write ub_fwctl bindings");

    // Note: We don't use these bindings directly in the code yet,
    // but they are available for future use if needed.
    // Currently, we define the structures manually in src/ioctl.rs
    // to have better control over the FFI interface.
}
