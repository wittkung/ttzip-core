// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration tests for ADB binary protocol framing,
//! 24-byte `amessage` codec, complementary magic verification, and byte checksums.

#[path = "../src/adb/mod.rs"]
pub mod adb;

pub use ttzip_device_android::error;
pub use ttzip_device_android::models;
pub use ttzip_device_android::traits;
pub use ttzip_device_android::transport;

use adb::protocol::*;
use ttzip_device_android::error::DeviceError;

#[test]
fn test_adb_message_encode_decode_roundtrip() {
    let payload = b"host::features=shell_v2,cmd,stat_v2,ls_v2";
    let msg = AdbMessage::new(A_CNXN, A_VERSION_SKIP_CHECKSUM, 1024 * 1024, payload);

    assert_eq!(msg.command, A_CNXN);
    assert_eq!(msg.arg0, A_VERSION_SKIP_CHECKSUM);
    assert_eq!(msg.arg1, 1024 * 1024);
    assert_eq!(msg.data_length, payload.len() as u32);
    assert!(msg.verify_magic());
    assert!(msg.verify_checksum(payload));

    let encoded = msg.encode();
    assert_eq!(encoded.len(), ADB_MESSAGE_HEADER_SIZE);

    let decoded = AdbMessage::decode(&encoded).expect("Failed to decode valid AdbMessage");
    assert_eq!(msg, decoded);
}

#[test]
fn test_adb_magic_complementary_invariant() {
    let commands = [
        A_SYNC, A_CNXN, A_AUTH, A_OPEN, A_OKAY, A_CLSE, A_WRTE, A_STLS,
    ];

    for &cmd in &commands {
        let msg = AdbMessage::new(cmd, 1, 2, b"");
        assert_eq!(msg.magic, cmd ^ 0xFFFF_FFFF);
        assert!(msg.verify_magic());
    }
}

#[test]
fn test_adb_corrupt_magic_rejected() {
    let msg = AdbMessage::new(A_OPEN, 100, 0, b"sync:\0");
    let mut encoded = msg.encode();

    // Corrupt the magic field (bytes 20..24)
    encoded[20] ^= 0x01;

    let res = AdbMessage::decode(&encoded);
    assert!(res.is_err());
    match res.unwrap_err() {
        DeviceError::ProtocolError(err) => {
            assert!(err.contains("Invalid ADB message magic"));
        }
        other => panic!("Expected ProtocolError, got: {:?}", other),
    }
}

#[test]
fn test_adb_checksum_calculation_and_wrapping() {
    assert_eq!(AdbMessage::calculate_crc32(b""), 0);

    let sample = [10u8, 20, 30, 40];
    assert_eq!(AdbMessage::calculate_crc32(&sample), 100);

    // Verify 32-bit wrapping arithmetic without panic on large payload
    let large_payload = vec![0xFFu8; 100_000];
    let expected = (100_000u64 * 0xFFu64) as u32;
    assert_eq!(AdbMessage::calculate_crc32(&large_payload), expected);
}

#[test]
fn test_adb_packet_framing_and_decode_stream() {
    let payload = b"shell:exec ls -la\0".to_vec();
    let packet = AdbPacket::new(A_OPEN, 42, 0, payload.clone());

    let encoded = packet.encode();
    assert_eq!(encoded.len(), ADB_MESSAGE_HEADER_SIZE + payload.len());

    // Incomplete buffer (< 24 bytes) returns None
    let res = AdbPacket::decode_from_slice(&encoded[..10]).expect("Partial slice check failed");
    assert!(res.is_none());

    // Incomplete payload returns None
    let res = AdbPacket::decode_from_slice(&encoded[..ADB_MESSAGE_HEADER_SIZE + 5])
        .expect("Partial payload check failed");
    assert!(res.is_none());

    // Full packet decodes successfully
    let (decoded_packet, consumed) = AdbPacket::decode_from_slice(&encoded)
        .expect("Full decode failed")
        .expect("Expected Some packet");

    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded_packet.message.command, A_OPEN);
    assert_eq!(decoded_packet.message.arg0, 42);
    assert_eq!(decoded_packet.payload, payload);
}

#[test]
fn test_adb_corrupt_payload_checksum_rejected() {
    let payload = b"important_data".to_vec();
    let packet = AdbPacket::new(A_WRTE, 1, 2, payload);
    let mut encoded = packet.encode();

    // Corrupt payload byte
    let last_idx = encoded.len() - 1;
    encoded[last_idx] ^= 0x55;

    let res = AdbPacket::decode_from_slice(&encoded);
    assert!(res.is_err());
    match res.unwrap_err() {
        DeviceError::ProtocolError(err) => {
            assert!(err.contains("ADB payload checksum mismatch"));
        }
        other => panic!("Expected ProtocolError, got: {:?}", other),
    }
}

#[test]
fn test_adb_packet_helpers() {
    let cnxn = helpers::build_cnxn(A_MAX_PAYLOAD, "device::ro.product.model=Pixel 8");
    assert_eq!(cnxn.message.command, A_CNXN);
    assert_eq!(cnxn.message.arg0, A_VERSION_SKIP_CHECKSUM);
    assert_eq!(cnxn.message.arg1, A_MAX_PAYLOAD);

    let open = helpers::build_open(10, "sync:");
    assert_eq!(open.message.command, A_OPEN);
    assert_eq!(open.message.arg0, 10);
    assert!(open.payload.ends_with(&[0]));

    let okay = helpers::build_okay(10, 20);
    assert_eq!(okay.message.command, A_OKAY);
    assert_eq!(okay.message.arg0, 10);
    assert_eq!(okay.message.arg1, 20);
    assert!(okay.payload.is_empty());

    let close = helpers::build_close(10, 20);
    assert_eq!(close.message.command, A_CLSE);
    assert_eq!(close.message.arg0, 10);
    assert_eq!(close.message.arg1, 20);

    let write = helpers::build_write(10, 20, b"chunk".to_vec());
    assert_eq!(write.message.command, A_WRTE);
    assert_eq!(write.payload, b"chunk");
}

#[test]
fn test_adb_command_enum_conversions() {
    assert_eq!(AdbCommand::from(A_SYNC), AdbCommand::Sync);
    assert_eq!(AdbCommand::from(A_CNXN), AdbCommand::Cnxn);
    assert_eq!(AdbCommand::from(A_AUTH), AdbCommand::Auth);
    assert_eq!(AdbCommand::from(A_OPEN), AdbCommand::Open);
    assert_eq!(AdbCommand::from(A_OKAY), AdbCommand::Okay);
    assert_eq!(AdbCommand::from(A_CLSE), AdbCommand::Clse);
    assert_eq!(AdbCommand::from(A_WRTE), AdbCommand::Wrte);
    assert_eq!(AdbCommand::from(A_STLS), AdbCommand::Stls);
    assert_eq!(AdbCommand::from(0xDEAD_BEEF), AdbCommand::Unknown(0xDEAD_BEEF));

    assert_eq!(u32::from(AdbCommand::Sync), A_SYNC);
    assert_eq!(u32::from(AdbCommand::Cnxn), A_CNXN);

    let msg = AdbMessage::new(A_SYNC, 0, 0, b"");
    assert_eq!(msg.command_name(), "SYNC");
}
