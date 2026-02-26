//! Memlink: a distributed memory pooling and linking tool
//!
//! This tool allows exporting and importing memory regions across different nodes
//! in a distributed system, enabling efficient memory sharing and management.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::Context;
use clap::{Parser, Subcommand};
use log::info;
use obmm_rs::{mem_export, ObmmExportFlags, UbPrivData, MAX_NUMA_NODES};

/// Memlink CLI arguments
#[derive(Parser, Debug)]
#[command(name = "memlink")]
#[command(about = "Memory linking and analysis utilities")]
#[command(version)]
struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Export memory for remote access
    Export {
        /// NUMA node ID to export memory from
        #[arg(short, long, default_value = "1")]
        node: usize,
        /// Size of memory to export in MB
        #[arg(short, long, default_value = "128")]
        size: usize,
    },
    /// Measure bandwidth and latency using mar_perf
    MarPerf {
        /// Chip ID
        #[arg(short, long, default_value = "0")]
        chip_id: u32,
        /// Die ID
        #[arg(short, long, default_value = "0")]
        die_id: u32,
        /// Port ID to measure
        #[arg(short, long, default_value = "0")]
        port: u32,
        /// Measurement time in milliseconds (1-3600)
        #[arg(short, long, default_value = "1000")]
        time: u32,
    },
    /// ETMEM: Enhanced Tiered Memory management
    Etmem {
        #[command(subcommand)]
        action: EtmemCommands,
    },
}

/// ETMEM subcommands for tiered memory management
#[derive(Subcommand, Debug)]
enum EtmemCommands {
    /// Scan process memory for idle/accessed pages
    Scan {
        /// Process ID to scan (default: current process)
        #[arg(short, long)]
        pid: Option<u32>,
        /// Only scan huge pages (2MB/1GB)
        #[arg(long)]
        huge_only: bool,
        /// Report dirty pages
        #[arg(long)]
        dirty: bool,
        /// Only show idle (cold) pages
        #[arg(long)]
        idle_only: bool,
    },
    /// Swap out cold pages to free memory
    Swap {
        /// Process ID to swap pages from
        #[arg(short, long)]
        pid: Option<u32>,
        /// Virtual addresses to swap (hex, comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        addrs: Vec<String>,
    },
    /// Configure kernel swap settings
    Config {
        /// Enable kernel swap
        #[arg(long)]
        enable: bool,
        /// Disable kernel swap
        #[arg(long)]
        disable: bool,
        /// Show current status
        #[arg(long)]
        status: bool,
    },
}

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let cli = Cli::parse();

    match cli.command {
        Commands::Export { node, size } => {
            info!("Exporting memory from NUMA node {node}, size: {size} MB");
            export_memory(node, size)?;
        }
        Commands::MarPerf {
            chip_id,
            die_id,
            port,
            time,
        } => {
            info!("Running mar_perf measurement on chip {chip_id}, die {die_id}, port {port}, time: {time}ms");
            run_mar_perf(chip_id, die_id, port, time)?;
        }
        Commands::Etmem { action } => {
            handle_etmem_command(action)?;
        }
    }

    Ok(())
}

/// Export memory from a NUMA node
fn export_memory(node: usize, size_mb: usize) -> anyhow::Result<()> {
    let export_id = node;
    let size_bytes = size_mb * 1024 * 1024;

    let mut lens = vec![0; MAX_NUMA_NODES];
    lens.get_mut(export_id)
        .map(|v| *v = size_bytes)
        .with_context(|| format!("Failed to set length for NUMA node {export_id}"))?;

    let (mem_id, desc) = mem_export::<UbPrivData>(&lens, ObmmExportFlags::ALLOWMMAP)
        .with_context(|| "Failed to export memory")?;

    info!("Exported memory with MemID: {mem_id}");
    info!("Memory Descriptor: {desc:?}");

    Ok(())
}

/// Run mar_perf measurement and display results
fn run_mar_perf(chip_id: u32, die_id: u32, port: u32, time: u32) -> anyhow::Result<()> {
    let result = ubfwctl::mar_perf_measure(chip_id, die_id, port, time)
        .with_context(|| "mar_perf measurement failed")?;

    println!("{result}");

    Ok(())
}

