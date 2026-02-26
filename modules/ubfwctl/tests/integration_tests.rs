//! Integration tests for ubfwctl

use ubfwctl::device::DiscoveredDevice;
use ubfwctl::types::{FwctlDeviceInfo, IoDieInfo, MarPerfQuery, MarPerfResult, PortInfo};
use ubfwctl::error::{MAX_TIME_MS, MIN_TIME_MS, UbfwctlError};
use ubfwctl::format_device_list;

#[test]
fn test_mar_perf_query_from_raw_data() {
    let raw_data: Vec<u32> = vec![
        0,      // PORT_ID_IDX
        1000,   // CLOCK_CYCLE_IDX
        10000,  // FLUX_WR_IDX
        20000,  // FLUX_RD_IDX
        30000,  // FLUX_SUM_IDX
        100,    // WR_CMD_IDX
        200,    // RD_CMD_IDX
        300,    // SUM_CMD_IDX
        50,     // WLATCNT_FIRST_IDX
        60,     // RLATCNT_FIRST_IDX
    ];

    let query = MarPerfQuery::from_raw_data(&raw_data);

    assert_eq!(query.port_id, 0);
    assert_eq!(query.flux_wr, 10000);
    assert_eq!(query.flux_rd, 20000);
    assert_eq!(query.flux_sum, 30000);
    assert_eq!(query.wr_cmd_cnt, 100);
    assert_eq!(query.rd_cmd_cnt, 200);
    assert_eq!(query.sum_cmd_cnt, 300);
    assert_eq!(query.wlatcnt_first, 50);
    assert_eq!(query.rlatcnt_first, 60);
}

#[test]
#[should_panic(expected = "Insufficient data from kernel")]
fn test_mar_perf_query_from_raw_data_insufficient() {
    let raw_data: Vec<u32> = vec![0, 1, 2]; // Too few elements
    let _query = MarPerfQuery::from_raw_data(&raw_data);
}

#[test]
fn test_mar_perf_result_calculate() {
    let query = MarPerfQuery {
        port_id: 0,
        flux_wr: 10000,
        flux_rd: 20000,
        flux_sum: 30000,
        wr_cmd_cnt: 100,
        rd_cmd_cnt: 200,
        sum_cmd_cnt: 300,
        wlatcnt_first: 50,
        rlatcnt_first: 60,
    };

    let time_ms = 1000u32;
    let clock_freq_hz = 1_000_000_000u32; // 1 GHz

    let result = MarPerfResult::calculate(&query, time_ms, clock_freq_hz);

    // Port pair calculation (even port -> second_port_id = port_id + 1)
    assert_eq!(result.first_port_id, 0);
    assert_eq!(result.second_port_id, 1);

    // Traffic calculation (bytes/s)
    // wr_traffic = 10000 / 1.0 = 10000
    assert_eq!(result.wr_traffic, 10000);
    // rd_traffic = 20000 / 1.0 = 20000
    assert_eq!(result.rd_traffic, 20000);
    // sum_traffic = 30000 / 1.0 = 30000
    assert_eq!(result.sum_traffic, 30000);

    // Payload length calculation (bytes)
    // wr_pld_avg_len = 10000 / 100 = 100
    assert_eq!(result.wr_pld_avg_len, 100);
    // rd_pld_avg_len = 20000 / 200 = 100
    assert_eq!(result.rd_pld_avg_len, 100);
    // pld_avg_len = 30000 / 300 = 100
    assert_eq!(result.pld_avg_len, 100);

    // Latency calculation (ns)
    // clock_freq_ns = 1e9 / 1e9 = 1.0 ns/cycle
    // wr_delayed = 50 * 1.0 = 50 ns
    assert_eq!(result.wr_delayed, 50);
    // rd_delayed = 60 * 1.0 = 60 ns
    assert_eq!(result.rd_delayed, 60);
}

