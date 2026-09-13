// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ADB SYNC streaming subprotocol framing and MediaScanner index synchronization.
//!
//! Provides binary codecs for directory traversal (`LIST` / `DENT` / `DONE`),
//! file attributes (`STAT`), payload transfer (`RECV` / `SEND` / `DATA`), and
//! Android Binder `content call` triggers to refresh gallery indices.

use crate::error::DeviceError;

/// 4-byte little-endian SYNC subprotocol opcode constants.
pub const SYNC_LIST: [u8; 4] = *b"LIST"; // 0x5453494C
pub const SYNC_DENT: [u8; 4] = *b"DENT"; // 0x544E4544
pub const SYNC_DONE: [u8; 4] = *b"DONE"; // 0x454E4F44
pub const SYNC_STAT: [u8; 4] = *b"STAT"; // 0x54415453
pub const SYNC_RECV: [u8; 4] = *b"RECV"; // 0x56434552
pub const SYNC_DATA: [u8; 4] = *b"DATA"; // 0x41544144
pub const SYNC_SEND: [u8; 4] = *b"SEND"; // 0x444E4553
pub const SYNC_QUIT: [u8; 4] = *b"QUIT"; // 0x54495551
pub const SYNC_FAIL: [u8; 4] = *b"FAIL"; // 0x4C494146

/// POSIX file mode bitmask definitions.
pub const S_IFMT: u32 = 0o170000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFLNK: u32 = 0o120000;

/// Default micro-buffer chunk size for SYNC stream transfers (Stream-First Invariant).
pub const SYNC_CHUNK_SIZE: usize = 64 * 1024;

/// Represents a single directory entry parsed from an ADB SYNC `DENT` stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDentEntry {
    /// POSIX file mode including permissions and type bits.
    pub mode: u32,
    /// File size in bytes.
    pub size: u32,
    /// Last modification timestamp (seconds since Unix epoch).
    pub mtime: u32,
    /// Entry basename string.
    pub name: String,
}

impl SyncDentEntry {
    /// Returns true if this entry represents a directory.
    pub fn is_dir(&self) -> bool {
        (self.mode & S_IFMT) == S_IFDIR
    }

    /// Returns true if this entry represents a regular file.
    pub fn is_file(&self) -> bool {
        (self.mode & S_IFMT) == S_IFREG
    }

    /// Returns true if this entry is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        (self.mode & S_IFMT) == S_IFLNK
    }

    /// Returns the POSIX permission bits (lower 9 bits: `rwxrwxrwx`).
    pub fn permissions(&self) -> u32 {
        self.mode & 0o777
    }
}

/// Represents the response payload of a `STAT` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncStatResponse {
    /// POSIX file mode (0 if the target does not exist).
    pub mode: u32,
    /// File size in bytes.
    pub size: u32,
    /// Last modification timestamp.
    pub mtime: u32,
}

impl SyncStatResponse {
    /// Returns true if the target exists on the device.
    pub fn exists(&self) -> bool {
        self.mode != 0
    }

    /// Returns true if the target exists and is a directory.
    pub fn is_dir(&self) -> bool {
        self.exists() && ((self.mode & S_IFMT) == S_IFDIR)
    }

    /// Returns true if the target exists and is a regular file.
    pub fn is_file(&self) -> bool {
        self.exists() && ((self.mode & S_IFMT) == S_IFREG)
    }
}

/// Encoders for constructing ADB SYNC subprotocol requests.
pub mod encoder {
    use super::*;

    /// Constructs a `LIST <length> <path>` request.
    pub fn build_list_request(path: &str) -> Vec<u8> {
        let path_bytes = path.as_bytes();
        let mut out = Vec::with_capacity(8 + path_bytes.len());
        out.extend_from_slice(&SYNC_LIST);
        out.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(path_bytes);
        out
    }

    /// Constructs a `STAT <length> <path>` request.
    pub fn build_stat_request(path: &str) -> Vec<u8> {
        let path_bytes = path.as_bytes();
        let mut out = Vec::with_capacity(8 + path_bytes.len());
        out.extend_from_slice(&SYNC_STAT);
        out.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(path_bytes);
        out
    }

    /// Constructs a `RECV <length> <path>` download request.
    pub fn build_recv_request(path: &str) -> Vec<u8> {
        let path_bytes = path.as_bytes();
        let mut out = Vec::with_capacity(8 + path_bytes.len());
        out.extend_from_slice(&SYNC_RECV);
        out.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(path_bytes);
        out
    }

    /// Constructs a `SEND <length> <path,mode>` upload request header.
    pub fn build_send_header(destination_path: &str, mode: u32) -> Vec<u8> {
        let descriptor = format!("{},{}", destination_path, mode);
        let desc_bytes = descriptor.as_bytes();
        let mut out = Vec::with_capacity(8 + desc_bytes.len());
        out.extend_from_slice(&SYNC_SEND);
        out.extend_from_slice(&(desc_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(desc_bytes);
        out
    }

    /// Constructs a `DATA <chunk_len> <bytes>` upload chunk.
    pub fn build_data_chunk(chunk: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + chunk.len());
        out.extend_from_slice(&SYNC_DATA);
        out.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(chunk);
        out
    }

    /// Constructs a `DONE <mtime>` finalization message for `SEND`.
    pub fn build_done_request(mtime: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&SYNC_DONE);
        out.extend_from_slice(&mtime.to_le_bytes());
        out
    }

