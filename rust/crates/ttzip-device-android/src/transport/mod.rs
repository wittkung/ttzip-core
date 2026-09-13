// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware and network transport subsystem for Android device communication.
//!
//! Provides macOS IOKit USB transport, deterministic 4-step pipe stall recovery,
//! and real-time USB device hotplug event monitoring.

pub mod hotplug;
pub mod recovery;
pub mod usb_iokit;

// Re-export primary types for ergonomic usage across crate modules.
pub use hotplug::{HotplugDeviceInfo, HotplugEvent, HotplugWatcher, PollingHotplugScanner};
pub use recovery::{PipeRecoveryStateMachine, RecoveryStep, TransportRecoveryTarget};
pub use usb_iokit::{
    is_android_mtp_or_adb, list_android_usb_devices, SeizeDiagnosticReport, UsbTransport,
    KIO_RETURN_EXCLUSIVE_ACCESS, KIO_RETURN_EXCLUSIVE_ACCESS_RAW, USB_CLASS_STILL_IMAGE,
    USB_CLASS_VENDOR_SPECIFIC, USB_PROTOCOL_ADB, USB_PROTOCOL_MTP, USB_SUBCLASS_ADB,
    USB_SUBCLASS_STILL_IMAGE_PTP,
};