#[test]
fn test_mar_perf_result_calculate_odd_port() {
    let query = MarPerfQuery {
        port_id: 1,
        flux_wr: 10000,
        flux_rd: 20000,
        flux_sum: 30000,
        wr_cmd_cnt: 100,
        rd_cmd_cnt: 200,
        sum_cmd_cnt: 300,
        wlatcnt_first: 50,
        rlatcnt_first: 60,
    };

    let result = MarPerfResult::calculate(&query, 1000, 1_000_000_000);

    // Port pair calculation (odd port -> first_port_id = port_id - 1)
    assert_eq!(result.first_port_id, 0);
    assert_eq!(result.second_port_id, 1);
}

#[test]
fn test_mar_perf_result_calculate_zero_time() {
    let query = MarPerfQuery {
        port_id: 0,
        flux_wr: 10000,
        flux_rd: 20000,
        flux_sum: 30000,
        wr_cmd_cnt: 100,
        rd_cmd_cnt: 200,
        sum_cmd_cnt: 300,
        wlatcnt_first: 50,
        rlatcnt_first: 60,
    };

    let result = MarPerfResult::calculate(&query, 0, 1_000_000_000);

    // All traffic should be 0 when time is 0 (to avoid division by zero)
    assert_eq!(result.wr_traffic, 0);
    assert_eq!(result.rd_traffic, 0);
    assert_eq!(result.sum_traffic, 0);
}

#[test]
fn test_mar_perf_result_calculate_zero_clock() {
    let query = MarPerfQuery {
        port_id: 0,
        flux_wr: 10000,
        flux_rd: 20000,
        flux_sum: 30000,
        wr_cmd_cnt: 100,
        rd_cmd_cnt: 200,
        sum_cmd_cnt: 300,
        wlatcnt_first: 50,
        rlatcnt_first: 60,
    };

    let result = MarPerfResult::calculate(&query, 1000, 0);

    // All latency should be 0 when clock freq is 0 (to avoid division by zero)
    assert_eq!(result.wr_delayed, 0);
    assert_eq!(result.rd_delayed, 0);
}

#[test]
fn test_mar_perf_result_calculate_zero_commands() {
    let query = MarPerfQuery {
        port_id: 0,
        flux_wr: 10000,
        flux_rd: 20000,
        flux_sum: 30000,
        wr_cmd_cnt: 0,
        rd_cmd_cnt: 0,
        sum_cmd_cnt: 0,
        wlatcnt_first: 50,
        rlatcnt_first: 60,
    };

    let result = MarPerfResult::calculate(&query, 1000, 1_000_000_000);

    // All payload lengths should be 0 when command count is 0 (to avoid division by zero)
    assert_eq!(result.wr_pld_avg_len, 0);
    assert_eq!(result.rd_pld_avg_len, 0);
    assert_eq!(result.pld_avg_len, 0);
}

#[test]
fn test_mar_perf_result_display() {
    let result = MarPerfResult {
        first_port_id: 0,
        second_port_id: 1,
        wr_traffic: 10000,
        rd_traffic: 20000,
        sum_traffic: 30000,
        wr_pld_avg_len: 100,
        rd_pld_avg_len: 100,
        pld_avg_len: 100,
        wr_delayed: 50,
        rd_delayed: 60,
    };

    let output = format!("{}", result);

    assert!(output.contains("ba-mar_perf"));
    assert!(output.contains("port_id: 0 1"));
    assert!(output.contains("wr_traffic: 10000"));
    assert!(output.contains("rd_traffic: 20000"));
    assert!(output.contains("sum_traffic: 30000"));
    assert!(output.contains("wr_pld_avg_len: 100"));
    assert!(output.contains("rd_pld_avg_len: 100"));
    assert!(output.contains("pld_avg_len: 100"));
    assert!(output.contains("wr_delayed: 50"));
    assert!(output.contains("rd_delayed: 60"));
}

#[test]
fn test_error_validate_time_valid() {
    assert!(UbfwctlError::validate_time(MIN_TIME_MS).is_ok());
    assert!(UbfwctlError::validate_time(MAX_TIME_MS).is_ok());
    assert!(UbfwctlError::validate_time(1000).is_ok());
}

