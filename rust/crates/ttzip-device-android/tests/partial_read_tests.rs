// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for MTP 1.1 64-bit partial object reads (opcode `0x9807`),
//! verifying instant archive inspection, 4GB+ offset parameter splitting,
//! and Scoped Storage protection barriers.

use std::collections::VecDeque;
use std::time::Duration;
use ttzip_device_android::error::DeviceError;
use ttzip_device_android::traits::DeviceStorageDriver;

// Direct path module inclusion to ensure isolated test compile passes without lib.rs edit
pub use ttzip_device_android::{error, models, traits, transport};
#[path = "../src/mtp/mod.rs"]
pub mod mtp;

use mtp::operations::{get_partial_object_64, MtpInOut};
use mtp::protocol::*;
use mtp::MtpDeviceDriver;

/// Mock bidirectional MTP transport recording requests and replying with queued packets.
struct MockPartialReadTransport {
    incoming_queue: VecDeque<Vec<u8>>,
    sent_commands: Vec<MtpContainer>,
}

impl MockPartialReadTransport {
    fn new() -> Self {
        Self {
            incoming_queue: VecDeque::new(),
            sent_commands: Vec::new(),
        }
    }

    fn queue_data_and_response(&mut self, opcode: u16, transaction_id: u32, payload: Vec<u8>, resp_code: u16) {
        let data = MtpContainer::new_data(opcode, transaction_id, payload);
        let resp = MtpContainer::new_response(resp_code, transaction_id, &[]);
        self.incoming_queue.push_back(data.encode());
        self.incoming_queue.push_back(resp.encode());
    }

    fn queue_immediate_response(&mut self, transaction_id: u32, resp_code: u16) {
        let resp = MtpContainer::new_response(resp_code, transaction_id, &[]);
        self.incoming_queue.push_back(resp.encode());
    }
}

impl MtpInOut for MockPartialReadTransport {
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
            Err(DeviceError::ProtocolTimeout("Mock incoming queue exhausted".to_string()))
        }
    }

    fn write(&mut self, data: &[u8], _timeout: Duration) -> Result<usize, DeviceError> {
        if let Ok(c) = MtpContainer::decode(data) {
            self.sent_commands.push(c);
        }
        Ok(data.len())
    }
}

#[test]
fn test_get_partial_object_64_parameter_splitting() {
    let mut mock = MockPartialReadTransport::new();
    let handle = 0x0000_1234;
    // 4.8 GB offset = 4_800_000_000 bytes
    // 4_800_000_000 = 0x1_1E1A_3000
    // offset_high = 0x0000_0001 (1), offset_low = 0x1E1A_3000 (505032704)
    let large_offset: u64 = 4_800_000_000;
    let read_len: u32 = 65536;

    let dummy_slice = vec![0xAB; 65536];
    mock.queue_data_and_response(OP_GET_PARTIAL_OBJECT_64, 1, dummy_slice.clone(), RESP_OK);

    let mut txn = 0;
    let result = get_partial_object_64(&mut mock, &mut txn, handle, large_offset, read_len)
        .expect("Partial read should succeed");

    assert_eq!(result.len(), 65536);
    assert_eq!(&result[..], &dummy_slice[..]);

    // Inspect sent command parameters
    let sent_cmd = mock.sent_commands.iter().find(|c| c.code == OP_GET_PARTIAL_OBJECT_64)
        .expect("Command OP_GET_PARTIAL_OBJECT_64 not found");

    assert_eq!(sent_cmd.container_type, CONTAINER_TYPE_COMMAND);
    assert_eq!(sent_cmd.transaction_id, 1);
    let params = sent_cmd.params().expect("Failed to get command params");
    assert_eq!(params.len(), 4);
    assert_eq!(params[0], handle);
    assert_eq!(params[1], (large_offset & 0xFFFF_FFFF) as u32);
    assert_eq!(params[2], (large_offset >> 32) as u32);
    assert_eq!(params[3], read_len);
}

#[test]
fn test_instant_archive_eocd_slice_inspection() {
    let mut mock = MockPartialReadTransport::new();
    let handle = 42;
    // Simulate ZIP End of Central Directory Record (22 bytes)
    let eocd_record = b"PK\x05\x06\x00\x00\x00\x00\x01\x00\x01\x00\x16\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
    let total_file_size: u64 = 5_000_000_000; // 5GB archive
    let eocd_offset = total_file_size - 22;

    mock.queue_data_and_response(OP_GET_PARTIAL_OBJECT_64, 10, eocd_record.clone(), RESP_OK);

    let mut txn = 9;
    let slice = get_partial_object_64(&mut mock, &mut txn, handle, eocd_offset, 22)
        .expect("EOCD slice read failed");

    assert_eq!(slice.len(), 22);
    assert_eq!(&slice[0..4], b"PK\x05\x06");
}

#[test]
fn test_partial_read_device_error_mapping() {
    let mut mock = MockPartialReadTransport::new();
    mock.queue_immediate_response(5, RESP_INVALID_OBJECT_HANDLE);

    let mut txn = 4;
    let res = get_partial_object_64(&mut mock, &mut txn, 0xDEAD, 0, 1024);
    assert!(res.is_err());
    match res.unwrap_err() {
        DeviceError::ProtocolError(msg) => {
            assert!(msg.contains("InvalidObjectHandle") || msg.contains("0x2009"));
        }
        other => panic!("Expected ProtocolError with InvalidObjectHandle, got {other:?}"),
    }
}

#[tokio::test]
async fn test_driver_get_partial_object_scoped_storage_block() {
    let mock = MockPartialReadTransport::new();
    let driver = MtpDeviceDriver::new_with_io(Box::new(mock), 0x00010001);

    // Any attempt to read partial object from /Android/data or /Android/obb must be blocked immediately
    let res = driver.get_partial_object("/Android/data/com.app/files/backup.zip", 0, 1024).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        DeviceError::RestrictedDirectoryAccess(p) => {
            assert_eq!(p, "/Android/data/com.app/files/backup.zip");
        }
        other => panic!("Expected RestrictedDirectoryAccess error, got {other:?}"),
    }
}
