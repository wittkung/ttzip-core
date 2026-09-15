// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! macOS IOKit USB transport, interface seizing, and Bulk I/O pipeline.
//!
//! Provides direct user-space USB communication via `nusb` and macOS IOKit,
//! implementing interface claiming, `USBInterfaceOpenSeize` fault diagnostics,
//! and integration with the 4-step pipe stall self-healing recovery state machine.

use crate::error::DeviceError;
use crate::transport::hotplug::HotplugDeviceInfo;
use crate::transport::recovery::TransportRecoveryTarget;
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{Endpoint, Interface, MaybeFuture};
use std::time::Duration;
use log::{debug, error, info, warn};

/// Apple IOKit kernel error code unsigned raw mask (`kIOReturnExclusiveAccess`).
pub const KIO_RETURN_EXCLUSIVE_ACCESS_RAW: u32 = 0xE000_02C5;

/// Apple IOKit kernel error code indicating exclusive interface access violation (`kIOReturnExclusiveAccess`).
///
/// In unsigned 32-bit representation: `0xE00002C5`; as a signed 32-bit integer: `-536870203`.
pub const KIO_RETURN_EXCLUSIVE_ACCESS: i32 = KIO_RETURN_EXCLUSIVE_ACCESS_RAW as i32;

/// USB interface class code for Still Image / PTP / MTP devices.
pub const USB_CLASS_STILL_IMAGE: u8 = 0x06;

/// USB interface subclass code for PTP / MTP.
pub const USB_SUBCLASS_STILL_IMAGE_PTP: u8 = 0x01;

/// USB interface protocol code for standard PTP / MTP.
pub const USB_PROTOCOL_MTP: u8 = 0x01;

/// USB interface class code for Vendor-Specific devices (used by ADB).
pub const USB_CLASS_VENDOR_SPECIFIC: u8 = 0xFF;

/// USB interface subclass code for Android ADB.
pub const USB_SUBCLASS_ADB: u8 = 0x42;

/// USB interface protocol code for Android ADB.
pub const USB_PROTOCOL_ADB: u8 = 0x01;

/// Diagnostics and recommended remediation for macOS PTPCamera / icdd interface locking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeizeDiagnosticReport {
    /// Kernel return code (usually `0xE00002C5`).
    pub kernel_code: i32,
    /// Human-readable explanation of the conflict.
    pub explanation: String,
    /// Terminal command to permanently prevent ImageCapture daemon conflicts.
    pub permanent_fix_command: String,
    /// Terminal command to immediately terminate conflicting PTPCamera process.
    pub temporary_kill_command: String,
}

impl SeizeDiagnosticReport {
    /// Creates a diagnostic report for an exclusive access error on macOS.
    pub fn new_exclusive_access_report() -> Self {
        Self {
            kernel_code: KIO_RETURN_EXCLUSIVE_ACCESS,
            explanation: "macOS system daemon (icdd / PTPCamera.app) holds exclusive USBInterfaceOpen lock on Android MTP interface.".to_string(),
            permanent_fix_command: "defaults write com.apple.ImageCapture disableHotPlug -bool YES".to_string(),
            temporary_kill_command: "pkill -9 PTPCamera".to_string(),
        }
    }
}

/// Identifies whether a given device descriptor matches Android MTP or ADB interfaces.
pub fn is_android_mtp_or_adb(info: &nusb::DeviceInfo) -> bool {
    for intf in info.interfaces() {
        let class = intf.class();
        let subclass = intf.subclass();
        let protocol = intf.protocol();

        // Android MTP: Still Image (0x06) / Subclass 1 / Protocol 1, or Vendor Specific (0xFF) MTP
        if class == USB_CLASS_STILL_IMAGE
            && subclass == USB_SUBCLASS_STILL_IMAGE_PTP
            && protocol == USB_PROTOCOL_MTP
        {
            return true;
        }

        // Android ADB: Vendor Specific (0xFF) / 0x42 / 0x01
        if class == USB_CLASS_VENDOR_SPECIFIC
            && subclass == USB_SUBCLASS_ADB
            && protocol == USB_PROTOCOL_ADB
        {
            return true;
        }
    }

    // Secondary heuristic: Google Android VID
    if info.vendor_id() == 0x18D1 {
        return true;
    }

    false
}

