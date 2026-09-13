// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Media Transfer Protocol (MTP 1.1 / PTP ISO 15740) binary container codec,
//! operation/response opcodes, and metadata serializers.

use crate::error::DeviceError;

/// Standard MTP 12-byte header length.
pub const MTP_CONTAINER_HEADER_SIZE: usize = 12;

/// MTP USB container types.
pub const CONTAINER_TYPE_UNDEFINED: u16 = 0x0000;
pub const CONTAINER_TYPE_COMMAND: u16 = 0x0001;
pub const CONTAINER_TYPE_DATA: u16 = 0x0002;
pub const CONTAINER_TYPE_RESPONSE: u16 = 0x0003;
pub const CONTAINER_TYPE_EVENT: u16 = 0x0004;

/// Standard MTP / PTP operation codes.
pub const OP_UNDEFINED: u16 = 0x1000;
pub const OP_OPEN_SESSION: u16 = 0x1001;
pub const OP_CLOSE_SESSION: u16 = 0x1002;
pub const OP_GET_STORAGE_IDS: u16 = 0x1004;
pub const OP_GET_STORAGE_INFO: u16 = 0x1005;
pub const OP_GET_NUM_OBJECTS: u16 = 0x1006;
pub const OP_GET_OBJECT_HANDLES: u16 = 0x1007;
pub const OP_GET_OBJECT_INFO: u16 = 0x1008;
pub const OP_GET_OBJECT: u16 = 0x1009;
pub const OP_GET_THUMB: u16 = 0x100A;
pub const OP_DELETE_OBJECT: u16 = 0x100B;
pub const OP_SEND_OBJECT_INFO: u16 = 0x100C;
pub const OP_SEND_OBJECT: u16 = 0x100D;
pub const OP_INITIATE_CAPTURE: u16 = 0x100E;
pub const OP_FORMAT_STORE: u16 = 0x100F;
pub const OP_RESET_DEVICE: u16 = 0x1010;
pub const OP_SELF_TEST: u16 = 0x1011;
pub const OP_SET_OBJECT_PROTECTION: u16 = 0x1012;
pub const OP_POWER_DOWN: u16 = 0x1013;
pub const OP_GET_DEVICE_PROP_DESC: u16 = 0x1014;
pub const OP_GET_DEVICE_PROP_VALUE: u16 = 0x1015;
pub const OP_SET_DEVICE_PROP_VALUE: u16 = 0x1016;
pub const OP_RESET_DEVICE_PROP_VALUE: u16 = 0x1017;
pub const OP_TERMINATE_OPEN_CAPTURE: u16 = 0x1018;
pub const OP_MOVE_OBJECT: u16 = 0x1019;
pub const OP_COPY_OBJECT: u16 = 0x101A;
pub const OP_GET_PARTIAL_OBJECT: u16 = 0x101B;

/// MTP 1.1 64-bit partial read extension opcode.
pub const OP_GET_PARTIAL_OBJECT_64: u16 = 0x9807;
pub const OP_SEND_PARTIAL_OBJECT: u16 = 0x9808;
pub const OP_TRUNCATE_OBJECT: u16 = 0x9809;
pub const OP_BEGIN_EDIT_OBJECT: u16 = 0x980B;
pub const OP_END_EDIT_OBJECT: u16 = 0x980C;

/// Standard MTP / PTP response codes.
pub const RESP_UNDEFINED: u16 = 0x2000;
pub const RESP_OK: u16 = 0x2001;
pub const RESP_GENERAL_ERROR: u16 = 0x2002;
pub const RESP_SESSION_NOT_OPEN: u16 = 0x2003;
pub const RESP_INVALID_TRANSACTION_ID: u16 = 0x2004;
pub const RESP_OPERATION_NOT_SUPPORTED: u16 = 0x2005;
pub const RESP_PARAMETER_NOT_SUPPORTED: u16 = 0x2006;
pub const RESP_INCOMPLETE_TRANSFER: u16 = 0x2007;
pub const RESP_INVALID_STORAGE_ID: u16 = 0x2008;
pub const RESP_INVALID_OBJECT_HANDLE: u16 = 0x2009;
pub const RESP_DEVICE_PROP_NOT_SUPPORTED: u16 = 0x200A;
pub const RESP_INVALID_OBJECT_FORMAT_CODE: u16 = 0x200B;
pub const RESP_STORE_FULL: u16 = 0x200C;
pub const RESP_OBJECT_WRITE_PROTECTED: u16 = 0x200D;
pub const RESP_STORE_READ_ONLY: u16 = 0x200E;
pub const RESP_ACCESS_DENIED: u16 = 0x200F;
pub const RESP_SPECIFICATION_BY_FORMAT_UNSUPPORTED: u16 = 0x2014;
pub const RESP_NO_VALID_OBJECT_INFO: u16 = 0x2015;
pub const RESP_DEVICE_BUSY: u16 = 0x2019;
pub const RESP_INVALID_PARENT_OBJECT: u16 = 0x201A;
pub const RESP_INVALID_PARAMETER: u16 = 0x201D;
pub const RESP_SESSION_ALREADY_OPEN: u16 = 0x201E;
pub const RESP_TRANSACTION_CANCELLED: u16 = 0x201F;

