// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! UniFFI data records and enum representations for Android device integration.

use ttzip_device_android::models::{
    AndroidDevice, AndroidVfsNode, ConnectionType, DeviceStatus, StoragePartition,
    TransferDirection, TransferJob, TransferStatus, VfsEntryType,
};

/// UniFFI-exported connection channel type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIConnectionType {
    UsbMtp,
    UsbAdb,
    WirelessAdb,
}

impl From<ConnectionType> for UniFFIConnectionType {
    fn from(c: ConnectionType) -> Self {
        match c {
            ConnectionType::UsbMtp => UniFFIConnectionType::UsbMtp,
            ConnectionType::UsbAdb => UniFFIConnectionType::UsbAdb,
            ConnectionType::WirelessAdb => UniFFIConnectionType::WirelessAdb,
        }
    }
}

/// UniFFI-exported device lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIDeviceStatus {
    Connecting,
    SeizingInterface,
    Connected,
    Stalled,
    Disconnected,
    Error,
}

impl From<DeviceStatus> for UniFFIDeviceStatus {
    fn from(s: DeviceStatus) -> Self {
        match s {
            DeviceStatus::Connecting => UniFFIDeviceStatus::Connecting,
            DeviceStatus::SeizingInterface => UniFFIDeviceStatus::SeizingInterface,
            DeviceStatus::Connected => UniFFIDeviceStatus::Connected,
            DeviceStatus::Stalled => UniFFIDeviceStatus::Stalled,
            DeviceStatus::Disconnected => UniFFIDeviceStatus::Disconnected,
            DeviceStatus::Error => UniFFIDeviceStatus::Error,
        }
    }
}

/// UniFFI-exported VFS entry category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIVfsEntryType {
    File,
    Directory,
    Symlink,
    RestrictedDirectory,
}

impl From<VfsEntryType> for UniFFIVfsEntryType {
    fn from(e: VfsEntryType) -> Self {
        match e {
            VfsEntryType::File => UniFFIVfsEntryType::File,
            VfsEntryType::Directory => UniFFIVfsEntryType::Directory,
            VfsEntryType::Symlink => UniFFIVfsEntryType::Symlink,
            VfsEntryType::RestrictedDirectory => UniFFIVfsEntryType::RestrictedDirectory,
        }
    }
}

/// UniFFI-exported file transfer pipeline direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFITransferDirection {
    MacToAndroid,
    AndroidToMac,
    DirectPipelineExtract,
}

impl From<TransferDirection> for UniFFITransferDirection {
    fn from(d: TransferDirection) -> Self {
        match d {
            TransferDirection::MacToAndroid => UniFFITransferDirection::MacToAndroid,
            TransferDirection::AndroidToMac => UniFFITransferDirection::AndroidToMac,
            TransferDirection::DirectPipelineExtract => UniFFITransferDirection::DirectPipelineExtract,
        }
    }
}

/// UniFFI-exported transfer execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFITransferStatus {
    Queued,
    Transferring,
    Paused,
    Cancelling,
    Completed,
    Failed,
}

impl From<TransferStatus> for UniFFITransferStatus {
    fn from(s: TransferStatus) -> Self {
        match s {
            TransferStatus::Queued => UniFFITransferStatus::Queued,
            TransferStatus::Transferring => UniFFITransferStatus::Transferring,
            TransferStatus::Paused => UniFFITransferStatus::Paused,
            TransferStatus::Cancelling => UniFFITransferStatus::Cancelling,
            TransferStatus::Completed => UniFFITransferStatus::Completed,
            TransferStatus::Failed => UniFFITransferStatus::Failed,
        }
    }
}

/// UniFFI record describing an individual storage volume or SD card partition.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIStoragePartition {
    pub partition_id: String,
    pub display_name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub root_path: String,
    pub is_removable: bool,
}

impl From<StoragePartition> for UniFFIStoragePartition {
    fn from(p: StoragePartition) -> Self {
        Self {
            partition_id: p.partition_id,
            display_name: p.display_name,
            total_bytes: p.total_bytes,
            available_bytes: p.available_bytes,
            root_path: p.root_path,
            is_removable: p.is_removable,
        }
    }
}

/// UniFFI record representing an attached Android hardware or network device.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIAndroidDevice {
    pub device_id: String,
    pub display_name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub connection_type: UniFFIConnectionType,
    pub status: UniFFIDeviceStatus,
    pub storage_partitions: Vec<UniFFIStoragePartition>,
}

impl From<AndroidDevice> for UniFFIAndroidDevice {
    fn from(d: AndroidDevice) -> Self {
        Self {
            device_id: d.device_id,
            display_name: d.display_name,
            vendor_id: d.vendor_id,
            product_id: d.product_id,
            serial_number: d.serial_number,
            connection_type: d.connection_type.into(),
            status: d.status.into(),
            storage_partitions: d.storage_partitions.into_iter().map(Into::into).collect(),
        }
    }
}

/// UniFFI record representing a remote VFS node in the Android file hierarchy.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIAndroidVfsNode {
    pub path: String,
    pub name: String,
    pub entry_type: UniFFIVfsEntryType,
    pub size_bytes: u64,
    pub modified_timestamp: u64,
    pub object_handle: Option<u32>,
    pub is_restricted: bool,
}

impl From<AndroidVfsNode> for UniFFIAndroidVfsNode {
    fn from(n: AndroidVfsNode) -> Self {
        Self {
            path: n.path,
            name: n.name,
            entry_type: n.entry_type.into(),
            size_bytes: n.size_bytes,
            modified_timestamp: n.modified_timestamp,
            object_handle: n.object_handle,
            is_restricted: n.is_restricted,
        }
    }
}

/// UniFFI record representing an ongoing or completed transfer job.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFITransferJob {
    pub job_id: String,
    pub direction: UniFFITransferDirection,
    pub source_path: String,
    pub destination_path: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub current_speed_bps: u64,
    pub status: UniFFITransferStatus,
    pub error_message: Option<String>,
}

impl From<TransferJob> for UniFFITransferJob {
    fn from(j: TransferJob) -> Self {
        Self {
            job_id: j.job_id,
            direction: j.direction.into(),
            source_path: j.source_path,
            destination_path: j.destination_path,
            total_bytes: j.total_bytes,
            transferred_bytes: j.transferred_bytes,
            current_speed_bps: j.current_speed_bps,
            status: j.status.into(),
            error_message: j.error_message,
        }
    }
}
