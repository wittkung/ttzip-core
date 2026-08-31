// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! POSIX.1-2001 PAX Extended Header Parsing, Formatting, and Nanosecond Timestamps.
//!
//! Provides:
//! 1. `PaxZeroScanner<'a>` and `PaxZeroEntry<'a>`: Zero-heap-allocation streaming iterator
//!    over contiguous raw `&'a [u8]` memory slices.
//! 2. `PaxRecord`: Owned key-value pair for arbitrary extended metadata.
//! 3. `format_pax_record`: Fixed-point $O(1)$ self-consistent variable-length record encoder.
//! 4. `parse_pax_time` & `format_pax_time`: Nanosecond-precision Unix timestamp parser and formatter.
//! 5. `PaxExtensionMap`: Type-safe attribute container for standard PAX keywords.
//! 6. GHSA-3cv2-h65g-fgmm Size Isolation: Guarantees PAX `size` takes strict precedence over header size.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Write;
use std::str;
use thiserror::Error;

use super::header::TarHeader;

/// Maximum allowed single PAX record size (2 MB) to prevent allocation denial-of-service.
pub const MAX_PAX_RECORD_SIZE: usize = 2 * 1024 * 1024;

/// Errors encountered during POSIX.1-2001 PAX extended header parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TarPaxError {
    /// Input buffer ends prematurely before reading declared record length.
    #[error("truncated PAX record: declared length {expected} bytes, but only {available} bytes available")]
    TruncatedRecord { expected: usize, available: usize },

    /// Missing ASCII space (' ') delimiter after the decimal length field.
    #[error("missing space delimiter separating length from key=value in PAX record")]
    MissingSpaceDelimiter,

    /// Length field could not be parsed as a valid base-10 positive integer.
    #[error("invalid decimal length field in PAX record: '{raw}'")]
    InvalidLength { raw: String },

    /// Declared length is too small to contain length digits, space, key, '=', and newline.
    #[error("PAX record declared length {length} is smaller than prefix length {prefix_len}")]
    LengthTooSmall { length: usize, prefix_len: usize },

    /// Missing ASCII equal sign ('=') separating keyword and value.
    #[error("missing '=' delimiter separating key and value in PAX record")]
    MissingEqualDelimiter,

    /// Missing mandatory trailing ASCII newline ('\n') character.
    #[error("missing mandatory trailing newline ('\\n') delimiter in PAX record")]
    MissingNewlineDelimiter,

    /// Keyword contains invalid UTF-8 byte sequences.
    #[error("invalid UTF-8 sequence in PAX keyword: {reason}")]
    InvalidUtf8Key { reason: String },

    /// Declared record size exceeds the maximum security threshold.
    #[error("PAX record size {size} exceeds maximum allowable threshold {max}")]
    RecordTooLarge { size: usize, max: usize },
}

/// Zero-heap-allocation view into a single PAX record inside a borrowed byte slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaxZeroEntry<'a> {
    /// Total byte length of the record including length digits and trailing `\n`.
    pub total_len: usize,
    /// UTF-8 keyword slice.
    pub key: &'a str,
    /// Raw unescaped value byte slice.
    pub value: &'a [u8],
}

impl<'a> PaxZeroEntry<'a> {
    /// Attempts to interpret the raw value slice as a valid UTF-8 string slice.
    #[inline]
    pub fn value_str(&self) -> Result<&'a str, str::Utf8Error> {
        str::from_utf8(self.value)
    }

    /// Converts the raw value slice into a `Cow<'a, str>` with lossy UTF-8 replacement.
    #[inline]
    pub fn value_lossy(&self) -> Cow<'a, str> {
        String::from_utf8_lossy(self.value)
    }

    /// Converts this borrowed zero-copy entry into an owned `PaxRecord`.
    #[inline]
    pub fn to_record(&self) -> PaxRecord {
        PaxRecord {
            key: self.key.to_string(),
            value: self.value.to_vec(),
        }
    }
}

/// Owned POSIX.1-2001 PAX key-value record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaxRecord {
    /// Metadata keyword (e.g., `path`, `linkpath`, `size`, `mtime`).
    pub key: String,
    /// Arbitrary raw value bytes.
    pub value: Vec<u8>,
}

impl PaxRecord {
    /// Constructs a new `PaxRecord` from key and value.
    #[inline]
    pub fn new(key: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Attempts to interpret the record's value as a UTF-8 string.
    #[inline]
    pub fn value_str(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.value)
    }