/// Standard MTP format codes.
pub const FORMAT_UNDEFINED: u16 = 0x3000;
pub const FORMAT_ASSOCIATION: u16 = 0x3001; // Directory / Folder

/// Special parent object handle indicating storage root directory.
pub const MTP_PARENT_ROOT: u32 = 0xFFFF_FFFF;

/// Represents a binary MTP 1.1 packet container adhering to USB-IF PTP-MTP specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtpContainer {
    /// Total packet length including the 12-byte header and payload.
    pub length: u32,
    /// Type classification: Command (1), Data (2), Response (3), or Event (4).
    pub container_type: u16,
    /// Operation code, response code, or event code.
    pub code: u16,
    /// Unique identifier tracking request-response pairing.
    pub transaction_id: u32,
    /// Raw payload bytes (32-bit parameters for commands/responses, or raw binary for data).
    pub payload: Vec<u8>,
}

impl MtpContainer {
    /// Constructs a command container with up to 5 parameters.
    pub fn new_command(code: u16, transaction_id: u32, params: &[u32]) -> Self {
        let mut payload = Vec::with_capacity(params.len() * 4);
        for &p in params {
            payload.extend_from_slice(&p.to_le_bytes());
        }
        let length = (MTP_CONTAINER_HEADER_SIZE + payload.len()) as u32;
        Self {
            length,
            container_type: CONTAINER_TYPE_COMMAND,
            code,
            transaction_id,
            payload,
        }
    }

    /// Constructs a data block container holding arbitrary byte payloads.
    pub fn new_data(code: u16, transaction_id: u32, payload: Vec<u8>) -> Self {
        let length = (MTP_CONTAINER_HEADER_SIZE + payload.len()) as u32;
        Self {
            length,
            container_type: CONTAINER_TYPE_DATA,
            code,
            transaction_id,
            payload,
        }
    }

    /// Constructs a response container with up to 5 return parameters.
    pub fn new_response(code: u16, transaction_id: u32, params: &[u32]) -> Self {
        let mut payload = Vec::with_capacity(params.len() * 4);
        for &p in params {
            payload.extend_from_slice(&p.to_le_bytes());
        }
        let length = (MTP_CONTAINER_HEADER_SIZE + payload.len()) as u32;
        Self {
            length,
            container_type: CONTAINER_TYPE_RESPONSE,
            code,
            transaction_id,
            payload,
        }
    }

