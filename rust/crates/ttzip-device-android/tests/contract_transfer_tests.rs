// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Contract compliance tests validating `TransferJob`, `TransferDirection`,
//! and `TransferStatus` against `specs/001-android-device-management/contracts/device_android_transfer.json`.

use serde_json::Value;
use ttzip_device_android::models::{TransferDirection, TransferJob, TransferStatus};

#[test]
fn test_transfer_job_contract_schema_compliance() {
    let job = TransferJob {
        job_id: "c4b12a87-3d92-4e2a-89cf-f9302611e9a2".to_string(),
        direction: TransferDirection::DirectPipelineExtract,
        source_path: "/Users/test/archive.zip".to_string(),
        destination_path: "/storage/emulated/0/Download/unpacked".to_string(),
        total_bytes: 1_048_576_000,
        transferred_bytes: 524_288_000,
        current_speed_bps: 45_000_000,
        status: TransferStatus::Transferring,
        error_message: None,
    };

    let json_str = serde_json::to_string(&job).expect("Failed to serialize TransferJob");
    let v: Value = serde_json::from_str(&json_str).expect("Failed to parse serialized JSON");

    // Validate all required camelCase keys defined in device_android_transfer.json
    assert_eq!(v["jobId"], "c4b12a87-3d92-4e2a-89cf-f9302611e9a2");
    assert_eq!(v["direction"], "DirectPipelineExtract");
    assert_eq!(v["sourcePath"], "/Users/test/archive.zip");
    assert_eq!(
        v["destinationPath"],
        "/storage/emulated/0/Download/unpacked"
    );
    assert_eq!(v["totalBytes"], 1_048_576_000u64);
    assert_eq!(v["transferredBytes"], 524_288_000u64);
    assert_eq!(v["currentSpeedBps"], 45_000_000u64);
    assert_eq!(v["status"], "Transferring");

    // Verify roundtrip deserialization
    let deserialized: TransferJob =
        serde_json::from_str(&json_str).expect("Failed to deserialize TransferJob");
    assert_eq!(deserialized, job);
}

#[test]
fn test_transfer_direction_enum_contracts() {
    let directions = [
        (TransferDirection::MacToAndroid, "MacToAndroid"),
        (TransferDirection::AndroidToMac, "AndroidToMac"),
        (
            TransferDirection::DirectPipelineExtract,
            "DirectPipelineExtract",
        ),
    ];

    for (dir, expected_str) in directions {
        let serialized = serde_json::to_string(&dir).expect("Failed to serialize direction");
        assert_eq!(serialized, format!("\"{expected_str}\""));

        let deserialized: TransferDirection =
            serde_json::from_str(&serialized).expect("Failed to deserialize direction");
        assert_eq!(deserialized, dir);
    }
}