#[test]
fn test_error_validate_time_invalid() {
    let result = UbfwctlError::validate_time(0);
    assert!(result.is_err());
    match result.unwrap_err() {
        UbfwctlError::InvalidTime(t) => assert_eq!(t, 0),
        _ => panic!("Expected InvalidTime error"),
    }

    let result = UbfwctlError::validate_time(MAX_TIME_MS + 1);
    assert!(result.is_err());
    match result.unwrap_err() {
        UbfwctlError::InvalidTime(t) => assert_eq!(t, MAX_TIME_MS + 1),
        _ => panic!("Expected InvalidTime error"),
    }
}

#[test]
fn test_error_display_variants() {
    let err = UbfwctlError::InvalidPort(99);
    let msg = format!("{}", err);
    assert!(msg.contains("Invalid port"));

    let err = UbfwctlError::IoctlFailed("test error".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("Ioctl failed"));

    let err = UbfwctlError::DeviceNotFound { chip_id: 0, die_id: 0 };
    let msg = format!("{}", err);
    assert!(msg.contains("not found"));

    let err = UbfwctlError::InvalidResponse("bad data".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("Invalid response"));

    let err = UbfwctlError::ShmLockFailed("lock failed".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("lock failed"));

    let err = UbfwctlError::CommandNotSupported("foo".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("not supported"));
}

#[test]
fn test_io_die_info_from_raw_data() {
    // Create raw data matching C struct layout
    // Header: port_count (4), chip_id (4), die_id (4), reserved[3] (12) = 24 bytes
    // Port info: port_id (4), link_status (4), link_state_info (4), port_type (4), reserved[2] (8) = 24 bytes

    let mut raw_data: Vec<u8> = Vec::new();

    // Header
    raw_data.extend_from_slice(&1u32.to_ne_bytes()); // port_count = 1
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // chip_id = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // die_id = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[0]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[1]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[2]

    // Port info
    raw_data.extend_from_slice(&0x0u32.to_ne_bytes()); // port_id = 0x0
    raw_data.extend_from_slice(&1u32.to_ne_bytes()); // link_status = 1 (up)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // link_state_info = 0
    raw_data.extend_from_slice(&1u32.to_ne_bytes()); // port_type = 1 (ub)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[0]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[1]

    let io_die_info = IoDieInfo::from_raw_data(&raw_data).unwrap();

    assert_eq!(io_die_info.port_count, 1);
    assert_eq!(io_die_info.chip_id, 0);
    assert_eq!(io_die_info.die_id, 0);
    assert_eq!(io_die_info.ports.len(), 1);
    assert_eq!(io_die_info.ports[0].port_id, 0x0);
    assert_eq!(io_die_info.ports[0].link_status, 1);
    assert_eq!(io_die_info.ports[0].port_type, 1);
}

#[test]
fn test_io_die_info_multiple_ports() {
    let mut raw_data: Vec<u8> = Vec::new();

    // Header
    raw_data.extend_from_slice(&2u32.to_ne_bytes()); // port_count = 2
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // chip_id = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // die_id = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[0]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[1]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[2]

    // Port 0 info
    raw_data.extend_from_slice(&0x0u32.to_ne_bytes()); // port_id = 0x0
    raw_data.extend_from_slice(&1u32.to_ne_bytes()); // link_status = 1 (up)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // link_state_info = 0
    raw_data.extend_from_slice(&1u32.to_ne_bytes()); // port_type = 1 (ub)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[0]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[1]

    // Port 1 info
    raw_data.extend_from_slice(&0x1u32.to_ne_bytes()); // port_id = 0x1
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // link_status = 0 (down)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // link_state_info = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // port_type = 0 (eth)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[0]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[1]

    let io_die_info = IoDieInfo::from_raw_data(&raw_data).unwrap();

    assert_eq!(io_die_info.port_count, 2);
    assert_eq!(io_die_info.ports.len(), 2);

    // Port 0
    assert_eq!(io_die_info.ports[0].port_id, 0x0);
    assert_eq!(io_die_info.ports[0].link_status, 1);
    assert_eq!(io_die_info.ports[0].port_type, 1);
    assert_eq!(io_die_info.ports[0].link_status_str(), "up");
    assert_eq!(io_die_info.ports[0].port_type_str(), "ub");

    // Port 1
    assert_eq!(io_die_info.ports[1].port_id, 0x1);
    assert_eq!(io_die_info.ports[1].link_status, 0);
    assert_eq!(io_die_info.ports[1].port_type, 0);
    assert_eq!(io_die_info.ports[1].link_status_str(), "down");
    assert_eq!(io_die_info.ports[1].port_type_str(), "eth");
}

