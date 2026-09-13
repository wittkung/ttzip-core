// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Android device management, hardware transport, and storage subsystem.
//!
//! Provides unified device discovery, high-speed ADB and MTP protocols, and
//! virtual filesystem drivers for direct archive operations on connected Android devices.

pub mod error;
pub mod models;
pub mod traits;

/// Hardware and network transport layer for USB (nusb / IOKit) and TCP channels.
pub mod transport;

/// Media Transfer Protocol (MTP 1.1) stack and storage operations.
pub mod mtp;

/// Android Debug Bridge (ADB) protocol client and SYNC stream pipeline.
pub mod adb;

/// Unified Android device management, auto-detection, and dynamic protocol elevation.
pub mod manager;

// Re-export primary types for ergonomic crate root usage.
pub use error::DeviceError;
pub use manager::DeviceManager;
pub use models::{
    AndroidDevice, AndroidVfsNode, ConnectionType, DeviceStatus, StoragePartition,
    TransferDirection, TransferJob, TransferStatus, VfsEntryType,
};
pub use traits::DeviceStorageDriver;

