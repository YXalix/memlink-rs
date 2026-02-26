//! List command for discovering and displaying fwctl devices
//!
//! This module provides the `ls` functionality similar to the C `ubctl ls` command,
//! which discovers all fwctl devices and displays their information including
//! `chip_id`, `die_id`, `port_count`, and per-port details.

use std::fmt::Write;

use crate::device::{scan_devices, DiscoveredDevice};
use crate::error::UbfwctlError;

/// List command for displaying device information
///
/// This struct implements the list functionality that scans for all fwctl devices
/// and displays their information in a format compatible with the C `ubctl ls` command.
///
/// # Example
/// ```no_run
/// use ubfwctl::commands::list::ListCommand;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let cmd = ListCommand::new();
///     let output = cmd.execute()?;
///     println!("{}", output);
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ListCommand;

impl ListCommand {
    /// Create a new list command
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Execute the list command and return formatted output
    ///
    /// Scans for all devices and formats their information.
    ///
    /// # Returns
    /// `Ok(String)` with formatted device list on success
    ///
    /// # Errors
    /// `UbfwctlError` if device scanning fails
    pub fn execute(&self) -> Result<String, UbfwctlError> {
        let devices = scan_devices()?;
        Ok(format_device_list(&devices))
    }

    /// Execute the list command and return raw device data
    ///
    /// Returns the discovered devices without formatting.
    ///
    /// # Returns
    /// `Ok(Vec<DiscoveredDevice>)` with device data on success
    ///
    /// # Errors
    /// `UbfwctlError` if device scanning fails
    pub fn execute_raw(&self) -> Result<Vec<DiscoveredDevice>, UbfwctlError> {
        scan_devices()
    }
}

/// High-level function to list all devices
///
/// This is a convenience function that creates a `ListCommand` and executes it.
///
/// # Returns
/// `Ok(String)` with formatted device list on success
///
/// # Errors
/// `UbfwctlError` if device scanning fails
///
/// # Example
/// ```no_run
/// use ubfwctl::list_devices;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let output = list_devices()?;
///     println!("{}", output);
///     Ok(())
/// }
/// ```
pub fn list_devices() -> Result<String, UbfwctlError> {
    let cmd = ListCommand::new();
    cmd.execute()
}

/// List devices and return raw device information
///
/// Similar to `list_devices()` but returns the raw `DiscoveredDevice` structs
/// instead of formatted output.
///
/// # Returns
/// `Ok(Vec<DiscoveredDevice>)` with device data on success
///
/// # Errors
/// `UbfwctlError` if device scanning fails
///
/// # Example
/// ```no_run
/// use ubfwctl::list_devices_raw;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let devices = list_devices_raw()?;
///     for device in &devices {
///         println!("Chip: {}, Die: {}, Ports: {}",
///             device.chip_id(), device.die_id(), device.port_count());
///     }
///     Ok(())
/// }
/// ```
pub fn list_devices_raw() -> Result<Vec<DiscoveredDevice>, UbfwctlError> {
    scan_devices()
}

/// Format a list of discovered devices into a string representation
///
/// The output format matches the C `ubctl ls` command:
/// ```text
/// ubctl_id: 0
///     chip_id: 0
///     die_id: 0
///     port_count: 4
///         port_id: 0x0
///         port_type: eth
///         link_status: up
///         ...
/// total ubctl count: 1
/// ```
///
/// # Arguments
/// * `devices` - Slice of discovered devices to format
///
/// # Returns
/// Formatted string representation of the devices
#[must_use]
pub fn format_device_list(devices: &[DiscoveredDevice]) -> String {
    if devices.is_empty() {
        return "No devices found.\n".to_string();
    }

    let mut output = String::new();

    for (index, device) in devices.iter().enumerate() {
        let idx = u32::try_from(index).unwrap_or(0);
        output.push_str(&format_device(device, idx));
        output.push('\n');
    }

    writeln!(output, "total ubctl count: {}", devices.len()).unwrap();

    output
}

/// Format a single device into a string representation
///
/// # Arguments
/// * `device` - The discovered device to format
/// * `ubctl_id` - The ubctl ID (sequential index)
///
/// # Returns
/// Formatted string representation of the device
#[must_use]
pub fn format_device(device: &DiscoveredDevice, ubctl_id: u32) -> String {
    let mut output = String::new();

    // Device header
    writeln!(output, "ubctl_id: {ubctl_id}").unwrap();
    writeln!(output, "\tchip_id: {}", device.chip_id()).unwrap();
    writeln!(output, "\tdie_id: {}", device.die_id()).unwrap();
    writeln!(output, "\tport_count: {}", device.port_count()).unwrap();

    // Port information
    for port in device.ports() {
        writeln!(output, "\t\tport_id: 0x{:x}", port.port_id).unwrap();
        writeln!(output, "\t\tport_type: {}", port.port_type_str()).unwrap();
        writeln!(output, "\t\tlink_status: {}", port.link_status_str()).unwrap();
    }

    output
}

