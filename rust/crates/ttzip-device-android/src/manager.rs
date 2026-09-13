// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified Android device manager, USB/network auto-detection, and protocol elevation.
//!
//! Provides the centralized `DeviceManager` coordinator to track attached Android devices,
//! scan physical USB interfaces via `nusb`, distinguish MTP vs ADB capabilities,
//! and dynamically elevate MTP drivers to high-speed ADB SYNC drivers upon user authorization.

use crate::adb::driver::AdbDeviceDriver;
use crate::error::DeviceError;
use crate::models::{
    AndroidDevice, ConnectionType, DeviceStatus, StoragePartition,
};
use crate::traits::DeviceStorageDriver;
use crate::transport::hotplug::HotplugDeviceInfo;
use crate::transport::usb_iokit::{
    USB_CLASS_STILL_IMAGE, USB_CLASS_VENDOR_SPECIFIC, USB_PROTOCOL_ADB, USB_PROTOCOL_MTP,
    USB_SUBCLASS_ADB, USB_SUBCLASS_STILL_IMAGE_PTP,
};
use nusb::MaybeFuture;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Internal tracked representation of an active or detected Android device.
#[derive(Clone)]
struct ManagedDevice {
    device: AndroidDevice,
    driver: Option<Arc<dyn DeviceStorageDriver>>,
}

