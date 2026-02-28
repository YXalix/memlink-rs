# memlink-rs

A Rust-based memory linking and analysis toolkit that provides utilities for examining, analyzing, and linking memory regions between processes. This project demonstrates Foreign Function Interface (FFI) capabilities with C libraries and cross-platform memory management.

## Features

- **Memory Analysis**: Analyze memory usage patterns in running processes
- **Memory Linking**: Create links between memory regions of different processes
- **Process Memory Inspection**: List and examine memory regions with detailed information
- **FFI Integration**: Demonstrates safe Rust wrappers around C libraries
- **Cross-Platform Support**: Designed to work on Linux, macOS, and Windows (with platform-specific implementations)

## Architecture

The project consists of several key components:

### Core Modules

- **`memory.rs`**: Core memory region representation and process memory enumeration
- **`analyzer.rs`**: Memory analysis utilities including statistics and pattern searching
- **`linker.rs`**: Memory linking functionality for creating inter-process memory connections
- **`main.rs`**: Command-line interface and application entry point

### FFI Module

- **`modules/math-utils/`**: Example C library with Rust FFI bindings demonstrating:
  - Basic math operations (add, multiply, divide, factorial)
  - Version management
  - String handling between C and Rust
  - Build system integration with `bindgen`

## Installation

### Prerequisites

- Rust toolchain (latest stable)
- C compiler (gcc/clang)
- Make
- Linux development headers (for `/proc` filesystem access)

### Build Instructions

```bash
# Clone the repository
git clone https://github.com/yourusername/memlink-rs.git
cd memlink-rs

# Build the project
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

## Usage

### Basic Commands

```bash
# Test the math utilities (FFI demonstration)
memlink test math

# List memory regions for a process
memlink list --pid 1234

# Analyze memory usage with detailed output
memlink analyze --pid 1234 --format text

# Export analysis as JSON
memlink analyze --pid 1234 --format json

# Link memory between processes (requires elevated privileges)
memlink link --source 1234 --target 5678 --address 0x7fff0000 --size 4096
```

### Command-Line Options

#### Global Options
- `-v, --verbose`: Increase verbosity (can be used multiple times)
- `-h, --help`: Print help information
- `-V, --version`: Print version information

#### Subcommands

##### `analyze`
Analyze memory usage patterns in a target process.

```bash
memlink analyze --pid <PID> [--format <FORMAT>]
```

Options:
- `--pid <PID>`: Process ID to analyze (required)
- `--format <FORMAT>`: Output format: `text` or `json` (default: text)

##### `link`
Create a memory link between two processes.

```bash
memlink link --source <PID> --target <PID> --address <ADDRESS> --size <SIZE>
```

Options:
- `--source <PID>`: Source process ID (required)
- `--target <PID>`: Target process ID (required)
- `--address <ADDRESS>`: Memory region address in hex format (required)
- `--size <SIZE>`: Size of memory region in bytes (required)

##### `list`
List memory regions for a process.

```bash
memlink list --pid <PID> [--detailed]
```

Options:
- `--pid <PID>`: Process ID (required)
- `--detailed`: Show detailed information for each region

##### `test`
Run built-in tests, including FFI demonstrations.

```bash
memlink test [--test <NAME>]
```

Options:
- `--test <NAME>`: Run specific test (currently only "math" available)

## Examples

### Memory Analysis Example

```bash
# Analyze a web browser process
$ memlink analyze --pid 12345 --format text
Memory Analysis for PID: 12345
--------------------------------------------------
0x7fff0000-0x7fff1000    4.0KB    Private    rw-p
0x7fff2000-0x7fff3000    4.0KB    Stack      rw-p
0x7fff4000-0x7fff5000    4.0KB    Heap       rw-p
...
```

### JSON Output Example

```bash
$ memlink analyze --pid 12345 --format json
[
  {
    "base_address": 2147450880,
    "size": 4096,
    "protection": "rw-p",
    "region_type": "Private",
    "state": "active",
    "mapped_file": null
  },
  ...
]
```

### Testing FFI Integration

```bash
$ memlink test math
Testing math utilities:
  Version: math_utils v1.0.0
  Add(5, 3) = 8
  Multiply(4, 7) = 28
  Divide(10, 2) = Some(5.0)
  Factorial(5) = Some(120)
Hello from memlink!

