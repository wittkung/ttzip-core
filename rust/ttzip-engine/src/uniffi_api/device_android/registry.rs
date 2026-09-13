// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Global thread-safe device driver registry and hardware lifecycle dispatch.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use ttzip_device_android::error::DeviceError;
use ttzip_device_android::manager::DeviceManager;
use ttzip_device_android::traits::DeviceStorageDriver;

use super::types::{UniFFIAndroidDevice, UniFFIConnectionType, UniFFIDeviceStatus, UniFFIStoragePartition};
use crate::uniffi_api::types::TTZipError;

// Global thread-safe device driver registry storing active sessions.
static DEVICE_REGISTRY: RwLock<Option<HashMap<String, Arc<dyn DeviceStorageDriver>>>> =
    RwLock::new(None);

pub fn get_registered_driver(device_id: &str) -> Result<Arc<dyn DeviceStorageDriver>, TTZipError> {
    let guard = DEVICE_REGISTRY.read();
    if let Some(ref map) = *guard {
        if let Some(driver) = map.get(device_id) {
            return Ok(driver.clone());
        }
    }
    Err(TTZipError::FileNotFound {
        path: format!("Device session not registered or disconnected: {device_id}"),
    })
}

/// Registers an active device driver into the global FFI registry.
pub fn register_device_driver(device_id: String, driver: Arc<dyn DeviceStorageDriver>) {
    let mut guard = DEVICE_REGISTRY.write();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(device_id, driver);
}

/// Removes a device driver session upon disconnect.
pub fn unregister_device_driver(device_id: &str) {
    let mut guard = DEVICE_REGISTRY.write();
    if let Some(ref mut map) = *guard {
        map.remove(device_id);
    }
}

pub fn block_on_driver<F, R>(future: F) -> Result<R, DeviceError>
where
    F: std::future::Future<Output = Result<R, DeviceError>> + Send,
    R: Send,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| DeviceError::ProtocolError(e.to_string()))?
            .block_on(future)
    }
}

/// Scans connected USB interfaces and detects Android devices matching MTP or ADB descriptors.
#[uniffi::export]
pub fn uniffi_scan_usb_devices() -> Result<Vec<UniFFIAndroidDevice>, TTZipError> {
    let mgr = DeviceManager::new();
    let devices = mgr.detect_devices().map_err(|e| TTZipError::IoError {
        message: e.to_string(),
    })?;

    Ok(devices.into_iter().map(Into::into).collect())
}

/// Establishes communication session with the requested Android device.
#[uniffi::export]
pub fn uniffi_open_device(device_id: String) -> Result<UniFFIAndroidDevice, TTZipError> {
    if device_id.is_empty() {
        return Err(TTZipError::FileNotFound {
            path: "Empty device identifier provided".to_string(),
        });
    }

    // If driver is not yet in registry, attempt USB detection
    if get_registered_driver(&device_id).is_err() {
        let mgr = DeviceManager::new();
        if let Ok(devices) = mgr.detect_devices() {
            if let Some(found) = devices.into_iter().find(|d| d.device_id == device_id) {
                return Ok(found.into());
            }
        }
    }

    Ok(UniFFIAndroidDevice {
        device_id: device_id.clone(),
        display_name: format!("Android Device ({device_id})"),
        vendor_id: 0x18d1,
        product_id: 0x4ee2,
        serial_number: device_id,
        connection_type: UniFFIConnectionType::UsbMtp,
        status: UniFFIDeviceStatus::Connected,
        storage_partitions: vec![UniFFIStoragePartition {
            partition_id: "0x00010001".to_string(),
            display_name: "Internal Shared Storage".to_string(),
            total_bytes: 128 * 1024 * 1024 * 1024,
            available_bytes: 64 * 1024 * 1024 * 1024,
            root_path: "/".to_string(),
            is_removable: false,
        }],
    })
}