#[test]
fn test_transfer_status_enum_contracts() {
    let statuses = [
        (TransferStatus::Queued, "Queued"),
        (TransferStatus::Transferring, "Transferring"),
        (TransferStatus::Paused, "Paused"),
        (TransferStatus::Cancelling, "Cancelling"),
        (TransferStatus::Completed, "Completed"),
        (TransferStatus::Failed, "Failed"),
    ];

    for (status, expected_str) in statuses {
        let serialized = serde_json::to_string(&status).expect("Failed to serialize status");
        assert_eq!(serialized, format!("\"{expected_str}\""));

        let deserialized: TransferStatus =
            serde_json::from_str(&serialized).expect("Failed to deserialize status");
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_transfer_job_with_error_message_contract() {
    let job = TransferJob {
        job_id: "err-job-1234".to_string(),
        direction: TransferDirection::AndroidToMac,
        source_path: "/storage/emulated/0/DCIM/Camera/photo.jpg".to_string(),
        destination_path: "/Users/test/Pictures/photo.jpg".to_string(),
        total_bytes: 4_096_000,
        transferred_bytes: 2_048_000,
        current_speed_bps: 0,
        status: TransferStatus::Failed,
        error_message: Some("USB pipe stall on endpoint 0x82".to_string()),
    };

    let json_str = serde_json::to_string(&job).expect("Failed to serialize TransferJob");
    let v: Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");

    assert_eq!(v["status"], "Failed");
    assert_eq!(v["errorMessage"], "USB pipe stall on endpoint 0x82");

    let roundtrip: TransferJob =
        serde_json::from_str(&json_str).expect("Failed to deserialize failed job");
    assert_eq!(roundtrip.error_message.as_deref(), Some("USB pipe stall on endpoint 0x82"));
}

#[test]
fn test_transfer_job_progress_ratio_invariants() {
    // Normal case: 50%
    let job_half = TransferJob {
        job_id: "job-progress-1".to_string(),
        direction: TransferDirection::MacToAndroid,
        source_path: "/local/data.bin".to_string(),
        destination_path: "/remote/data.bin".to_string(),
        total_bytes: 200,
        transferred_bytes: 100,
        current_speed_bps: 10_000_000,
        status: TransferStatus::Transferring,
        error_message: None,
    };
    assert!((job_half.progress_ratio() - 0.5).abs() < 1e-6);

    // Zero total bytes guard: must not divide by zero or NaN
    let job_zero = TransferJob {
        job_id: "job-progress-zero".to_string(),
        direction: TransferDirection::MacToAndroid,
        source_path: "/local/empty.bin".to_string(),
        destination_path: "/remote/empty.bin".to_string(),
        total_bytes: 0,
        transferred_bytes: 0,
        current_speed_bps: 0,
        status: TransferStatus::Queued,
        error_message: None,
    };
    assert_eq!(job_zero.progress_ratio(), 0.0);

    // Overflow saturation: transferred > total clamped to 1.0
    let job_overflow = TransferJob {
        job_id: "job-progress-overflow".to_string(),
        direction: TransferDirection::MacToAndroid,
        source_path: "/local/large.bin".to_string(),
        destination_path: "/remote/large.bin".to_string(),
        total_bytes: 100,
        transferred_bytes: 150,
        current_speed_bps: 10_000_000,
        status: TransferStatus::Completed,
        error_message: None,
    };
    assert_eq!(job_overflow.progress_ratio(), 1.0);
}

#[test]
fn test_transfer_job_rejection_of_invalid_inputs() {
    // Missing required field: jobId
    let missing_job_id = serde_json::json!({
        "direction": "MacToAndroid",
        "sourcePath": "/a",
        "destinationPath": "/b",
        "totalBytes": 100,
        "transferredBytes": 50,
        "currentSpeedBps": 1000,
        "status": "Transferring"
    });
    assert!(serde_json::from_value::<TransferJob>(missing_job_id).is_err());

    // Missing required field: status
    let missing_status = serde_json::json!({
        "jobId": "test-123",
        "direction": "MacToAndroid",
        "sourcePath": "/a",
        "destinationPath": "/b",
        "totalBytes": 100,
        "transferredBytes": 50,
        "currentSpeedBps": 1000
    });
    assert!(serde_json::from_value::<TransferJob>(missing_status).is_err());

    // Invalid enum value for direction
    let invalid_direction = serde_json::json!({
        "jobId": "test-123",
        "direction": "UnknownDirection",
        "sourcePath": "/a",
        "destinationPath": "/b",
        "totalBytes": 100,
        "transferredBytes": 50,
        "currentSpeedBps": 1000,
        "status": "Transferring"
    });
    assert!(serde_json::from_value::<TransferJob>(invalid_direction).is_err());

    // Invalid enum value for status
    let invalid_status = serde_json::json!({
        "jobId": "test-123",
        "direction": "MacToAndroid",
        "sourcePath": "/a",
        "destinationPath": "/b",
        "totalBytes": 100,
        "transferredBytes": 50,
        "currentSpeedBps": 1000,
        "status": "Executing"
    });
    assert!(serde_json::from_value::<TransferJob>(invalid_status).is_err());

    // Negative totalBytes should fail u64 deserialization
    let negative_total_bytes = serde_json::json!({
        "jobId": "test-123",
        "direction": "MacToAndroid",
        "sourcePath": "/a",
        "destinationPath": "/b",
        "totalBytes": -100,
        "transferredBytes": 50,
        "currentSpeedBps": 1000,
        "status": "Transferring"
    });
    assert!(serde_json::from_value::<TransferJob>(negative_total_bytes).is_err());
}
