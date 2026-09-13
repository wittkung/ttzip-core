// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Media Transfer Protocol (MTP 1.1) high-level operations pipeline.
//!
//! Provides transactional Bulk I/O encapsulation for session management,
//! storage querying, hierarchical object traversal, 64-bit partial reads,
//! and direct streaming object writes.

use crate::error::DeviceError;
use crate::mtp::protocol::*;
use crate::transport::UsbTransport;
use bytes::Bytes;
use std::time::Duration;

/// Default USB Bulk I/O transfer timeout for control and small payload operations.
pub const DEFAULT_MTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Bulk I/O transfer timeout for large streaming payload transfers.
pub const STREAM_MTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Abstract bidirectional bulk byte transport interface decoupling MTP logic from physical hardware.
pub trait MtpInOut: Send {
    /// Reads up to `length` bytes from the bulk IN endpoint.
    fn read(&mut self, length: usize, timeout: Duration) -> Result<Vec<u8>, DeviceError>;
    /// Writes byte slice to the bulk OUT endpoint.
    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<usize, DeviceError>;
}

impl MtpInOut for UsbTransport {
    fn read(&mut self, length: usize, timeout: Duration) -> Result<Vec<u8>, DeviceError> {
        self.bulk_read(length, timeout)
    }

    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<usize, DeviceError> {
        self.bulk_write(data, timeout)
    }
}

/// Reads a complete MTP container from the transport, handling packet reassembly.
pub fn read_container(io: &mut (impl MtpInOut + ?Sized), timeout: Duration) -> Result<MtpContainer, DeviceError> {
    // Initial read for container header (or up to 16KB if available)
    let mut raw = io.read(16384, timeout)?;
    if raw.len() < MTP_CONTAINER_HEADER_SIZE {
        return Err(DeviceError::ProtocolError(format!(
            "MTP header too short: read {} bytes, expected at least {MTP_CONTAINER_HEADER_SIZE}",
            raw.len()
        )));
    }

    let expected_len = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    if expected_len < MTP_CONTAINER_HEADER_SIZE {
        return Err(DeviceError::ProtocolError(format!(
            "Invalid MTP declared length: {expected_len}"
        )));
    }

    // Accumulate remaining payload bytes if packet was fragmented
    while raw.len() < expected_len {
        let remaining = expected_len - raw.len();
        let chunk = io.read(remaining.min(65536), timeout)?;
        if chunk.is_empty() {
            return Err(DeviceError::ProtocolError(format!(
                "Transport EOF while reading MTP packet (got {} of {expected_len} bytes)",
                raw.len()
            )));
        }
        raw.extend_from_slice(&chunk);
    }

    MtpContainer::decode(&raw[..expected_len])
}

/// Writes an MTP container to the transport, ensuring all bytes are drained.
pub fn write_container(
    io: &mut (impl MtpInOut + ?Sized),
    container: &MtpContainer,
    timeout: Duration,
) -> Result<(), DeviceError> {
    let encoded = container.encode();
    let mut sent = 0;
    while sent < encoded.len() {
        let n = io.write(&encoded[sent..], timeout)?;
        if n == 0 {
            return Err(DeviceError::ProtocolError("Zero bytes written to USB transport".to_string()));
        }
        sent += n;
    }
    Ok(())
}

/// Executes an MTP command that expects a single Response container without data phase.
pub fn execute_simple_command(
    io: &mut (impl MtpInOut + ?Sized),
    opcode: u16,
    transaction_id: u32,
    params: &[u32],
    timeout: Duration,
) -> Result<MtpContainer, DeviceError> {
    let cmd = MtpContainer::new_command(opcode, transaction_id, params);
    write_container(io, &cmd, timeout)?;

    let resp = read_container(io, timeout)?;
    if resp.container_type != CONTAINER_TYPE_RESPONSE {
        return Err(DeviceError::ProtocolError(format!(
            "Expected Response container type (3), got {}",
            resp.container_type
        )));
    }

    if resp.code != RESP_OK {
        return Err(map_mtp_response_error(resp.code, transaction_id));
    }

    Ok(resp)
}

