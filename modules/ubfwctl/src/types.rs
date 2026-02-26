//! Data types and structures for ubfwctl operations

use serde::{Deserialize, Serialize};

/// Number of ports measured together (`mar_perf` measures pairs of ports)
pub const BA_MAR_PERF_NUM_TWO: u32 = 2;
/// Validate port count (max 20 ports as per C code)
pub const MAX_PORTS: u32 = 20;

/// Data indices in the kernel response array
pub mod data_indices {
    /// Port ID index
    pub const PORT_ID_IDX: usize = 0;
    /// Clock cycle frequency index
    pub const CLOCK_CYCLE_IDX: usize = 1;
    /// Write flux (bytes) index
    pub const FLUX_WR_IDX: usize = 2;
    /// Read flux (bytes) index
    pub const FLUX_RD_IDX: usize = 3;
    /// Total flux (bytes) index
    pub const FLUX_SUM_IDX: usize = 4;
    /// Write command count index
    pub const WR_CMD_IDX: usize = 5;
    /// Read command count index
    pub const RD_CMD_IDX: usize = 6;
    /// Total command count index
    pub const SUM_CMD_IDX: usize = 7;
    /// Write latency cycles index
    pub const WLATCNT_FIRST_IDX: usize = 8;
    /// Read latency cycles index
    pub const RLATCNT_FIRST_IDX: usize = 9;
    /// Maximum expected data array size
    pub const BA_MAR_PERF_MAX_SIZE: usize = 64;
}

/// Raw `mar_perf` query data from kernel
///
/// This structure matches the data layout returned by the kernel
/// when querying `BA_MAR_PEFR_STATS`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct MarPerfQuery {
    /// Port ID
    pub port_id: u32,
    /// Write flux in bytes
    pub flux_wr: u32,
    /// Read flux in bytes
    pub flux_rd: u32,
    /// Total flux (write + read) in bytes
    pub flux_sum: u32,
    /// Write command count
    pub wr_cmd_cnt: u32,
    /// Read command count
    pub rd_cmd_cnt: u32,
    /// Total command count
    pub sum_cmd_cnt: u32,
    /// Write latency in clock cycles
    pub wlatcnt_first: u32,
    /// Read latency in clock cycles
    pub rlatcnt_first: u32,
}

impl MarPerfQuery {
    /// Extract data from raw kernel response array
    ///
    /// # Arguments
    /// * `data` - Raw u32 array from kernel
    ///
    /// # Returns
    /// `MarPerfQuery` populated with extracted values
    ///
    /// # Panics
    /// Panics if `data` has fewer than 10 elements
    #[must_use]
    pub fn from_raw_data(data: &[u32]) -> Self {
        assert!(
            data.len() > data_indices::RLATCNT_FIRST_IDX,
            "Insufficient data from kernel"
        );

        Self {
            port_id: data[data_indices::PORT_ID_IDX],
            flux_wr: data[data_indices::FLUX_WR_IDX],
            flux_rd: data[data_indices::FLUX_RD_IDX],
            flux_sum: data[data_indices::FLUX_SUM_IDX],
            wr_cmd_cnt: data[data_indices::WR_CMD_IDX],
            rd_cmd_cnt: data[data_indices::RD_CMD_IDX],
            sum_cmd_cnt: data[data_indices::SUM_CMD_IDX],
            wlatcnt_first: data[data_indices::WLATCNT_FIRST_IDX],
            rlatcnt_first: data[data_indices::RLATCNT_FIRST_IDX],
        }
    }
}

/// Configuration for `mar_perf` measurement
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct MarPerfConfig {
    /// Port ID to measure
    pub port_id: u32,
    /// Measurement time in milliseconds
    pub time_ms: u32,
}

impl MarPerfConfig {
    /// Create a new `mar_perf` configuration
    ///
    /// # Arguments
    /// * `port_id` - Port ID to measure
    /// * `time_ms` - Measurement time in milliseconds
    #[must_use]
    pub const fn new(port_id: u32, time_ms: u32) -> Self {
        Self { port_id, time_ms }
    }
}

/// Calculated `mar_perf` results
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct MarPerfResult {
    /// First port ID in the pair
    pub first_port_id: u32,
    /// Second port ID in the pair
    pub second_port_id: u32,
    /// Write traffic in bytes/second
    pub wr_traffic: u32,
    /// Read traffic in bytes/second
    pub rd_traffic: u32,
    /// Total traffic in bytes/second
    pub sum_traffic: u32,
    /// Average write payload length in bytes
    pub wr_pld_avg_len: u32,
    /// Average read payload length in bytes
    pub rd_pld_avg_len: u32,
    /// Average payload length in bytes
    pub pld_avg_len: u32,
    /// Write latency in nanoseconds
    pub wr_delayed: u32,
    /// Read latency in nanoseconds
    pub rd_delayed: u32,
}