/// Discovers connected Android devices matching MTP or ADB USB descriptors.
pub fn list_android_usb_devices() -> Result<Vec<HotplugDeviceInfo>, DeviceError> {
    let list_iter = nusb::list_devices()
        .wait()
        .map_err(|e| DeviceError::TransportError(format!("Failed to list USB devices: {e}")))?;

    let mut android_devices = Vec::new();
    for dev in list_iter {
        if is_android_mtp_or_adb(&dev) {
            android_devices.push(HotplugDeviceInfo::from_nusb(&dev));
        }
    }

    Ok(android_devices)
}

/// Hardware USB transport session wrapping `nusb` device, interface, and endpoints.
pub struct UsbTransport {
    device_id: String,
    device: nusb::Device,
    interface: Option<Interface>,
    in_endpoint: Option<Endpoint<Bulk, In>>,
    out_endpoint: Option<Endpoint<Bulk, Out>>,
    interface_number: Option<u8>,
    in_ep_addr: Option<u8>,
    out_ep_addr: Option<u8>,
}

impl UsbTransport {
    /// Opens a USB device by its bus and address identifier (e.g. `usb:020:004`).
    pub fn open_by_id(target_id: &str) -> Result<Self, DeviceError> {
        let list_iter = nusb::list_devices()
            .wait()
            .map_err(|e| {
                DeviceError::TransportError(format!("Failed to query USB device list: {e}"))
            })?;

        for info in list_iter {
            let dev_id = HotplugDeviceInfo::format_device_id(info.bus_id(), info.device_address());
            if dev_id == target_id {
                let device = info.open().wait().map_err(|e| {
                    DeviceError::TransportError(format!("Failed to open USB device {target_id}: {e}"))
                })?;

                return Ok(Self {
                    device_id: target_id.to_string(),
                    device,
                    interface: None,
                    in_endpoint: None,
                    out_endpoint: None,
                    interface_number: None,
                    in_ep_addr: None,
                    out_ep_addr: None,
                });
            }
        }

        Err(DeviceError::DeviceNotFound(target_id.to_string()))
    }

    /// Constructs a `UsbTransport` from an already opened `nusb::Device`.
    pub fn from_opened_device(device_id: String, device: nusb::Device) -> Self {
        Self {
            device_id,
            device,
            interface: None,
            in_endpoint: None,
            out_endpoint: None,
            interface_number: None,
            in_ep_addr: None,
            out_ep_addr: None,
        }
    }

    /// Returns the unique device identifier.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Returns the claimed interface number, if claimed.
    pub fn interface_number(&self) -> Option<u8> {
        self.interface_number
    }

    /// Returns true if an interface is actively claimed.
    pub fn is_claimed(&self) -> bool {
        self.interface.is_some()
    }