    /// Returns the record's value as a `Cow<str>` with lossy UTF-8 replacement.
    #[inline]
    pub fn value_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.value)
    }

    /// Formats this record into canonical POSIX PAX `"<length> <key>=<value>\n"` byte representation.
    #[inline]
    pub fn format(&self) -> Vec<u8> {
        format_pax_record(&self.key, &self.value)
    }
}

/// Streaming zero-heap-allocation scanner for POSIX.1-2001 PAX extended headers.
///
/// Iterates over a raw byte slice and yields `PaxZeroEntry<'a>` items without any heap allocation.
/// Gracefully handles trailing TAR 512-byte zero padding by stopping at the first NUL byte.
#[derive(Debug, Clone)]
pub struct PaxZeroScanner<'a> {
    remaining: &'a [u8],
}

impl<'a> PaxZeroScanner<'a> {
    /// Creates a new `PaxZeroScanner` borrowing the supplied byte slice.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { remaining: data }
    }

    /// Returns the remaining unparsed byte slice.
    #[inline]
    pub fn remaining(&self) -> &'a [u8] {
        self.remaining
    }
}

impl<'a> Iterator for PaxZeroScanner<'a> {
    type Item = Result<PaxZeroEntry<'a>, TarPaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Fast-path: Check for empty input or standard TAR 512-byte zero sector padding
        if self.remaining.is_empty() || self.remaining[0] == 0 {
            return None;
        }

        // 1. Locate the first ASCII space (' ') delimiter separating length from key
        let space_pos = match self.remaining.iter().position(|&b| b == b' ') {
            Some(pos) => pos,
            None => {
                // If trailing bytes are all zeros, terminate cleanly; otherwise error
                if self.remaining.iter().all(|&b| b == 0) {
                    return None;
                }
                return Some(Err(TarPaxError::MissingSpaceDelimiter));
            }
        };

        // 2. Parse the decimal length string
        let len_slice = &self.remaining[..space_pos];
        let len_str = match str::from_utf8(len_slice) {
            Ok(s) => s.trim(),
            Err(_) => {
                return Some(Err(TarPaxError::InvalidLength {
                    raw: String::from_utf8_lossy(len_slice).into_owned(),
                }));
            }
        };

        let record_len: usize = match len_str.parse() {
            Ok(n) if n > 0 => n,
            _ => {
                return Some(Err(TarPaxError::InvalidLength {
                    raw: len_str.to_string(),
                }));
            }
        };

        // 3. Security bound check against oversized records
        if record_len > MAX_PAX_RECORD_SIZE {
            return Some(Err(TarPaxError::RecordTooLarge {
                size: record_len,
                max: MAX_PAX_RECORD_SIZE,
            }));
        }

        // 4. Verify buffer sufficiency
        if record_len > self.remaining.len() {
            return Some(Err(TarPaxError::TruncatedRecord {
                expected: record_len,
                available: self.remaining.len(),
            }));
        }

        // Minimum valid record is "<len> k=\n" -> at least space_pos + 1 (space) + 1 (key) + 1 (=) + 1 (\n)
        let min_len = space_pos + 4;
        if record_len < min_len {
            return Some(Err(TarPaxError::LengthTooSmall {
                length: record_len,
                prefix_len: min_len,
            }));
        }

        let record_bytes = &self.remaining[..record_len];

        // 5. Verify mandatory trailing newline delimiter ('\n')
        if record_bytes[record_len - 1] != b'\n' {
            return Some(Err(TarPaxError::MissingNewlineDelimiter));
        }

        // 6. Slice between '<length> ' and trailing '\n'
        let kv_bytes = &record_bytes[space_pos + 1..record_len - 1];

        // 7. Locate '=' delimiter
        let eq_pos = match kv_bytes.iter().position(|&b| b == b'=') {
            Some(pos) => pos,
            None => return Some(Err(TarPaxError::MissingEqualDelimiter)),
        };

        let key_bytes = &kv_bytes[..eq_pos];
        let val_bytes = &kv_bytes[eq_pos + 1..];

        // 8. Validate UTF-8 keyword
        let key = match str::from_utf8(key_bytes) {
            Ok(k) => k,
            Err(e) => {
                return Some(Err(TarPaxError::InvalidUtf8Key {
                    reason: e.to_string(),
                }));
            }
        };

        // Advance internal cursor past consumed record
        self.remaining = &self.remaining[record_len..];

        Some(Ok(PaxZeroEntry {
            total_len: record_len,
            key,
            value: val_bytes,
        }))
    }
}

/// Parses all PAX records from a raw byte slice into a `Vec<PaxRecord>`.
pub fn parse_pax_records(data: &[u8]) -> Result<Vec<PaxRecord>, TarPaxError> {
    let mut records = Vec::new();
    for item in PaxZeroScanner::new(data) {
        records.push(item?.to_record());
    }
    Ok(records)
}

