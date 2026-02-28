//! ETMEM Swap Example - Simple Page Swapping
//!
//! This example demonstrates ETMEM swap functionality:
//! 1. Allocate 10MB of memory using mmap
//! 2. Scan pages to mark them as idle (required before swap)
//! 3. Swap out idle pages
//! 4. Verify swap by reading /proc/self/smaps
//!
//! # Running the Example
//!
//! ```bash
//! sudo cargo run --example swap_example --package etmem-rs
//! ```

use etmem_rs::{
    AddressRange, ScanConfig, ScanSession, SwapConfig, SwapSession, SwapcacheConfig,
};
use std::env;
use std::process;

// Memory allocation size: 10 MB
const ALLOC_SIZE: usize = 10 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::args().any(|arg| arg == "--help") {
        println!("Usage: swap_example");
        println!();
        println!("Simple ETMEM swap example that:");
        println!("  1. Allocates 10MB of memory");
        println!("  2. Scans pages to mark as idle");
        println!("  3. Swaps out idle pages");
        println!("  4. Verifies swap via /proc/self/smaps");
        return Ok(());
    }

    // Check permissions
    if !etmem_rs::has_permission() {
        eprintln!("Error: This example requires root privileges (CAP_SYS_ADMIN)");
        eprintln!("Please run with sudo");
        std::process::exit(1);
    }

    // Check ETMEM availability
    if !etmem_rs::is_available() {
        eprintln!("Error: ETMEM is not available on this system");
        eprintln!("Please ensure etmem_scan.ko and etmem_swap.ko are loaded");
        std::process::exit(1);
    }

    println!("ETMEM Swap Example");
    println!("==================\n");

    // Enable kernel swap
    println!("Enabling kernel swap...");
    if let Err(e) = SwapcacheConfig::enable() {
        eprintln!("Warning: Failed to enable kernel swap: {}", e);
        eprintln!("This is expected if swap is already enabled.");
    } else {
        println!("Kernel swap enabled\n");
    }

    // Allocate memory
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

    let start_addr = ptr as u64;
    let end_addr = start_addr + ALLOC_SIZE as u64;

    println!(
        "Allocated {} MB at 0x{:x}-0x{:x}",
        ALLOC_SIZE / 1024 / 1024,
        start_addr,
        end_addr
    );

    // Touch all pages to ensure they're mapped
    unsafe {
        std::ptr::write_bytes(ptr, 0xAB, ALLOC_SIZE);
    }
    println!("Touched all pages to ensure they're mapped");

    // Get baseline swap stats
    let baseline = get_swap_for_range(start_addr, end_addr);
    println!("\nBaseline swap: {} KB", baseline / 1024);

    // Step 1: Scan pages to mark as idle (required before swap)
    // In ETMEM, pages become idle when they haven't been accessed since the last scan.
    // We scan once (marks current state), wait, then scan again to detect idle pages.
    println!("\nScanning pages to identify idle pages...");

    let scan_config = ScanConfig::default();
    let mut scan_session = ScanSession::new(process::id(), scan_config)?;

    let range = AddressRange {
        start: start_addr,
        end: end_addr,
    };

    // First scan - establishes baseline
    let _ = scan_session.read_range(range)?;

    // Wait for pages to become idle (not accessed)
    println!("Waiting 2 seconds for pages to become idle...");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Second scan - pages not accessed since first scan show as idle
    let pages = scan_session.read_range(range)?;
    let idle_pages: Vec<u64> = pages.iter().filter(|p| p.is_idle()).map(|p| p.address).collect();
    println!("Found {} idle pages out of {} total", idle_pages.len(), pages.len());

    // Step 2: Create swap session and swap out idle pages
    let swap_config = SwapConfig::default();
    let mut session = SwapSession::new(process::id(), swap_config)?;

    println!("\nSwapping out {} idle pages...", idle_pages.len());

    // Add idle page addresses and swap out
    let mut added = 0;
    for addr in &idle_pages {
        if session.add_address(*addr).is_ok() {
            added += 1;
        }
    }
    println!("Added {} pages to swap session", added);

    // Flush to swap pages (note: auto-flush may have already occurred)
    let flushed = session.flush()?;
    println!("Final flush: {} pages", flushed);
    println!("Total pages sent to kernel: {}", added);

    // Wait for swap to complete
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Get final swap stats
    let final_swap = get_swap_for_range(start_addr, end_addr);
    let swapped_amount = final_swap.saturating_sub(baseline);

    println!("\n========================================");
    println!("Results:");
    println!("  Baseline swap:  {} KB", baseline / 1024);
    println!("  Final swap:     {} KB", final_swap / 1024);
    println!(
        "  Swapped out:    {} KB ({} MB)",
        swapped_amount / 1024,
        swapped_amount / 1024 / 1024
    );
    println!(
        "  Expected:       {} KB ({} MB)",
        ALLOC_SIZE / 1024,
        ALLOC_SIZE / 1024 / 1024
    );

    if swapped_amount >= ALLOC_SIZE as u64 {
        println!("\n✓ SUCCESS: All pages were swapped out!");
    } else if swapped_amount > 0 {
        let pct = (swapped_amount as f64 / ALLOC_SIZE as f64) * 100.0;
        println!("\n⚠ PARTIAL: Only {:.1}% of pages swapped", pct);
    } else {
        println!("\n✗ No pages were swapped to disk");
        println!("  Note: This may be expected if:");
        println!("    - Swap space is not configured (check with 'swapon -s')");
        println!("    - Kernel is not configured to swap anonymous pages");
        println!("    - The ETMEM swap feature has additional requirements");
    }
    println!("========================================");

    // Cleanup
    unsafe {
        libc::munmap(ptr, ALLOC_SIZE);
    }
    println!("\nMemory freed.");

    Ok(())
}

/// Get swap usage for a memory range from /proc/self/smaps
fn get_swap_for_range(start: u64, end: u64) -> u64 {
    let smaps = match std::fs::read_to_string("/proc/self/smaps") {
        Ok(content) => content,
        Err(_) => return 0,
    };

    let mut total_swap = 0u64;
    let mut in_range = false;

    for line in smaps.lines() {
        // Parse region header like "7f8b4000000-7f8b4a00000 rw-p 00000000 00:00 0"
        if line.contains('-') && line.contains(':') {
            in_range = false;
            if let Some((addr_part, _)) = line.split_once(' ')
                && let Some((range_start, range_end)) = addr_part.split_once('-')
                && let (Ok(rs), Ok(re)) = (u64::from_str_radix(range_start, 16), u64::from_str_radix(range_end, 16))
            {
                // Check if this region overlaps with our range
                if rs <= end && re >= start {
                    in_range = true;
                }
            }
        }

        // Parse Swap: line like "Swap:                  0 kB"
        if in_range
            && line.starts_with("Swap:")
            && let Some(kb_part) = line.split_whitespace().nth(1)
            && let Ok(kb) = kb_part.parse::<u64>()
        {
            total_swap += kb * 1024;
        }
    }

    total_swap
}
