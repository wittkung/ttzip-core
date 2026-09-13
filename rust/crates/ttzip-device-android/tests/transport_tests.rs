// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and unit tests for macOS IOKit USB transport,
//! pipe stall recovery state machine, and hotplug monitoring.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use ttzip_device_android::error::DeviceError;
use ttzip_device_android::transport::hotplug::start_monitoring;
use ttzip_device_android::transport::{
    HotplugDeviceInfo, HotplugEvent, HotplugWatcher, PipeRecoveryStateMachine,
    PollingHotplugScanner, RecoveryStep, SeizeDiagnosticReport, TransportRecoveryTarget,
    KIO_RETURN_EXCLUSIVE_ACCESS, KIO_RETURN_EXCLUSIVE_ACCESS_RAW, USB_CLASS_STILL_IMAGE,
    USB_CLASS_VENDOR_SPECIFIC, USB_PROTOCOL_ADB, USB_PROTOCOL_MTP, USB_SUBCLASS_ADB,
    USB_SUBCLASS_STILL_IMAGE_PTP,
};

/// Mock recovery target recording operations and simulating hardware behavior.
#[derive(Default)]
struct MockRecoveryTarget {
    abort_calls: Arc<AtomicUsize>,
    clear_stall_calls: Arc<AtomicUsize>,
    protocol_reset_calls: Arc<AtomicUsize>,
    reset_device_calls: Arc<AtomicUsize>,
    fail_abort: bool,
    fail_clear_stall: bool,
    fail_protocol_reset: bool,
    fail_reset_device: bool,
}

impl TransportRecoveryTarget for MockRecoveryTarget {
    fn abort_pipe(&mut self, _ep_addr: u8) -> Result<(), DeviceError> {
        self.abort_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_abort {
            Err(DeviceError::TransportError("AbortPipe mock error".to_string()))
        } else {
            Ok(())
        }
    }

    fn clear_pipe_stall(&mut self, _ep_addr: u8) -> Result<(), DeviceError> {
        self.clear_stall_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_clear_stall {
            Err(DeviceError::TransportError("ClearPipeStall mock error".to_string()))
        } else {
            Ok(())
        }
    }

    fn protocol_reset(&mut self) -> Result<(), DeviceError> {
        self.protocol_reset_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_protocol_reset {
            Err(DeviceError::ProtocolError("ProtocolReset mock error".to_string()))
        } else {
            Ok(())
        }
    }

    fn reset_device(&mut self) -> Result<(), DeviceError> {
        self.reset_device_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_reset_device {
            Err(DeviceError::TransportError("ResetDevice mock error".to_string()))
        } else {
            Ok(())
        }
    }
}

#[test]
fn test_recovery_state_machine_manual_escalation() {
    let mut sm = PipeRecoveryStateMachine::new(1);
    assert_eq!(sm.current_step(), RecoveryStep::Idle);
    assert!(sm.stalled_endpoint().is_none());

    sm.start(0x81);
    assert_eq!(sm.current_step(), RecoveryStep::AbortPipe);
    assert_eq!(sm.stalled_endpoint(), Some(0x81));

    // Advance through all 4 steps to failure
    assert_eq!(sm.advance_step(), RecoveryStep::ClearPipeStallBothEnds);
    assert_eq!(sm.advance_step(), RecoveryStep::ProtocolReset);
    assert_eq!(sm.advance_step(), RecoveryStep::ResetDevice);
    assert_eq!(sm.advance_step(), RecoveryStep::Failed);

    let history = sm.history();
    assert_eq!(history.len(), 5);
    assert_eq!(history[0], RecoveryStep::AbortPipe);
    assert_eq!(history[1], RecoveryStep::ClearPipeStallBothEnds);
    assert_eq!(history[2], RecoveryStep::ProtocolReset);
    assert_eq!(history[3], RecoveryStep::ResetDevice);
    assert_eq!(history[4], RecoveryStep::Failed);

    let diags = sm.diagnostics();
    assert!(!diags.is_empty());
}

#[test]
fn test_recovery_state_machine_with_retries() {
    let mut sm = PipeRecoveryStateMachine::new(2);
    sm.start(0x02);

    // Attempt 1 of AbortPipe: retries step
    assert_eq!(sm.advance_step(), RecoveryStep::AbortPipe);
    // Attempt 2 of AbortPipe: escalates
    assert_eq!(sm.advance_step(), RecoveryStep::ClearPipeStallBothEnds);
}

