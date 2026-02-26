//! Device discovery and enumeration for fwctl devices
//!
//! This module provides functionality to scan for and discover fwctl devices
//! in the system, similar to the C `ubctl ls` command.
//!
//! # Device Discovery Process
//!
//! 1. Scan `/dev/fwctl/` directory for device nodes matching `fwctl*`
//! 2. Verify each device is a ubase device by checking `/sys/class/fwctl/{device}/device/uevent`
//! 3. Query IO die information from each device to get `chip_id`, `die_id`, and port details
//! 4. Return a list of discovered devices with their metadata

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::UbfwctlError;
use crate::ioctl::{FwctlDevice, FWCTL_DEV_DIR, FWCTL_DEV_PREFIX};
use crate::types::{FwctlDeviceInfo, IoDieInfo};

/// Sysfs path for fwctl class devices
const SYS_CLASS_FWCTL_PATH: &str = "/sys/class/fwctl";

/// Uevent file name within device sysfs
const UEVENT_FILE: &str = "device/uevent";

/// Driver key in uevent file
const DRIVER_KEY: &str = "DRIVER";

/// Expected driver name for ubase devices
const UB_DRIVER_NAME: &str = "ubase";

/// Entity name key in uevent file
const UB_ENTITY_NAME_KEY: &str = "UB_ENTITY_NAME";

/// Represents a discovered fwctl device with its metadata
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    /// Device information (`chip_id`, `die_id`, path)
    pub info: FwctlDeviceInfo,
    /// IO die information including port details
    pub io_die_info: IoDieInfo,
    /// Entity name from sysfs
    pub entity_name: String,
}

impl DiscoveredDevice {
    /// Create a new discovered device
    ///
    /// # Arguments
    /// * `info` - Device identification information
    /// * `io_die_info` - IO die information with port details
    /// * `entity_name` - Entity name from sysfs
    #[must_use]
pub const fn new(
        info: FwctlDeviceInfo,
        io_die_info: IoDieInfo,
        entity_name: String,
    ) -> Self {
        Self {
            info,
            io_die_info,
            entity_name,
        }
    }

    /// Get the device path
    #[must_use]
pub fn path(&self) -> &str {
        &self.info.path
    }

    /// Get the chip ID
    #[must_use]
pub const fn chip_id(&self) -> u32 {
        self.info.chip_id
    }

    /// Get the die ID
    #[must_use]
pub const fn die_id(&self) -> u32 {
        self.info.die_id
    }

    /// Get the entity name
    #[must_use]
pub fn entity_name(&self) -> &str {
        &self.entity_name
    }

    /// Get the number of ports
    #[must_use]
pub const fn port_count(&self) -> u32 {
        self.io_die_info.port_count
    }

    /// Get port information
    #[must_use]
pub fn ports(&self) -> &[crate::types::PortInfo] {
        &self.io_die_info.ports
    }
}

/// Scan for all fwctl devices in the system
///
/// This function scans `/dev/fwctl/` directory, verifies each device is a ubase
/// device by checking sysfs, and queries IO die information from each device.
///
/// # Returns
/// `Ok(Vec<DiscoveredDevice>)` containing all discovered devices, or `Err(UbfwctlError)`
/// if an error occurs during scanning.
///
/// # Errors
/// - `UbfwctlError::IoError` if filesystem operations fail
/// - `UbfwctlError::DeviceNotFound` if no fwctl devices are found
///
/// # Example
/// ```no_run
/// use ubfwctl::device::scan_devices;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let devices = scan_devices()?;
///     for device in &devices {
///         println!("Found device: chip={}, die={}", device.chip_id(), device.die_id());
///     }
///     Ok(())
/// }
/// ```
pub fn scan_devices() -> Result<Vec<DiscoveredDevice>, UbfwctlError> {
    let dev_path = Path::new(FWCTL_DEV_DIR);

    if !dev_path.exists() {
        return Err(UbfwctlError::DeviceNotFound {
            chip_id: 0,
            die_id: 0,
        });
    }

    let mut devices = Vec::new();
    let entries = fs::read_dir(dev_path).map_err(UbfwctlError::IoError)?;

    for entry in entries {
        let entry = entry.map_err(UbfwctlError::IoError)?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip non-fwctl entries
        if !name_str.starts_with(FWCTL_DEV_PREFIX) {
            continue;
        }

        // Check if this is a ubase device
        let Some(entity_name) = check_ubase_device(&name_str)
        else {
            continue;
        };

        // Parse chip_id and die_id from device name
        // Format: fwctl{chip_id}{die_id} where combined = (chip_id << 16) | die_id
        let (chip_id, die_id) = parse_device_id(&name_str)?;

        // Open device and query IO die info
        let device = match FwctlDevice::open(chip_id, die_id) {
            Ok(dev) => dev,
            Err(e) => {
                // Log warning but continue scanning other devices
                eprintln!("Warning: Failed to open device {name_str}: {e}");
                continue;
            }
        };

        // Query IO die information
        let io_die_info = match device.query_io_die_info() {
            Ok(info) => info,
            Err(e) => {
                eprintln!("Warning: Failed to query IO die info for {name_str}: {e}");
                continue;
            }
        };

        let device_info = FwctlDeviceInfo::new(chip_id, die_id, format!("{FWCTL_DEV_DIR}/{name_str}"));

        devices.push(DiscoveredDevice::new(device_info, io_die_info, entity_name));
    }

    if devices.is_empty() {
        return Err(UbfwctlError::DeviceNotFound {
            chip_id: 0,
            die_id: 0,
        });
    }

    // Sort devices by chip_id, then die_id for consistent ordering
    devices.sort_by(|a, b| {
        a.chip_id()
            .cmp(&b.chip_id())
            .then_with(|| a.die_id().cmp(&b.die_id()))
    });

    Ok(devices)
}

