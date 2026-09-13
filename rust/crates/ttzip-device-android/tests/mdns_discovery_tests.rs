// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit and integration tests for Bonjour / mDNS zero-configuration discovery,
//! service record parsing for wireless pairing and direct connect endpoints.

#[path = "../src/transport/mdns.rs"]
pub mod mdns;

pub use ttzip_device_android::error;
pub use ttzip_device_android::models;
pub use ttzip_device_android::traits;
pub use ttzip_device_android::transport;

use mdns::*;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn test_mdns_service_constants_and_classification() {
    assert_eq!(ADB_TLS_PAIRING_SERVICE, "_adb-tls-pairing._tcp.local.");
    assert_eq!(ADB_TLS_CONNECT_SERVICE, "_adb-tls-connect._tcp.local.");

    assert_eq!(
        MdnsServiceKind::from_service_type("_adb-tls-pairing._tcp.local."),
        Some(MdnsServiceKind::Pairing)
    );
    assert_eq!(
        MdnsServiceKind::from_service_type("_adb-tls-connect._tcp.local."),
        Some(MdnsServiceKind::Connect)
    );
    assert_eq!(
        MdnsServiceKind::from_service_type("_http._tcp.local."),
        None
    );

    assert_eq!(
        MdnsServiceKind::Pairing.service_type(),
        "_adb-tls-pairing._tcp.local."
    );
    assert_eq!(
        MdnsServiceKind::Connect.service_type(),
        "_adb-tls-connect._tcp.local."
    );
}

#[test]
fn test_mdns_discovered_device_model() {
    let mut txt = HashMap::new();
    txt.insert("v".to_string(), "1".to_string());
    txt.insert("name".to_string(), "Pixel 8".to_string());

    let device = MdnsDiscoveredDevice {
        service_kind: MdnsServiceKind::Pairing,
        instance_name: "adb-88421a-pair".to_string(),
        full_name: "adb-88421a-pair._adb-tls-pairing._tcp.local.".to_string(),
        ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 120)),
        port: 38455,
        txt_properties: txt.clone(),
    };

    assert_eq!(device.service_kind, MdnsServiceKind::Pairing);
    assert_eq!(device.instance_name, "adb-88421a-pair");
    assert_eq!(device.port, 38455);
    assert_eq!(device.ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 120)));
    assert_eq!(device.txt_properties.get("name").map(|s| s.as_str()), Some("Pixel 8"));
}

#[test]
fn test_mdns_discovery_events() {
    let dev = MdnsDiscoveredDevice {
        service_kind: MdnsServiceKind::Connect,
        instance_name: "adb-connect-dev1".to_string(),
        full_name: "adb-connect-dev1._adb-tls-connect._tcp.local.".to_string(),
        ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 105)),
        port: 5555,
        txt_properties: HashMap::new(),
    };

    let event1 = MdnsDiscoveryEvent::Discovered(dev.clone());
    let event2 = MdnsDiscoveryEvent::Removed {
        full_name: dev.full_name.clone(),
    };

    match event1 {
        MdnsDiscoveryEvent::Discovered(d) => assert_eq!(d.port, 5555),
        _ => panic!("Expected Discovered event"),
    }

    match event2 {
        MdnsDiscoveryEvent::Removed { full_name } => assert!(full_name.contains("dev1")),
        _ => panic!("Expected Removed event"),
    }
}

#[test]
fn test_mdns_browser_instantiation_and_shutdown() {
    let browser = MdnsServiceBrowser::new().expect("Failed to create MdnsServiceBrowser");
    assert!(browser.shutdown().is_ok());
}
