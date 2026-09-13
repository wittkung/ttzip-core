// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for MTP 1.1 binary container codec, string encoding,
//! hierarchical object traversal, and Android 11+ Scoped Storage isolation.

use std::collections::VecDeque;
use std::time::Duration;
use bytes::Bytes;
use ttzip_device_android::error::DeviceError;
use ttzip_device_android::models::VfsEntryType;
use ttzip_device_android::traits::DeviceStorageDriver;

// Direct path module inclusion to ensure isolated test compile passes without lib.rs edit
pub use ttzip_device_android::{error, models, traits, transport};
#[path = "../src/mtp/mod.rs"]
pub mod mtp;

use mtp::protocol::*;
use mtp::{MtpDeviceDriver, MtpInOut};

/// Mock bidirectional MTP transport simulating device bulk endpoints.
struct MockMtpTransport {
    incoming_queue: VecDeque<Vec<u8>>,
    written_log: Vec<MtpContainer>,
}

impl MockMtpTransport {
    fn new() -> Self {
        Self {
            incoming_queue: VecDeque::new(),
            written_log: Vec::new(),
        }
    }

    fn queue_data_and_ok(&mut self, opcode: u16, transaction_id: u32, payload: Vec<u8>) {
        let data = MtpContainer::new_data(opcode, transaction_id, payload);
        let resp = MtpContainer::new_response(RESP_OK, transaction_id, &[]);
        self.incoming_queue.push_back(data.encode());
        self.incoming_queue.push_back(resp.encode());
    }
}

impl MtpInOut for MockMtpTransport {
    fn read(&mut self, length: usize, _timeout: Duration) -> Result<Vec<u8>, DeviceError> {
        if let Some(front) = self.incoming_queue.front_mut() {
            if front.len() <= length {
                Ok(self.incoming_queue.pop_front().unwrap())
            } else {
                let chunk = front[..length].to_vec();
                *front = front[length..].to_vec();
                Ok(chunk)
            }
        } else {
            Err(DeviceError::ProtocolTimeout("Mock incoming buffer empty".to_string()))
        }
    }

    fn write(&mut self, data: &[u8], _timeout: Duration) -> Result<usize, DeviceError> {
        if let Ok(c) = MtpContainer::decode(data) {
            self.written_log.push(c);
        }
        Ok(data.len())
    }
}

#[test]
fn test_mtp_container_codec_roundtrip() {
    let cmd = MtpContainer::new_command(OP_OPEN_SESSION, 101, &[1]);
    let encoded = cmd.encode();
    assert_eq!(encoded.len(), 16); // 12 header + 4 param

    let decoded = MtpContainer::decode(&encoded).expect("Failed to decode command");
    assert_eq!(decoded.length, 16);
    assert_eq!(decoded.container_type, CONTAINER_TYPE_COMMAND);
    assert_eq!(decoded.code, OP_OPEN_SESSION);
    assert_eq!(decoded.transaction_id, 101);
    assert_eq!(decoded.params().expect("Params failed"), vec![1]);
    assert_eq!(decoded.param(0).expect("Param(0) failed"), 1);

    // Data container roundtrip
    let payload = b"Hello MTP 1.1 Payload Stream".to_vec();
    let data_c = MtpContainer::new_data(OP_SEND_OBJECT, 102, payload.clone());
    let encoded_data = data_c.encode();
    let decoded_data = MtpContainer::decode(&encoded_data).expect("Failed to decode data");
    assert_eq!(decoded_data.container_type, CONTAINER_TYPE_DATA);
    assert_eq!(decoded_data.payload, payload);

    // Response container roundtrip
    let resp = MtpContainer::new_response(RESP_OK, 103, &[0x0001, 0x0002]);
    let encoded_resp = resp.encode();
    let decoded_resp = MtpContainer::decode(&encoded_resp).expect("Failed to decode resp");
    assert_eq!(decoded_resp.container_type, CONTAINER_TYPE_RESPONSE);
    assert_eq!(decoded_resp.code, RESP_OK);
    assert_eq!(decoded_resp.params().expect("Resp params failed"), vec![0x0001, 0x0002]);
}