/// Thread-safe coordinator for Android device discovery, driver lifecycle, and protocol elevation.
pub struct DeviceManager {
    devices: RwLock<HashMap<String, ManagedDevice>>,
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager {
    /// Creates a new empty `DeviceManager`.
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
        }
    }

    /// Registers or updates an active Android device and its optional storage driver.
    pub fn register_device(
        &self,
        device: AndroidDevice,
        driver: Option<Arc<dyn DeviceStorageDriver>>,
    ) {
        let mut guard = self.devices.write().expect("DeviceManager lock poisoned");
        guard.insert(
            device.device_id.clone(),
            ManagedDevice { device, driver },
        );
    }

    /// Unregisters an Android device by identifier, returning its metadata if present.
    pub fn unregister_device(&self, device_id: &str) -> Option<AndroidDevice> {
        let mut guard = self.devices.write().expect("DeviceManager lock poisoned");
        guard.remove(device_id).map(|entry| entry.device)
    }

    /// Retrieves a cloned snapshot of a registered Android device by identifier.
    pub fn get_device(&self, device_id: &str) -> Option<AndroidDevice> {
        let guard = self.devices.read().expect("DeviceManager lock poisoned");
        guard.get(device_id).map(|entry| entry.device.clone())
    }

    /// Retrieves an active storage driver for the specified device.
    pub fn get_driver(
        &self,
        device_id: &str,
    ) -> Result<Arc<dyn DeviceStorageDriver>, DeviceError> {
        let guard = self.devices.read().expect("DeviceManager lock poisoned");
        let entry = guard
            .get(device_id)
            .ok_or_else(|| DeviceError::DeviceNotFound(device_id.to_string()))?;

        entry
            .driver
            .clone()
            .ok_or(DeviceError::DeviceDisconnected)
    }

    /// Attaches or updates an active storage driver for a registered device.
    pub fn set_driver(
        &self,
        device_id: &str,
        driver: Arc<dyn DeviceStorageDriver>,
    ) -> Result<(), DeviceError> {
        let mut guard = self.devices.write().expect("DeviceManager lock poisoned");
        let entry = guard
            .get_mut(device_id)
            .ok_or_else(|| DeviceError::DeviceNotFound(device_id.to_string()))?;

        entry.driver = Some(driver);
        Ok(())
    }

    /// Lists all currently registered Android devices.
    pub fn list_devices(&self) -> Vec<AndroidDevice> {
        let guard = self.devices.read().expect("DeviceManager lock poisoned");
        guard.values().map(|entry| entry.device.clone()).collect()
    }

    /// Scans connected USB interfaces and detects Android devices matching MTP or ADB descriptors.
    ///
    /// Differentiates between MTP (`Class 0x06`, `Subclass 0x01`, `Protocol 0x01`) and
    /// ADB (`Class 0xFF`, `Subclass 0x42`, `Protocol 0x01`). Discovered devices are registered
    /// in the manager and returned.
    pub fn detect_devices(&self) -> Result<Vec<AndroidDevice>, DeviceError> {
        let list_iter = nusb::list_devices()
            .wait()
            .map_err(|e| DeviceError::TransportError(format!("Failed to list USB devices: {e}")))?;

        let mut detected = Vec::new();

        for dev in list_iter {
            let mut has_adb = false;
            let mut has_mtp = false;

            for intf in dev.interfaces() {
                let class = intf.class();
                let subclass = intf.subclass();
                let protocol = intf.protocol();

                // ADB: Vendor Specific (0xFF) / 0x42 / 0x01
                if class == USB_CLASS_VENDOR_SPECIFIC
                    && subclass == USB_SUBCLASS_ADB
                    && protocol == USB_PROTOCOL_ADB
                {
                    has_adb = true;
                }

                // MTP: Still Image (0x06) / Subclass 1 / Protocol 1, or generic Still Image
                if (class == USB_CLASS_STILL_IMAGE
                    && subclass == USB_SUBCLASS_STILL_IMAGE_PTP
                    && protocol == USB_PROTOCOL_MTP)
                    || class == USB_CLASS_STILL_IMAGE
                {
                    has_mtp = true;
                }
            }

            // Google Android VID fallback heuristic
            if !has_adb && !has_mtp && dev.vendor_id() == 0x18D1 {
                has_mtp = true;
            }

            if !has_adb && !has_mtp {
                continue;
            }

            let connection_type = if has_adb {
                ConnectionType::UsbAdb
            } else {
                ConnectionType::UsbMtp
            };

            let device_id =
                HotplugDeviceInfo::format_device_id(dev.bus_id(), dev.device_address());
            let display_name = dev
                .product_string()
                .or_else(|| dev.manufacturer_string())
                .unwrap_or("Android Device")
                .to_string();

            let serial_number = dev.serial_number().unwrap_or("UNKNOWN").to_string();

            let root_path = if connection_type == ConnectionType::UsbAdb {
                "/storage/emulated/0".to_string()
            } else {
                "/".to_string()
            };

            let default_partition = StoragePartition {
                partition_id: if connection_type == ConnectionType::UsbAdb {
                    "emulated_0".to_string()
                } else {
                    "0x00010001".to_string()
                },
                display_name: "Internal Shared Storage".to_string(),
                total_bytes: 128_000_000_000,
                available_bytes: 64_000_000_000,
                root_path,
                is_removable: false,
            };

            let android_device = AndroidDevice {
                device_id: device_id.clone(),
                display_name,
                vendor_id: dev.vendor_id(),
                product_id: dev.product_id(),
                serial_number,
                connection_type,
                status: DeviceStatus::Connected,
                storage_partitions: vec![default_partition],
            };

            // Register device without driver initially until claimed
            {
                let mut guard = self.devices.write().expect("DeviceManager lock poisoned");
                if let Some(existing) = guard.get_mut(&device_id) {
                    existing.device = android_device.clone();
                } else {
                    guard.insert(
                        device_id,
                        ManagedDevice {
                            device: android_device.clone(),
                            driver: None,
                        },
                    );
                }
            }

            detected.push(android_device);
        }

        Ok(detected)
    }

    /// Seamlessly elevates an active MTP device session to a high-speed ADB session.
    ///
    /// Triggered when the user enables USB debugging on the target device. This switches
    /// the driver pipeline from standard MTP to an authenticated ADB SYNC driver, unlocking
    /// fast batch traversal and penetration into `/Android/data` and `/Android/obb`.
    pub fn elevate_to_adb(
        &self,
        device_id: &str,
    ) -> Result<Arc<dyn DeviceStorageDriver>, DeviceError> {
        let mut guard = self.devices.write().expect("DeviceManager lock poisoned");
        let entry = guard
            .get_mut(device_id)
            .ok_or_else(|| DeviceError::DeviceNotFound(device_id.to_string()))?;

        // If already UsbAdb and has an active driver, return driver directly (idempotent)
        if entry.device.connection_type == ConnectionType::UsbAdb {
            if let Some(ref active_driver) = entry.driver {
                return Ok(active_driver.clone());
            }
        }

        let serial = if entry.device.serial_number.is_empty()
            || entry.device.serial_number == "UNKNOWN"
        {
            &entry.device.device_id
        } else {
            &entry.device.serial_number
        };

        let is_wireless = entry.device.connection_type == ConnectionType::WirelessAdb;
        let adb_driver: Arc<dyn DeviceStorageDriver> =
            Arc::new(AdbDeviceDriver::new(serial, is_wireless));

        // Update device model attributes to reflect ADB elevation
        entry.device.connection_type = ConnectionType::UsbAdb;
        entry.device.status = DeviceStatus::Connected;

        for partition in &mut entry.device.storage_partitions {
            partition.root_path = "/storage/emulated/0".to_string();
        }

        entry.driver = Some(adb_driver.clone());

        Ok(adb_driver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AndroidVfsNode;
    use bytes::Bytes;

    struct MockStorageDriver;

    impl crate::traits::DeviceStorageDriver for MockStorageDriver {
        fn list_directory<'a>(
            &'a self,
            _path: &'a str,
        ) -> crate::traits::BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>> {
            Box::pin(async move { Ok(vec![]) })
        }

        fn get_partial_object<'a>(
            &'a self,
            _path: &'a str,
            _offset: u64,
            _length: u32,
        ) -> crate::traits::BoxFuture<'a, Result<Bytes, DeviceError>> {
            Box::pin(async move { Ok(Bytes::from_static(b"PK\x05\x06")) })
        }

        fn send_object<'a>(
            &'a self,
            _destination_path: &'a str,
            _data: Bytes,
        ) -> crate::traits::BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn delete_object<'a>(
            &'a self,
            _path: &'a str,
        ) -> crate::traits::BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn disconnect<'a>(
            &'a self,
        ) -> crate::traits::BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }
    }

    fn sample_mtp_device(id: &str) -> AndroidDevice {
        AndroidDevice {
            device_id: id.to_string(),
            display_name: "Test Android Device".to_string(),
            vendor_id: 0x18D1,
            product_id: 0x4EE2,
            serial_number: "SERIAL1234".to_string(),
            connection_type: ConnectionType::UsbMtp,
            status: DeviceStatus::Connected,
            storage_partitions: vec![StoragePartition {
                partition_id: "0x00010001".to_string(),
                display_name: "Internal Storage".to_string(),
                total_bytes: 64_000_000_000,
                available_bytes: 32_000_000_000,
                root_path: "/".to_string(),
                is_removable: false,
            }],
        }
    }

    #[tokio::test]
    async fn test_manager_registration_and_driver_routing() {
        let manager = DeviceManager::new();
        let dev = sample_mtp_device("usb:001:002");

        let mock_driver: Arc<dyn DeviceStorageDriver> = Arc::new(MockStorageDriver);
        manager.register_device(dev.clone(), Some(mock_driver));

        assert_eq!(manager.list_devices().len(), 1);
        let fetched_dev = manager.get_device("usb:001:002").expect("Device not found");
        assert_eq!(fetched_dev.connection_type, ConnectionType::UsbMtp);

        let driver = manager.get_driver("usb:001:002").expect("Driver not found");
        let bytes = driver
            .get_partial_object("/test.zip", 0, 4)
            .await
            .expect("Failed to read");
        assert_eq!(&bytes[..], b"PK\x05\x06");
    }

    #[tokio::test]
    async fn test_manager_elevation_from_mtp_to_adb() {
        let manager = DeviceManager::new();
        let dev = sample_mtp_device("usb:001:003");
        manager.register_device(dev, None);

        // Elevate device to ADB
        let adb_driver = manager
            .elevate_to_adb("usb:001:003")
            .expect("Elevation to ADB failed");

        // Verify device metadata is elevated to UsbAdb
        let updated_dev = manager
            .get_device("usb:001:003")
            .expect("Device not found after elevation");
        assert_eq!(updated_dev.connection_type, ConnectionType::UsbAdb);
        assert_eq!(
            updated_dev.storage_partitions[0].root_path,
            "/storage/emulated/0"
        );

        // Verify ADB driver penetrates Scoped Storage (/Android/data)
        let entries = adb_driver
            .list_directory("/storage/emulated/0/Android/data")
            .await
            .expect("ADB driver should list restricted directories");
        assert!(entries
            .iter()
            .any(|e| e.name == "com.android.providers.media"));

        // Verify elevation idempotency
        let second_driver = manager
            .elevate_to_adb("usb:001:003")
            .expect("Second elevation should succeed");
        assert!(Arc::ptr_eq(&adb_driver, &second_driver));
    }

    #[test]
    fn test_manager_elevation_nonexistent_device_fails() {
        let manager = DeviceManager::new();
        let result = manager.elevate_to_adb("nonexistent-device");
        assert!(matches!(result, Err(DeviceError::DeviceNotFound(_))));
    }

    #[test]
    fn test_manager_unregister_device() {
        let manager = DeviceManager::new();
        let dev = sample_mtp_device("usb:001:004");
        manager.register_device(dev, None);

        let removed = manager.unregister_device("usb:001:004");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().device_id, "usb:001:004");
        assert!(manager.get_device("usb:001:004").is_none());
        assert!(matches!(
            manager.get_driver("usb:001:004"),
            Err(DeviceError::DeviceNotFound(_))
        ));
    }

    #[test]
    fn test_manager_detect_devices_usb_call() {
        let manager = DeviceManager::new();
        // nusb::list_devices should succeed in host environment
        let result = manager.detect_devices();
        assert!(result.is_ok());
    }
}
