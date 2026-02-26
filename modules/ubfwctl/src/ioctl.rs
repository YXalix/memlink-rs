//! Kernel communication via ioctl

use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use crate::error::UbfwctlError;
use crate::types::{FwctlDeviceInfo, IoDieInfo, MarPerfConfig, UbFwctlCmd};

/// fwctl device directory
pub const FWCTL_DEV_DIR: &str = "/dev/fwctl";
/// fwctl device prefix
pub const FWCTL_DEV_PREFIX: &str = "fwctl";

/// Ioctl command for fwctl RPC
pub const FWCTL_RPC: u64 = 0xC0_9A_00_01; // _IO(FWCTL_TYPE(0x9A), FWCTL_CMD_RPC(1))

/// RPC scope for configuration access
pub const FWCTL_RPC_CONFIGURATION: u32 = 0;

/// fwctl RPC structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FwctlRpc {
    /// Size of this structure
    pub size: u32,
    /// RPC scope
    pub scope: u32,
    /// Input length
    pub in_len: u32,
    /// Output length
    pub out_len: u32,
    /// Input buffer pointer
    pub in_ptr: u64,
    /// Output buffer pointer
    pub out_ptr: u64,
}

impl FwctlRpc {
    /// Create a new RPC structure
    ///
    /// # Arguments
    /// * `scope` - RPC scope
    /// * `in_len` - Input buffer length
    /// * `out_len` - Output buffer length
    /// * `in_ptr` - Input buffer pointer
    /// * `out_ptr` - Output buffer pointer
    #[must_use]
    pub fn new(scope: u32, in_len: u32, out_len: u32, in_ptr: u64, out_ptr: u64) -> Self {
        Self {
            size: u32::try_from(size_of::<Self>()).unwrap_or(0),
            scope,
            in_len,
            out_len,
            in_ptr,
            out_ptr,
        }
    }
}

/// fwctl RPC input structure (device-specific format)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FwctlRpcUbIn {
    /// RPC command
    pub rpc_cmd: u32,
    /// Data size
    pub data_size: u32,
    /// Version
    pub version: u32,
    /// Reserved
    pub rsvd: u32,
    /// Variable-length data
    pub data: [u32; 0],
}

/// fwctl RPC output structure (device-specific format)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FwctlRpcUbOut {
    /// Return value
    pub retval: i32,
    /// Data size
    pub data_size: u32,
    /// Variable-length data
    pub data: [u32; 0],
}

/// Wrapper for fwctl device operations
#[derive(Debug)]
pub struct FwctlDevice {
    /// File descriptor for the device
    fd: RawFd,
    /// Device information
    pub info: FwctlDeviceInfo,
}

impl FwctlDevice {
    /// Open a fwctl device by chip and die ID
    ///
    /// # Arguments
    /// * `chip_id` - Chip ID
    /// * `die_id` - Die ID
    ///
    /// # Returns
    /// `Ok(FwctlDevice)` on success, `Err(UbfwctlError)` on failure
    ///
    /// # Errors
    /// Returns `DeviceNotFound` if no matching device is found
    pub fn open(chip_id: u32, die_id: u32) -> Result<Self, UbfwctlError> {
        let path = Self::find_device(chip_id, die_id)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(UbfwctlError::IoError)?;

        let fd = file.as_raw_fd();
        // Keep file open by forgetting it - we'll manage the fd manually
        std::mem::forget(file);

        Ok(Self {
            fd,
            info: FwctlDeviceInfo::new(chip_id, die_id, path),
        })
    }