    /// Constructs a `QUIT <0>` request terminating the SYNC session.
    pub fn build_quit_request() -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&SYNC_QUIT);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }
}

/// Decoders for parsing responses emitted by the device `adbd` SYNC service.
pub mod decoder {
    use super::*;

    /// Decodes a 16-byte `STAT` response into `SyncStatResponse`.
    ///
    /// Expected format: `STAT` (4 bytes) + `mode` (4 bytes LE) + `size` (4 bytes LE) + `mtime` (4 bytes LE).
    pub fn decode_stat_response(buf: &[u8]) -> Result<SyncStatResponse, DeviceError> {
        if buf.len() < 16 {
            return Err(DeviceError::ProtocolError(format!(
                "Incomplete STAT response buffer: {} bytes (required 16)",
                buf.len()
            )));
        }

        if buf[0..4] != SYNC_STAT {
            return Err(DeviceError::ProtocolError(format!(
                "Invalid STAT opcode: {:?}",
                &buf[0..4]
            )));
        }

        let mode = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let mtime = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);

        Ok(SyncStatResponse { mode, size, mtime })
    }

    /// Parses a single `DENT` entry from the stream cursor.
    ///
    /// Expected format: `DENT` (4 bytes) + `mode` (4) + `size` (4) + `mtime` (4) + `namelen` (4) + `name` (namelen).
    /// Returns `Ok(Some((entry, bytes_consumed)))`, `Ok(None)` if `DONE` is reached, or `Err`.
    pub fn decode_dent_entry(buf: &[u8]) -> Result<Option<(SyncDentEntry, usize)>, DeviceError> {
        if buf.len() < 4 {
            return Err(DeviceError::ProtocolError("Buffer too short for SYNC opcode".into()));
        }

        let opcode = &buf[0..4];
        if opcode == SYNC_DONE {
            // Reached end of directory listing (DONE + 12 trailing bytes)
            let _consumed = if buf.len() >= 16 { 16 } else { 4 };
            return Ok(None);
        }

        if opcode == SYNC_FAIL {
            let msg_len = if buf.len() >= 8 {
                u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize
            } else {
                0
            };
            let err_text = if buf.len() >= 8 + msg_len {
                String::from_utf8_lossy(&buf[8..8 + msg_len]).to_string()
            } else {
                "Unknown SYNC failure".to_string()
            };
            return Err(DeviceError::ProtocolError(format!("ADB SYNC FAIL: {}", err_text)));
        }

        if opcode != SYNC_DENT {
            return Err(DeviceError::ProtocolError(format!(
                "Expected DENT or DONE opcode, got: {:?}",
                opcode
            )));
        }

        if buf.len() < 20 {
            return Err(DeviceError::ProtocolError("Incomplete DENT header".into()));
        }

        let mode = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let mtime = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let name_len = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]) as usize;

        let total_entry_len = 20 + name_len;
        if buf.len() < total_entry_len {
            return Err(DeviceError::ProtocolError(format!(
                "Incomplete DENT filename: need {} bytes, buffer has {}",
                total_entry_len,
                buf.len()
            )));
        }

        let name = String::from_utf8_lossy(&buf[20..20 + name_len]).to_string();

        Ok(Some((
            SyncDentEntry {
                mode,
                size,
                mtime,
                name,
            },
            total_entry_len,
        )))
    }

    /// Continuously parses a stream of contiguous `DENT` entries until `DONE` is reached.
    pub fn parse_dent_stream(stream_bytes: &[u8]) -> Result<Vec<SyncDentEntry>, DeviceError> {
        let mut entries = Vec::new();
        let mut cursor = 0;

        while cursor < stream_bytes.len() {
            let remaining = &stream_bytes[cursor..];
            match decode_dent_entry(remaining)? {
                Some((entry, consumed)) => {
                    // Filter out POSIX '.' and '..' pseudo-directories
                    if entry.name != "." && entry.name != ".." {
                        entries.push(entry);
                    }
                    cursor += consumed;
                }
                None => {
                    // Reached DONE opcode
                    break;
                }
            }
        }

        Ok(entries)
    }
}

/// Android MediaScanner synchronization helpers.
pub mod media_scanner {
    /// Formats the direct ContentProvider Binder command for immediate media indexing.
    ///
    /// Executes `content call --uri content://media/ --method scan_file --arg <path>`
    /// which triggers synchronous index ingestion in Android 10+ without deprecated broadcasts.
    pub fn format_media_scan_command(absolute_path: &str) -> String {
        // Sanitize path to prevent shell command injection
        let escaped_path = absolute_path.replace('"', "\\\"");
        format!(
            "content call --uri content://media/ --method scan_file --arg \"{}\"",
            escaped_path
        )
    }

    /// Formats the ADB shell protocol service destination for direct execution.
    pub fn build_media_scan_shell_request(absolute_path: &str) -> String {
        let cmd = format_media_scan_command(absolute_path);
        format!("shell:exec {}", cmd)
    }
}
