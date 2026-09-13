// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and unit tests for Android 11+ TLS 1.3 SPAKE2 6-digit numeric PIN
//! wireless pairing protocol, packet codecs, state machine, and zeroize erasure.

#[path = "../src/adb/mod.rs"]
pub mod adb;

pub use ttzip_device_android::error;
pub use ttzip_device_android::models;
pub use ttzip_device_android::traits;
pub use ttzip_device_android::transport;

use adb::wireless_pairing::*;
use ttzip_device_android::error::DeviceError;
use zeroize::Zeroize;

#[test]
fn test_spake2_pin_parsing_and_validation() {
    // Valid 6-digit PIN
    let pin = Spake2Pin::parse("123456").expect("Failed to parse valid PIN");
    assert_eq!(pin.as_bytes(), b"123456");

    let zero_pin = Spake2Pin::parse("000000").expect("Failed to parse all-zero PIN");
    assert_eq!(zero_pin.as_bytes(), b"000000");

    // Invalid length
    assert!(Spake2Pin::parse("12345").is_err());
    assert!(Spake2Pin::parse("1234567").is_err());
    assert!(Spake2Pin::parse("").is_err());

    // Non-digit characters
    assert!(Spake2Pin::parse("12345A").is_err());
    assert!(Spake2Pin::parse("12 456").is_err());
    assert!(Spake2Pin::parse("abcdef").is_err());
}

#[test]
fn test_spake2_pin_zeroize_memory() {
    let mut pin = Spake2Pin::parse("987654").expect("Failed to parse PIN");
    assert_eq!(pin.as_bytes(), b"987654");

    pin.zeroize();
    assert_eq!(pin.as_bytes(), &[0u8; 6]);
}

#[test]
fn test_pairing_packet_header_wire_format() {
    let header = PairingPacketHeader::new(PairingPacketType::Spake2Msg, 32);
    assert_eq!(header.version, CURRENT_PAIRING_VERSION);
    assert_eq!(header.packet_type, PairingPacketType::Spake2Msg);
    assert_eq!(header.payload_length, 32);

    let wire_bytes = header.encode();
    assert_eq!(wire_bytes.len(), PAIRING_HEADER_SIZE);
    assert_eq!(wire_bytes[0], 1); // version
    assert_eq!(wire_bytes[1], 0); // Spake2Msg
    assert_eq!(&wire_bytes[2..6], &32u32.to_be_bytes()); // big endian payload size

    let decoded = PairingPacketHeader::decode(&wire_bytes).expect("Failed to decode header");
    assert_eq!(header, decoded);

    // Corrupt version check
    let mut corrupt_version = wire_bytes;
    corrupt_version[0] = 2;
    let res = PairingPacketHeader::decode(&corrupt_version);
    assert!(res.is_err());

    // Payload length bound check
    let mut huge_payload = wire_bytes;
    huge_payload[2..6].copy_from_slice(&(MAX_PAIRING_PAYLOAD_SIZE as u32 + 1).to_be_bytes());
    let res = PairingPacketHeader::decode(&huge_payload);
    assert!(res.is_err());
}

#[test]
fn test_pairing_peer_info_codec() {
    let info = PairingPeerInfo {
        name: "MacBook Pro M3 Max".to_string(),
        certificate: vec![0x30, 0x82, 0x01, 0x0A, 0xDE, 0xAD, 0xBE, 0xEF],
    };

    let encoded = info.encode();
    assert!(encoded.len() > 6);

    let decoded = PairingPeerInfo::decode(&encoded).expect("Failed to decode PeerInfo");
    assert_eq!(info, decoded);
}

#[test]
fn test_spake2_pairing_handshake_success_flow() {
    let pin = Spake2Pin::parse("654321").expect("Valid PIN");
    let tls_keys = [0x7A; TLS_EXPORTED_KEY_MATERIAL_SIZE];

    let mut session = Spake2ClientSession::new(
        pin,
        &tls_keys,
        "TTZip Host Client",
        b"mock_client_certificate".to_vec(),
    );

    assert_eq!(session.current_state(), Spake2HandshakeState::Ready);

    // 1. Generate client SPAKE2 message
    let client_msg = session
        .generate_spake2_message()
        .expect("Failed to generate SPAKE2 msg");
    assert_eq!(client_msg.len(), 32);
    assert_eq!(session.current_state(), Spake2HandshakeState::ExchangingSpakeMsgs);

    // 2. Synthesize correct peer response matching derived session key
    // For test simulation: server computes expected = key ^ 0x55
    let test_pin = Spake2Pin::parse("654321").unwrap();
    let server_creds = PairingCredentials::derive(&test_pin, &tls_keys);
    let mut server_msg = [0u8; 32];
    for (i, b) in server_msg.iter_mut().enumerate() {
        *b = server_creds.session_key()[i] ^ 0x55;
    }

    session
        .process_peer_spake2_message(&server_msg)
        .expect("Failed to process peer message");
    assert_eq!(session.current_state(), Spake2HandshakeState::ExchangingPeerInfo);

    // 3. Encrypt local peer info
    let encrypted_local = session
        .encrypt_local_peer_info()
        .expect("Failed to encrypt local peer info");
    assert!(!encrypted_local.is_empty());

    // 4. Synthesize encrypted remote peer info
    let remote_info = PairingPeerInfo {
        name: "Google Pixel 8 Pro".to_string(),
        certificate: b"remote_device_pubkey_cert".to_vec(),
    };
    let remote_plain = remote_info.encode();
    let mut remote_encrypted = Vec::with_capacity(remote_plain.len());
    let server_key = server_creds.session_key();
    for (i, &b) in remote_plain.iter().enumerate() {
        remote_encrypted.push(b ^ server_key[i % 32]);
    }

    let paired_info = session
        .decrypt_remote_peer_info(&remote_encrypted)
        .expect("Failed to decrypt remote peer info");
    assert_eq!(session.current_state(), Spake2HandshakeState::Paired);
    assert_eq!(paired_info.name, "Google Pixel 8 Pro");
    assert_eq!(session.paired_peer_info().unwrap().name, "Google Pixel 8 Pro");
}

#[test]
fn test_spake2_pairing_handshake_wrong_pin_fails() {
    let pin = Spake2Pin::parse("111111").unwrap();
    let tls_keys = [0x55; TLS_EXPORTED_KEY_MATERIAL_SIZE];

    let mut session = Spake2ClientSession::new(
        pin,
        &tls_keys,
        "TTZip Host",
        b"cert".to_vec(),
    );

    let _ = session.generate_spake2_message().unwrap();

    // Fabricate invalid server response computed with different PIN
    let wrong_pin = Spake2Pin::parse("222222").unwrap();
    let wrong_creds = PairingCredentials::derive(&wrong_pin, &tls_keys);
    let mut wrong_msg = [0u8; 32];
    for (i, b) in wrong_msg.iter_mut().enumerate() {
        *b = wrong_creds.session_key()[i] ^ 0x55;
    }

    let res = session.process_peer_spake2_message(&wrong_msg);
    assert!(res.is_err());
    match res.unwrap_err() {
        DeviceError::PermissionDenied(msg) => {
            assert!(msg.contains("incorrect PIN"));
        }
        other => panic!("Expected PermissionDenied, got: {:?}", other),
    }

    assert_eq!(session.current_state(), Spake2HandshakeState::Failed);
}