    /// Find fwctl device path by chip and die ID
    ///
    /// # Arguments
    /// * `chip_id` - Chip ID
    /// * `die_id` - Die ID
    ///
    /// # Returns
    /// `Ok(String)` with device path on success, `Err(UbfwctlError)` on failure
    fn find_device(chip_id: u32, die_id: u32) -> Result<String, UbfwctlError> {
        use std::path::Path;
        let dir_path = Path::new(FWCTL_DEV_DIR);

        if !dir_path.exists() {
            return Err(UbfwctlError::DeviceNotFound { chip_id, die_id });
        }

        let entries = fs::read_dir(dir_path).map_err(UbfwctlError::IoError)?;

        for entry in entries {
            let entry = entry.map_err(UbfwctlError::IoError)?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with(FWCTL_DEV_PREFIX) {
                // Parse device number from name (e.g., fwctl00 -> chip 0, die 0)
                // Device names use hexadecimal format: fwctl{chip_id:04x}{die_id:04x}
                if let Some(num_str) = name_str.strip_prefix(FWCTL_DEV_PREFIX)
                    && let Ok(num) = u32::from_str_radix(num_str, 16)
                {
                    // Assuming format: fwctl{chip_id}{die_id} combined as (chip_id << 16) | die_id
                    // Based on C code: UTOOL_DEV_CHIP_DIE_ID_MAX = 1 << 16
                    let device_chip_id = num >> 16;
                    let device_die_id = num & 0xFFFF;

                    if device_chip_id == chip_id && device_die_id == die_id {
                        return Ok(format!("{FWCTL_DEV_DIR}/{name_str}"));
                    }
                }
            }
        }

        Err(UbfwctlError::DeviceNotFound { chip_id, die_id })
    }

    /// Send an RPC command to the kernel
    ///
    /// # Arguments
    /// * `cmd` - RPC command type
    /// * `input` - Input data buffer
    /// * `output` - Output data buffer
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(UbfwctlError)` on failure
    ///
    /// # Errors
    /// Returns an error if:
    /// - The ioctl call fails
    /// - The kernel returns an error
    /// - The response data is invalid
    ///
    /// # Safety
    /// This function uses unsafe ioctl calls
    #[allow(
        clippy::as_conversions,
        clippy::cast_ptr_alignment,
        clippy::manual_slice_size_calculation
    )]
    pub fn send_rpc(
        &self,
        cmd: UbFwctlCmd,
        input: &[u32],
        output: &mut [u32],
    ) -> Result<(), UbfwctlError> {
        // Prepare input structure
        let in_size = size_of::<FwctlRpcUbIn>() + input.len() * size_of::<u32>();
        let mut in_buf = vec![0u8; in_size];

        // Fill input header
        // SAFETY: in_buf is large enough to hold FwctlRpcUbIn and the pointer is valid
        let header = unsafe { &mut *(in_buf.as_mut_ptr().cast::<FwctlRpcUbIn>()) };
        header.rpc_cmd = cmd.as_u32();
        header.data_size = u32::try_from(input.len() * size_of::<u32>())
            .map_err(|_| UbfwctlError::InvalidResponse("Input too large".to_string()))?;
        header.version = 0;
        header.rsvd = 0;

        // Copy input data
        if !input.is_empty() {
            let data_offset = size_of::<FwctlRpcUbIn>();
            // SAFETY: in_buf is large enough (size_of::<FwctlRpcUbIn>() + input.len() * 4 bytes)
            // and the pointer is properly aligned for u32
            let data_slice = unsafe {
                std::slice::from_raw_parts_mut(
                    in_buf.as_mut_ptr().add(data_offset).cast::<u32>(),
                    input.len(),
                )
            };
            data_slice.copy_from_slice(input);
        }

        // Prepare output buffer
        let out_size = size_of::<FwctlRpcUbOut>() + output.len() * size_of::<u32>();
        let mut out_buf = vec![0u8; out_size];

        // Prepare RPC structure
        let rpc = FwctlRpc::new(
            FWCTL_RPC_CONFIGURATION,
            u32::try_from(in_buf.len())
                .map_err(|_| UbfwctlError::InvalidResponse("Input too large".to_string()))?,
            u32::try_from(out_buf.len())
                .map_err(|_| UbfwctlError::InvalidResponse("Output too large".to_string()))?,
            in_buf.as_ptr() as u64,
            out_buf.as_mut_ptr() as u64,
        );

        // Execute ioctl
        // SAFETY: ioctl is called with a valid file descriptor and properly initialized rpc struct
        let ret = unsafe { libc::ioctl(self.fd, FWCTL_RPC, &rpc) };

        if ret < 0 {
            return Err(UbfwctlError::IoctlFailed(format!(
                "ioctl failed with errno: {}",
                io::Error::last_os_error()
            )));
        }

        // Parse output
        // SAFETY: out_buf is large enough to hold FwctlRpcUbOut and the pointer is valid
        let out_header = unsafe { &*(out_buf.as_ptr().cast::<FwctlRpcUbOut>()) };

        if out_header.retval != 0 {
            return Err(UbfwctlError::IoctlFailed(format!(
                "Kernel returned error: {}",
                out_header.retval
            )));
        }

        // Copy output data
        let data_offset = size_of::<FwctlRpcUbOut>();
        let data_len = usize::try_from(out_header.data_size / 4)
            .map_err(|_| UbfwctlError::InvalidResponse("Invalid data size".to_string()))?;
        let copy_len = data_len.min(output.len());

        if copy_len > 0 {
            // SAFETY: out_buf has enough data (data_offset + copy_len * 4 bytes)
            // and the pointer is properly aligned for u32
            let data_slice = unsafe {
                std::slice::from_raw_parts(
                    out_buf.as_ptr().add(data_offset).cast::<u32>(),
                    copy_len,
                )
            };
            output[..copy_len].copy_from_slice(data_slice);
        }

        Ok(())
    }

    /// Configure `mar_perf` measurement
    ///
    /// # Arguments
    /// * `port` - Port ID
    /// * `time_ms` - Measurement time in milliseconds
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(UbfwctlError)` on failure
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    pub fn mar_perf_config(&self, port: u32, time_ms: u32) -> Result<(), UbfwctlError> {
        let config = MarPerfConfig::new(port, time_ms);
        let input = [config.port_id, config.time_ms];
        let mut output = [0u32; 64];

        self.send_rpc(UbFwctlCmd::ConfigBaMarPerfStats, &input, &mut output)?;

        // Sleep for the configured time (convert ms to us)
        let sleep_us = time_ms * crate::error::MS_TO_US;
        std::thread::sleep(std::time::Duration::from_micros(u64::from(sleep_us)));

        Ok(())
    }

