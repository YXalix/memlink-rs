# ETMEM Examples

This directory contains example programs demonstrating the usage of the `etmem-rs` crate for memory management with the Linux ETMEM (Enhanced Tiered Memory) subsystem.

## Prerequisites

- Linux kernel with ETMEM support (`CONFIG_ETMEM=y` or modules)
- Root privileges (CAP_SYS_ADMIN capability required)
- ETMEM kernel modules loaded:
  ```bash
  modprobe etmem_scan   # For scanning functionality
  modprobe etmem_swap   # For swapping functionality
  ```

## Examples

### 1. Scan Example (`scan_example.rs`)

A simple self-contained example that demonstrates basic ETMEM scanning functionality:

```bash
# Default: May show huge pages (2MB) for large allocations
cargo run --example scan_example --package etmem-rs

# Force 4KB page granularity
cargo run --example scan_example --package etmem-rs -- --no-huge
```

**What it does:**
- Allocates 10 MB of memory using mmap
- Touches all pages to make them "accessed"
- Scans the allocated memory range using ETMEM
- Displays memory statistics (accessed pages, idle pages, etc.)

**Page Size Options:**

| Flag | Page Size | Use Case |
|------|-----------|----------|
| (none) | 2MB (huge pages) | Default kernel behavior |
| `--no-huge` | 4KB (standard pages) | Fine-grained memory analysis |

**Example output (with --no-huge for 4KB pages):**
```
ETMEM Scan Example
==================

Allocated 10 MB of memory at 0xffff9e400000
Disabled transparent huge pages for this allocation
Initialized memory (all pages touched)

Scanning memory range: 0xffff9e400000 - 0xffff9ee00000

Scan Results:
Address              Type             Count        Size
----------------------------------------------------------------------
0x0000ffff9e400000 PteAccessed      16           64.00 KB
0x0000ffff9e410000 PteAccessed      16           64.00 KB
0x0000ffff9e420000 PteAccessed      16           64.00 KB
...
----------------------------------------------------------------------

Summary:
  Total pages found: 160
  Accessed (hot):    10.00 MB
  Idle (cold):       0 B
  Holes (unmapped):  0 B
```

**Without --no-huge (huge pages):**
```
Scan Results:
Address              Type             Count        Size
----------------------------------------------------------------------
0x0000ffffa1800000 PmdAccessed      6            12.00 MB
----------------------------------------------------------------------

Summary:
  Total pages found: 1
  Accessed (hot):    12.00 MB
  Idle (cold):       0 B
```

**Features demonstrated:**
- Basic memory allocation and scanning
- Using `ScanSession` with specific address ranges
- Interpreting page scan results

---

### 2. Swap Example (`swap_example.rs`)

Simple demonstration of ETMEM swap functionality:

```bash
cargo run --example swap_example --package etmem-rs
```

**What it does:**
- Allocates 10 MB of memory using mmap
- Touches all pages to ensure they're mapped
- Scans pages to identify idle pages (required before swap)
- Swaps out idle pages
- Verifies swap by reading `/proc/self/smaps`

**Requirements:**
- Swap space must be configured (check with `swapon -s`)
- `etmem_swap.ko` kernel module must be loaded

**Example output:**
```
ETMEM Swap Example
==================

Enabling kernel swap...
Kernel swap enabled

Allocated 10 MB at 0xffffb9a00000-0xffffba400000
Touched all pages to ensure they're mapped

Baseline swap: 0 KB

Scanning pages to identify idle pages...
Waiting 2 seconds for pages to become idle...
Found 160 idle pages out of 160 total

Swapping out 160 idle pages...
Added 160 pages to swap session
Final flush: 0 pages
Total pages sent to kernel: 160

========================================
Results:
  Baseline swap:  0 KB
  Final swap:     0 KB
  Swapped out:    0 KB (0 MB)
  Expected:       10240 KB (10 MB)

✗ No pages were swapped to disk
  Note: This may be expected if:
    - Swap space is not configured (check with 'swapon -s')
    - Kernel is not configured to swap anonymous pages
    - The ETMEM swap feature has additional requirements
========================================

Memory freed.
```

**Features demonstrated:**
- Enabling kernel swap via `SwapcacheConfig::enable()`
- Using `SwapSession` to swap out idle pages
- Scan-then-swap workflow pattern
- Verifying swap via `/proc/self/smaps`

---

## Technical Details

### PIP (Proc Idle Page) Format

The kernel returns idle page data in a compact PIP encoding format (based on etmemd_scan.c):

```
[a0] [XX XX XX XX XX XX XX XX] [YY] ...
 |    |                        |
 |    |                        +-- Page type/count byte
 |    +-- 64-bit address (8 bytes, big-endian)
 +-- SET_HVA command (0xa0 = type=10, count=0)
```

The Rust library correctly decodes this format:
- SET_HVA (0xa0) marks an address update
- 8 bytes encode the 64-bit physical address (big-endian)
- Subsequent bytes encode page types and counts

Example decoding (from etmemd_scan.c get_address_from_buf):
- Input: `[a0, 00, 00, ff, ff, b2, 40, 00, 00, 0f, ...]`
- Address: `0x0000ffffb2400000` (read as big-endian 64-bit)

### API Usage

#### Scanning Memory
```rust
use etmem_rs::{AddressRange, ScanConfig, ScanSession};

// Create a scan session for the current process
let config = ScanConfig::default();
let mut session = ScanSession::new(std::process::id(), config)?;

// Define a memory range to scan
let range = AddressRange {
    start: 0xffff8aa00000,
    end: 0xffff8b400000,
};

// Scan the range
let pages = session.read_range(range)?;

// Process results
for page in pages {
    println!("Address: 0x{:x}, Type: {:?}, Count: {}",
             page.address, page.page_type, page.count);
}
```

#### Swapping Memory
```rust
use etmem_rs::{SwapConfig, SwapSession, SwapcacheConfig};

// Enable kernel swap globally
SwapcacheConfig::enable()?;

// Create a swap session for the current process
let config = SwapConfig::default();
let mut session = SwapSession::new(std::process::id(), config)?;

// Add addresses to swap (must be page-aligned)
session.add_address(0x7fff0000)?;
session.add_address(0x7fff1000)?;

// Flush to perform the swap
session.flush()?;
```

---

## Troubleshooting

### "No pages found in scanned range"

If you get no results:
1. Verify kernel modules are loaded: `lsmod | grep etmem`
2. Check you have root privileges: `id` should show uid=0
3. Ensure the memory range is valid and mapped
4. Check dmesg for kernel errors: `dmesg | tail -20`

### Permission denied

ETMEM requires CAP_SYS_ADMIN capability (root). Run with sudo:
```bash
sudo cargo run --example scan_example --package etmem-rs
sudo cargo run --example swap_example --package etmem-rs
```

### Module not found

If you see "ETMEM is not available":
1. Check kernel config: `zgrep ETMEM /proc/config.gz` or `grep ETMEM /boot/config-$(uname -r)`
2. Build and load the kernel modules if not present

### Swap errors

If swapping fails:
1. Check swap space: `swapon -s` or `cat /proc/swaps`
2. Ensure `etmem_swap.ko` is loaded: `lsmod | grep etmem_swap`
3. Check kernel messages: `dmesg | tail -20`

---

## Testing

These examples have been tested on Linux with ETMEM kernel modules:

```bash
# Load modules
modprobe etmem_scan
modprobe etmem_swap
lsmod | grep etmem
# etmem_swap             16384  0
# etmem_scan             24576  0

# Run examples
cargo run --example scan_example --package etmem-rs
sudo cargo run --example swap_example --package etmem-rs
```
