// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! POSIX USTAR path split/join and GNU TAR `././@LongLink` extension stream manager.
//!
//! Provides intelligent 155-byte USTAR path splitting, safe prefix/name joining,
//! GNU Type 'L' (LongName) and Type 'K' (LongLink) header formatting with 512-byte
//! block alignment, and streaming state machine injection with strict anti-tampering bounds.

use super::header::{
    parse_null_trimmed_str, TarHeader, MAGIC_GNU, TAR_BLOCK_SIZE, TYPE_GNU_LONGLINK,
    TYPE_GNU_LONGNAME,
};
use crate::types::TTZipStatus;
use std::fmt;

/// Maximum length of a POSIX USTAR filename field (100 bytes).
pub const NAME_LEN: usize = 100;

/// Maximum length of a POSIX USTAR prefix field (155 bytes).
pub const USTAR_PREFIX_LEN: usize = 155;

/// Maximum representable path in standard USTAR without GNU/PAX extensions (155 + 1 + 100 = 256 bytes).
pub const USTAR_MAX_PATH_LEN: usize = USTAR_PREFIX_LEN + 1 + NAME_LEN;

/// Standard GNU LongLink special header name identifier.
pub const GNU_LONGLINK_NAME: &str = "././@LongLink";

/// Maximum permitted payload size for a GNU LongLink block (64 MiB security limit).
pub const DEFAULT_MAX_GNU_PAYLOAD_SIZE: u64 = 64 * 1024 * 1024;

/// Errors arising during USTAR path processing or GNU LongLink streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarGnuError {
    /// Path exceeds the maximum permitted bounds.
    PathTooLong { length: usize, max: usize },
    /// Invalid UTF-8 encoding encountered in path or link target.
    InvalidUtf8(String),
    /// GNU LongLink payload is missing the terminating NUL byte.
    MissingNulTerminator,
    /// GNU LongLink payload size exceeds the maximum security budget.
    PayloadTooLarge { size: u64, max: u64 },
    /// Path is empty or contains only invalid characters.
    EmptyPath,
    /// Header or payload structure is malformed.
    MalformedLongLink(String),
    /// State machine encountered corrupted or unexpected state.
    CorruptedState(String),
}

impl fmt::Display for TarGnuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathTooLong { length, max } => {
                write!(f, "path length {} exceeds maximum allowed {}", length, max)
            }
            Self::InvalidUtf8(msg) => write!(f, "invalid UTF-8 sequence: {}", msg),
            Self::MissingNulTerminator => {
                write!(f, "GNU LongLink payload missing required NUL terminator")
            }
            Self::PayloadTooLarge { size, max } => {
                write!(f, "payload size {} exceeds maximum budget {}", size, max)
            }
            Self::EmptyPath => write!(f, "path cannot be empty"),
            Self::MalformedLongLink(msg) => write!(f, "malformed GNU LongLink: {}", msg),
            Self::CorruptedState(msg) => write!(f, "corrupted LongLink state: {}", msg),
        }
    }
}

impl std::error::Error for TarGnuError {}

impl From<TarGnuError> for TTZipStatus {
    fn from(err: TarGnuError) -> Self {
        match err {
            TarGnuError::PathTooLong { .. } => TTZipStatus::ErrPathTooLong,
            TarGnuError::InvalidUtf8(_) => TTZipStatus::ErrInvalidParam,
            TarGnuError::MissingNulTerminator => TTZipStatus::ErrCorruptHeader,
            TarGnuError::PayloadTooLarge { .. } => TTZipStatus::ErrCorruptHeader,
            TarGnuError::EmptyPath => TTZipStatus::ErrInvalidParam,
            TarGnuError::MalformedLongLink(_) => TTZipStatus::ErrCorruptHeader,
            TarGnuError::CorruptedState(_) => TTZipStatus::ErrCorruptHeader,
        }
    }
}

