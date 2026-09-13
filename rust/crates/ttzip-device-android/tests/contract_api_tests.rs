// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Contract compliance tests validating `AndroidDevice`, `StoragePartition`,
//! and device status models against `device_android_api.json`.

use serde_json::Value;
use ttzip_device_android::models::{
    AndroidDevice, AndroidVfsNode, ConnectionType, DeviceStatus, StoragePartition, VfsEntryType,
};

#[test]
fn test_device_api_contract_schema_compliance() {
    let device = AndroidDevice {
        device_id: "usb:18d1:4ee2:PIXEL8001".to_string(),
        display_name: "Google Pixel 8 Pro".to_string(),
        vendor_id: 0x18D1,
        product_id: 0x4EE2,
        serial_number: "PIXEL8001".to_string(),
        connection_type: ConnectionType::UsbMtp,
        status: DeviceStatus::Connected,
        storage_partitions: vec![
            StoragePartition {
                partition_id: "0x00010001".to_string(),
                display_name: "Internal shared storage".to_string(),
                total_bytes: 128_000_000_000,
                available_bytes: 64_000_000_000,
                root_path: "/".to_string(),
                is_removable: false,
            },
            StoragePartition {
                partition_id: "0x00020001".to_string(),
                display_name: "SanDisk Ultra 256GB".to_string(),
                total_bytes: 256_000_000_000,
                available_bytes: 120_000_000_000,
                root_path: "/storage/SD_CARD".to_string(),
                is_removable: true,
            },
        ],
    };

    let json_str = serde_json::to_string(&device).expect("Failed to serialize AndroidDevice");
    let v: Value = serde_json::from_str(&json_str).expect("Failed to parse JSON value");

    // Top-level required fields validation
    assert!(v.get("deviceId").and_then(Value::as_str).is_some());
    assert_eq!(v["deviceId"], "usb:18d1:4ee2:PIXEL8001");
    assert_eq!(v["displayName"], "Google Pixel 8 Pro");
    assert_eq!(v["vendorId"], 0x18D1);
    assert_eq!(v["productId"], 0x4EE2);
    assert_eq!(v["serialNumber"], "PIXEL8001");
    assert_eq!(v["connectionType"], "UsbMtp");
    assert_eq!(v["status"], "Connected");

    // Array of partitions validation
    let partitions = v["storagePartitions"].as_array().expect("Expected array");
    assert_eq!(partitions.len(), 2);

    let p0 = &partitions[0];
    assert_eq!(p0["partitionId"], "0x00010001");
    assert_eq!(p0["displayName"], "Internal shared storage");
    assert_eq!(p0["totalBytes"], 128_000_000_000u64);
    assert_eq!(p0["availableBytes"], 64_000_000_000u64);
    assert_eq!(p0["rootPath"], "/");
    assert_eq!(p0["isRemovable"], false);

    let p1 = &partitions[1];
    assert_eq!(p1["partitionId"], "0x00020001");
    assert_eq!(p1["isRemovable"], true);
}

#[test]
fn test_device_status_enum_values() {
    let statuses = [
        (DeviceStatus::Connecting, "Connecting"),
        (DeviceStatus::SeizingInterface, "SeizingInterface"),
        (DeviceStatus::Connected, "Connected"),
        (DeviceStatus::Stalled, "Stalled"),
        (DeviceStatus::Disconnected, "Disconnected"),
        (DeviceStatus::Error, "Error"),
    ];

    for (status, expected_str) in statuses {
        let serialized = serde_json::to_string(&status).expect("Serialization failed");
        assert_eq!(serialized, format!("\"{expected_str}\""));

        let deserialized: DeviceStatus =
            serde_json::from_str(&serialized).expect("Deserialization failed");
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_connection_type_enum_values() {
    let types = [
        (ConnectionType::UsbMtp, "UsbMtp"),
        (ConnectionType::UsbAdb, "UsbAdb"),
        (ConnectionType::WirelessAdb, "WirelessAdb"),
    ];

    for (conn, expected_str) in types {
        let serialized = serde_json::to_string(&conn).expect("Serialization failed");
        assert_eq!(serialized, format!("\"{expected_str}\""));

        let deserialized: ConnectionType =
            serde_json::from_str(&serialized).expect("Deserialization failed");
        assert_eq!(deserialized, conn);
    }
}

#[test]
fn test_storage_partition_used_bytes_overflow_protection() {
    let partition = StoragePartition {
        partition_id: "0x00010001".to_string(),
        display_name: "Internal".to_string(),
        total_bytes: 100_000,
        available_bytes: 40_000,
        root_path: "/".to_string(),
        is_removable: false,
    };
    assert_eq!(partition.used_bytes(), 60_000);

    // Test underflow saturation when available > total
    let underflow_partition = StoragePartition {
        partition_id: "0x00010001".to_string(),
        display_name: "Internal".to_string(),
        total_bytes: 50_000,
        available_bytes: 80_000,
        root_path: "/".to_string(),
        is_removable: false,
    };
    assert_eq!(underflow_partition.used_bytes(), 0);
}

#[test]
fn test_vfs_node_serialization_and_predicates() {
    let node = AndroidVfsNode {
        path: "/Download/archive.7z".to_string(),
        name: "archive.7z".to_string(),
        entry_type: VfsEntryType::File,
        size_bytes: 52_428_800,
        modified_timestamp: 1773400000,
        object_handle: Some(0x0000_1234),
        is_restricted: false,
    };

    assert!(node.is_file());
    assert!(!node.is_dir());

    let json_val = serde_json::to_value(&node).expect("Failed to serialize node");
    assert_eq!(json_val["path"], "/Download/archive.7z");
    assert_eq!(json_val["name"], "archive.7z");
    assert_eq!(json_val["entryType"], "File");
    assert_eq!(json_val["sizeBytes"], 52_428_800u64);
    assert_eq!(json_val["objectHandle"], 0x0000_1234);
    assert_eq!(json_val["isRestricted"], false);

    let restricted_dir = AndroidVfsNode {
        path: "/Android/data".to_string(),
        name: "data".to_string(),
        entry_type: VfsEntryType::RestrictedDirectory,
        size_bytes: 0,
        modified_timestamp: 1773400000,
        object_handle: Some(0x0000_9999),
        is_restricted: true,
    };

    assert!(restricted_dir.is_dir());
    assert!(!restricted_dir.is_file());
    assert!(restricted_dir.is_restricted);
}