#[test]
fn test_mtp_container_invalid_length_guards() {
    // Truncated header
    let short_buf = vec![0x04, 0x00, 0x00, 0x00];
    assert!(MtpContainer::decode(&short_buf).is_err());

    // Header declaring fewer than 12 bytes
    let invalid_len_buf = vec![
        0x08, 0x00, 0x00, 0x00, // length = 8
        0x01, 0x00, 0x01, 0x10, 0x01, 0x00, 0x00, 0x00,
    ];
    assert!(MtpContainer::decode(&invalid_len_buf).is_err());
}

#[test]
fn test_mtp_string_codec_various_inputs() {
    // Empty string
    let empty_enc = encode_mtp_string("");
    assert_eq!(empty_enc, vec![0x00]);
    let mut off = 0;
    assert_eq!(decode_mtp_string(&empty_enc, &mut off).unwrap(), "");

    // Standard ASCII
    let ascii_str = "Android_Backup.tar.gz";
    let ascii_enc = encode_mtp_string(ascii_str);
    let mut off = 0;
    assert_eq!(decode_mtp_string(&ascii_enc, &mut off).unwrap(), ascii_str);

    // Unicode with Chinese and Emoji
    let unicode_str = "归档测试_2026_📁.zip";
    let unicode_enc = encode_mtp_string(unicode_str);
    let mut off = 0;
    assert_eq!(decode_mtp_string(&unicode_enc, &mut off).unwrap(), unicode_str);
}

#[test]
fn test_mtp_timestamp_bidirectional() {
    let epoch = 1773400000; // 2026-03-13...
    let formatted = encode_mtp_timestamp(epoch);
    assert!(formatted.contains("2026"));
    let recovered = decode_mtp_timestamp(&formatted);
    // Allow seconds deviation if time zone difference
    assert_eq!(epoch, recovered);
}

#[test]
fn test_mtp_object_info_codec_roundtrip() {
    let info = MtpObjectInfo {
        storage_id: 0x00010001,
        object_format: FORMAT_ASSOCIATION,
        protection_status: 0,
        object_compressed_size: 0,
        thumb_format: 0,
        thumb_compressed_size: 0,
        thumb_pix_width: 0,
        thumb_pix_height: 0,
        image_pix_width: 0,
        image_pix_height: 0,
        image_bit_depth: 0,
        parent_object: MTP_PARENT_ROOT,
        association_type: 1,
        association_desc: 0,
        sequence_number: 0,
        filename: "Download".to_string(),
        date_created: "20260313T073000".to_string(),
        date_modified: "20260313T073000".to_string(),
        keywords: "user".to_string(),
    };

    assert!(info.is_dir());

    let encoded = info.encode();
    let decoded = MtpObjectInfo::decode(&encoded).expect("Failed to decode ObjectInfo");
    assert_eq!(decoded.storage_id, 0x00010001);
    assert_eq!(decoded.object_format, FORMAT_ASSOCIATION);
    assert_eq!(decoded.filename, "Download");
    assert_eq!(decoded.keywords, "user");
    assert!(decoded.is_dir());
}

