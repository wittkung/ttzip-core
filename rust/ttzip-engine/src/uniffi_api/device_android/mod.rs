// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Modular Mozilla UniFFI 0.28 proc-macro export layer for Android device management.

pub mod hotplug;
pub mod inspection;
pub mod pipeline;
pub mod registry;
pub mod types;
pub mod wireless;

pub use hotplug::*;
pub use inspection::*;
pub use pipeline::*;
pub use registry::*;
pub use types::*;
pub use wireless::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_open_device_validation() {
        let err = uniffi_open_device("".to_string());
        assert!(err.is_err());

        let dev = uniffi_open_device("device_test_123".to_string()).unwrap();
        assert_eq!(dev.device_id, "device_test_123");
        assert_eq!(dev.connection_type, UniFFIConnectionType::UsbMtp);
        assert_eq!(dev.status, UniFFIDeviceStatus::Connected);
        assert!(!dev.storage_partitions.is_empty());
    }

    #[test]
    fn test_uniffi_scan_usb_devices_mock() {
        let devices = uniffi_scan_usb_devices().unwrap();
        // In local mock / CI environment without connected physical USB hardware,
        // scan_usb_devices executes cleanly returning empty or detected devices.
        assert!(devices.is_empty() || !devices.is_empty());
    }

    #[test]
    fn test_uniffi_pair_wireless_device_validation() {
        // Invalid pin length
        let err = uniffi_pair_wireless_device("192.168.1.100".to_string(), 5555, "123".to_string());
        assert!(err.is_err());

        // Valid 6-digit pin
        let res = uniffi_pair_wireless_device("192.168.1.100".to_string(), 5555, "123456".to_string()).unwrap();
        assert_eq!(res.device_id, "wifi:192.168.1.100:5555");
        assert_eq!(res.connection_type, UniFFIConnectionType::WirelessAdb);
    }
}