#[test]
fn test_io_die_info_from_raw_data_insufficient() {
    let raw_data: Vec<u8> = vec![0; 10]; // Too small
    let result = IoDieInfo::from_raw_data(&raw_data);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Insufficient data"));
}

#[test]
fn test_io_die_info_invalid_port_count() {
    let mut raw_data: Vec<u8> = Vec::new();

    // Header with invalid port_count (0)
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // port_count = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // chip_id = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // die_id = 0
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[0]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[1]
    raw_data.extend_from_slice(&0u32.to_ne_bytes()); // reserved[2]

    let result = IoDieInfo::from_raw_data(&raw_data);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid port count"));
}

#[test]
fn test_format_device_list_empty() {
    let devices: Vec<DiscoveredDevice> = vec![];
    let output = format_device_list(&devices);
    assert!(output.contains("No devices found"));
}

#[test]
fn test_format_device_list_single() {
    let device_info = FwctlDeviceInfo::new(0, 0, "/dev/fwctl/fwctl00");
    let io_die_info = IoDieInfo {
        port_count: 1,
        chip_id: 0,
        die_id: 0,
        reserved: [0; 3],
        ports: vec![PortInfo {
            port_id: 0,
            link_status: 1,
            link_state_info: 0,
            port_type: 1,
            reserved: [0; 2],
        }],
    };
    let devices = vec![DiscoveredDevice::new(
        device_info,
        io_die_info,
        "test_entity".to_string(),
    )];

    let output = format_device_list(&devices);
    assert!(output.contains("ubctl_id: 0"));
    assert!(output.contains("chip_id: 0"));
    assert!(output.contains("die_id: 0"));
    assert!(output.contains("port_count: 1"));
    assert!(output.contains("port_id: 0x0"));
    assert!(output.contains("port_type: ub"));
    assert!(output.contains("link_status: up"));
    assert!(output.contains("total ubctl count: 1"));
}

#[test]
fn test_format_device_list_multiple() {
    let devices = vec![
        DiscoveredDevice::new(
            FwctlDeviceInfo::new(0, 0, "/dev/fwctl/fwctl00"),
            IoDieInfo {
                port_count: 2,
                chip_id: 0,
                die_id: 0,
                reserved: [0; 3],
                ports: vec![
                    PortInfo {
                        port_id: 0,
                        link_status: 1,
                        link_state_info: 0,
                        port_type: 1,
                        reserved: [0; 2],
                    },
                    PortInfo {
                        port_id: 1,
                        link_status: 0,
                        link_state_info: 0,
                        port_type: 0,
                        reserved: [0; 2],
                    },
                ],
            },
            "entity0".to_string(),
        ),
        DiscoveredDevice::new(
            FwctlDeviceInfo::new(1, 0, "/dev/fwctl/fwctl00010000"),
            IoDieInfo {
                port_count: 1,
                chip_id: 1,
                die_id: 0,
                reserved: [0; 3],
                ports: vec![PortInfo {
                    port_id: 0,
                    link_status: 1,
                    link_state_info: 0,
                    port_type: 1,
                    reserved: [0; 2],
                }],
            },
            "entity1".to_string(),
        ),
    ];

    let output = format_device_list(&devices);

    // First device
    assert!(output.contains("ubctl_id: 0"));
    assert!(output.contains("chip_id: 0"));
    assert!(output.contains("die_id: 0"));
    assert!(output.contains("port_count: 2"));

    // Second device
    assert!(output.contains("ubctl_id: 1"));
    assert!(output.contains("chip_id: 1"));
    assert!(output.contains("die_id: 0"));
    assert!(output.contains("port_count: 1"));

    // Total count
    assert!(output.contains("total ubctl count: 2"));
}
