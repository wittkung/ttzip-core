// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core entity models, enumerations, and state representations for Android devices.

use serde::{Deserialize, Serialize};

/// Communication protocol channel used to interact with the Android device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// Standard MTP 1.1 USB connection (zero-configuration plug-and-play).
    UsbMtp,
    /// High-speed ADB protocol over physical USB cable.
    UsbAdb,
    /// Wireless ADB protocol over local network (Wi-Fi).
    WirelessAdb,
}

/// Lifecycle connection state of an Android physical or wireless device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// Device attachment or mDNS service discovery in progress.
    Connecting,
    /// Acquiring exclusive hardware USB interface claim (e.g. `USBInterfaceOpenSeize`).
    SeizingInterface,
    /// Fully connected, authenticated, and ready for operations.
    Connected,
    /// Pipe communication stalled, undergoing recovery state machine.
    Stalled,
    /// Hardware detached, cable unplugged, or session terminated.
    Disconnected,
    /// Fatal unrecoverable communication or protocol error.
    Error,
}

/// Type classification of an entry in the remote virtual file system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VfsEntryType {
    /// Regular file with readable/writable content.
    File,
    /// Standard traversable directory.
    Directory,
    /// Symbolic link pointing to another path.
    Symlink,
    /// Protected directory restricted by Android 11+ Scoped Storage (e.g. `/Android/data`).
    RestrictedDirectory,
}

/// Direction of a file transfer or decompression stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    /// Transfer payload from host macOS to Android device storage.
    MacToAndroid,
    /// Transfer payload from Android device storage to host macOS.
    AndroidToMac,
    /// Decompress directly from archive into Android device storage without local disk staging.
    DirectPipelineExtract,
}

/// Execution status of a file transfer or stream extraction job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    /// Transfer task is queued waiting for execution resources.
    Queued,
    /// Active payload transmission in progress.
    Transferring,
    /// Transfer temporarily paused by user or network jitter.
    Paused,
    /// Cancellation initiated, rolling back partial state.
    Cancelling,
    /// Successfully transferred all bytes and finalized media scan.
    Completed,
    /// Transfer failed due to I/O error or resource exhaustion.
    Failed,
}

/// Represents an active or detected Android physical or wireless device connected to the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevice {
    /// Unique identifier (e.g. `usb:05ac:1234:ABCDEF` or `wifi:192.168.1.50:5555`).
    pub device_id: String,
    /// Human-readable display label (e.g. "Pixel 8 Pro", "Samsung Galaxy S24").
    pub display_name: String,
    /// USB Vendor ID (or 0 for network devices).
    pub vendor_id: u16,
    /// USB Product ID (or 0 for network devices).
    pub product_id: u16,
    /// Hardware serial number or ADB device serial.
    pub serial_number: String,
    /// Protocol channel: `UsbMtp`, `UsbAdb`, or `WirelessAdb`.
    pub connection_type: ConnectionType,
    /// Current lifecycle state.
    pub status: DeviceStatus,
    /// Array of mounted logical storage partitions.
    pub storage_partitions: Vec<StoragePartition>,
}

/// Represents a logical storage volume reported by the device (e.g., Internal Storage, SD Card).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePartition {
    /// Partition identifier (MTP 32-bit StorageID string or mount path).
    pub partition_id: String,
    /// Display label (e.g. "Internal Storage", "SanDisk SD Card").
    pub display_name: String,
    /// Total storage capacity in bytes.
    pub total_bytes: u64,
    /// Available free space in bytes.
    pub available_bytes: u64,
    /// Root path (e.g. `/` for MTP, `/storage/emulated/0` for ADB).
    pub root_path: String,
    /// True if removable media (SD card, OTG USB drive).
    pub is_removable: bool,
}

impl StoragePartition {
    /// Computes the number of used bytes on this partition.
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }
}

/// Represents a file or directory node in the remote virtual file system hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidVfsNode {
    /// Full normalized path from partition root.
    pub path: String,
    /// Base file or directory name.
    pub name: String,
    /// Entry classification: `File`, `Directory`, `Symlink`, or `RestrictedDirectory`.
    pub entry_type: VfsEntryType,
    /// File size in bytes (0 for directories).
    pub size_bytes: u64,
    /// Last modification time (Unix epoch seconds).
    pub modified_timestamp: u64,
    /// MTP 32-bit ObjectHandle if communicating via MTP.
    pub object_handle: Option<u32>,
    /// True if protected by Android 11+ Scoped Storage (e.g. `/Android/data`).
    pub is_restricted: bool,
}

