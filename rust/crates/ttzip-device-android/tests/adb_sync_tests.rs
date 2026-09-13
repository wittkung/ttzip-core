// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and unit tests for ADB SYNC stream protocol, directory listing (`DENT`),
//! MediaScanner index refreshes, and `AdbDeviceDriver` Scoped Storage penetration.

#[path = "../src/adb/mod.rs"]
pub mod adb;

pub use ttzip_device_android::error;
pub use ttzip_device_android::models;
pub use ttzip_device_android::traits;
pub use ttzip_device_android::transport;

use adb::driver::AdbDeviceDriver;
use adb::sync_service::{
    decoder::*, encoder::*, media_scanner, S_IFDIR, S_IFREG, SYNC_DATA, SYNC_DENT, SYNC_DONE,
    SYNC_LIST, SYNC_QUIT, SYNC_RECV, SYNC_SEND, SYNC_STAT,
};
use bytes::Bytes;
use ttzip_device_android::traits::DeviceStorageDriver;

#[test]
fn test_sync_encoder_request_builders() {
    let list_req = build_list_request("/sdcard/Download");
    assert_eq!(&list_req[0..4], &SYNC_LIST);
    let path_len = u32::from_le_bytes([list_req[4], list_req[5], list_req[6], list_req[7]]) as usize;
    assert_eq!(path_len, "/sdcard/Download".len());
    assert_eq!(&list_req[8..], b"/sdcard/Download");

    let stat_req = build_stat_request("/sdcard/DCIM/photo.jpg");
    assert_eq!(&stat_req[0..4], &SYNC_STAT);

    let recv_req = build_recv_request("/sdcard/archive.zip");
    assert_eq!(&recv_req[0..4], &SYNC_RECV);

    let send_hdr = build_send_header("/sdcard/new.txt", 0o644);
    assert_eq!(&send_hdr[0..4], &SYNC_SEND);
    assert!(String::from_utf8_lossy(&send_hdr[8..]).contains("/sdcard/new.txt,420"));

    let data_chunk = build_data_chunk(b"hello world payload");
    assert_eq!(&data_chunk[0..4], &SYNC_DATA);
    let chunk_len = u32::from_le_bytes([data_chunk[4], data_chunk[5], data_chunk[6], data_chunk[7]]) as usize;
    assert_eq!(chunk_len, b"hello world payload".len());
    assert_eq!(&data_chunk[8..], b"hello world payload");

    let done_req = build_done_request(1700000000);
    assert_eq!(&done_req[0..4], &SYNC_DONE);
    assert_eq!(
        u32::from_le_bytes([done_req[4], done_req[5], done_req[6], done_req[7]]),
        1700000000
    );

    let quit_req = build_quit_request();
    assert_eq!(&quit_req[0..4], &SYNC_QUIT);
}

#[test]
fn test_sync_stat_response_decoding() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&SYNC_STAT);
    buf.extend_from_slice(&(S_IFREG | 0o644).to_le_bytes()); // mode
    buf.extend_from_slice(&2048000u32.to_le_bytes()); // size
    buf.extend_from_slice(&1700000000u32.to_le_bytes()); // mtime

    let stat = decode_stat_response(&buf).expect("Failed to decode valid STAT response");
    assert!(stat.exists());
    assert!(stat.is_file());
    assert!(!stat.is_dir());
    assert_eq!(stat.size, 2048000);
    assert_eq!(stat.mtime, 1700000000);

    // Non-existent target
    let mut zero_buf = Vec::new();
    zero_buf.extend_from_slice(&SYNC_STAT);
    zero_buf.extend_from_slice(&0u32.to_le_bytes());
    zero_buf.extend_from_slice(&0u32.to_le_bytes());
    zero_buf.extend_from_slice(&0u32.to_le_bytes());

    let missing = decode_stat_response(&zero_buf).expect("Failed to decode 0 mode STAT");
    assert!(!missing.exists());
}

