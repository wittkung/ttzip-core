// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Binary Android Debug Bridge (ADB) transport framing and packet protocol codecs.
//!
//! Implements the 24-byte `amessage` wire format, complementary magic validation,
//! byte-sum data checksums, and standard ADB command negotiation primitives.

use crate::error::DeviceError;

/// ADB binary protocol commands (32-bit little-endian words).
pub const A_SYNC: u32 = 0x434E_5953; // "SYNC"
pub const A_CNXN: u32 = 0x4E58_4E43; // "CNXN"
pub const A_AUTH: u32 = 0x4854_5541; // "AUTH"
pub const A_OPEN: u32 = 0x4E45_504F; // "OPEN"
pub const A_OKAY: u32 = 0x5941_4B4F; // "OKAY"
pub const A_CLSE: u32 = 0x4553_4C43; // "CLSE"
pub const A_WRTE: u32 = 0x4554_5257; // "WRTE"
pub const A_STLS: u32 = 0x534C_5453; // "STLS"

/// Wire size of the ADB packet header in bytes.
pub const ADB_MESSAGE_HEADER_SIZE: usize = 24;

/// Protocol version 1.0.0.
pub const A_VERSION_MIN: u32 = 0x0100_0000;

/// Protocol version supporting checksum bypass.
pub const A_VERSION_SKIP_CHECKSUM: u32 = 0x0100_0001;

/// Maximum payload size negotiated over ADB channel (1MB standard).
pub const A_MAX_PAYLOAD: u32 = 1024 * 1024;

/// Represents an ADB command identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdbCommand {
    Sync,
    Cnxn,
    Auth,
    Open,
    Okay,
    Clse,
    Wrte,
    Stls,
    Unknown(u32),
}

impl From<u32> for AdbCommand {
    fn from(val: u32) -> Self {
        match val {
            A_SYNC => Self::Sync,
            A_CNXN => Self::Cnxn,
            A_AUTH => Self::Auth,
            A_OPEN => Self::Open,
            A_OKAY => Self::Okay,
            A_CLSE => Self::Clse,
            A_WRTE => Self::Wrte,
            A_STLS => Self::Stls,
            other => Self::Unknown(other),
        }
    }
}

impl From<AdbCommand> for u32 {
    fn from(cmd: AdbCommand) -> Self {
        match cmd {
            AdbCommand::Sync => A_SYNC,
            AdbCommand::Cnxn => A_CNXN,
            AdbCommand::Auth => A_AUTH,
            AdbCommand::Open => A_OPEN,
            AdbCommand::Okay => A_OKAY,
            AdbCommand::Clse => A_CLSE,
            AdbCommand::Wrte => A_WRTE,
            AdbCommand::Stls => A_STLS,
            AdbCommand::Unknown(val) => val,
        }
    }
}

/// Binary ADB message header matching the native 24-byte `amessage` structure.
///
/// ```c
/// struct amessage {
///     uint32_t command;     /* command identifier constant      */
///     uint32_t arg0;        /* first argument                   */
///     uint32_t arg1;        /* second argument                  */
///     uint32_t data_length; /* length of payload (0 is allowed) */
///     uint32_t data_crc32;  /* crc32 of data payload            */
///     uint32_t magic;       /* command ^ 0xffffffff             */
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdbMessage {
    /// Command identifier constant (e.g., `A_CNXN`, `A_OPEN`, `A_WRTE`).
    pub command: u32,
    /// First command-specific argument (e.g., version or local stream ID).
    pub arg0: u32,
    /// Second command-specific argument (e.g., max payload or remote stream ID).
    pub arg1: u32,
    /// Length of the following data payload in bytes.
    pub data_length: u32,
    /// Byte-sum checksum of the data payload.
    pub data_crc32: u32,
    /// Integrity magic word (`command ^ 0xFFFF_FFFF`).
    pub magic: u32,
}

impl AdbMessage {
    /// Constructs a new message header with computed checksum and magic fields.
    pub fn new(command: u32, arg0: u32, arg1: u32, payload: &[u8]) -> Self {
        let data_length = payload.len() as u32;
        let data_crc32 = Self::calculate_crc32(payload);
        let magic = command ^ 0xFFFF_FFFF;

        Self {
            command,
            arg0,
            arg1,
            data_length,
            data_crc32,
            magic,
        }
    }