#[test]
fn test_recovery_execution_succeeds_at_step_2() {
    let mut sm = PipeRecoveryStateMachine::new(1);
    let mut mock = MockRecoveryTarget {
        fail_abort: true,
        fail_clear_stall: false,
        ..Default::default()
    };

    let res = sm.run_full_recovery(&mut mock, 0x82);
    assert!(res.is_ok());
    assert_eq!(sm.current_step(), RecoveryStep::Recovered);
    assert_eq!(mock.abort_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.clear_stall_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.protocol_reset_calls.load(Ordering::SeqCst), 0);
    assert_eq!(mock.reset_device_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn test_recovery_execution_exhausts_to_failure() {
    let mut sm = PipeRecoveryStateMachine::new(1);
    let mut mock = MockRecoveryTarget {
        fail_abort: true,
        fail_clear_stall: true,
        fail_protocol_reset: true,
        fail_reset_device: true,
        ..Default::default()
    };

    let res = sm.run_full_recovery(&mut mock, 0x81);
    assert!(res.is_err());
    match res.unwrap_err() {
        DeviceError::PipeStall(ep) => assert_eq!(ep, 0x81),
        other => panic!("Expected PipeStall error, got: {:?}", other),
    }

    assert_eq!(sm.current_step(), RecoveryStep::Failed);
    assert_eq!(mock.abort_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.clear_stall_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.protocol_reset_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.reset_device_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_recovery_state_machine_reset() {
    let mut sm = PipeRecoveryStateMachine::new(1);
    sm.start(0x81);
    sm.mark_recovered();
    assert_eq!(sm.current_step(), RecoveryStep::Recovered);

    sm.reset();
    assert_eq!(sm.current_step(), RecoveryStep::Idle);
    assert!(sm.stalled_endpoint().is_none());
    assert!(sm.history().is_empty());
    assert!(sm.diagnostics().is_empty());
}

#[test]
fn test_hotplug_scanner_diff_lifecycle() {
    let mut scanner = PollingHotplugScanner::new();
    assert_eq!(scanner.active_count(), 0);

    let dev1 = HotplugDeviceInfo {
        device_id: "usb:020:001".to_string(),
        vendor_id: 0x18D1,
        product_id: 0x4EE1,
        serial_number: Some("PIXEL8001".to_string()),
        manufacturer: Some("Google".to_string()),
        product: Some("Pixel 8".to_string()),
        bus_id: "020".to_string(),
        device_address: 1,
    };

    let dev2 = HotplugDeviceInfo {
        device_id: "usb:020:002".to_string(),
        vendor_id: 0x04E8,
        product_id: 0x6860,
        serial_number: Some("SAMS24001".to_string()),
        manufacturer: Some("Samsung".to_string()),
        product: Some("Galaxy S24".to_string()),
        bus_id: "020".to_string(),
        device_address: 2,
    };

    // Initial snapshot with 2 devices
    let events = scanner.scan_diff(vec![dev1.clone(), dev2.clone()]);
    assert_eq!(events.len(), 2);
    assert!(events.contains(&HotplugEvent::DeviceAttached(dev1.clone())));
    assert!(events.contains(&HotplugEvent::DeviceAttached(dev2.clone())));
    assert_eq!(scanner.active_count(), 2);

    // Identical snapshot produces 0 events
    let events = scanner.scan_diff(vec![dev1.clone(), dev2.clone()]);
    assert!(events.is_empty());
    assert_eq!(scanner.active_count(), 2);

    // Detach dev1, attach dev3
    let dev3 = HotplugDeviceInfo {
        device_id: "usb:020:003".to_string(),
        vendor_id: 0x2717,
        product_id: 0xFF40,
        serial_number: Some("XIAOMI14001".to_string()),
        manufacturer: Some("Xiaomi".to_string()),
        product: Some("Xiaomi 14".to_string()),
        bus_id: "020".to_string(),
        device_address: 3,
    };

    let events = scanner.scan_diff(vec![dev2.clone(), dev3.clone()]);
    assert_eq!(events.len(), 2);
    assert!(events.contains(&HotplugEvent::DeviceDetached {
        device_id: "usb:020:001".to_string()
    }));
    assert!(events.contains(&HotplugEvent::DeviceAttached(dev3)));
    assert_eq!(scanner.active_count(), 2);

    // Empty snapshot detaches all remaining devices
    let events = scanner.scan_diff(vec![]);
    assert_eq!(events.len(), 2);
    assert_eq!(scanner.active_count(), 0);
}

#[test]
fn test_iokit_seize_constants_and_diagnostics() {
    assert_eq!(KIO_RETURN_EXCLUSIVE_ACCESS_RAW, 0xE000_02C5);
    assert_eq!(KIO_RETURN_EXCLUSIVE_ACCESS, -536870203);

    let report = SeizeDiagnosticReport::new_exclusive_access_report();
    assert_eq!(report.kernel_code, KIO_RETURN_EXCLUSIVE_ACCESS);
    assert!(report.explanation.contains("PTPCamera"));
    assert!(report.permanent_fix_command.contains("disableHotPlug -bool YES"));
    assert_eq!(report.temporary_kill_command, "pkill -9 PTPCamera");
}

#[test]
fn test_usb_descriptor_class_constants() {
    assert_eq!(USB_CLASS_STILL_IMAGE, 0x06);
    assert_eq!(USB_SUBCLASS_STILL_IMAGE_PTP, 0x01);
    assert_eq!(USB_PROTOCOL_MTP, 0x01);

    assert_eq!(USB_CLASS_VENDOR_SPECIFIC, 0xFF);
    assert_eq!(USB_SUBCLASS_ADB, 0x42);
    assert_eq!(USB_PROTOCOL_ADB, 0x01);
}

#[test]
fn test_device_error_formatting() {
    let seize_err = DeviceError::InterfaceSeizeFailed(KIO_RETURN_EXCLUSIVE_ACCESS);
    assert_eq!(
        format!("{seize_err}"),
        format!("Failed to seize USB interface, kernel status: {KIO_RETURN_EXCLUSIVE_ACCESS}")
    );

    let stall_err = DeviceError::PipeStall(0x81);
    assert_eq!(format!("{stall_err}"), "USB pipe stall on endpoint 0x81");

    let timeout_err = DeviceError::ProtocolTimeout("Bulk read 5000ms".to_string());
    assert_eq!(format!("{timeout_err}"), "Protocol timeout: Bulk read 5000ms");

    let disconnect_err = DeviceError::DeviceDisconnected;
    assert_eq!(format!("{disconnect_err}"), "Device disconnected");
}

#[test]
fn test_hotplug_format_device_id() {
    assert_eq!(
        HotplugDeviceInfo::format_device_id("020", 4),
        "usb:020:004"
    );
    assert_eq!(
        HotplugDeviceInfo::format_device_id("001", 12),
        "usb:001:012"
    );
}

#[tokio::test]
async fn test_hotplug_watcher_lifecycle_start_stop() {
    let watcher = HotplugWatcher::new(std::time::Duration::from_millis(500));
    let mut rx = watcher.start();

    // Consume any initial attachment events without hanging
    let timeout_res = tokio::time::timeout(std::time::Duration::from_millis(150), rx.recv()).await;
    if let Ok(Some(event)) = timeout_res {
        match event {
            HotplugEvent::DeviceAttached(dev) => {
                assert!(!dev.device_id.is_empty());
            }
            HotplugEvent::DeviceDetached { device_id } => {
                assert!(!device_id.is_empty());
            }
        }
    }

    watcher.stop();
}

#[tokio::test]
async fn test_hotplug_start_monitoring_stream_consumption() {
    let mut rx = start_monitoring();

    // The channel is immediately open and yields initial discovered devices or waits
    let timeout_res = tokio::time::timeout(std::time::Duration::from_millis(150), rx.recv()).await;
    if let Ok(Some(HotplugEvent::DeviceAttached(dev))) = timeout_res {
        assert!(!dev.device_id.is_empty());
        assert!(!dev.bus_id.is_empty());
    }

    // Drop receiver; background monitoring task should cleanly stop
    drop(rx);
}

#[tokio::test]
async fn test_hotplug_watcher_default_instance() {
    let watcher = HotplugWatcher::default();
    let mut rx = watcher.start_monitoring();
    let timeout_res = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await;
    let _ = timeout_res;
    watcher.stop();
}