impl AndroidVfsNode {
    /// Returns true if this node is a directory or restricted directory.
    pub fn is_dir(&self) -> bool {
        matches!(
            self.entry_type,
            VfsEntryType::Directory | VfsEntryType::RestrictedDirectory
        )
    }

    /// Returns true if this node represents a regular file.
    pub fn is_file(&self) -> bool {
        matches!(self.entry_type, VfsEntryType::File)
    }
}

/// Represents an ongoing or completed read, write, or direct decompression operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferJob {
    /// UUID string uniquely identifying the transfer task.
    pub job_id: String,
    /// Transfer direction and pipeline mode.
    pub direction: TransferDirection,
    /// Local or remote source path.
    pub source_path: String,
    /// Local or remote destination path.
    pub destination_path: String,
    /// Total byte count of files to transfer.
    pub total_bytes: u64,
    /// Number of bytes written so far.
    pub transferred_bytes: u64,
    /// Rolling throughput in bytes per second.
    pub current_speed_bps: u64,
    /// Lifecycle status of the transfer.
    pub status: TransferStatus,
    /// Failure description if status is `Failed`.
    pub error_message: Option<String>,
}

impl TransferJob {
    /// Calculates current progress as a normalized ratio between 0.0 and 1.0.
    pub fn progress_ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.transferred_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_serialization_contract() {
        let device = AndroidDevice {
            device_id: "usb:18d1:4ee2:TEST1234".to_string(),
            display_name: "Pixel 8 Pro".to_string(),
            vendor_id: 0x18d1,
            product_id: 0x4ee2,
            serial_number: "TEST1234".to_string(),
            connection_type: ConnectionType::UsbMtp,
            status: DeviceStatus::Connected,
            storage_partitions: vec![StoragePartition {
                partition_id: "0x00010001".to_string(),
                display_name: "Internal Storage".to_string(),
                total_bytes: 128_000_000_000,
                available_bytes: 64_000_000_000,
                root_path: "/".to_string(),
                is_removable: false,
            }],
        };

        let json = serde_json::to_string(&device).expect("Failed to serialize AndroidDevice");
        assert!(json.contains("\"deviceId\":\"usb:18d1:4ee2:TEST1234\""));
        assert!(json.contains("\"displayName\":\"Pixel 8 Pro\""));
        assert!(json.contains("\"vendorId\":6353"));
        assert!(json.contains("\"connectionType\":\"UsbMtp\""));
        assert!(json.contains("\"status\":\"Connected\""));
        assert!(json.contains("\"storagePartitions\":["));

        let deserialized: AndroidDevice =
            serde_json::from_str(&json).expect("Failed to deserialize AndroidDevice");
        assert_eq!(device, deserialized);
        assert_eq!(deserialized.storage_partitions[0].used_bytes(), 64_000_000_000);
    }

    #[test]
    fn test_transfer_job_contract() {
        let job = TransferJob {
            job_id: "e58ed763-928c-4155-bee9-fdbaaadc15f3".to_string(),
            direction: TransferDirection::DirectPipelineExtract,
            source_path: "/archive.zip".to_string(),
            destination_path: "/Download/extracted".to_string(),
            total_bytes: 1000,
            transferred_bytes: 500,
            current_speed_bps: 45_000_000,
            status: TransferStatus::Transferring,
            error_message: None,
        };

        let json = serde_json::to_string(&job).expect("Failed to serialize TransferJob");
        assert!(json.contains("\"jobId\":\"e58ed763-928c-4155-bee9-fdbaaadc15f3\""));
        assert!(json.contains("\"direction\":\"DirectPipelineExtract\""));
        assert!(json.contains("\"currentSpeedBps\":45000000"));
        assert!((job.progress_ratio() - 0.5).abs() < f64::EPSILON);

        let deserialized: TransferJob =
            serde_json::from_str(&json).expect("Failed to deserialize TransferJob");
        assert_eq!(job, deserialized);
    }

    #[test]
    fn test_vfs_node_predicates() {
        let dir = AndroidVfsNode {
            path: "/Download".to_string(),
            name: "Download".to_string(),
            entry_type: VfsEntryType::Directory,
            size_bytes: 0,
            modified_timestamp: 1700000000,
            object_handle: Some(42),
            is_restricted: false,
        };
        assert!(dir.is_dir());
        assert!(!dir.is_file());

        let file = AndroidVfsNode {
            path: "/Download/sample.zip".to_string(),
            name: "sample.zip".to_string(),
            entry_type: VfsEntryType::File,
            size_bytes: 1048576,
            modified_timestamp: 1700000000,
            object_handle: Some(43),
            is_restricted: false,
        };
        assert!(!file.is_dir());
        assert!(file.is_file());
    }
}