    /// Computes the historical ADB payload checksum (sum of all payload bytes).
    #[inline]
    pub fn calculate_crc32(payload: &[u8]) -> u32 {
        payload.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32))
    }

    /// Validates the complementary magic integrity invariant (`command ^ 0xFFFF_FFFF == magic`).
    #[inline]
    pub fn verify_magic(&self) -> bool {
        (self.command ^ 0xFFFF_FFFF) == self.magic
    }

    /// Validates that the payload matches the expected length and byte-sum checksum.
    pub fn verify_checksum(&self, payload: &[u8]) -> bool {
        if payload.len() != self.data_length as usize {
            return false;
        }
        Self::calculate_crc32(payload) == self.data_crc32
    }

    /// Encodes the message header into standard 24-byte little-endian wire format.
    pub fn encode(&self) -> [u8; ADB_MESSAGE_HEADER_SIZE] {
        let mut buf = [0u8; ADB_MESSAGE_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.command.to_le_bytes());
        buf[4..8].copy_from_slice(&self.arg0.to_le_bytes());
        buf[8..12].copy_from_slice(&self.arg1.to_le_bytes());
        buf[12..16].copy_from_slice(&self.data_length.to_le_bytes());
        buf[16..20].copy_from_slice(&self.data_crc32.to_le_bytes());
        buf[20..24].copy_from_slice(&self.magic.to_le_bytes());
        buf
    }

    /// Decodes a 24-byte slice into an `AdbMessage`, validating magic integrity.
    pub fn decode(buf: &[u8; ADB_MESSAGE_HEADER_SIZE]) -> Result<Self, DeviceError> {
        let command = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let arg0 = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let arg1 = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let data_length = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let data_crc32 = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let magic = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);

        let msg = Self {
            command,
            arg0,
            arg1,
            data_length,
            data_crc32,
            magic,
        };

        if !msg.verify_magic() {
            return Err(DeviceError::ProtocolError(format!(
                "Invalid ADB message magic: command 0x{:08X} ^ magic 0x{:08X} != 0xFFFFFFFF",
                command, magic
            )));
        }

        Ok(msg)
    }

    /// Returns human-readable command acronym for logging and diagnostics.
    pub fn command_name(&self) -> &'static str {
        match self.command {
            A_SYNC => "SYNC",
            A_CNXN => "CNXN",
            A_AUTH => "AUTH",
            A_OPEN => "OPEN",
            A_OKAY => "OKAY",
            A_CLSE => "CLSE",
            A_WRTE => "WRTE",
            A_STLS => "STLS",
            _ => "UNKNOWN",
        }
    }
}

/// Fully framed ADB transport packet containing the header and trailing payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbPacket {
    /// 24-byte message header.
    pub message: AdbMessage,
    /// Associated payload bytes (length must equal `message.data_length`).
    pub payload: Vec<u8>,
}

impl AdbPacket {
    /// Creates a complete ADB packet from command parameters and payload.
    pub fn new(command: u32, arg0: u32, arg1: u32, payload: Vec<u8>) -> Self {
        let message = AdbMessage::new(command, arg0, arg1, &payload);
        Self { message, payload }
    }

    /// Serializes header and payload into a single contiguous byte buffer.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ADB_MESSAGE_HEADER_SIZE + self.payload.len());
        out.extend_from_slice(&self.message.encode());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parses a complete packet from an incoming stream buffer if sufficient bytes are present.
    ///
    /// Returns `Ok(Some((packet, bytes_consumed)))` on successful full frame decode,
    /// `Ok(None)` if more data bytes are required, or `Err` if the header is corrupt.
    pub fn decode_from_slice(buf: &[u8]) -> Result<Option<(Self, usize)>, DeviceError> {
        if buf.len() < ADB_MESSAGE_HEADER_SIZE {
            return Ok(None);
        }

        let mut header_bytes = [0u8; ADB_MESSAGE_HEADER_SIZE];
        header_bytes.copy_from_slice(&buf[..ADB_MESSAGE_HEADER_SIZE]);
        let message = AdbMessage::decode(&header_bytes)?;

        let total_frame_len = ADB_MESSAGE_HEADER_SIZE + message.data_length as usize;
        if buf.len() < total_frame_len {
            return Ok(None);
        }

        let payload = buf[ADB_MESSAGE_HEADER_SIZE..total_frame_len].to_vec();
        if !message.verify_checksum(&payload) {
            return Err(DeviceError::ProtocolError(format!(
                "ADB payload checksum mismatch: expected 0x{:08X}, calculated 0x{:08X}",
                message.data_crc32,
                AdbMessage::calculate_crc32(&payload)
            )));
        }

        Ok(Some((Self { message, payload }, total_frame_len)))
    }
}

/// Packet construction helpers for standard ADB multiplexing states.
pub mod helpers {
    use super::*;

    /// Constructs an `A_CNXN` initial connection handshake packet.
    pub fn build_cnxn(max_payload: u32, banner: &str) -> AdbPacket {
        let payload = banner.as_bytes().to_vec();
        AdbPacket::new(A_CNXN, A_VERSION_SKIP_CHECKSUM, max_payload, payload)
    }

    /// Constructs an `A_OPEN` request to open a remote service stream (e.g. `sync:` or `shell:`).
    pub fn build_open(local_id: u32, destination: &str) -> AdbPacket {
        let mut payload = destination.as_bytes().to_vec();
        // Null termination is standard for destination service strings
        if !payload.ends_with(&[0]) {
            payload.push(0);
        }
        AdbPacket::new(A_OPEN, local_id, 0, payload)
    }

    /// Constructs an `A_OKAY` packet acknowledging a stream transition.
    pub fn build_okay(local_id: u32, remote_id: u32) -> AdbPacket {
        AdbPacket::new(A_OKAY, local_id, remote_id, Vec::new())
    }

    /// Constructs an `A_CLSE` packet closing a stream session.
    pub fn build_close(local_id: u32, remote_id: u32) -> AdbPacket {
        AdbPacket::new(A_CLSE, local_id, remote_id, Vec::new())
    }

    /// Constructs an `A_WRTE` packet carrying stream payload data.
    pub fn build_write(local_id: u32, remote_id: u32, payload: Vec<u8>) -> AdbPacket {
        AdbPacket::new(A_WRTE, local_id, remote_id, payload)
    }
}