    /// Serializes the container into binary bytes ready for USB Bulk OUT transmission.
    pub fn encode(&self) -> Vec<u8> {
        let total_len = MTP_CONTAINER_HEADER_SIZE + self.payload.len();
        let mut buf = Vec::with_capacity(total_len);
        buf.extend_from_slice(&(total_len as u32).to_le_bytes());
        buf.extend_from_slice(&self.container_type.to_le_bytes());
        buf.extend_from_slice(&self.code.to_le_bytes());
        buf.extend_from_slice(&self.transaction_id.to_le_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parses a raw byte buffer into an `MtpContainer`.
    pub fn decode(buf: &[u8]) -> Result<Self, DeviceError> {
        if buf.len() < MTP_CONTAINER_HEADER_SIZE {
            return Err(DeviceError::ProtocolError(format!(
                "MTP buffer too short: expected at least {MTP_CONTAINER_HEADER_SIZE} bytes, got {}",
                buf.len()
            )));
        }

        let length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let container_type = u16::from_le_bytes([buf[4], buf[5]]);
        let code = u16::from_le_bytes([buf[6], buf[7]]);
        let transaction_id = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);

        if (length as usize) < MTP_CONTAINER_HEADER_SIZE {
            return Err(DeviceError::ProtocolError(format!(
                "Invalid MTP container length field: {length} is less than header size"
            )));
        }

        let payload_len = (length as usize) - MTP_CONTAINER_HEADER_SIZE;
        if buf.len() < length as usize {
            return Err(DeviceError::ProtocolError(format!(
                "Truncated MTP container: header declared {length} bytes, received {}",
                buf.len()
            )));
        }

        let payload = buf[MTP_CONTAINER_HEADER_SIZE..MTP_CONTAINER_HEADER_SIZE + payload_len].to_vec();

        Ok(Self {
            length,
            container_type,
            code,
            transaction_id,
            payload,
        })
    }

    /// Extracts 32-bit parameters from command or response payload.
    pub fn params(&self) -> Result<Vec<u32>, DeviceError> {
        if !self.payload.len().is_multiple_of(4) {
            return Err(DeviceError::ProtocolError(format!(
                "Payload size {} is not a multiple of 4 bytes",
                self.payload.len()
            )));
        }
        let count = self.payload.len() / 4;
        let mut params = Vec::with_capacity(count);
        for i in 0..count {
            let offset = i * 4;
            let val = u32::from_le_bytes([
                self.payload[offset],
                self.payload[offset + 1],
                self.payload[offset + 2],
                self.payload[offset + 3],
            ]);
            params.push(val);
        }
        Ok(params)
    }

    /// Gets parameter at index or returns error if out of bounds.
    pub fn param(&self, index: usize) -> Result<u32, DeviceError> {
        let offset = index * 4;
        if self.payload.len() < offset + 4 {
            return Err(DeviceError::ProtocolError(format!(
                "MTP parameter index {index} out of bounds (payload length {})",
                self.payload.len()
            )));
        }
        Ok(u32::from_le_bytes([
            self.payload[offset],
            self.payload[offset + 1],
            self.payload[offset + 2],
            self.payload[offset + 3],
        ]))
    }
}

/// Encodes a standard MTP string (UTF-16LE with leading character count byte and null terminator).
pub fn encode_mtp_string(s: &str) -> Vec<u8> {
    if s.is_empty() {
        return vec![0x00];
    }
    let u16_chars: Vec<u16> = s.encode_utf16().collect();
    let num_chars = (u16_chars.len() + 1).min(255) as u8;
    let mut buf = Vec::with_capacity(1 + (num_chars as usize) * 2);
    buf.push(num_chars);
    for &ch in &u16_chars[..(num_chars as usize - 1)] {
        buf.extend_from_slice(&ch.to_le_bytes());
    }
    buf.extend_from_slice(&0u16.to_le_bytes()); // Null terminator
    buf
}

/// Decodes an MTP string from a byte slice at specified offset, advancing the offset.
pub fn decode_mtp_string(buf: &[u8], offset: &mut usize) -> Result<String, DeviceError> {
    if *offset >= buf.len() {
        return Err(DeviceError::ProtocolError(
            "Unexpected end of buffer reading MTP string length".to_string(),
        ));
    }
    let num_chars = buf[*offset] as usize;
    *offset += 1;

    if num_chars == 0 {
        return Ok(String::new());
    }

    let byte_len = num_chars * 2;
    if *offset + byte_len > buf.len() {
        return Err(DeviceError::ProtocolError(format!(
            "MTP string requires {byte_len} bytes, only {} remaining",
            buf.len().saturating_sub(*offset)
        )));
    }

    let mut u16_chars = Vec::with_capacity(num_chars);
    for i in 0..num_chars {
        let pos = *offset + i * 2;
        let val = u16::from_le_bytes([buf[pos], buf[pos + 1]]);
        u16_chars.push(val);
    }
    *offset += byte_len;

    // Strip trailing null if present
    if u16_chars.last() == Some(&0) {
        u16_chars.pop();
    }

    Ok(String::from_utf16_lossy(&u16_chars))
}

/// Formats a Unix epoch timestamp (seconds) into standard MTP ISO-8601 string `YYYYMMDDThhmmss`.
pub fn encode_mtp_timestamp(epoch_secs: u64) -> String {
    let days = epoch_secs / 86400;
    let time_secs = epoch_secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Convert epoch days to approximate Gregorian calendar (valid for 1970-2099)
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}{month:02}{day:02}T{hours:02}{minutes:02}{seconds:02}")
}