/// Device information structure for display
///
/// This is a simplified version of `DiscoveredDevice` for public API use.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// ubctl ID (sequential index)
    pub ubctl_id: u32,
    /// Chip ID
    pub chip_id: u32,
    /// Die ID
    pub die_id: u32,
    /// Port count
    pub port_count: u32,
    /// Port information
    pub ports: Vec<PortDisplayInfo>,
}

/// Port information for display
#[derive(Debug, Clone)]
pub struct PortDisplayInfo {
    /// Port ID
    pub port_id: u32,
    /// Port type ("eth" or "ub")
    pub port_type: String,
    /// Link status ("up" or "down")
    pub link_status: String,
}

impl From<&DiscoveredDevice> for DeviceInfo {
    fn from(device: &DiscoveredDevice) -> Self {
        let ports: Vec<PortDisplayInfo> = device
            .ports()
            .iter()
            .map(|p| PortDisplayInfo {
                port_id: p.port_id,
                port_type: p.port_type_str().to_string(),
                link_status: p.link_status_str().to_string(),
            })
            .collect();

        Self {
            ubctl_id: 0, // Will be set by caller
            chip_id: device.chip_id(),
            die_id: device.die_id(),
            port_count: device.port_count(),
            ports,
        }
    }
}

/// Convert discovered devices to display information
///
/// # Arguments
/// * `devices` - Slice of discovered devices
///
/// # Returns
/// Vector of `DeviceInfo` structures with `ubctl_id` assigned
#[must_use]
pub fn to_device_info(devices: &[DiscoveredDevice]) -> Vec<DeviceInfo> {
    devices
        .iter()
        .enumerate()
        .map(|(idx, device)| {
            let mut info = DeviceInfo::from(device);
            info.ubctl_id = u32::try_from(idx).unwrap_or(0);
            info
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{IoDieInfo, PortInfo, FwctlDeviceInfo};

    fn create_test_device() -> DiscoveredDevice {
        let device_info = FwctlDeviceInfo::new(0, 0, "/dev/fwctl/fwctl00");

        let ports = vec![
            PortInfo {
                port_id: 0,
                link_status: 1,
                link_state_info: 0,
                port_type: 0,
                reserved: [0; 2],
            },
            PortInfo {
                port_id: 1,
                link_status: 0,
                link_state_info: 0,
                port_type: 1,
                reserved: [0; 2],
            },
        ];

        let io_die_info = IoDieInfo {
            port_count: 2,
            chip_id: 0,
            die_id: 0,
            reserved: [0; 3],
            ports,
        };

        DiscoveredDevice::new(device_info, io_die_info, "test_entity".to_string())
    }

    #[test]
    fn test_format_device() {
        let device = create_test_device();
        let output = format_device(&device, 0);

        assert!(output.contains("ubctl_id: 0"));
        assert!(output.contains("chip_id: 0"));
        assert!(output.contains("die_id: 0"));
        assert!(output.contains("port_count: 2"));
        assert!(output.contains("port_id: 0x0"));
        assert!(output.contains("port_type: eth"));
        assert!(output.contains("link_status: up"));
        assert!(output.contains("port_id: 0x1"));
        assert!(output.contains("port_type: ub"));
        assert!(output.contains("link_status: down"));
    }

    #[test]
    fn test_format_device_list() {
        let device = create_test_device();
        let devices = vec![device];
        let output = format_device_list(&devices);

        assert!(output.contains("ubctl_id: 0"));
        assert!(output.contains("total ubctl count: 1"));
    }

    #[test]
    fn test_format_empty_device_list() {
        let devices: Vec<DiscoveredDevice> = vec![];
        let output = format_device_list(&devices);

        assert_eq!(output, "No devices found.\n");
    }

    #[test]
    fn test_to_device_info() {
        let device = create_test_device();
        let devices = vec![device];
        let info_list = to_device_info(&devices);

        assert_eq!(info_list.len(), 1);
        assert_eq!(info_list[0].ubctl_id, 0);
        assert_eq!(info_list[0].chip_id, 0);
        assert_eq!(info_list[0].die_id, 0);
        assert_eq!(info_list[0].port_count, 2);
        assert_eq!(info_list[0].ports.len(), 2);
    }

    #[test]
    fn test_list_command_new() {
        let _cmd = ListCommand::new();
        // Just verify it can be created
    }
}