#[test]
fn test_sync_dent_stream_parsing() {
    let mut stream = Vec::new();

    // Entry 1: "." (should be filtered)
    stream.extend_from_slice(&SYNC_DENT);
    stream.extend_from_slice(&(S_IFDIR | 0o755).to_le_bytes());
    stream.extend_from_slice(&0u32.to_le_bytes());
    stream.extend_from_slice(&1690000000u32.to_le_bytes());
    stream.extend_from_slice(&1u32.to_le_bytes());
    stream.extend_from_slice(b".");

    // Entry 2: "file1.txt" (regular file, 500 bytes)
    stream.extend_from_slice(&SYNC_DENT);
    stream.extend_from_slice(&(S_IFREG | 0o644).to_le_bytes());
    stream.extend_from_slice(&500u32.to_le_bytes());
    stream.extend_from_slice(&1700000001u32.to_le_bytes());
    stream.extend_from_slice(&9u32.to_le_bytes());
    stream.extend_from_slice(b"file1.txt");

    // Entry 3: "Documents" (directory)
    stream.extend_from_slice(&SYNC_DENT);
    stream.extend_from_slice(&(S_IFDIR | 0o755).to_le_bytes());
    stream.extend_from_slice(&0u32.to_le_bytes());
    stream.extend_from_slice(&1700000002u32.to_le_bytes());
    stream.extend_from_slice(&9u32.to_le_bytes());
    stream.extend_from_slice(b"Documents");

    // Terminator: DONE
    stream.extend_from_slice(&SYNC_DONE);
    stream.extend_from_slice(&0u32.to_le_bytes());
    stream.extend_from_slice(&0u32.to_le_bytes());
    stream.extend_from_slice(&0u32.to_le_bytes());

    let entries = parse_dent_stream(&stream).expect("Failed to parse DENT stream");
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].name, "file1.txt");
    assert!(entries[0].is_file());
    assert!(!entries[0].is_dir());
    assert_eq!(entries[0].size, 500);
    assert_eq!(entries[0].permissions(), 0o644);

    assert_eq!(entries[1].name, "Documents");
    assert!(entries[1].is_dir());
    assert!(!entries[1].is_file());
}

#[test]
fn test_media_scanner_command_formatting() {
    let cmd = media_scanner::format_media_scan_command("/storage/emulated/0/DCIM/Camera/IMG_001.jpg");
    assert_eq!(
        cmd,
        "content call --uri content://media/ --method scan_file --arg \"/storage/emulated/0/DCIM/Camera/IMG_001.jpg\""
    );

    let shell_req = media_scanner::build_media_scan_shell_request("/sdcard/Download/test.zip");
    assert!(shell_req.starts_with("shell:exec "));
    assert!(shell_req.contains("scan_file"));
}

#[tokio::test]
async fn test_adb_driver_scoped_storage_penetration() {
    let driver = AdbDeviceDriver::new("emulator-5554", false);
    assert_eq!(driver.serial(), "emulator-5554");
    assert!(!driver.is_wireless());

    // List root directory
    let root_nodes = driver.list_directory("/").await.expect("Failed to list root");
    assert!(!root_nodes.is_empty());
    let android_dir = root_nodes.iter().find(|n| n.name == "Android");
    assert!(android_dir.is_some());
    assert!(android_dir.unwrap().is_dir());

    // Traverse into protected /Android/data directly (UID 2000 penetration)
    let data_nodes = driver
        .list_directory("/storage/emulated/0/Android/data")
        .await
        .expect("Failed to list /Android/data");
    assert!(!data_nodes.is_empty());

    let app_pkg = data_nodes
        .iter()
        .find(|n| n.name == "com.android.providers.media");
    assert!(app_pkg.is_some());
    // In ADB mode, Scoped Storage directories are accessible (not restricted)
    assert!(!app_pkg.unwrap().is_restricted);
}

#[tokio::test]
async fn test_adb_driver_write_and_media_scanner_trigger() {
    let driver = AdbDeviceDriver::new("192.168.1.50:5555", true);
    assert!(driver.is_wireless());

    let target_path = "/storage/emulated/0/DCIM/photo.jpg";
    let test_data = Bytes::from_static(b"\xFF\xD8\xFF\xE0MockJpegPayload");

    // Write file
    driver
        .send_object(target_path, test_data.clone())
        .await
        .expect("Failed to send object");

    // Read back partial object bytes
    let read_slice = driver
        .get_partial_object(target_path, 0, 4)
        .await
        .expect("Failed to read partial object");
    assert_eq!(&read_slice[..], b"\xFF\xD8\xFF\xE0");

    // Verify MediaScanner command was synchronously dispatched
    let history = driver.media_scan_history().await;
    assert_eq!(history.len(), 1);
    assert!(history[0].contains("content call --uri content://media/ --method scan_file"));
    assert!(history[0].contains(target_path));

    // Delete object
    driver.delete_object(target_path).await.expect("Failed to delete object");

    // Verify deleted
    let read_err = driver.get_partial_object(target_path, 0, 4).await;
    assert!(read_err.is_err());

    // Disconnect
    driver.disconnect().await.expect("Failed to disconnect");
    let after_disc = driver.list_directory("/").await;
    assert!(after_disc.is_err());
}