/// Parses an MTP timestamp string `YYYYMMDDThhmmss[.s]` into Unix epoch seconds.
pub fn decode_mtp_timestamp(s: &str) -> u64 {
    let clean = s.trim();
    if clean.len() < 15 {
        return 0;
    }
    let year: u64 = clean[0..4].parse().unwrap_or(1970);
    let month: u64 = clean[4..6].parse().unwrap_or(1);
    let day: u64 = clean[6..8].parse().unwrap_or(1);
    let hour: u64 = clean[9..11].parse().unwrap_or(0);
    let min: u64 = clean[11..13].parse().unwrap_or(0);
    let sec: u64 = clean[13..15].parse().unwrap_or(0);

    ymd_to_epoch_days(year, month, day) * 86400 + hour * 3600 + min * 60 + sec
}

fn days_to_ymd(epoch_days: u64) -> (u64, u64, u64) {
    let mut d = epoch_days;
    let mut year = 1970;
    loop {
        let leap = is_leap_year(year);
        let days_in_year = if leap { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &md in &month_days {
        if d < md {
            break;
        }
        d -= md;
        month += 1;
    }
    let day = d + 1;
    (year, month, day)
}

fn ymd_to_epoch_days(year: u64, month: u64, day: u64) -> u64 {
    let mut days = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    let leap = is_leap_year(year);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let m_limit = month.saturating_sub(1).min(11) as usize;
    for &md in &month_days[..m_limit] {
        days += md;
    }
    days += day.saturating_sub(1);
    days
}

fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// Decodes an MTP 32-bit array: 4-byte count followed by `count` u32 values in LE.
pub fn decode_u32_array(buf: &[u8]) -> Result<Vec<u32>, DeviceError> {
    if buf.len() < 4 {
        return Err(DeviceError::ProtocolError(
            "Buffer too short for array count".to_string(),
        ));
    }
    let count = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let expected_len = 4 + count * 4;
    if buf.len() < expected_len {
        return Err(DeviceError::ProtocolError(format!(
            "Array declares {count} elements (need {expected_len} bytes), got {}",
            buf.len()
        )));
    }
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let pos = 4 + i * 4;
        let val = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        result.push(val);
    }
    Ok(result)
}

/// Encodes a 32-bit array into standard MTP array format.
pub fn encode_u32_array(arr: &[u32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + arr.len() * 4);
    buf.extend_from_slice(&(arr.len() as u32).to_le_bytes());
    for &val in arr {
        buf.extend_from_slice(&val.to_le_bytes());
    }
    buf
}

/// Parsed MTP StorageInfo dataset returned by `GetStorageInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtpStorageInfo {
    pub storage_type: u16,
    pub filesystem_type: u16,
    pub access_capability: u16,
    pub max_capacity: u64,
    pub free_space_bytes: u64,
    pub free_space_objects: u32,
    pub storage_description: String,
    pub volume_identifier: String,
}

impl MtpStorageInfo {
    /// Decodes a StorageInfo dataset from binary payload.
    pub fn decode(buf: &[u8]) -> Result<Self, DeviceError> {
        if buf.len() < 26 {
            return Err(DeviceError::ProtocolError(format!(
                "StorageInfo buffer too short: expected at least 26 bytes, got {}",
                buf.len()
            )));
        }
        let storage_type = u16::from_le_bytes([buf[0], buf[1]]);
        let filesystem_type = u16::from_le_bytes([buf[2], buf[3]]);
        let access_capability = u16::from_le_bytes([buf[4], buf[5]]);
        let max_capacity = u64::from_le_bytes([
            buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13],
        ]);
        let free_space_bytes = u64::from_le_bytes([
            buf[14], buf[15], buf[16], buf[17], buf[18], buf[19], buf[20], buf[21],
        ]);
        let free_space_objects = u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]);

        let mut offset = 26;
        let storage_description = decode_mtp_string(buf, &mut offset)?;
        let volume_identifier = decode_mtp_string(buf, &mut offset)?;

        Ok(Self {
            storage_type,
            filesystem_type,
            access_capability,
            max_capacity,
            free_space_bytes,
            free_space_objects,
            storage_description,
            volume_identifier,
        })
    }
}