    /// Query `mar_perf` results
    ///
    /// # Arguments
    /// * `port` - Port ID
    ///
    /// # Returns
    /// `Ok(Vec<u32>)` with raw data on success, `Err(UbfwctlError)` on failure
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    pub fn mar_perf_query(&self, port: u32) -> Result<Vec<u32>, UbfwctlError> {
        let input = [port];
        let mut output = [0u32; 64];

        self.send_rpc(UbFwctlCmd::QueryBaMarPerfStats, &input, &mut output)?;

        Ok(output.to_vec())
    }

    /// Query IO die port information
    ///
    /// # Returns
    /// `Ok(IoDieInfo)` on success, `Err(UbfwctlError)` on failure
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the response is invalid
    pub fn query_io_die_info(&self) -> Result<IoDieInfo, UbfwctlError> {
        // Max size: header (28 bytes) + 20 ports * 24 bytes = 28 + 480 = 508 bytes
        // Using u32 array: 508 / 4 = 127 u32s, round up to 128
        const MAX_OUTPUT_SIZE: usize = 128;

        let input: [u32; 0] = [];
        let mut output = [0u32; MAX_OUTPUT_SIZE];

        self.send_rpc(UbFwctlCmd::QueryIoDiePortInfo, &input, &mut output)?;

        // Convert output to bytes for parsing
        let output_bytes: Vec<u8> = output
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();

        IoDieInfo::from_raw_data(&output_bytes)
            .map_err(|e| UbfwctlError::InvalidResponse(format!("Failed to parse IO die info: {e}")))
    }
}

impl Drop for FwctlDevice {
    fn drop(&mut self) {
        // SAFETY: close is safe to call with a valid file descriptor
        unsafe {
            // Close the file descriptor
            let _ = libc::close(self.fd);
        }
    }
}

// Safety: FwctlDevice only contains a RawFd which is Send + Sync
unsafe impl Send for FwctlDevice {}
unsafe impl Sync for FwctlDevice {}