    /// Claims the specified USB interface and initializes Bulk IN and OUT endpoints.
    ///
    /// If macOS `PTPCamera` or `icdd` holds exclusive access (`kIOReturnExclusiveAccess`),
    /// this function diagnoses the conflict and returns `DeviceError::InterfaceSeizeFailed`.
    pub fn claim_interface(
        &mut self,
        interface_number: u8,
        in_ep: u8,
        out_ep: u8,
    ) -> Result<(), DeviceError> {
        info!(
            "Claiming USB interface {} for device {} (IN: 0x{:02x}, OUT: 0x{:02x})",
            interface_number, self.device_id, in_ep, out_ep
        );

        let intf_res = self.device.claim_interface(interface_number).wait();
        let interface = match intf_res {
            Ok(intf) => intf,
            Err(e) => {
                let is_busy = e.kind() == nusb::ErrorKind::Busy
                    || e.kind() == nusb::ErrorKind::PermissionDenied
                    || e.os_error() == Some(KIO_RETURN_EXCLUSIVE_ACCESS_RAW)
                    || e.os_error() == Some(16);

                if is_busy {
                    let diag = SeizeDiagnosticReport::new_exclusive_access_report();
                    error!(
                        "USBInterfaceOpenSeize required: {}. Remediation: run `{}` or `{}`",
                        diag.explanation, diag.temporary_kill_command, diag.permanent_fix_command
                    );
                    return Err(DeviceError::InterfaceSeizeFailed(KIO_RETURN_EXCLUSIVE_ACCESS));
                }

                return Err(DeviceError::TransportError(format!(
                    "Failed to claim interface {interface_number}: {e}"
                )));
            }
        };

        // Initialize Bulk IN endpoint
        let in_endpoint = interface
            .endpoint::<Bulk, In>(in_ep)
            .map_err(|e| DeviceError::TransportError(format!("Failed to open Bulk IN endpoint 0x{in_ep:02x}: {e}")))?;

        // Initialize Bulk OUT endpoint
        let out_endpoint = interface
            .endpoint::<Bulk, Out>(out_ep)
            .map_err(|e| DeviceError::TransportError(format!("Failed to open Bulk OUT endpoint 0x{out_ep:02x}: {e}")))?;

        self.interface = Some(interface);
        self.in_endpoint = Some(in_endpoint);
        self.out_endpoint = Some(out_endpoint);
        self.interface_number = Some(interface_number);
        self.in_ep_addr = Some(in_ep);
        self.out_ep_addr = Some(out_ep);

        info!("Successfully claimed interface {} and opened endpoints", interface_number);
        Ok(())
    }

    /// Performs a synchronous blocking read from the Bulk IN endpoint.
    pub fn bulk_read(&mut self, length: usize, timeout: Duration) -> Result<Vec<u8>, DeviceError> {
        let ep = self
            .in_endpoint
            .as_mut()
            .ok_or_else(|| DeviceError::TransportError("Bulk IN endpoint not configured".to_string()))?;

        let buffer = Buffer::new(length);
        let completion = ep.transfer_blocking(buffer, timeout);

        match completion.into_result() {
            Ok(buf) => Ok(buf.to_vec()),
            Err(TransferError::Stall) => {
                let addr = self.in_ep_addr.unwrap_or(0x81);
                warn!("Bulk IN endpoint 0x{:02x} stalled", addr);
                Err(DeviceError::PipeStall(addr))
            }
            Err(TransferError::Cancelled) => {
                Err(DeviceError::ProtocolTimeout(format!("Bulk read timed out after {timeout:?}")))
            }
            Err(TransferError::Disconnected) => Err(DeviceError::DeviceDisconnected),
            Err(e) => Err(DeviceError::TransportError(format!("Bulk read failed: {e}"))),
        }
    }

    /// Performs a synchronous blocking write to the Bulk OUT endpoint.
    pub fn bulk_write(&mut self, data: &[u8], timeout: Duration) -> Result<usize, DeviceError> {
        let ep = self
            .out_endpoint
            .as_mut()
            .ok_or_else(|| DeviceError::TransportError("Bulk OUT endpoint not configured".to_string()))?;

        let buffer = Buffer::from(data);
        let completion = ep.transfer_blocking(buffer, timeout);

        match completion.into_result() {
            Ok(buf) => Ok(buf.len()),
            Err(TransferError::Stall) => {
                let addr = self.out_ep_addr.unwrap_or(0x02);
                warn!("Bulk OUT endpoint 0x{:02x} stalled", addr);
                Err(DeviceError::PipeStall(addr))
            }
            Err(TransferError::Cancelled) => {
                Err(DeviceError::ProtocolTimeout(format!("Bulk write timed out after {timeout:?}")))
            }
            Err(TransferError::Disconnected) => Err(DeviceError::DeviceDisconnected),
            Err(e) => Err(DeviceError::TransportError(format!("Bulk write failed: {e}"))),
        }
    }