Tests completed successfully!
```

## API Documentation

### MemoryRegion

Represents a memory region in a process.

```rust
pub struct MemoryRegion {
    pub base_address: usize,
    pub size: usize,
    pub protection: String,
    pub region_type: RegionType,
    pub state: String,
    pub mapped_file: Option<String>,
}
```

### MemoryAnalyzer

Provides memory analysis capabilities.

```rust
impl MemoryAnalyzer {
    pub fn new(pid: u32) -> Result<Self>;
    pub fn analyze(&self) -> Result<Vec<MemoryRegion>>;
    pub fn get_memory_stats(&self) -> Result<MemoryStats>;
    pub fn find_heap_regions(&self) -> Result<Vec<MemoryRegion>>;
    pub fn find_stack_regions(&self) -> Result<Vec<MemoryRegion>>;
    pub fn find_executable_regions(&self) -> Result<Vec<MemoryRegion>>;
}
```

### MathUtils (FFI Example)

Safe Rust wrapper around C math library.

```rust
impl MathUtils {
    pub fn version() -> String;
    pub fn add(a: i32, b: i32) -> i32;
    pub fn multiply(a: i32, b: i32) -> i32;
    pub fn divide(a: f64, b: f64) -> Option<f64>;
    pub fn factorial(n: i32) -> Option<i32>;
    pub fn print_message(message: &str) -> Result<(), std::ffi::NulError>;
}
```

## Platform Support

### Linux
- Full support via `/proc` filesystem
- Memory region enumeration
- Process memory analysis
- Memory linking (simulated)

### macOS
- Planned support using `vm_region` APIs
- Currently returns "not implemented" errors

### Windows
- Planned support using `VirtualQueryEx` APIs
- Currently returns "not implemented" errors

## Development

### Project Structure

```
memlink-rs/
├── Cargo.toml                 # Main workspace configuration
├── src/
│   ├── main.rs               # CLI application entry point
│   ├── memory.rs             # Memory region definitions
│   ├── analyzer.rs           # Memory analysis utilities
│   └── linker.rs             # Memory linking functionality
├── modules/
│   └── math-utils/           # FFI example module
│       ├── Cargo.toml
│       ├── build.rs          # Build script for C library
│       ├── src/
│       │   └── lib.rs        # Rust FFI wrapper
│       └── hugemem/
│           ├── math_utils.h  # C library header
│           ├── math_utils.c  # C library implementation
│           ├── Makefile      # C library build system
│           └── test.c        # C library tests
├── examples/                 # Usage examples
├── tests/                    # Integration tests
└── target/                   # Build artifacts
```

### Adding New Features

1. **New Memory Analysis**: Add methods to `MemoryAnalyzer`
2. **New Memory Operations**: Extend `MemoryLinker`
3. **Platform Support**: Add `#[cfg(target_os = "...")]` implementations
4. **FFI Modules**: Create new modules in `modules/` following the math-utils pattern

### Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test -p math-utils

# Run with verbose output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration
```

## Security Considerations

⚠️ **Important Security Notes:**

1. **Process Memory Access**: This tool requires elevated privileges to access other processes' memory
2. **Memory Linking**: Creating memory links between processes can be dangerous and requires careful permission management
3. **Production Use**: This is a demonstration tool - additional security measures would be needed for production use
4. **Platform Permissions**: Ensure proper file system permissions on `/proc` and related system interfaces

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Add tests for new functionality
- Update documentation as needed
- Ensure all tests pass (`cargo test`)

### CI/CD Checks

All pull requests must pass the following checks before merging:

```bash
# Check code formatting
cargo fmt --all -- --check

# Build and test the workspace
cargo build --workspace
cargo test --workspace

# Run clippy lints (treat warnings as errors)
cargo clippy --workspace --no-default-features -- -D warnings

# Generate documentation
cargo doc --workspace --no-deps --no-default-features

# Check code with doc tests
cargo test --doc --workspace
```

**Required Checks:**

| Check | Command | Description |
|-------|---------|-------------|
| Format | `cargo fmt --all -- --check` | Ensures consistent code formatting |
| Lint | `cargo clippy --workspace --no-default-features -- -D warnings` | Static analysis with zero warnings tolerance |
| Documentation | `cargo doc --workspace --no-deps --no-default-features` | Verifies docs build without errors |
| Tests | `cargo test --workspace` | Runs all unit and integration tests |

Run these checks locally before pushing to ensure your PR passes CI.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Rust FFI documentation and examples
- Linux `/proc` filesystem documentation
- Cross-platform memory management research
- C library integration techniques

## Roadmap

- [ ] Complete macOS implementation with `vm_region` APIs
- [ ] Add Windows support with `VirtualQueryEx` APIs
- [ ] Implement actual memory linking with shared memory
- [ ] Add memory pattern recognition and analysis
- [ ] Create GUI interface for memory visualization
- [ ] Add more comprehensive FFI examples
- [ ] Implement memory protection analysis
- [ ] Add memory leak detection capabilities
- [ ] Create plugin system for custom analyzers
- [ ] Add support for containerized processes

## Contact

Nash Zhou <zhouhaipeng@nuaa.edu.cn>

Project Link: [https://github.com/yourusername/memlink-rs](https://github.com/yourusername/memlink-rs)