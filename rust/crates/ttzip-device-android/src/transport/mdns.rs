// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bonjour / mDNS zero-configuration network discovery for wireless Android devices.
//!
//! Utilizes `mdns-sd 0.19.2` to browse and resolve Android 11+ pairing services
//! (`_adb-tls-pairing._tcp.local.`) and direct connect services (`_adb-tls-connect._tcp.local.`).

use crate::error::DeviceError;
use mdns_sd::{ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// mDNS service name for Android 11+ SPAKE2 pairing sessions.
pub const ADB_TLS_PAIRING_SERVICE: &str = "_adb-tls-pairing._tcp.local.";

/// mDNS service name for established Android 11+ wireless ADB debugging sessions.
pub const ADB_TLS_CONNECT_SERVICE: &str = "_adb-tls-connect._tcp.local.";

/// Classification of discovered wireless ADB service endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdnsServiceKind {
    /// Pairing endpoint requiring 6-digit PIN handshake.
    Pairing,
    /// Established connection endpoint ready for ADB transport multiplexing.
    Connect,
}

impl MdnsServiceKind {
    /// Returns the corresponding mDNS service type domain string.
    pub fn service_type(&self) -> &'static str {
        match self {
            Self::Pairing => ADB_TLS_PAIRING_SERVICE,
            Self::Connect => ADB_TLS_CONNECT_SERVICE,
        }
    }

    /// Matches an mDNS full service name string to a known service kind.
    pub fn from_service_type(service_type: &str) -> Option<Self> {
        if service_type.starts_with("_adb-tls-pairing._tcp") {
            Some(Self::Pairing)
        } else if service_type.starts_with("_adb-tls-connect._tcp") {
            Some(Self::Connect)
        } else {
            None
        }
    }
}

/// Represents a resolved Android wireless debugging endpoint on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdnsDiscoveredDevice {
    /// Type of service (`Pairing` vs `Connect`).
    pub service_kind: MdnsServiceKind,
    /// Short instance name (e.g. `adb-988a1b2c-X4Y8`).
    pub instance_name: String,
    /// Fully qualified domain name.
    pub full_name: String,
    /// Target network host IP address (preferred IPv4).
    pub ip: IpAddr,
    /// Target TCP listener port.
    pub port: u16,
    /// Parsed TXT record key-value pairs.
    pub txt_properties: HashMap<String, String>,
}

/// Event notification emitted by the background mDNS discovery watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdnsDiscoveryEvent {
    /// A new device service has been discovered and resolved.
    Discovered(MdnsDiscoveredDevice),
    /// A previously known service was withdrawn or dropped from network.
    Removed { full_name: String },
}

/// Background mDNS discovery daemon controller.
pub struct MdnsServiceBrowser {
    daemon: ServiceDaemon,
    is_running: Arc<AtomicBool>,
}

impl MdnsServiceBrowser {
    /// Initializes a new mDNS service browser instance.
    pub fn new() -> Result<Self, DeviceError> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| DeviceError::TransportError(format!("Failed to initialize mDNS daemon: {}", e)))?;

        Ok(Self {
            daemon,
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Starts asynchronous browsing for the specified service kind, emitting events to a channel.
    pub fn start_discovery(
        &self,
        service_kind: MdnsServiceKind,
    ) -> Result<Receiver<MdnsDiscoveryEvent>, DeviceError> {
        let service_type = service_kind.service_type();
        let receiver = self
            .daemon
            .browse(service_type)
            .map_err(|e| DeviceError::TransportError(format!("Failed to browse mDNS service {}: {}", service_type, e)))?;

        let (tx, rx): (Sender<MdnsDiscoveryEvent>, Receiver<MdnsDiscoveryEvent>) = mpsc::channel(64);
        let is_running = self.is_running.clone();
        is_running.store(true, Ordering::SeqCst);

        // Spawn detached event forwarder thread translating mdns-sd events to tokio channel
        std::thread::Builder::new()
            .name(format!("mdns-browse-{}", service_type))
            .spawn(move || {
                while is_running.load(Ordering::SeqCst) {
                    match receiver.recv() {
                        Ok(event) => match event {
                            ServiceEvent::ServiceResolved(info) => {
                                if let Some(device) = Self::parse_service_info(&info, service_kind) {
                                    if tx.blocking_send(MdnsDiscoveryEvent::Discovered(device)).is_err() {
                                        break;
                                    }
                                }
                            }
                            ServiceEvent::ServiceRemoved(_, full_name) => {
                                if tx.blocking_send(MdnsDiscoveryEvent::Removed { full_name }).is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        },
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| DeviceError::TransportError(format!("Failed to spawn mDNS thread: {}", e)))?;

        Ok(rx)
    }

    /// Parses raw `ServiceInfo` into normalized `MdnsDiscoveredDevice`.
    pub fn parse_service_info(info: &ResolvedService, kind: MdnsServiceKind) -> Option<MdnsDiscoveredDevice> {
        let full_name = info.get_fullname().to_string();
        let instance_name = full_name
            .split('.')
            .next()
            .unwrap_or(&full_name)
            .to_string();

        let addresses = info.get_addresses();
        // Prefer IPv4 for local network ADB stability
        let ip = addresses
            .iter()
            .map(ScopedIp::to_ip_addr)
            .find(|addr| addr.is_ipv4())
            .or_else(|| addresses.iter().map(ScopedIp::to_ip_addr).next())?;

        let port = info.get_port();
        let mut txt_properties = HashMap::new();
        for prop in info.get_properties().iter() {
            txt_properties.insert(prop.key().to_string(), prop.val_str().to_string());
        }

        Some(MdnsDiscoveredDevice {
            service_kind: kind,
            instance_name,
            full_name,
            ip,
            port,
            txt_properties,
        })
    }

    /// Stops browsing for the specified service.
    pub fn stop_discovery(&self, service_kind: MdnsServiceKind) -> Result<(), DeviceError> {
        self.daemon
            .stop_browse(service_kind.service_type())
            .map_err(|e| DeviceError::TransportError(format!("Failed to stop mDNS browsing: {}", e)))?;
        Ok(())
    }

    /// Shuts down the mDNS background engine.
    pub fn shutdown(&self) -> Result<(), DeviceError> {
        self.is_running.store(false, Ordering::SeqCst);
        let _ = self.daemon.shutdown();
        Ok(())
    }
}

impl Drop for MdnsServiceBrowser {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