/// Parsed MTP ObjectInfo dataset returned by `GetObjectInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtpObjectInfo {
    pub storage_id: u32,
    pub object_format: u16,
    pub protection_status: u16,
    pub object_compressed_size: u32,
    pub thumb_format: u16,
    pub thumb_compressed_size: u32,
    pub thumb_pix_width: u32,
    pub thumb_pix_height: u32,
    pub image_pix_width: u32,
    pub image_pix_height: u32,
    pub image_bit_depth: u32,
    pub parent_object: u32,
    pub association_type: u16,
    pub association_desc: u32,
    pub sequence_number: u32,
    pub filename: String,
    pub date_created: String,
    pub date_modified: String,
    pub keywords: String,
}

impl MtpObjectInfo {
    /// Decodes an ObjectInfo dataset from binary payload.
    pub fn decode(buf: &[u8]) -> Result<Self, DeviceError> {
        if buf.len() < 52 {
            return Err(DeviceError::ProtocolError(format!(
                "ObjectInfo buffer too short: expected at least 52 bytes, got {}",
                buf.len()
            )));
        }

        let storage_id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let object_format = u16::from_le_bytes([buf[4], buf[5]]);
        let protection_status = u16::from_le_bytes([buf[6], buf[7]]);
        let object_compressed_size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let thumb_format = u16::from_le_bytes([buf[12], buf[13]]);
        let thumb_compressed_size = u32::from_le_bytes([buf[14], buf[15], buf[16], buf[17]]);
        let thumb_pix_width = u32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]);
        let thumb_pix_height = u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]);
        let image_pix_width = u32::from_le_bytes([buf[26], buf[27], buf[28], buf[29]]);
        let image_pix_height = u32::from_le_bytes([buf[30], buf[31], buf[32], buf[33]]);
        let image_bit_depth = u32::from_le_bytes([buf[34], buf[35], buf[36], buf[37]]);
        let parent_object = u32::from_le_bytes([buf[38], buf[39], buf[40], buf[41]]);
        let association_type = u16::from_le_bytes([buf[42], buf[43]]);
        let association_desc = u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]);
        let sequence_number = u32::from_le_bytes([buf[48], buf[49], buf[50], buf[51]]);

        let mut offset = 52;
        let filename = decode_mtp_string(buf, &mut offset)?;
        let date_created = decode_mtp_string(buf, &mut offset)?;
        let date_modified = decode_mtp_string(buf, &mut offset)?;
        let keywords = decode_mtp_string(buf, &mut offset)?;

        Ok(Self {
            storage_id,
            object_format,
            protection_status,
            object_compressed_size,
            thumb_format,
            thumb_compressed_size,
            thumb_pix_width,
            thumb_pix_height,
            image_pix_width,
            image_pix_height,
            image_bit_depth,
            parent_object,
            association_type,
            association_desc,
            sequence_number,
            filename,
            date_created,
            date_modified,
            keywords,
        })
    }

    /// Serializes ObjectInfo dataset for `SendObjectInfo`.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(&self.storage_id.to_le_bytes());
        buf.extend_from_slice(&self.object_format.to_le_bytes());
        buf.extend_from_slice(&self.protection_status.to_le_bytes());
        buf.extend_from_slice(&self.object_compressed_size.to_le_bytes());
        buf.extend_from_slice(&self.thumb_format.to_le_bytes());
        buf.extend_from_slice(&self.thumb_compressed_size.to_le_bytes());
        buf.extend_from_slice(&self.thumb_pix_width.to_le_bytes());
        buf.extend_from_slice(&self.thumb_pix_height.to_le_bytes());
        buf.extend_from_slice(&self.image_pix_width.to_le_bytes());
        buf.extend_from_slice(&self.image_pix_height.to_le_bytes());
        buf.extend_from_slice(&self.image_bit_depth.to_le_bytes());
        buf.extend_from_slice(&self.parent_object.to_le_bytes());
        buf.extend_from_slice(&self.association_type.to_le_bytes());
        buf.extend_from_slice(&self.association_desc.to_le_bytes());
        buf.extend_from_slice(&self.sequence_number.to_le_bytes());

        buf.extend_from_slice(&encode_mtp_string(&self.filename));
        buf.extend_from_slice(&encode_mtp_string(&self.date_created));
        buf.extend_from_slice(&encode_mtp_string(&self.date_modified));
        buf.extend_from_slice(&encode_mtp_string(&self.keywords));
        buf
    }

    /// Returns true if this object represents a directory / folder.
    pub fn is_dir(&self) -> bool {
        self.object_format == FORMAT_ASSOCIATION
    }
}