/// Returns the number of decimal digits required to represent a non-negative integer `n`.
#[inline]
pub const fn decimal_digits(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    while n > 0 {
        count += 1;
        n /= 10;
    }
    count
}

/// Computes the exact self-consistent total length of a PAX record in $O(1)$ fixed-point iterations.
///
/// Formula: `total_len = digits(total_len) + 1 (space) + key_len + 1 (=) + val_len + 1 (\n)`.
#[inline]
pub fn compute_pax_record_len(key_len: usize, val_len: usize) -> usize {
    // rest_len = ' ' (1) + key + '=' (1) + value + '\n' (1)
    let rest_len = key_len.saturating_add(val_len).saturating_add(3);
    let mut total_len = rest_len + 1;

    loop {
        let digits = decimal_digits(total_len);
        let needed = rest_len + digits;
        if needed == total_len {
            return total_len;
        }
        total_len = needed;
    }
}

/// Formats a single PAX record into a canonical byte vector: `"<length> <key>=<value>\n"`.
///
/// Solves the variable-length length prefix self-consistently.
pub fn format_pax_record(key: &str, value: &[u8]) -> Vec<u8> {
    let total_len = compute_pax_record_len(key.len(), value.len());
    let mut out = Vec::with_capacity(total_len);
    let _ = write!(&mut out, "{} {}=", total_len, key);
    out.extend_from_slice(value);
    out.push(b'\n');
    debug_assert_eq!(out.len(), total_len);
    out
}

/// Parses a POSIX PAX timestamp string formatted as `<seconds>[.<subseconds>]`.
///
/// Supports:
/// - Integer seconds: `"1700000000"` -> `(1700000000, 0)`
/// - Subsecond decimals with automatic right zero-padding: `"1700000000.5"` -> `(1700000000, 500000000)`
/// - Arbitrary subsecond precision (>9 digits truncated): `"1700000000.123456789999"` -> `(1700000000, 123456789)`
/// - Negative timestamps (pre-1970 / BCE): `"-100.5"` -> `(-100, 500000000)`
/// - Malformed or empty strings fallback safely to `(0, 0)` with zero panics.
pub fn parse_pax_time(val: &str) -> (i64, u32) {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return (0, 0);
    }

    if let Some((secs_str, frac_str)) = trimmed.split_once('.') {
        let secs = secs_str.parse::<i64>().unwrap_or(0);
        let mut nanos_buf = [b'0'; 9];
        let frac_bytes = frac_str.as_bytes();

        let copy_len = frac_bytes.len().min(9);
        let mut valid_digits = 0;
        for &b in &frac_bytes[..copy_len] {
            if b.is_ascii_digit() {
                nanos_buf[valid_digits] = b;
                valid_digits += 1;
            } else {
                break;
            }
        }

        let nanos_str = str::from_utf8(&nanos_buf).unwrap_or("0");
        let nanos = nanos_str.parse::<u32>().unwrap_or(0);
        (secs, nanos)
    } else {
        (trimmed.parse::<i64>().unwrap_or(0), 0)
    }
}

/// Formats a nanosecond Unix timestamp into a standard POSIX PAX string.
///
/// Formats as:
/// - `"{secs}"` when `nanos == 0`.
/// - `"{secs}.{nanos:09}"` when `nanos > 0`.
#[inline]
pub fn format_pax_time(secs: i64, nanos: u32) -> String {
    if nanos == 0 {
        format!("{}", secs)
    } else {
        let clamped_nanos = nanos % 1_000_000_000;
        format!("{}.{:09}", secs, clamped_nanos)
    }
}

/// Resolved entry metadata after applying PAX extension overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaxTarEntry {
    /// Fully resolved filename or directory path.
    pub path: String,
    /// Optional resolved symlink or hardlink target.
    pub linkpath: Option<String>,
    /// Exact file payload size in bytes.
    pub size: u64,
    /// POSIX file permission mode.
    pub mode: u32,
    /// Owner user ID (UID).
    pub uid: u64,
    /// Owner group ID (GID).
    pub gid: u64,
    /// Optional user name.
    pub uname: Option<String>,
    /// Optional group name.
    pub gname: Option<String>,
    /// Modification time seconds since Unix epoch.
    pub mtime_secs: i64,
    /// Modification time nanosecond fraction (0..999,999,999).
    pub mtime_nanos: u32,
    /// Optional access time `(seconds, nanoseconds)`.
    pub atime: Option<(i64, u32)>,
    /// Optional status change time `(seconds, nanoseconds)`.
    pub ctime: Option<(i64, u32)>,
    /// Header entry typeflag byte.
    pub typeflag: u8,
}