impl MarPerfResult {
    /// Calculate results from raw query data
    ///
    /// # Arguments
    /// * `query` - Raw query data from kernel
    /// * `time_ms` - Measurement time in milliseconds
    /// * `clock_freq_hz` - Clock frequency in Hz (cycles per second)
    ///
    /// # Returns
    /// `MarPerfResult` with calculated values
    #[must_use]
    pub fn calculate(query: &MarPerfQuery, time_ms: u32, clock_freq_hz: u32) -> Self {
        let duration_secs = f64::from(time_ms) * crate::error::MS_TO_S;
        // Convert clock frequency to cycle period in nanoseconds
        // clock_cycle_ns = 1e9 ns/s / clock_freq_hz cycles/s = ns/cycle
        let clock_cycle_ns = if clock_freq_hz > 0 {
            1e9_f64 / f64::from(clock_freq_hz)
        } else {
            0.0
        };

        // Calculate traffic in bytes/second
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let wr_traffic = if duration_secs > 0.0 {
            (f64::from(query.flux_wr) / duration_secs) as u32
        } else {
            0
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rd_traffic = if duration_secs > 0.0 {
            (f64::from(query.flux_rd) / duration_secs) as u32
        } else {
            0
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sum_traffic = if duration_secs > 0.0 {
            (f64::from(query.flux_sum) / duration_secs) as u32
        } else {
            0
        };

        // Calculate average payload lengths
        let wr_pld_avg_len = if query.wr_cmd_cnt > 0 {
            query.flux_wr / query.wr_cmd_cnt
        } else {
            0
        };

        let rd_pld_avg_len = if query.rd_cmd_cnt > 0 {
            query.flux_rd / query.rd_cmd_cnt
        } else {
            0
        };

        let pld_avg_len = if query.sum_cmd_cnt > 0 {
            query.flux_sum / query.sum_cmd_cnt
        } else {
            0
        };

        // Calculate latency in nanoseconds
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let wr_delayed = if clock_cycle_ns > 0.0 {
            (f64::from(query.wlatcnt_first) * clock_cycle_ns) as u32
        } else {
            0
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rd_delayed = if clock_cycle_ns > 0.0 {
            (f64::from(query.rlatcnt_first) * clock_cycle_ns) as u32
        } else {
            0
        };

        // Determine first and second port IDs (mar_perf measures pairs)
        let mut first_port_id = query.port_id;
        let mut second_port_id = query.port_id;

        if query.port_id.is_multiple_of(BA_MAR_PERF_NUM_TWO) {
            second_port_id += 1;
        } else {
            first_port_id -= 1;
        }

        Self {
            first_port_id,
            second_port_id,
            wr_traffic,
            rd_traffic,
            sum_traffic,
            wr_pld_avg_len,
            rd_pld_avg_len,
            pld_avg_len,
            wr_delayed,
            rd_delayed,
        }
    }
}

impl std::fmt::Display for MarPerfResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "-------------------------- ba-mar_perf --------------------------")?;
        writeln!(f, "port_id: {} {}", self.first_port_id, self.second_port_id)?;
        writeln!(f, "wr_traffic: {}", self.wr_traffic)?;
        writeln!(f, "rd_traffic: {}", self.rd_traffic)?;
        writeln!(f, "sum_traffic: {}", self.sum_traffic)?;
        writeln!(f, "wr_pld_avg_len: {}", self.wr_pld_avg_len)?;
        writeln!(f, "rd_pld_avg_len: {}", self.rd_pld_avg_len)?;
        writeln!(f, "pld_avg_len: {}", self.pld_avg_len)?;
        writeln!(f, "wr_delayed: {}", self.wr_delayed)?;
        write!(f, "rd_delayed: {}", self.rd_delayed)
    }
}

/// Port information structure
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct PortInfo {
    /// Port ID
    pub port_id: u32,
    /// Link status (0 = down, 1 = up)
    pub link_status: u32,
    /// Link state information
    pub link_state_info: u32,
    /// Port type (0 = eth, 1 = ub)
    pub port_type: u32,
    /// Reserved
    pub reserved: [u32; 2],
}