/// Splits a file path into POSIX USTAR `(prefix, name)` components.
///
/// POSIX.1-1988 USTAR limits filenames to 100 bytes and prefix to 155 bytes,
/// separated by a slash `/` (which is not stored in either field).
///
/// Returns:
/// - `Some(("", path))` if `path.len() <= 100` (fits entirely in the name field).
/// - `Some((prefix, name))` if `path` can be split at a `/` such that `prefix.len() <= 155`
///   and `name.len() <= 100` (both non-empty).
/// - `None` if `path` cannot be represented in USTAR format without GNU `@LongLink` or PAX headers.
pub fn split_ustar_path(path: &str) -> Option<(&str, &str)> {
    let bytes = path.as_bytes();
    if bytes.len() <= NAME_LEN {
        return Some(("", path));
    }
    if bytes.len() > USTAR_MAX_PATH_LEN {
        return None;
    }

    // Prefix must be 1..=155 bytes, name must be 1..=100 bytes.
    // The separator '/' sits at index `idx`, so prefix is `path[..idx]` (length idx)
    // and name is `path[idx + 1..]` (length path.len() - idx - 1).
    let min_idx = 1.max(bytes.len().saturating_sub(NAME_LEN + 1));
    let max_idx = USTAR_PREFIX_LEN.min(bytes.len().saturating_sub(2));

    if min_idx > max_idx {
        return None;
    }

    // Intelligently scan from max_idx down to min_idx to favor placing more path in prefix
    for idx in (min_idx..=max_idx).rev() {
        if bytes[idx] == b'/' {
            let prefix = &path[..idx];
            let name = &path[idx + 1..];
            if !prefix.is_empty()
                && prefix.len() <= USTAR_PREFIX_LEN
                && !name.is_empty()
                && name.len() <= NAME_LEN
            {
                return Some((prefix, name));
            }
        }
    }

    None
}

/// Safely joins POSIX USTAR prefix and name byte slices into a normalized UTF-8 path string.
///
/// Strips trailing NUL bytes from fields and verifies UTF-8 validity.
pub fn join_ustar_path(prefix: &[u8], name: &[u8]) -> Result<String, TarGnuError> {
    let prefix_str = parse_null_trimmed_str(prefix);
    let name_str = parse_null_trimmed_str(name);

    if prefix_str.is_empty() && name_str.is_empty() {
        return Err(TarGnuError::EmptyPath);
    }

    if prefix_str.is_empty() {
        return Ok(name_str.to_string());
    }

    if name_str.is_empty() {
        return Ok(prefix_str.to_string());
    }

    if prefix_str.ends_with('/') {
        Ok(format!("{}{}", prefix_str, name_str))
    } else {
        Ok(format!("{}/{}", prefix_str, name_str))
    }
}

/// GNU TAR LongName and LongLink header & payload builder.
pub struct GnuLongLinkManager;

impl GnuLongLinkManager {
    /// Formats a GNU `@LongLink` header (Type 'L') and 512-byte aligned payload for an extended file path.
    pub fn format_long_name_header(name: &str) -> (TarHeader, Vec<u8>) {
        Self::format_gnu_extension_block(name, TYPE_GNU_LONGNAME)
    }

    /// Formats a GNU `@LongLink` header (Type 'K') and 512-byte aligned payload for an extended symlink/hardlink target.
    pub fn format_long_link_header(target: &str) -> (TarHeader, Vec<u8>) {
        Self::format_gnu_extension_block(target, TYPE_GNU_LONGLINK)
    }

    /// Internal generator for Type 'L' and Type 'K' GNU extension records.
    fn format_gnu_extension_block(text: &str, typeflag: u8) -> (TarHeader, Vec<u8>) {
        let text_bytes = text.as_bytes();
        let payload_size = (text_bytes.len() + 1) as u64; // Include terminating NUL

        let header = TarHeader {
            name: GNU_LONGLINK_NAME.to_string(),
            mode: 0,
            uid: 0,
            gid: 0,
            size: payload_size,
            mtime: 0,
            chksum: 0,
            typeflag,
            linkname: String::new(),
            magic: *MAGIC_GNU,
            version: *b"  ",
            uname: String::new(),
            gname: String::new(),
            devmajor: 0,
            devminor: 0,
            prefix: String::new(),
        };

        // Construct 512-byte aligned payload
        let mut payload = Vec::with_capacity(text_bytes.len() + 1 + TAR_BLOCK_SIZE);
        payload.extend_from_slice(text_bytes);
        payload.push(0); // NUL terminator

        let remainder = payload.len() % TAR_BLOCK_SIZE;
        if remainder != 0 {
            let padding = TAR_BLOCK_SIZE - remainder;
            payload.resize(payload.len() + padding, 0);
        }

        (header, payload)
    }