/// High-level container for POSIX.1-2001 PAX extended header metadata records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaxExtensionMap {
    /// Deterministically sorted mapping of metadata keywords to raw values.
    records: BTreeMap<String, Vec<u8>>,
}

impl PaxExtensionMap {
    /// Creates an empty `PaxExtensionMap`.
    #[inline]
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    /// Constructs a `PaxExtensionMap` from an iterable collection of `PaxRecord`s.
    ///
    /// Per POSIX specification, later records override earlier occurrences of identical keys.
    pub fn from_records(records: Vec<PaxRecord>) -> Self {
        let mut map = Self::new();
        for record in records {
            map.records.insert(record.key, record.value);
        }
        map
    }

    /// Parses a raw byte slice into a `PaxExtensionMap`.
    pub fn from_slice(data: &[u8]) -> Result<Self, TarPaxError> {
        let mut map = Self::new();
        for item in PaxZeroScanner::new(data) {
            let entry = item?;
            map.records.insert(entry.key.to_string(), entry.value.to_vec());
        }
        Ok(map)
    }

    /// Inserts a key-value record into the map.
    #[inline]
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Vec<u8>>) {
        self.records.insert(key.into(), value.into());
    }

    /// Inserts a UTF-8 string record into the map.
    #[inline]
    pub fn insert_str(&mut self, key: impl Into<String>, value: &str) {
        self.records.insert(key.into(), value.as_bytes().to_vec());
    }

    /// Returns a reference to the raw value slice associated with `key`.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.records.get(key).map(|v| v.as_slice())
    }

    /// Returns the value associated with `key` interpreted as a UTF-8 string slice.
    #[inline]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|bytes| str::from_utf8(bytes).ok())
    }

    /// Removes a key from the map, returning the previous value if present.
    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
        self.records.remove(key)
    }

    /// Checks whether the map contains the specified key.
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.records.contains_key(key)
    }

    /// Returns the number of distinct records in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if the map contains no records.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns an iterator over borrowed `(&str, &[u8])` key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.records.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    // --- Strongly-Typed Standard PAX Getters ---

    /// Extracts the extended file path (`"path"`).
    #[inline]
    pub fn path(&self) -> Option<&str> {
        self.get_str("path")
    }

    /// Extracts the extended link target path (`"linkpath"`).
    #[inline]
    pub fn linkpath(&self) -> Option<&str> {
        self.get_str("linkpath")
    }

    /// Extracts the 64-bit file payload size (`"size"`).
    #[inline]
    pub fn size(&self) -> Option<u64> {
        self.get_str("size").and_then(|s| s.parse::<u64>().ok())
    }

    /// Extracts the owner user ID (`"uid"`).
    #[inline]
    pub fn uid(&self) -> Option<u64> {
        self.get_str("uid").and_then(|s| s.parse::<u64>().ok())
    }

    /// Extracts the owner group ID (`"gid"`).
    #[inline]
    pub fn gid(&self) -> Option<u64> {
        self.get_str("gid").and_then(|s| s.parse::<u64>().ok())
    }

    /// Extracts the owner user name string (`"uname"`).
    #[inline]
    pub fn uname(&self) -> Option<&str> {
        self.get_str("uname")
    }

    /// Extracts the owner group name string (`"gname"`).
    #[inline]
    pub fn gname(&self) -> Option<&str> {
        self.get_str("gname")
    }

    /// Extracts the modification time with nanosecond precision (`"mtime"`).
    #[inline]
    pub fn mtime(&self) -> Option<(i64, u32)> {
        self.get_str("mtime").map(parse_pax_time)
    }

    /// Extracts the access time with nanosecond precision (`"atime"`).
    #[inline]
    pub fn atime(&self) -> Option<(i64, u32)> {
        self.get_str("atime").map(parse_pax_time)
    }

    /// Extracts the status change time with nanosecond precision (`"ctime"`).
    #[inline]
    pub fn ctime(&self) -> Option<(i64, u32)> {
        self.get_str("ctime").map(parse_pax_time)
    }

    // --- Strongly-Typed Standard PAX Setters ---

    /// Sets the extended path keyword (`"path"`).
    #[inline]
    pub fn set_path(&mut self, path: &str) {
        self.insert_str("path", path);
    }

    /// Sets the extended link target keyword (`"linkpath"`).
    #[inline]
    pub fn set_linkpath(&mut self, linkpath: &str) {
        self.insert_str("linkpath", linkpath);
    }

    /// Sets the extended size keyword (`"size"`).
    #[inline]
    pub fn set_size(&mut self, size: u64) {
        self.insert_str("size", &size.to_string());
    }

    /// Sets the extended user ID keyword (`"uid"`).
    #[inline]
    pub fn set_uid(&mut self, uid: u64) {
        self.insert_str("uid", &uid.to_string());
    }

    /// Sets the extended group ID keyword (`"gid"`).
    #[inline]
    pub fn set_gid(&mut self, gid: u64) {
        self.insert_str("gid", &gid.to_string());
    }

    /// Sets the extended user name keyword (`"uname"`).
    #[inline]
    pub fn set_uname(&mut self, uname: &str) {
        self.insert_str("uname", uname);
    }

    /// Sets the extended group name keyword (`"gname"`).
    #[inline]
    pub fn set_gname(&mut self, gname: &str) {
        self.insert_str("gname", gname);
    }

    /// Sets the modification time keyword (`"mtime"`).
    #[inline]
    pub fn set_mtime(&mut self, secs: i64, nanos: u32) {
        self.insert_str("mtime", &format_pax_time(secs, nanos));
    }

    /// Sets the access time keyword (`"atime"`).
    #[inline]
    pub fn set_atime(&mut self, secs: i64, nanos: u32) {
        self.insert_str("atime", &format_pax_time(secs, nanos));
    }

    /// Sets the status change time keyword (`"ctime"`).
    #[inline]
    pub fn set_ctime(&mut self, secs: i64, nanos: u32) {
        self.insert_str("ctime", &format_pax_time(secs, nanos));
    }

    /// Serializes all records in deterministic key order into contiguous PAX payload bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (key, val) in &self.records {
            out.extend_from_slice(&format_pax_record(key, val));
        }
        out
    }

    /// Applies PAX attributes onto a base 512-byte `TarHeader` in place.
    ///
    /// # GHSA-3cv2-h65g-fgmm Size Precedence:
    /// If PAX `size` is present, it strictly overrides the 512-byte header size field.
    pub fn apply_to_header(&self, header: &mut TarHeader) {
        if let Some(path) = self.path() {
            header.set_name(path);
        }
        if let Some(linkpath) = self.linkpath() {
            header.set_linkname(linkpath);
        }
        if let Some(size) = self.size() {
            header.set_size(size);
        }
        if let Some(uid) = self.uid() {
            header.set_uid(uid);
        }
        if let Some(gid) = self.gid() {
            header.set_gid(gid);
        }
        if let Some(uname) = self.uname() {
            header.set_uname(uname);
        }
        if let Some(gname) = self.gname() {
            header.set_gname(gname);
        }
        if let Some((secs, _)) = self.mtime() {
            if secs >= 0 {
                header.set_mtime(secs as u64);
            }
        }
        header.update_checksum();
    }

    /// Resolves full entry metadata by applying PAX overrides over a base `TarHeader`.
    ///
    /// # GHSA-3cv2-h65g-fgmm Security Invariant:
    /// PAX `size` strictly overrides the 512-byte header size. The header size is used
    /// only as a fallback if PAX `size` is absent.
    pub fn apply_to_entry(&self, header: &TarHeader) -> PaxTarEntry {
        let size = self.size().unwrap_or_else(|| header.size());
        let path = self.path().map(ToString::to_string).unwrap_or_else(|| {
            let prefix = header.prefix();
            let name = header.name();
            if !prefix.is_empty() {
                format!("{}/{}", prefix, name)
            } else {
                name.to_string()
            }
        });
        let linkpath = self.linkpath().map(ToString::to_string).or_else(|| {
            let l = header.linkname();
            if !l.is_empty() {
                Some(l.to_string())
            } else {
                None
            }
        });
        let uid = self.uid().unwrap_or_else(|| header.uid());
        let gid = self.gid().unwrap_or_else(|| header.gid());
        let uname = self.uname().map(ToString::to_string).or_else(|| {
            let u = header.uname();
            if !u.is_empty() {
                Some(u.to_string())
            } else {
                None
            }
        });
        let gname = self.gname().map(ToString::to_string).or_else(|| {
            let g = header.gname();
            if !g.is_empty() {
                Some(g.to_string())
            } else {
                None
            }
        });
        let (mtime_secs, mtime_nanos) = self
            .mtime()
            .unwrap_or_else(|| (header.mtime() as i64, 0));
        let atime = self.atime();
        let ctime = self.ctime();

        PaxTarEntry {
            path,
            linkpath,
            size,
            mode: header.mode(),
            uid,
            gid,
            uname,
            gname,
            mtime_secs,
            mtime_nanos,
            atime,
            ctime,
            typeflag: header.typeflag_byte(),
        }
    }
}