impl PortInfo {
    /// Get port type as string
    #[must_use]
    pub const fn port_type_str(&self) -> &'static str {
        if self.port_type == 0 {
            "eth"
        } else {
            "ub"
        }
    }

    /// Get link status as string
    #[must_use]
    pub const fn link_status_str(&self) -> &'static str {
        if self.link_status == 0 {
            "down"
        } else {
            "up"
        }
    }
}

/// IO die information with port details
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoDieInfo {
    /// Number of ports
    pub port_count: u32,
    /// Chip ID
    pub chip_id: u32,
    /// Die ID
    pub die_id: u32,
    /// Reserved
    pub reserved: [u32; 3],
    /// Port information array (flexible)
    pub ports: Vec<PortInfo>,
}

impl IoDieInfo {
    /// Parse IO die info from raw kernel response data
    ///
    /// # Arguments
    /// * `data` - Raw u8 array from kernel containing `fwctl_io_die_info` followed by `port_info` array
    ///
    /// # Returns
    /// `Ok(IoDieInfo)` on success, `Err(String)` on failure
    ///
    /// # Errors
    /// Returns an error if the data is too small or malformed
    pub fn from_raw_data(data: &[u8]) -> Result<Self, String> {
        // Header size: port_count (4) + chip_id (4) + die_id (4) + reserved[3] (12) = 24 bytes
        const HEADER_SIZE: usize = 24;
        const PORT_INFO_SIZE: usize = 24; // size_of::<PortInfo>()

        if data.len() < HEADER_SIZE {
            return Err(format!(
                "Insufficient data: got {} bytes, need at least {}",
                data.len(),
                HEADER_SIZE
            ));
        }

        // Parse header fields
        let port_count = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
        let chip_id = u32::from_ne_bytes([data[4], data[5], data[6], data[7]]);
        let die_id = u32::from_ne_bytes([data[8], data[9], data[10], data[11]]);

        if port_count == 0 || port_count > MAX_PORTS {
            return Err(format!("Invalid port count: {port_count}"));
        }

        let expected_size = HEADER_SIZE + (port_count as usize) * PORT_INFO_SIZE;
        if data.len() < expected_size {
            return Err(format!(
                "Insufficient data for {} ports: got {} bytes, need {}",
                port_count,
                data.len(),
                expected_size
            ));
        }

        // Parse port info array
        let mut ports = Vec::with_capacity(port_count as usize);
        let mut offset = HEADER_SIZE;

        for _ in 0..port_count {
            let port_info = PortInfo {
                port_id: u32::from_ne_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]),
                link_status: u32::from_ne_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]),
                link_state_info: u32::from_ne_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]),
                port_type: u32::from_ne_bytes([
                    data[offset + 12],
                    data[offset + 13],
                    data[offset + 14],
                    data[offset + 15],
                ]),
                reserved: [
                    u32::from_ne_bytes([
                        data[offset + 16],
                        data[offset + 17],
                        data[offset + 18],
                        data[offset + 19],
                    ]),
                    u32::from_ne_bytes([
                        data[offset + 20],
                        data[offset + 21],
                        data[offset + 22],
                        data[offset + 23],
                    ]),
                ],
            };
            ports.push(port_info);
            offset += PORT_INFO_SIZE;
        }

        Ok(Self {
            port_count,
            chip_id,
            die_id,
            reserved: [0; 3],
            ports,
        })
    }
}

/// Device identification information
#[derive(Debug, Clone)]
pub struct FwctlDeviceInfo {
    /// Chip ID
    pub chip_id: u32,
    /// Die ID
    pub die_id: u32,
    /// Device path
    pub path: String,
}

impl FwctlDeviceInfo {
    /// Create new device info
    ///
    /// # Arguments
    /// * `chip_id` - Chip ID
    /// * `die_id` - Die ID
    /// * `path` - Device path
    #[must_use]
    pub fn new(chip_id: u32, die_id: u32, path: impl Into<String>) -> Self {
        Self {
            chip_id,
            die_id,
            path: path.into(),
        }
    }
}

/// RPC command types for fwctl operations
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbFwctlCmd {
    /// Config BA layer `MAR_PEFR_STATS`
    ConfigBaMarPerfStats = 0x0047,
    /// Query BA layer `MAR_PEFR_STATS`
    QueryBaMarPerfStats = 0x0048,
    /// Query IO die port information
    QueryIoDiePortInfo = 0x0082,
}

impl UbFwctlCmd {
    /// Get the command value as u32
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}