#[tokio::test]
async fn test_mtp_traversal_and_scoped_storage_isolation() {
    let mut mock = MockMtpTransport::new();

    // 1. Setup Root traversal response
    // GetObjectHandles(storage=0x10001, parent=0xFFFFFFFF) -> [10 (Download), 20 (Android)]
    mock.queue_data_and_ok(OP_GET_OBJECT_HANDLES, 100, encode_u32_array(&[10, 20]));

    // GetObjectInfo(10) -> Download (Folder)
    let download_info = MtpObjectInfo {
        storage_id: 0x10001,
        object_format: FORMAT_ASSOCIATION,
        protection_status: 0,
        object_compressed_size: 0,
        thumb_format: 0,
        thumb_compressed_size: 0,
        thumb_pix_width: 0,
        thumb_pix_height: 0,
        image_pix_width: 0,
        image_pix_height: 0,
        image_bit_depth: 0,
        parent_object: MTP_PARENT_ROOT,
        association_type: 1,
        association_desc: 0,
        sequence_number: 0,
        filename: "Download".to_string(),
        date_created: "20260101T000000".to_string(),
        date_modified: "20260101T000000".to_string(),
        keywords: String::new(),
    };
    mock.queue_data_and_ok(OP_GET_OBJECT_INFO, 101, download_info.encode());

    // GetObjectInfo(20) -> Android (Folder)
    let android_info = MtpObjectInfo {
        storage_id: 0x10001,
        object_format: FORMAT_ASSOCIATION,
        protection_status: 0,
        object_compressed_size: 0,
        thumb_format: 0,
        thumb_compressed_size: 0,
        thumb_pix_width: 0,
        thumb_pix_height: 0,
        image_pix_width: 0,
        image_pix_height: 0,
        image_bit_depth: 0,
        parent_object: MTP_PARENT_ROOT,
        association_type: 1,
        association_desc: 0,
        sequence_number: 0,
        filename: "Android".to_string(),
        date_created: "20260101T000000".to_string(),
        date_modified: "20260101T000000".to_string(),
        keywords: String::new(),
    };
    mock.queue_data_and_ok(OP_GET_OBJECT_INFO, 102, android_info.encode());

    let driver = MtpDeviceDriver::new_with_io(Box::new(mock), 0x10001);

    // List root directory
    let root_items = driver.list_directory("/").await.expect("Failed to list root");
    assert_eq!(root_items.len(), 2);
    assert_eq!(root_items[0].path, "/Download");
    assert_eq!(root_items[0].entry_type, VfsEntryType::Directory);
    assert!(!root_items[0].is_restricted);

    assert_eq!(root_items[1].path, "/Android");
    assert_eq!(root_items[1].entry_type, VfsEntryType::Directory);
    assert!(!root_items[1].is_restricted);

    // 2. Setup /Android traversal response
    // GetObjectHandles(storage=0x10001, parent=20) -> [21 (data), 22 (obb), 23 (media)]
    {
        let mut io_guard = driver.io_for_test().await;
        // Inject child responses into mock transport
        let mut sub_mock = MockMtpTransport::new();
        sub_mock.queue_data_and_ok(OP_GET_OBJECT_HANDLES, 103, encode_u32_array(&[21, 22, 23]));

        let data_info = MtpObjectInfo {
            filename: "data".to_string(),
            object_format: FORMAT_ASSOCIATION,
            ..android_info.clone()
        };
        sub_mock.queue_data_and_ok(OP_GET_OBJECT_INFO, 104, data_info.encode());

        let obb_info = MtpObjectInfo {
            filename: "obb".to_string(),
            object_format: FORMAT_ASSOCIATION,
            ..android_info.clone()
        };
        sub_mock.queue_data_and_ok(OP_GET_OBJECT_INFO, 105, obb_info.encode());

        let media_info = MtpObjectInfo {
            filename: "media".to_string(),
            object_format: FORMAT_ASSOCIATION,
            ..android_info.clone()
        };
        sub_mock.queue_data_and_ok(OP_GET_OBJECT_INFO, 106, media_info.encode());

        *io_guard = Box::new(sub_mock);
    }

    let android_items = driver.list_directory("/Android").await.expect("Failed to list /Android");
    assert_eq!(android_items.len(), 3);

    // Scoped Storage restriction verification
    let data_node = android_items.iter().find(|n| n.name == "data").unwrap();
    assert_eq!(data_node.path, "/Android/data");
    assert_eq!(data_node.entry_type, VfsEntryType::RestrictedDirectory);
    assert!(data_node.is_restricted);

    let obb_node = android_items.iter().find(|n| n.name == "obb").unwrap();
    assert_eq!(obb_node.path, "/Android/obb");
    assert_eq!(obb_node.entry_type, VfsEntryType::RestrictedDirectory);
    assert!(obb_node.is_restricted);

    let media_node = android_items.iter().find(|n| n.name == "media").unwrap();
    assert_eq!(media_node.path, "/Android/media");
    assert_eq!(media_node.entry_type, VfsEntryType::Directory);
    assert!(!media_node.is_restricted);

    // Verify writing directly to restricted Scoped Storage path is strictly prevented
    let send_res = driver.send_object("/Android/data/payload.bin", Bytes::from_static(b"exploit")).await;
    assert!(send_res.is_err());
    match send_res.unwrap_err() {
        DeviceError::RestrictedDirectoryAccess(p) => assert_eq!(p, "/Android/data/payload.bin"),
        other => panic!("Expected RestrictedDirectoryAccess error, got: {other:?}"),
    }
}
