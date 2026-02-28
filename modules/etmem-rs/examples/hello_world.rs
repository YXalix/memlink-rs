//! ETMEM Hello World Example - Self-Scanning Memory
//!
//! This simple example demonstrates ETMEM functionality by:
//! 1. Allocating memory using mmap
//! 2. Scanning its own memory pages
//! 3. Displaying memory statistics
//!
//! # Running the Example
//!
//! ```bash
//! # Scan with default settings (may show huge pages for large allocations)
//! sudo cargo run --example hello_world --package etmem-rs
//!
//! # Force 4KB page scanning (disable huge pages)
//! sudo cargo run --example hello_world --package etmem-rs -- --no-huge
//! ```
//!
//! # Requirements
//!
//! - Linux kernel with ETMEM support (CONFIG_ETMEM=y)
//! - CAP_SYS_ADMIN capability (root access)
//! - ETMEM kernel modules loaded (etmem_scan.ko)
//!
//! # Page Size Notes
//!
//! The kernel may use Transparent Huge Pages (THP) for large allocations,
//! causing scans to report 2MB (PMD) pages instead of 4KB (PTE) pages.
//! Use the `--no-huge` flag to disable huge page allocation via madvise.

use etmem_rs::{AddressRange, IdlePageInfo, ScanConfig, ScanSession};
use std::env;
use std::process;

// Memory allocation size: 10 MB
const ALLOC_SIZE: usize = 10 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let disable_huge_pages = args.contains(&"--no-huge".to_string());

    if args.len() > 1 && args[1] == "--help" {
        println!("Usage: hello_world [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --no-huge    Disable transparent huge pages for 4KB page granularity");
        println!("  --help       Show this help message");
        return Ok(());
    }

    // Check if running as root
    if !etmem_rs::has_permission() {
        eprintln!("Error: This example requires root privileges (CAP_SYS_ADMIN)");
        eprintln!("Please run with sudo");
        std::process::exit(1);
    }

    // Check if ETMEM is available
    if !etmem_rs::is_available() {
        eprintln!("Error: ETMEM is not available on this system");
        eprintln!("Please check that:");
        eprintln!("  - Kernel is built with CONFIG_ETMEM=y");
        eprintln!("  - etmem_scan.ko module is loaded");
        std::process::exit(1);
    }

    println!("ETMEM Hello World Example");
    println!("=========================\n");

    // Allocate memory using mmap
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            ALLOC_SIZE,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
            -1,
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err(Box::new(std::io::Error::last_os_error()));
    }
    println!("Allocated {} MB of memory at {:p}", ALLOC_SIZE / 1024 / 1024, ptr);

    // Disable transparent huge pages if requested (for 4KB page granularity)
    if disable_huge_pages {
        unsafe {
            libc::madvise(ptr, ALLOC_SIZE, libc::MADV_NOHUGEPAGE);
        }
        println!("Disabled transparent huge pages for this allocation");
    }

    // Initialize allocated memory (touch all pages to make them "accessed")
    unsafe {
        std::ptr::write_bytes(ptr, 0xAB, ALLOC_SIZE);
    }
    println!("Initialized memory (all pages touched)\n");

    // Configure scan - use default flags to scan all pages
    let config = ScanConfig::default();
    let mut session = ScanSession::new(process::id(), config)?;

    let range = AddressRange {
        start: ptr as u64,
        end: (ptr as u64) + (ALLOC_SIZE as u64),
    };

    println!("Scanning memory range: 0x{:x} - 0x{:x}", range.start, range.end);

    // Read page information from the range
    let pages = session.read_range(range)?;

    // Display results
    print_scan_results(&pages);

    // Cleanup
    unsafe {
        libc::munmap(ptr, ALLOC_SIZE);
    }
    println!("\nMemory freed. Example completed successfully!");

    Ok(())
}

/// Print scan results in a formatted way
fn print_scan_results(pages: &[IdlePageInfo]) {
    if pages.is_empty() {
        println!("No pages found in scanned range");
        return;
    }

    println!("\nScan Results:");
    println!("{:<20} {:<16} {:<12} {:<16}", "Address", "Type", "Count", "Size");
    println!("{}", "-".repeat(70));

    let mut total_idle = 0u64;
    let mut total_accessed = 0u64;
    let mut total_holes = 0u64;

    for page in pages {
        let size = page.total_size();
        let type_str = format!("{:?}", page.page_type);

        println!(
            "0x{:016x} {:<16} {:<12} {}",
            page.address,
            type_str,
            page.count,
            format_bytes(size)
        );

        // Accumulate statistics
        if page.is_idle() {
            total_idle += size;
        } else if page.is_accessed() {
            total_accessed += size;
        } else if page.page_type.is_hole() {
            total_holes += size;
        }
    }

    println!("{}", "-".repeat(70));
    println!("\nSummary:");
    println!("  Total pages found: {}", pages.len());
    println!("  Accessed (hot):    {}", format_bytes(total_accessed));
    println!("  Idle (cold):       {}", format_bytes(total_idle));
    println!("  Holes (unmapped):  {}", format_bytes(total_holes));
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}
