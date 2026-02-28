//! ETMEM VMA Example - Virtual Memory Area Discovery
//!
//! This example demonstrates the new VMA-aware API:
//! 1. Discover all VMAs for the current process
//! 2. Display memory layout information
//! 3. Scan specific VMAs (heap, stack, anonymous)
//! 4. Use builder pattern for targeted scans
//!
//! # Running the Example
//!
//! ```bash
//! sudo cargo run --example vma_example --package etmem-rs
//! ```

use etmem_rs::builder::ScanBuilder;
use etmem_rs::vma::{VmaFilter, VmaMap};
use etmem_rs::{EtmemSession, ScanConfig, SessionConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check permissions
    if !etmem_rs::has_permission() {
        eprintln!("Error: This example requires root privileges (CAP_SYS_ADMIN)");
        eprintln!("Please run with sudo");
        std::process::exit(1);
    }

    // Check ETMEM availability
    if !etmem_rs::is_available() {
        eprintln!("Error: ETMEM is not available on this system");
        eprintln!("Please ensure etmem_scan.ko is loaded");
        std::process::exit(1);
    }

    println!("ETMEM VMA Example");
    println!("=================\n");

    let pid = std::process::id();
    println!("Discovering VMAs for PID {}...\n", pid);

    // Discover VMAs using the new API
    let vma_map = VmaMap::for_process(pid)?;

    // Display VMA information
    print_vma_info(&vma_map);

    // Demonstrate builder pattern - scan heap
    println!("\n--- Scanning Heap (using builder pattern) ---");
    match ScanBuilder::for_process(pid)?.for_heap().scan() {
        Ok(pages) => {
            println!("Found {} pages in heap", pages.len());
            print_page_summary(&pages);
        }
        Err(e) => eprintln!("Failed to scan heap: {}", e),
    }

    // Demonstrate unified session - scan all scannable VMAs
    println!("\n--- Scanning All Scannable VMAs (using unified session) ---");
    let mut session = EtmemSession::new(pid, SessionConfig::default())?;

    match session.scan_all_vmas(ScanConfig::default()) {
        Ok(results) => {
            println!("Scanned {} VMAs", results.per_vma.len());
            println!("Total idle: {} bytes", format_bytes(results.total_idle_bytes));
            println!(
                "Total accessed: {} bytes",
                format_bytes(results.total_accessed_bytes)
            );
            println!("Idle ratio: {:.2}%", results.idle_ratio() * 100.0);
        }
        Err(e) => eprintln!("Failed to scan all VMAs: {}", e),
    }

    // Demonstrate filtering - scan only anonymous, writable regions
    println!("\n--- Scanning Anonymous Writable Regions ---");
    let filter = VmaFilter::ANONYMOUS | VmaFilter::WRITABLE;
    let anon_writable = vma_map.filter(filter);
    println!("Found {} anonymous writable VMAs", anon_writable.len());

    for vma in anon_writable.iter().take(3) {
        println!("  {} - {} bytes", vma.name(), format_bytes(vma.size()));
    }

    println!("\nExample completed successfully!");
    Ok(())
}

/// Print VMA information
fn print_vma_info(vma_map: &VmaMap) {
    println!("Memory Layout:");
    println!("  Total VMAs: {}", vma_map.len());
    println!(
        "  Total virtual memory: {}",
        format_bytes(vma_map.total_size())
    );
    println!(
        "  Anonymous memory: {}",
        format_bytes(vma_map.anonymous_size())
    );

    // Heap info
    if let Some(heap) = vma_map.heap() {
        println!("  Heap: {} bytes at 0x{:x}", format_bytes(heap.size()), heap.start);
    } else {
        println!("  Heap: not found");
    }

    // Stack info
    let stacks = vma_map.stacks();
    if !stacks.is_empty() {
        println!("  Stacks: {} thread(s)", stacks.len());
        for (i, stack) in stacks.iter().enumerate() {
            println!(
                "    Stack {}: {} bytes",
                i,
                format_bytes(stack.size())
            );
        }
    }

    // Scannable/swappable summary
    let scannable = vma_map.scannable();
    let swappable = vma_map.swappable();
    println!("\nVMA Categories:");
    println!("  Scannable regions: {}", scannable.len());
    println!("  Swappable regions: {}", swappable.len());
}

/// Print page summary
fn print_page_summary(pages: &[etmem_rs::IdlePageInfo]) {
    if pages.is_empty() {
        return;
    }

    let idle_count = pages.iter().filter(|p| p.is_idle()).count();
    let accessed_count = pages.iter().filter(|p| p.is_accessed()).count();

    println!("  Idle pages: {}", idle_count);
    println!("  Accessed pages: {}", accessed_count);
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}
