// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! USB device hotplug monitoring and event streaming subsystem.
//!
//! Provides deterministic hotplug event generation (device attached / detached)
//! through an event-driven Apple IOKit notification pipeline on macOS, eliminating
//! periodic polling loops and ensuring zero steady-state CPU utilization.

use nusb::MaybeFuture;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

#[cfg(target_os = "macos")]
mod iokit {
    use std::ffi::{c_char, c_void};
    use std::os::raw::c_int;

    pub type KernReturnT = c_int;
    pub type IoObjectT = u32;
    pub type IoIteratorT = IoObjectT;
    pub type IoNotificationPortRef = *mut c_void;
    pub type CfDictionaryRef = *const c_void;
    pub type CfRunLoopRef = *mut c_void;
    pub type CfRunLoopSourceRef = *mut c_void;
    pub type CfStringRef = *const c_void;

    pub const KIO_RETURN_SUCCESS: KernReturnT = 0;
    pub const IO_OBJECT_NULL: IoObjectT = 0;

    pub const K_IO_USB_DEVICE_CLASS_NAME: &[u8] = b"IOUSBDevice\0";
    pub const K_IO_FIRST_MATCH_NOTIFICATION: &[u8] = b"IOServiceFirstMatch\0";
    pub const K_IO_TERMINATED_NOTIFICATION: &[u8] = b"IOServiceTerminate\0";

    pub type IoServiceMatchingCallback =
        unsafe extern "C" fn(refcon: *mut c_void, iterator: IoIteratorT);

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        pub fn IONotificationPortCreate(main_port: u32) -> IoNotificationPortRef;
        pub fn IONotificationPortDestroy(notify: IoNotificationPortRef);
        pub fn IONotificationPortGetRunLoopSource(notify: IoNotificationPortRef) -> CfRunLoopSourceRef;

        pub fn IOServiceMatching(name: *const c_char) -> CfDictionaryRef;

        pub fn IOServiceAddMatchingNotification(
            notify_port: IoNotificationPortRef,
            notification_type: *const c_char,
            matching: CfDictionaryRef,
            callback: IoServiceMatchingCallback,
            ref_con: *mut c_void,
            notification: *mut IoIteratorT,
        ) -> KernReturnT;

        pub fn IOIteratorNext(iterator: IoIteratorT) -> IoObjectT;
        pub fn IOObjectRelease(object: IoObjectT) -> KernReturnT;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFRunLoopGetCurrent() -> CfRunLoopRef;
        pub fn CFRunLoopRun();
        pub fn CFRunLoopStop(rl: CfRunLoopRef);
        pub fn CFRunLoopWakeUp(rl: CfRunLoopRef);
        pub fn CFRunLoopAddSource(
            rl: CfRunLoopRef,
            source: CfRunLoopSourceRef,
            mode: CfStringRef,
        );

        pub static kCFRunLoopDefaultMode: CfStringRef;
    }
}

/// Strongly typed snapshot of a discovered USB physical device for hotplug tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotplugDeviceInfo {
    /// Unique hardware bus identifier (e.g. `usb:020:004`).
    pub device_id: String,
    /// USB Vendor ID.
    pub vendor_id: u16,
    /// USB Product ID.
    pub product_id: u16,
    /// Device serial number if exposed by the descriptor.
    pub serial_number: Option<String>,
    /// Manufacturer string descriptor.
    pub manufacturer: Option<String>,
    /// Product name string descriptor.
    pub product: Option<String>,
    /// Physical USB bus identifier.
    pub bus_id: String,
    /// Physical USB device address on the bus.
    pub device_address: u8,
}

impl HotplugDeviceInfo {
    /// Formats a deterministic device identifier from bus and device address numbers.
    pub fn format_device_id(bus_id: &str, device_address: u8) -> String {
        format!("usb:{bus_id}:{device_address:03}")
    }

    /// Constructs a `HotplugDeviceInfo` from a `nusb::DeviceInfo` handle.
    pub fn from_nusb(info: &nusb::DeviceInfo) -> Self {
        let bus_id = info.bus_id().to_string();
        let device_address = info.device_address();
        let device_id = Self::format_device_id(&bus_id, device_address);

        Self {
            device_id,
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            serial_number: info.serial_number().map(|s| s.to_string()),
            manufacturer: info.manufacturer_string().map(|s| s.to_string()),
            product: info.product_string().map(|s| s.to_string()),
            bus_id,
            device_address,
        }
    }
}

/// Hotplug lifecycle event emitted when USB device state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotplugEvent {
    /// A new USB physical device was connected and recognized.
    DeviceAttached(HotplugDeviceInfo),
    /// A previously connected USB device was disconnected.
    DeviceDetached {
        /// Identifier of the removed device (e.g. `usb:020:004`).
        device_id: String,
    },
}