    /// Parses and validates a raw GNU LongLink payload slice against safety limits.
    pub fn parse_gnu_payload(
        payload: &[u8],
        expected_size: u64,
        max_payload_size: u64,
    ) -> Result<String, TarGnuError> {
        if expected_size > max_payload_size {
            return Err(TarGnuError::PayloadTooLarge {
                size: expected_size,
                max: max_payload_size,
            });
        }

        let size_usize = expected_size as usize;
        if payload.len() < size_usize {
            return Err(TarGnuError::MalformedLongLink(format!(
                "payload slice length {} is smaller than expected size {}",
                payload.len(),
                expected_size
            )));
        }

        let data = &payload[..size_usize];
        if data.is_empty() || !data.contains(&0) {
            return Err(TarGnuError::MissingNulTerminator);
        }

        let nul_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        let str_slice = &data[..nul_pos];

        std::str::from_utf8(str_slice)
            .map(|s| s.to_string())
            .map_err(|e| TarGnuError::InvalidUtf8(e.to_string()))
    }
}

/// Streaming accumulator state for GNU LongName ('L') and LongLink ('K') extensions.
#[derive(Debug, Clone)]
pub struct GnuLongLinkState {
    pending_long_name: Option<String>,
    pending_long_link: Option<String>,
    max_payload_size: u64,
}

impl Default for GnuLongLinkState {
    fn default() -> Self {
        Self::new()
    }
}

impl GnuLongLinkState {
    /// Creates a new state manager with default security payload budget (64 MiB).
    pub fn new() -> Self {
        Self {
            pending_long_name: None,
            pending_long_link: None,
            max_payload_size: DEFAULT_MAX_GNU_PAYLOAD_SIZE,
        }
    }

    /// Creates a new state manager with custom payload size limits.
    pub fn with_max_payload_size(max_payload_size: u64) -> Self {
        Self {
            pending_long_name: None,
            pending_long_link: None,
            max_payload_size,
        }
    }

    /// Consumes a TAR entry header and associated payload.
    ///
    /// Returns:
    /// - `Ok(true)` if the entry was a GNU LongName or LongLink extension record (consumed by state).
    /// - `Ok(false)` if the entry is a normal archive entry.
    /// - `Err(TarGnuError)` if the GNU payload was malformed or breached safety limits.
    pub fn feed_header(&mut self, header: &TarHeader, payload: &[u8]) -> Result<bool, TarGnuError> {
        match header.typeflag {
            TYPE_GNU_LONGNAME => {
                let parsed = GnuLongLinkManager::parse_gnu_payload(
                    payload,
                    header.size,
                    self.max_payload_size,
                )?;
                self.pending_long_name = Some(parsed);
                Ok(true)
            }
            TYPE_GNU_LONGLINK => {
                let parsed = GnuLongLinkManager::parse_gnu_payload(
                    payload,
                    header.size,
                    self.max_payload_size,
                )?;
                self.pending_long_link = Some(parsed);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Applies any pending long name or long link target to the given entity header and resets state.
    pub fn apply_to_entry(&mut self, header: &mut TarHeader) {
        if let Some(name) = self.pending_long_name.take() {
            header.name = name;
        }
        if let Some(link) = self.pending_long_link.take() {
            header.linkname = link;
        }
    }

    /// Takes the pending long file name, if present.
    pub fn take_long_name(&mut self) -> Option<String> {
        self.pending_long_name.take()
    }

    /// Takes the pending long link target, if present.
    pub fn take_long_link(&mut self) -> Option<String> {
        self.pending_long_link.take()
    }

    /// Returns `true` if there are unconsumed pending GNU extension headers.
    pub fn has_pending(&self) -> bool {
        self.pending_long_name.is_some() || self.pending_long_link.is_some()
    }

    /// Clears any accumulated state.
    pub fn clear(&mut self) {
        self.pending_long_name = None;
        self.pending_long_link = None;
    }
}