/// Check if a device is a ubase device by reading its uevent file
///
/// # Arguments
/// * `device_name` - Name of the device (e.g., "fwctl00")
///
/// # Returns
/// `Some(String)` with the entity name if it's a ubase device, `None` otherwise
fn check_ubase_device(device_name: &str) -> Option<String> {
    let uevent_path = PathBuf::from(SYS_CLASS_FWCTL_PATH)
        .join(device_name)
        .join(UEVENT_FILE);

    let Ok(contents) = fs::read_to_string(&uevent_path) else {
        return None;
    };

    let mut is_ubase = false;
    let mut entity_name = String::new();

    for line in contents.lines() {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0].trim();
        let value = parts[1].trim();

        if key == DRIVER_KEY && value == UB_DRIVER_NAME {
            is_ubase = true;
        } else if key == UB_ENTITY_NAME_KEY {
            entity_name = value.to_string();
        }
    }

    if is_ubase && !entity_name.is_empty() {
        Some(entity_name)
    } else {
        None
    }
}

/// Parse `chip_id` and `die_id` from device name
///
/// Device names follow the format: `fwctl{combined_id}`
/// where `combined_id = (chip_id << 16) | die_id` (in hexadecimal)
///
/// # Arguments
/// * `device_name` - Device name (e.g., "fwctl00", "fwctl0001")
///
/// # Returns
/// `Ok((u32, u32))` containing (`chip_id`, `die_id`) on success
///
/// # Errors
/// `UbfwctlError::InvalidResponse` if the device name format is invalid
fn parse_device_id(device_name: &str) -> Result<(u32, u32), UbfwctlError> {
    let num_str = device_name
        .strip_prefix(FWCTL_DEV_PREFIX)
        .ok_or_else(|| UbfwctlError::InvalidResponse(format!("Invalid device name: {device_name}")))?;

    // Parse as hexadecimal (e.g., "00010000" -> chip 1, die 0)
    let combined_id = u32::from_str_radix(num_str, 16)
        .map_err(|_| UbfwctlError::InvalidResponse(format!("Invalid device number: {num_str}")))?;

    let chip_id = combined_id >> 16;
    let die_id = combined_id & 0xFFFF;

    Ok((chip_id, die_id))
}

/// List all available device paths
///
/// Returns a list of device paths without opening them or querying IO die info.
/// This is a lightweight alternative to `scan_devices()` when only path information
/// is needed.
///
/// # Returns
/// `Ok(Vec<PathBuf>)` containing paths to all fwctl devices
///
/// # Errors
/// `UbfwctlError::IoError` if directory reading fails
pub fn list_device_paths() -> Result<Vec<PathBuf>, UbfwctlError> {
    let dev_path = Path::new(FWCTL_DEV_DIR);

    if !dev_path.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    let entries = fs::read_dir(dev_path).map_err(UbfwctlError::IoError)?;

    for entry in entries {
        let entry = entry.map_err(UbfwctlError::IoError)?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with(FWCTL_DEV_PREFIX) {
            // Verify it's a ubase device
            if check_ubase_device(&name_str).is_some() {
                paths.push(entry.path());
            }
        }
    }

    paths.sort();
    Ok(paths)
}

/// Get device count
///
/// Returns the number of fwctl devices in the system.
///
/// # Returns
/// `Ok(usize)` with the device count
///
/// # Errors
/// `UbfwctlError::IoError` if directory reading fails
pub fn device_count() -> Result<usize, UbfwctlError> {
    let paths = list_device_paths()?;
    Ok(paths.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_device_id() {
        // Test fwctl00 -> chip 0, die 0 (hex: 0x00)
        let (chip, die) = parse_device_id("fwctl00").unwrap();
        assert_eq!(chip, 0);
        assert_eq!(die, 0);

        // Test fwctl0001 -> chip 0, die 1 (hex: 0x0001)
        let (chip, die) = parse_device_id("fwctl0001").unwrap();
        assert_eq!(chip, 0);
        assert_eq!(die, 1);

        // Test fwctl00010001 -> chip 1, die 1
        let (chip, die) = parse_device_id("fwctl00010001").unwrap();
        assert_eq!(chip, 1);
        assert_eq!(die, 1);

        // Test fwctlFFFF0000 -> chip 65535, die 0
        let (chip, die) = parse_device_id("fwctlFFFF0000").unwrap();
        assert_eq!(chip, 65535);
        assert_eq!(die, 0);
    }

    #[test]
    fn test_parse_device_id_invalid() {
        assert!(parse_device_id("invalid").is_err());
        assert!(parse_device_id("fwctl").is_err());
        assert!(parse_device_id("fwctlxyz").is_err());
    }

    #[test]
    fn test_discovered_device_accessors() {
        let device_info = FwctlDeviceInfo::new(1, 2, "/dev/fwctl/test");
        let io_die_info = IoDieInfo {
            port_count: 4,
            chip_id: 1,
            die_id: 2,
            reserved: [0; 3],
            ports: Vec::new(),
        };

        let discovered = DiscoveredDevice::new(device_info, io_die_info, "test_entity".to_string());

        assert_eq!(discovered.chip_id(), 1);
        assert_eq!(discovered.die_id(), 2);
        assert_eq!(discovered.port_count(), 4);
        assert_eq!(discovered.entity_name(), "test_entity");
        assert_eq!(discovered.path(), "/dev/fwctl/test");
    }
}