/// Deterministic state-based device snapshot difference analyzer.
///
/// Compares consecutive device snapshots to identify newly attached and
/// detached devices without memory allocations in steady state.
#[derive(Debug, Default, Clone)]
pub struct PollingHotplugScanner {
    active_devices: HashMap<String, HotplugDeviceInfo>,
}

impl PollingHotplugScanner {
    /// Creates an empty scanner.
    pub fn new() -> Self {
        Self {
            active_devices: HashMap::new(),
        }
    }

    /// Returns currently tracked active device identifiers.
    pub fn active_device_ids(&self) -> HashSet<String> {
        self.active_devices.keys().cloned().collect()
    }

    /// Returns the number of currently tracked devices.
    pub fn active_count(&self) -> usize {
        self.active_devices.len()
    }

    /// Feeds a new snapshot of discovered devices and returns generated hotplug events.
    pub fn scan_diff(&mut self, current_devices: Vec<HotplugDeviceInfo>) -> Vec<HotplugEvent> {
        let mut new_map = HashMap::with_capacity(current_devices.len());
        let mut events = Vec::new();

        for dev in current_devices {
            let id = dev.device_id.clone();
            if !self.active_devices.contains_key(&id) {
                events.push(HotplugEvent::DeviceAttached(dev.clone()));
            }
            new_map.insert(id, dev);
        }

        for id in self.active_devices.keys() {
            if !new_map.contains_key(id) {
                events.push(HotplugEvent::DeviceDetached {
                    device_id: id.clone(),
                });
            }
        }

        self.active_devices = new_map;
        events
    }

    /// Clears internal state and resets tracking.
    pub fn clear(&mut self) {
        self.active_devices.clear();
    }
}

#[cfg(target_os = "macos")]
struct RunLoopHandle {
    run_loop: iokit::CfRunLoopRef,
}

#[cfg(target_os = "macos")]
unsafe impl Send for RunLoopHandle {}
#[cfg(target_os = "macos")]
unsafe impl Sync for RunLoopHandle {}

/// Background asynchronous hotplug monitoring task streaming events over an MPSC channel.
pub struct HotplugWatcher {
    _poll_interval: Duration,
    is_running: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    run_loop: Arc<Mutex<Option<RunLoopHandle>>>,
}

impl Default for HotplugWatcher {
    fn default() -> Self {
        Self::new(Duration::from_millis(1000))
    }
}

