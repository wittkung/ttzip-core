// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Event-driven USB hotplug notification bridge between macOS IOKit and Swift.

use parking_lot::RwLock;
use std::sync::Arc;
use ttzip_device_android::transport::hotplug::HotplugWatcher;

use crate::uniffi_api::types::TTZipError;

/// Cross-language callback interface notified upon physical USB hardware change.
#[uniffi::export(callback_interface)]
pub trait UniFFIDeviceEventListener: Send + Sync {
    /// Invoked whenever a USB device matching MTP/ADB is attached or detached.
    fn on_devices_changed(&self);
}

static ACTIVE_HOTPLUG_WATCHER: RwLock<Option<Arc<HotplugWatcher>>> = RwLock::new(None);

/// Starts the event-driven macOS IOKit notification runloop and registers the Swift listener.
#[uniffi::export]
pub fn uniffi_start_hotplug_monitoring(
    listener: Box<dyn UniFFIDeviceEventListener>,
) -> Result<(), TTZipError> {
    let mut guard = ACTIVE_HOTPLUG_WATCHER.write();
    if guard.is_some() {
        // Already monitoring
        return Ok(());
    }

    let watcher = Arc::new(HotplugWatcher::default());
    let mut rx = watcher.start_monitoring();
    *guard = Some(watcher);

    // Spawn background task dispatching IOKit notifications to Swift listener
    std::thread::Builder::new()
        .name("ttzip-uniffi-hotplug-dispatcher".to_string())
        .spawn(move || {
            while let Some(_event) = rx.blocking_recv() {
                listener.on_devices_changed();
            }
        })
        .map_err(|e| TTZipError::IoError {
            message: format!("Failed to spawn hotplug dispatch thread: {e}"),
        })?;

    Ok(())
}

/// Stops active hardware hotplug monitoring.
#[uniffi::export]
pub fn uniffi_stop_hotplug_monitoring() {
    let mut guard = ACTIVE_HOTPLUG_WATCHER.write();
    if let Some(watcher) = guard.take() {
        watcher.stop();
    }
}