/// Executes an MTP command that expects incoming Data followed by a final Response container.
pub fn execute_data_in_command(
    io: &mut (impl MtpInOut + ?Sized),
    opcode: u16,
    transaction_id: u32,
    params: &[u32],
    timeout: Duration,
) -> Result<(Vec<u8>, MtpContainer), DeviceError> {
    let cmd = MtpContainer::new_command(opcode, transaction_id, params);
    write_container(io, &cmd, timeout)?;

    let first_container = read_container(io, timeout)?;
    if first_container.container_type == CONTAINER_TYPE_RESPONSE {
        // Device aborted data phase and returned immediate response (likely error)
        if first_container.code != RESP_OK {
            return Err(map_mtp_response_error(first_container.code, transaction_id));
        }
        return Ok((Vec::new(), first_container));
    }

    if first_container.container_type != CONTAINER_TYPE_DATA {
        return Err(DeviceError::ProtocolError(format!(
            "Expected Data container type (2), got {}",
            first_container.container_type
        )));
    }

    let resp = read_container(io, timeout)?;
    if resp.container_type != CONTAINER_TYPE_RESPONSE {
        return Err(DeviceError::ProtocolError(format!(
            "Expected final Response container type (3), got {}",
            resp.container_type
        )));
    }

    if resp.code != RESP_OK {
        return Err(map_mtp_response_error(resp.code, transaction_id));
    }

    Ok((first_container.payload, resp))
}

/// Executes an MTP command sending outgoing Data followed by receiving a final Response container.
pub fn execute_data_out_command(
    io: &mut (impl MtpInOut + ?Sized),
    opcode: u16,
    transaction_id: u32,
    params: &[u32],
    payload: Vec<u8>,
    timeout: Duration,
) -> Result<MtpContainer, DeviceError> {
    let cmd = MtpContainer::new_command(opcode, transaction_id, params);
    write_container(io, &cmd, timeout)?;

    let data_block = MtpContainer::new_data(opcode, transaction_id, payload);
    write_container(io, &data_block, timeout)?;

    let resp = read_container(io, timeout)?;
    if resp.container_type != CONTAINER_TYPE_RESPONSE {
        return Err(DeviceError::ProtocolError(format!(
            "Expected final Response container type (3), got {}",
            resp.container_type
        )));
    }

    if resp.code != RESP_OK {
        return Err(map_mtp_response_error(resp.code, transaction_id));
    }

    Ok(resp)
}

/// Translates standard MTP response status codes into strongly typed `DeviceError`.
pub fn map_mtp_response_error(code: u16, transaction_id: u32) -> DeviceError {
    match code {
        RESP_INVALID_STORAGE_ID => DeviceError::ProtocolError(format!(
            "MTP error: InvalidStorageID (0x{code:04x}) in txn {transaction_id}"
        )),
        RESP_INVALID_OBJECT_HANDLE => DeviceError::ProtocolError(format!(
            "MTP error: InvalidObjectHandle (0x{code:04x}) in txn {transaction_id}"
        )),
        RESP_ACCESS_DENIED | RESP_OBJECT_WRITE_PROTECTED => DeviceError::PermissionDenied(format!(
            "MTP access denied or write protected (0x{code:04x}) in txn {transaction_id}"
        )),
        RESP_STORE_FULL => DeviceError::InsufficientDeviceStorage {
            required_bytes: 0,
            available_bytes: 0,
        },
        RESP_OPERATION_NOT_SUPPORTED | RESP_SPECIFICATION_BY_FORMAT_UNSUPPORTED => {
            DeviceError::ProtocolError(format!(
                "MTP operation not supported (0x{code:04x}) in txn {transaction_id}"
            ))
        }
        _ => DeviceError::ProtocolError(format!(
            "MTP error response 0x{code:04x} in txn {transaction_id}"
        )),
    }
}

/// Opens an MTP communication session with the device.
pub fn open_session(
    io: &mut (impl MtpInOut + ?Sized),
    session_id: u32,
    transaction_id: &mut u32,
) -> Result<(), DeviceError> {
    *transaction_id += 1;
    execute_simple_command(io, OP_OPEN_SESSION, *transaction_id, &[session_id], DEFAULT_MTP_TIMEOUT)?;
    Ok(())
}

/// Closes the currently active MTP session.
pub fn close_session(
    io: &mut (impl MtpInOut + ?Sized),
    session_id: u32,
    transaction_id: &mut u32,
) -> Result<(), DeviceError> {
    *transaction_id += 1;
    execute_simple_command(io, OP_CLOSE_SESSION, *transaction_id, &[session_id], DEFAULT_MTP_TIMEOUT)?;
    Ok(())
}