/// Handle ETMEM subcommands
fn handle_etmem_command(action: EtmemCommands) -> anyhow::Result<()> {
    use etmem_rs::{
        IdlePageScanner, PageSwapper, ScanConfig, ScanFlags, SwapcacheConfig,
    };

    match action {
        EtmemCommands::Scan {
            pid,
            huge_only,
            dirty,
            idle_only,
        } => {
            let pid = pid.unwrap_or_else(|| std::process::id());
            println!("Scanning process {pid} for memory pages...");

            // Check if ETMEM is available
            if !etmem_rs::is_available() {
                anyhow::bail!("ETMEM is not available. Check kernel configuration (CONFIG_ETMEM=y).");
            }

            // Build scan configuration
            let mut flags = ScanFlags::empty();
            if huge_only {
                flags |= ScanFlags::SCAN_HUGE_PAGE;
            }
            if dirty {
                flags |= ScanFlags::SCAN_DIRTY_PAGE;
            }
            let config = ScanConfig::default().with_flags(flags);

            // Scan the process
            let pages = IdlePageScanner::scan_process(pid, config)
                .with_context(|| format!("Failed to scan process {pid}"))?;

            // Filter and display results
            let filtered_pages: Vec<_> = if idle_only {
                pages.into_iter().filter(|p| p.is_idle()).collect()
            } else {
                pages
            };

            println!("\nFound {} memory regions:", filtered_pages.len());
            println!("{:-^60}", "");
            println!("{:>16}  {:<15}  {:<10}  {:<12}", "Address", "Type", "Count", "Size");
            println!("{:-^60}", "");

            let mut total_bytes = 0u64;
            for page in &filtered_pages {
                let size = page.total_size();
                total_bytes += size;
                println!(
                    "{:>16x}  {:<15?}  {:<10}  {:<12}",
                    page.address,
                    page.page_type,
                    page.count,
                    etmem_rs::format_bytes(size)
                );
            }

            println!("{:-^60}", "");
            println!("Total: {} bytes ({})", total_bytes, etmem_rs::format_bytes(total_bytes));

            // Show statistics
            let stats = etmem_rs::IdlePageStats::from_pages(&filtered_pages);
            println!("\nStatistics:");
            println!("  Idle pages:     {} ({})", stats.idle_pages, etmem_rs::format_bytes(stats.idle_bytes));
            println!("  Accessed pages: {} ({})", stats.accessed_pages, etmem_rs::format_bytes(stats.accessed_bytes));
            println!("  Huge pages:     {}", stats.huge_pages);
            println!("  Idle ratio:     {:.1}%", stats.idle_ratio() * 100.0);
        }
        EtmemCommands::Swap { pid, addrs } => {
            if addrs.is_empty() {
                anyhow::bail!("No addresses provided. Use --addrs to specify addresses to swap.");
            }

            let pid = pid.unwrap_or_else(|| std::process::id());
            println!("Swapping pages in process {pid}...");

            // Parse addresses
            let mut parsed_addrs = Vec::new();
            for addr_str in &addrs {
                let addr = u64::from_str_radix(addr_str.trim_start_matches("0x"), 16)
                    .with_context(|| format!("Invalid address: {addr_str}"))?;
                parsed_addrs.push(addr);
            }

            // Swap the pages
            let swapped = PageSwapper::swap_pages(pid, &parsed_addrs)
                .with_context(|| format!("Failed to swap pages in process {pid}"))?;

            println!("Successfully swapped {swapped} pages");
        }
        EtmemCommands::Config {
            enable,
            disable,
            status,
        } => {
            if enable {
                SwapcacheConfig::enable()
                    .with_context(|| "Failed to enable kernel swap")?;
                println!("Kernel swap enabled");
            } else if disable {
                SwapcacheConfig::disable()
                    .with_context(|| "Failed to disable kernel swap")?;
                println!("Kernel swap disabled");
            } else if status || (!enable && !disable) {
                let enabled = SwapcacheConfig::is_enabled()
                    .with_context(|| "Failed to check kernel swap status")?;
                println!("Kernel swap status: {}", if enabled { "enabled" } else { "disabled" });

                // Also check ETMEM availability
                println!("ETMEM available: {}", etmem_rs::is_available());
                println!("Root privileges: {}", etmem_rs::has_permission());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etmem_availability() {
        // Just check that the function works
        let available = etmem_rs::is_available();
        println!("ETMEM available: {available}");
    }

    #[test]
    fn test_etmem_permission() {
        // Just check that the function works
        let has_perm = etmem_rs::has_permission();
        println!("Has root permission: {has_perm}");
    }
}
