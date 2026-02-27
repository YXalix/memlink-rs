use std::env;
use std::path::Path;

const OBMMSYS_BUILD_PATH: &str = "obmm-sys/build";
const OBMM_LIB_NAME: &str = "libobmm.so";

fn setup_linking() -> Result<(), String> {
    // Check if obmm-sys/build directory exists
    let obmm_lib_path = Path::new(OBMMSYS_BUILD_PATH);
    if !obmm_lib_path.exists() {
        let msg = format!(
            "OBMM C library build directory not found: {}

To build the C library, run:
    cd obmm-sys && mkdir -p build && cd build && cmake .. && make

Alternatively, disable the 'native' feature to build without the C library:
    cargo build --no-default-features",
            obmm_lib_path.display()
        );
        return Err(msg);
    }

    // Get canonical path (handle symlinks)
    let canonical_path = std::fs::canonicalize(obmm_lib_path).map_err(|e| {
        format!(
            "Failed to resolve OBMM library path '{}': {e}",
            obmm_lib_path.display()
        )
    })?;

    // Check if the actual library file exists
    let lib_file = canonical_path.join(OBMM_LIB_NAME);
    if !lib_file.exists() {
        let msg = format!(
            "OBMM C library not found: {}

The build directory exists but {} is missing.
Please rebuild the C library:
    cd obmm-sys/build && make",
            lib_file.display(),
            OBMM_LIB_NAME
        );
        return Err(msg);
    }

    // Set up library search path
    println!("cargo:rustc-link-search=native={}", canonical_path.display());

    // Set up platform-specific rpath for runtime library loading
    match env::consts::OS {
        "linux" | "macos" => {
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{}",
                canonical_path.display()
            );
        }
        "windows" => {
            println!(
                "cargo:rustc-link-arg=/LIBPATH:{}",
                canonical_path.display()
            );
        }
        _ => {
            // Unknown OS - emit warning but continue
            println!(
                "cargo:warning=Unknown OS '{}', rpath may not be set correctly",
                env::consts::OS
            );
        }
    }

    println!("cargo:rustc-link-lib=obmm");

    // Re-run build script if the library changes
    println!("cargo:rerun-if-changed={}", lib_file.display());

    Ok(())
}

fn main() {
    // Only link the C library when the "native" feature is enabled
    let is_native = env::var("CARGO_FEATURE_NATIVE").is_ok();

    if is_native {
        if let Err(e) = setup_linking() {
            // Use env var from build.rs to determine if this is a hard error
            // Users can set OBMM_ALLOW_MISSING_LIB=1 to skip linking (for docs, etc.)
            if env::var("OBMM_ALLOW_MISSING_LIB").is_ok() {
                println!("cargo:warning={e}");
                println!(
                    "cargo:warning=Skipping OBMM linking (OBMM_ALLOW_MISSING_LIB is set)"
                );
            } else {
                panic!("{e}");
            }
        }
    } else {
        println!("cargo:warning=Building with mock-impl feature (no real OBMM calls)");
    }
}