/// Retrieves list of 32-bit storage partition IDs mounted on the device.
pub fn get_storage_ids(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
) -> Result<Vec<u32>, DeviceError> {
    *transaction_id += 1;
    let (data, _) = execute_data_in_command(
        io,
        OP_GET_STORAGE_IDS,
        *transaction_id,
        &[],
        DEFAULT_MTP_TIMEOUT,
    )?;
    decode_u32_array(&data)
}

/// Queries volume capacity and filesystem metadata for the specified storage partition ID.
pub fn get_storage_info(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    storage_id: u32,
) -> Result<MtpStorageInfo, DeviceError> {
    *transaction_id += 1;
    let (data, _) = execute_data_in_command(
        io,
        OP_GET_STORAGE_INFO,
        *transaction_id,
        &[storage_id],
        DEFAULT_MTP_TIMEOUT,
    )?;
    MtpStorageInfo::decode(&data)
}

/// Queries child object handles within a given parent handle directory.
pub fn get_object_handles(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    storage_id: u32,
    parent_handle: u32,
) -> Result<Vec<u32>, DeviceError> {
    *transaction_id += 1;
    // param 0: storage_id, param 1: format_code (0 = all), param 2: parent_handle
    let (data, _) = execute_data_in_command(
        io,
        OP_GET_OBJECT_HANDLES,
        *transaction_id,
        &[storage_id, 0, parent_handle],
        DEFAULT_MTP_TIMEOUT,
    )?;
    decode_u32_array(&data)
}

/// Retrieves metadata dataset (filename, size, modification timestamp, format) for an object handle.
pub fn get_object_info(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    handle: u32,
) -> Result<MtpObjectInfo, DeviceError> {
    *transaction_id += 1;
    let (data, _) = execute_data_in_command(
        io,
        OP_GET_OBJECT_INFO,
        *transaction_id,
        &[handle],
        DEFAULT_MTP_TIMEOUT,
    )?;
    MtpObjectInfo::decode(&data)
}

/// Reads a 64-bit ranged byte slice from a remote object using MTP 1.1 opcode `0x9807`.
///
/// Permits instant archive inspection (reading EOCD / central directory records)
/// without downloading gigabytes of archive contents to local disk.
pub fn get_partial_object_64(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    handle: u32,
    offset: u64,
    max_bytes: u32,
) -> Result<Bytes, DeviceError> {
    *transaction_id += 1;
    let offset_low = (offset & 0xFFFF_FFFF) as u32;
    let offset_high = (offset >> 32) as u32;

    let (data, _) = execute_data_in_command(
        io,
        OP_GET_PARTIAL_OBJECT_64,
        *transaction_id,
        &[handle, offset_low, offset_high, max_bytes],
        STREAM_MTP_TIMEOUT,
    )?;

    Ok(Bytes::from(data))
}

/// Transmits object metadata prior to sending binary content, allocating a remote `ObjectHandle`.
pub fn send_object_info(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    storage_id: u32,
    parent_handle: u32,
    info: &MtpObjectInfo,
) -> Result<u32, DeviceError> {
    *transaction_id += 1;
    let payload = info.encode();
    let resp = execute_data_out_command(
        io,
        OP_SEND_OBJECT_INFO,
        *transaction_id,
        &[storage_id, parent_handle],
        payload,
        DEFAULT_MTP_TIMEOUT,
    )?;

    // Response param 2 contains the created ObjectHandle
    resp.param(2)
}

/// Streams binary payload to the remote object initiated by `SendObjectInfo`.
pub fn send_object(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    data: Bytes,
) -> Result<(), DeviceError> {
    *transaction_id += 1;
    execute_data_out_command(
        io,
        OP_SEND_OBJECT,
        *transaction_id,
        &[],
        data.to_vec(),
        STREAM_MTP_TIMEOUT,
    )?;
    Ok(())
}

/// Deletes an object (file or directory) by its MTP ObjectHandle.
pub fn delete_object(
    io: &mut (impl MtpInOut + ?Sized),
    transaction_id: &mut u32,
    handle: u32,
) -> Result<(), DeviceError> {
    *transaction_id += 1;
    execute_simple_command(
        io,
        OP_DELETE_OBJECT,
        *transaction_id,
        &[handle, 0], // format_code = 0 (all)
        DEFAULT_MTP_TIMEOUT,
    )?;
    Ok(())
}