impl HotplugWatcher {
    /// Creates a new watcher instance.
    pub fn new(poll_interval: Duration) -> Self {
        Self {
            _poll_interval: poll_interval,
            is_running: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "macos")]
            run_loop: Arc::new(Mutex::new(None)),
        }
    }

    /// Spawns the background monitoring pipeline and returns an event receiver.
    ///
    /// Employs atomic CAS idempotency protection to prevent duplicate RunLoop thread
    /// spawning or resource leaks when invoked repeatedly.
    pub fn start(&self) -> mpsc::Receiver<HotplugEvent> {
        let (tx, rx) = mpsc::channel(64);

        // Atomic CAS: ensure exactly one background pipeline is active at any time
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("HotplugWatcher is already active; returning single-consumer channel");
            return rx;
        }

        let is_running = self.is_running.clone();

        #[cfg(target_os = "macos")]
        {
            let run_loop_holder = self.run_loop.clone();
            self.start_macos_pipeline(tx, is_running, run_loop_holder);
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.start_fallback_pipeline(tx, is_running);
        }

        rx
    }

    /// Spawns the background monitoring pipeline and returns an event receiver.
    pub fn start_monitoring(&self) -> mpsc::Receiver<HotplugEvent> {
        self.start()
    }

    /// Signals the background task to stop.
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        #[cfg(target_os = "macos")]
        {
            let mut guard = self.run_loop.lock();
            if let Some(handle) = guard.take() {
                unsafe {
                    iokit::CFRunLoopStop(handle.run_loop);
                    iokit::CFRunLoopWakeUp(handle.run_loop);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn start_macos_pipeline(
        &self,
        tx: mpsc::Sender<HotplugEvent>,
        is_running: Arc<AtomicBool>,
        run_loop_holder: Arc<Mutex<Option<RunLoopHandle>>>,
    ) {
        let (trigger_tx, trigger_rx) = std::sync::mpsc::sync_channel::<()>(16);
        let trigger_tx_arc = Arc::new(trigger_tx);
        let thread_is_running = is_running.clone();
        let thread_run_loop_holder = run_loop_holder.clone();

        // Spawn dedicated OS thread for IOKit CFRunLoop event notification
        let iokit_thread_res = std::thread::Builder::new()
            .name("ttzip-iokit-hotplug".to_string())
            .spawn(move || {
                run_iokit_notification_loop(
                    trigger_tx_arc,
                    thread_is_running,
                    thread_run_loop_holder,
                );
            });

        if let Err(e) = iokit_thread_res {
            error!("Failed to spawn IOKit hotplug thread: {e}");
            is_running.store(false, Ordering::SeqCst);
            return;
        }

        // Spawn dedicated OS thread for event snapshot diffing and streaming without Tokio runtime dependency
        let diff_is_running = is_running.clone();
        let diff_run_loop_holder = run_loop_holder.clone();

        let diff_thread_res = std::thread::Builder::new()
            .name("ttzip-hotplug-diff".to_string())
            .spawn(move || {
                info!("Starting IOKit event-driven USB hotplug monitoring worker");
                let mut scanner = PollingHotplugScanner::new();

                // Initial snapshot enumeration to discover currently connected devices
                if let Ok(devices_iter) = nusb::list_devices().wait() {
                    let current_devices: Vec<HotplugDeviceInfo> =
                        devices_iter.map(|d| HotplugDeviceInfo::from_nusb(&d)).collect();
                    let events = scanner.scan_diff(current_devices);
                    for event in events {
                        debug!("Emitting initial USB hotplug event: {:?}", event);
                        if tx.blocking_send(event).is_err() {
                            debug!("Hotplug receiver dropped during initial enumeration; stopping");
                            stop_task_internal(&diff_is_running, &diff_run_loop_holder);
                            return;
                        }
                    }
                }

                // Await physical IOKit Mach port notifications without periodic polling
                while diff_is_running.load(Ordering::Relaxed) {
                    match trigger_rx.recv() {
                        Ok(()) => {
                            if !diff_is_running.load(Ordering::Relaxed) {
                                break;
                            }

                            match nusb::list_devices().wait() {
                                Ok(devices_iter) => {
                                    let current_devices: Vec<HotplugDeviceInfo> =
                                        devices_iter.map(|d| HotplugDeviceInfo::from_nusb(&d)).collect();
                                    let events = scanner.scan_diff(current_devices);
                                    for event in events {
                                        debug!("Emitting IOKit USB hotplug event: {:?}", event);
                                        if tx.blocking_send(event).is_err() {
                                            debug!("Hotplug receiver dropped; stopping watcher");
                                            stop_task_internal(&diff_is_running, &diff_run_loop_holder);
                                            return;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to enumerate USB devices on IOKit event: {e}");
                                }
                            }
                        }
                        Err(_) => {
                            // Trigger channel disconnected; IOKit thread exited
                            break;
                        }
                    }
                }

                info!("IOKit USB hotplug monitoring worker terminated");
                stop_task_internal(&diff_is_running, &diff_run_loop_holder);
            });

        if let Err(e) = diff_thread_res {
            error!("Failed to spawn hotplug diff worker thread: {e}");
            stop_task_internal(&is_running, &run_loop_holder);
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn start_fallback_pipeline(
        &self,
        tx: mpsc::Sender<HotplugEvent>,
        is_running: Arc<AtomicBool>,
    ) {
        let _ = std::thread::Builder::new()
            .name("ttzip-hotplug-fallback".to_string())
            .spawn(move || {
                info!("Starting fallback USB hotplug monitoring worker");
                let mut scanner = PollingHotplugScanner::new();

                while is_running.load(Ordering::Relaxed) {
                    match nusb::list_devices().wait() {
                        Ok(devices_iter) => {
                            let current_devices: Vec<HotplugDeviceInfo> =
                                devices_iter.map(|d| HotplugDeviceInfo::from_nusb(&d)).collect();
                            let events = scanner.scan_diff(current_devices);
                            for event in events {
                                if tx.blocking_send(event).is_err() {
                                    is_running.store(false, Ordering::SeqCst);
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to enumerate USB devices during fallback hotplug check: {e}");
                        }
                    }
                    std::thread::sleep(Duration::from_millis(2000));
                }
            });
    }
}

#[cfg(target_os = "macos")]
fn stop_task_internal(
    is_running: &Arc<AtomicBool>,
    run_loop_holder: &Arc<Mutex<Option<RunLoopHandle>>>,
) {
    is_running.store(false, Ordering::SeqCst);
    let mut guard = run_loop_holder.lock();
    if let Some(handle) = guard.take() {
        unsafe {
            iokit::CFRunLoopStop(handle.run_loop);
            iokit::CFRunLoopWakeUp(handle.run_loop);
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn hotplug_notification_callback(
    refcon: *mut std::ffi::c_void,
    iterator: iokit::IoIteratorT,
) {
    // Drain the iterator to release Mach port handles and re-arm future notifications
    loop {
        let service = iokit::IOIteratorNext(iterator);
        if service == iokit::IO_OBJECT_NULL {
            break;
        }
        iokit::IOObjectRelease(service);
    }

    // Notify channel of physical hardware state transition
    if !refcon.is_null() {
        let trigger_tx = &*(refcon as *const std::sync::mpsc::SyncSender<()>);
        let _ = trigger_tx.try_send(());
    }
}

#[cfg(target_os = "macos")]
fn run_iokit_notification_loop(
    trigger_tx: Arc<std::sync::mpsc::SyncSender<()>>,
    is_running: Arc<AtomicBool>,
    run_loop_holder: Arc<Mutex<Option<RunLoopHandle>>>,
) {
    unsafe {
        let notify_port = iokit::IONotificationPortCreate(0);
        if notify_port.is_null() {
            error!("Failed to create IOKit notification port");
            return;
        }

        let run_loop_source = iokit::IONotificationPortGetRunLoopSource(notify_port);
        let current_run_loop = iokit::CFRunLoopGetCurrent();

        iokit::CFRunLoopAddSource(
            current_run_loop,
            run_loop_source,
            iokit::kCFRunLoopDefaultMode,
        );

        {
            let mut guard = run_loop_holder.lock();
            *guard = Some(RunLoopHandle {
                run_loop: current_run_loop,
            });
        }

        let raw_refcon = Arc::into_raw(trigger_tx) as *mut std::ffi::c_void;

        // Register for USB device publish (First Match) notification
        let mut publish_iter: iokit::IoIteratorT = 0;
        let publish_dict = iokit::IOServiceMatching(
            iokit::K_IO_USB_DEVICE_CLASS_NAME.as_ptr() as *const std::ffi::c_char,
        );
        let kr_publish = iokit::IOServiceAddMatchingNotification(
            notify_port,
            iokit::K_IO_FIRST_MATCH_NOTIFICATION.as_ptr() as *const std::ffi::c_char,
            publish_dict,
            hotplug_notification_callback,
            raw_refcon,
            &mut publish_iter,
        );

        if kr_publish != iokit::KIO_RETURN_SUCCESS {
            error!("Failed to register IOKit publish notification: {kr_publish}");
        } else {
            // Drain initial matched devices to arm notification
            while iokit::IOIteratorNext(publish_iter) != iokit::IO_OBJECT_NULL {}
        }

        // Register for USB device terminate notification
        let mut terminate_iter: iokit::IoIteratorT = 0;
        let terminate_dict = iokit::IOServiceMatching(
            iokit::K_IO_USB_DEVICE_CLASS_NAME.as_ptr() as *const std::ffi::c_char,
        );
        let kr_terminate = iokit::IOServiceAddMatchingNotification(
            notify_port,
            iokit::K_IO_TERMINATED_NOTIFICATION.as_ptr() as *const std::ffi::c_char,
            terminate_dict,
            hotplug_notification_callback,
            raw_refcon,
            &mut terminate_iter,
        );

        if kr_terminate != iokit::KIO_RETURN_SUCCESS {
            error!("Failed to register IOKit terminate notification: {kr_terminate}");
        } else {
            // Drain initial terminated devices to arm notification
            while iokit::IOIteratorNext(terminate_iter) != iokit::IO_OBJECT_NULL {}
        }

        debug!("Entering IOKit CFRunLoop event wait");
        iokit::CFRunLoopRun();
        debug!("Exited IOKit CFRunLoop event wait");

        // Cleanup resources
        if publish_iter != 0 {
            iokit::IOObjectRelease(publish_iter);
        }
        if terminate_iter != 0 {
            iokit::IOObjectRelease(terminate_iter);
        }
        iokit::IONotificationPortDestroy(notify_port);

        // Reclaim raw refcon
        drop(Arc::from_raw(raw_refcon as *const std::sync::mpsc::SyncSender<()>));

        {
            let mut guard = run_loop_holder.lock();
            *guard = None;
        }
        is_running.store(false, Ordering::SeqCst);
    }
}

/// Spawns the default event-driven USB hotplug monitoring task and returns an event receiver.
///
/// Under macOS, this utilizes native IOKit kernel notifications with zero steady-state CPU polling.
pub fn start_monitoring() -> mpsc::Receiver<HotplugEvent> {
    HotplugWatcher::default().start()
}

