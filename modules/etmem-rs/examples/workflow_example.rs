//! ETMEM Workflow Example - Declarative Scan-and-Swap
//!
//! This example demonstrates the workflow builder API:
//! 1. Create a ScanAndSwapWorkflow with filtering criteria
//! 2. Execute the workflow with thresholds
//! 3. Analyze the results
//!
//! # Running the Example
//!
//! ```bash
//! sudo cargo run --example workflow_example --package etmem-rs
//! ```

use etmem_rs::builder::{quick_scan_heap, quick_scan_idle};
use etmem_rs::vma::VmaFilter;
use etmem_rs::workflow::ScanAndSwapWorkflow;

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
        std::process::exit(1);
    }

    println!("ETMEM Workflow Example");
    println!("======================\n");

    let pid = std::process::id();

    // Example 1: Quick one-liner - scan heap for idle pages
    println!("--- Example 1: Quick Scan Heap ---");
    match quick_scan_heap(pid) {
        Ok(pages) => {
            println!("Found {} pages in heap", pages.len());
            let idle_count = pages.iter().filter(|p| p.is_idle()).count();
            println!("  Idle pages: {}", idle_count);
        }
        Err(e) => eprintln!("Failed: {}", e),
    }

    // Example 2: Quick scan all idle pages
    println!("\n--- Example 2: Quick Scan All Idle ---");
    match quick_scan_idle(pid) {
        Ok(pages) => {
            println!("Found {} idle pages total", pages.len());
        }
        Err(e) => eprintln!("Failed: {}", e),
    }

    // Example 3: Full workflow with filtering (dry run)
    println!("\n--- Example 3: Workflow with Filtering (Dry Run) ---");
    match ScanAndSwapWorkflow::new(pid) {
        Ok(workflow) => {
            let report = workflow
                .target_vma_types(VmaFilter::ANONYMOUS | VmaFilter::WRITABLE)
                .with_idle_threshold(0.5) // Only consider VMAs with 50%+ idle
                .dry_run()
                .execute()?;

            println!("Workflow Results (Dry Run):");
            println!("  VMAs scanned: {}", report.vmas_scanned);
            println!("  Pages scanned: {}", report.pages_scanned);
            println!("  Pages would swap: {}", report.pages_swapped);
            println!("  Bytes would swap: {}", format_bytes(report.bytes_swapped));
            println!(
                "  Overall idle ratio: {:.2}%",
                report.overall_idle_ratio * 100.0
            );
            println!("  Duration: {:?}", report.duration);

            // Print per-VMA results
            if !report.vma_results.is_empty() {
                println!("\n  Per-VMA Details:");
                for result in report.vma_results.iter().take(5) {
                    if result.pages_found > 0 {
                        println!(
                            "    {}: {} pages, {} idle ratio{}",
                            result.name,
                            result.pages_found,
                            format_percent(result.idle_ratio),
                            if result.met_criteria {
                                " [WOULD SWAP]"
                            } else {
                                ""
                            }
                        );
                    }
                }
            }
        }
        Err(e) => eprintln!("Failed to create workflow: {}", e),
    }

    // Example 4: Analyze memory without swapping
    println!("\n--- Example 4: Memory Analysis ---");
    use etmem_rs::workflow::analyze_memory;
    match analyze_memory(pid) {
        Ok(report) => {
            println!("Memory Analysis:");
            println!("  Total scanned: {} VMAs", report.vmas_scanned);
            println!(
                "  Memory efficiency: {:.2}% idle",
                report.overall_idle_ratio * 100.0
            );
            if report.was_effective() {
                println!(
                    "  Potential savings: {} bytes",
                    format_bytes(report.bytes_swapped)
                );
            }
        }
        Err(e) => eprintln!("Failed: {}", e),
    }

    println!("\nExample completed successfully!");
    Ok(())
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

/// Format ratio as percentage string
fn format_percent(ratio: f64) -> String {
    format!("{:.1}%", ratio * 100.0)
}