    /// Aborts all in-flight asynchronous requests on an endpoint.
    pub fn abort_pipe_endpoint(&mut self, ep_addr: u8) -> Result<(), DeviceError> {
        if self.in_ep_addr == Some(ep_addr) {
            if let Some(ep) = self.in_endpoint.as_mut() {
                debug!("Aborting in-flight transfers on Bulk IN endpoint 0x{:02x}", ep_addr);
                ep.cancel_all();
                return Ok(());
            }
        }

        if self.out_ep_addr == Some(ep_addr) {
            if let Some(ep) = self.out_endpoint.as_mut() {
                debug!("Aborting in-flight transfers on Bulk OUT endpoint 0x{:02x}", ep_addr);
                ep.cancel_all();
                return Ok(());
            }
        }

        Err(DeviceError::TransportError(format!(
            "Endpoint 0x{ep_addr:02x} not attached to current transport"
        )))
    }

    /// Clears the halt / stall condition on the specified endpoint.
    pub fn clear_pipe_stall_endpoint(&mut self, ep_addr: u8) -> Result<(), DeviceError> {
        if self.in_ep_addr == Some(ep_addr) {
            if let Some(ep) = self.in_endpoint.as_mut() {
                debug!("Clearing stall condition on Bulk IN endpoint 0x{:02x}", ep_addr);
                ep.clear_halt()
                    .wait()
                    .map_err(|e| DeviceError::TransportError(format!("Failed to clear halt on 0x{ep_addr:02x}: {e}")))?;
                return Ok(());
            }
        }

        if self.out_ep_addr == Some(ep_addr) {
            if let Some(ep) = self.out_endpoint.as_mut() {
                debug!("Clearing stall condition on Bulk OUT endpoint 0x{:02x}", ep_addr);
                ep.clear_halt()
                    .wait()
                    .map_err(|e| DeviceError::TransportError(format!("Failed to clear halt on 0x{ep_addr:02x}: {e}")))?;
                return Ok(());
            }
        }

        Err(DeviceError::TransportError(format!(
            "Endpoint 0x{ep_addr:02x} not attached to current transport"
        )))
    }

    /// Releases claimed interface and drops endpoints.
    pub fn release_interface(&mut self) {
        self.in_endpoint = None;
        self.out_endpoint = None;
        self.interface = None;
        self.interface_number = None;
        self.in_ep_addr = None;
        self.out_ep_addr = None;
    }
}

impl TransportRecoveryTarget for UsbTransport {
    fn abort_pipe(&mut self, ep_addr: u8) -> Result<(), DeviceError> {
        self.abort_pipe_endpoint(ep_addr)
    }

    fn clear_pipe_stall(&mut self, ep_addr: u8) -> Result<(), DeviceError> {
        self.clear_pipe_stall_endpoint(ep_addr)
    }

    fn protocol_reset(&mut self) -> Result<(), DeviceError> {
        debug!("Executing upper protocol reset for USB device {}", self.device_id);
        // Protocol layer reset: release and re-claim interface
        if let Some(intf_num) = self.interface_number {
            let in_ep = self.in_ep_addr.unwrap_or(0x81);
            let out_ep = self.out_ep_addr.unwrap_or(0x02);
            self.release_interface();
            self.claim_interface(intf_num, in_ep, out_ep)?;
        }
        Ok(())
    }

    fn reset_device(&mut self) -> Result<(), DeviceError> {
        info!("Executing hardware bus-level port reset for USB device {}", self.device_id);
        self.release_interface();
        self.device
            .reset()
            .wait()
            .map_err(|e| DeviceError::TransportError(format!("Hardware USB device reset failed: {e}")))?;
        Ok(())
    }
}
