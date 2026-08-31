// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7z Varint variable-length integer and Tar Pax negative timestamp security guard (`VarintPaxSecurityGuard`).
//!
//! Provides defense-in-depth against:
//! 1. 7z Varint Shift Overflow & Loop Exhaustion Attacks: Enforces a strict $\le 10$ byte limit
//!    and bit-shift boundary checks, preventing undefined bit-shifting and CPU denial of service.
//! 2. Tar Pax Timestamp Clamping: Sanitizes POSIX Pax extended headers containing negative timestamps
//!    (BCE or pre-1970 dates) or extreme fractional nanoseconds, preventing `std::time::SystemTime` panics.

use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Maximum allowed bytes for decoding a single 7z variable-length integer (Varint).
pub const MAX_7Z_VARINT_BYTES: usize = 10;

/// Maximum allowed bytes for a single Pax extended header record (64 KB).
pub const MAX_PAX_RECORD_SIZE: usize = 64 * 1024;

/// Maximum allowed number of records per Pax header block (10,000).
pub const MAX_PAX_RECORDS_PER_BLOCK: usize = 10_000;

/// Maximum safe Unix timestamp in seconds (Year 9999-12-31T23:59:59Z: 253,402,300,799).
pub const MAX_SAFE_PAX_SECONDS: i64 = 253_402_300_799;

/// Minimum safe Unix timestamp in seconds (1970-01-01T00:00:00Z: 0).
pub const MIN_SAFE_PAX_SECONDS: i64 = 0;

/// Errors detected during 7z Varint decoding and bounds enforcement.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VarintSecurityError {
    /// Varint decoding exceeded the maximum allowed byte length.
    #[error("7z Varint length violation: consumed {bytes_read} bytes, maximum allowed is {max_allowed}")]
    ByteLengthExceeded {
        bytes_read: usize,
        max_allowed: usize,
    },

    /// Bit shift overflow attempt detected during varint composition.
    #[error("7z Varint shift overflow detected at byte index {byte_index}: shift amount {shift_bits} >= 64 bits")]
    ShiftOverflow {
        byte_index: usize,
        shift_bits: usize,
    },

    /// Unexpected end of byte slice while reading varint continuation bytes.
    #[error("7z Varint truncated: expected {expected_bytes} follow-up bytes, only {available_bytes} available")]
    TruncatedInput {
        expected_bytes: usize,
        available_bytes: usize,
    },

    /// Varint encodes a non-canonical or malformed sequence.
    #[error("7z Varint malformed: {reason}")]
    MalformedVarint { reason: String },

    /// I/O error encountered while reading from stream.
    #[error("I/O error reading 7z Varint: {0}")]
    IoError(String),
}

/// Errors detected during Tar Pax extended header parsing and timestamp sanitization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaxSecurityError {
    /// Record declared length does not match actual length.
    #[error("Pax record length mismatch: declared {declared} bytes, parsed {actual} bytes")]
    RecordLengthMismatch { declared: usize, actual: usize },

    /// Individual Pax record exceeds the maximum allowed buffer size.
    #[error("Pax record too large: {size} bytes exceeds limit of {max_allowed} bytes")]
    RecordTooLarge { size: usize, max_allowed: usize },

    /// Total number of Pax records exceeds quota.
    #[error("Pax header records quota exceeded: {count} records > {max_allowed} limit")]
    RecordQuotaExceeded { count: usize, max_allowed: usize },

    /// Malformed Pax record format (missing space or '=' separator, or missing trailing newline).
    #[error("Malformed Pax record line: {reason}")]
    MalformedRecord { reason: String },

    /// Non-numeric or malformed timestamp format in Pax record value.
    #[error("Invalid Pax timestamp '{raw}': {reason}")]
    InvalidTimestamp { raw: String, reason: String },

    /// Invalid UTF-8 sequence in Pax record key or value.
    #[error("Pax record UTF-8 error: {reason}")]
    InvalidUtf8 { reason: String },
}

/// Sanitized and bounds-checked Tar Pax timestamp with nanosecond precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaxTimestamp {
    /// Raw parsed seconds (may be negative for pre-1970 dates).
    pub raw_secs: i64,
    /// Raw parsed nanoseconds (0..999,999,999).
    pub raw_nanos: u32,
    /// Clamped safe seconds for Unix SystemTime ($\ge 0$ and $\le \text{MAX\_SAFE\_PAX\_SECONDS}$).
    pub clamped_secs: u64,
    /// Sanitized nanoseconds (0..999,999,999).
    pub clamped_nanos: u32,
    /// True if the raw timestamp was negative (pre-1970 / BCE).
    pub was_negative: bool,
    /// True if the timestamp was clamped due to upper/lower bound overflow.
    pub was_clamped: bool,
}

impl PaxTimestamp {
    /// Converts the timestamp to a standard `std::time::SystemTime` without panicking.
    #[inline]
    pub fn to_system_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::new(self.clamped_secs, self.clamped_nanos)
    }

    /// Returns the timestamp as a floating-point seconds value.
    #[inline]
    pub fn as_secs_f64(&self) -> f64 {
        if self.was_negative {
            self.raw_secs as f64 - (self.raw_nanos as f64 / 1_000_000_000.0)
        } else {
            self.raw_secs as f64 + (self.raw_nanos as f64 / 1_000_000_000.0)
        }
    }
}

/// Decodes a 7z variable-length integer (NUMBER format) from a byte slice with strict overflow defense.
///
/// Returns `(decoded_value, bytes_consumed)`.
///
/// # Security Guarantees:
/// - Maximum bytes consumed is strictly $\le \text{MAX\_7Z\_VARINT\_BYTES}$ (10 bytes).
/// - Shift amounts are bounds-checked: shift bit operations strictly verify $8 \times i < 64$.
/// - Zero panics on arbitrary malicious inputs.
pub fn decode_7z_varint(bytes: &[u8]) -> Result<(u64, usize), VarintSecurityError> {
    if bytes.is_empty() {
        return Err(VarintSecurityError::TruncatedInput {
            expected_bytes: 1,
            available_bytes: 0,
        });
    }

    let first = bytes[0];
    let mut mask = 0x80u8;
    let mut value = 0u64;
    let mut bytes_consumed = 1;

    for i in 0..8 {
        if (first & mask) == 0 {
            let high_bits = (first & (mask - 1)) as u64;
            let shift = i * 8;
            if shift >= 64 {
                return Err(VarintSecurityError::ShiftOverflow {
                    byte_index: i,
                    shift_bits: shift,
                });
            }
            value |= high_bits << shift;
            return Ok((value, bytes_consumed));
        }

        if bytes_consumed >= bytes.len() {
            let needed = 8 - i;
            return Err(VarintSecurityError::TruncatedInput {
                expected_bytes: needed,
                available_bytes: bytes.len() - bytes_consumed,
            });
        }

        let next_byte = bytes[bytes_consumed] as u64;
        bytes_consumed += 1;

        if bytes_consumed > MAX_7Z_VARINT_BYTES {
            return Err(VarintSecurityError::ByteLengthExceeded {
                bytes_read: bytes_consumed,
                max_allowed: MAX_7Z_VARINT_BYTES,
            });
        }

        let shift = i * 8;
        if shift >= 64 {
            return Err(VarintSecurityError::ShiftOverflow {
                byte_index: i,
                shift_bits: shift,
            });
        }
        value |= next_byte << shift;
        mask >>= 1;
    }

    Ok((value, bytes_consumed))
}

/// Decodes a 7z variable-length integer directly from a streaming reader with strict byte limits.
pub fn decode_7z_varint_from_reader<R: Read>(
    reader: &mut R,
) -> Result<(u64, usize), VarintSecurityError> {
    let mut first = [0u8; 1];
    reader
        .read_exact(&mut first)
        .map_err(|e| VarintSecurityError::IoError(e.to_string()))?;

    let first_byte = first[0];
    let mut mask = 0x80u8;
    let mut value = 0u64;
    let mut bytes_consumed = 1;

    for i in 0..8 {
        if (first_byte & mask) == 0 {
            let high_bits = (first_byte & (mask - 1)) as u64;
            let shift = i * 8;
            if shift >= 64 {
                return Err(VarintSecurityError::ShiftOverflow {
                    byte_index: i,
                    shift_bits: shift,
                });
            }
            value |= high_bits << shift;
            return Ok((value, bytes_consumed));
        }

        let mut next_byte = [0u8; 1];
        reader
            .read_exact(&mut next_byte)
            .map_err(|e| VarintSecurityError::IoError(e.to_string()))?;
        bytes_consumed += 1;

        if bytes_consumed > MAX_7Z_VARINT_BYTES {
            return Err(VarintSecurityError::ByteLengthExceeded {
                bytes_read: bytes_consumed,
                max_allowed: MAX_7Z_VARINT_BYTES,
            });
        }

        let shift = i * 8;
        if shift >= 64 {
            return Err(VarintSecurityError::ShiftOverflow {
                byte_index: i,
                shift_bits: shift,
            });
        }
        value |= (next_byte[0] as u64) << shift;
        mask >>= 1;
    }

    Ok((value, bytes_consumed))
}

/// Encodes a 64-bit unsigned integer into 7z NUMBER variable-length format.
pub fn encode_7z_varint(value: u64, out: &mut Vec<u8>) {
    let mut first: u8 = 0;
    let mut mask: u8 = 0x80;
    let mut low = Vec::with_capacity(8);
    let mut i = 0u32;

    while i < 8 {
        if value < (1u64 << (7 * (i + 1))) {
            first |= (value >> (8 * i)) as u8;
            break;
        }
        first |= mask;
        mask >>= 1;
        low.push((value >> (8 * i)) as u8);
        i += 1;
    }

    out.push(first);
    out.extend_from_slice(&low);
}

/// Parses and sanitizes a Tar Pax timestamp string (e.g. `"1700000000.123456789"`, `"-12345.50"`).
///
/// Clamps negative timestamps to `UNIX_EPOCH` (0) and limits nanoseconds strictly to $< 10^9$.
pub fn parse_pax_timestamp(
    raw_str: &str,
    clamp_negative: bool,
) -> Result<PaxTimestamp, PaxSecurityError> {
    let s = raw_str.trim();
    if s.is_empty() {
        return Err(PaxSecurityError::InvalidTimestamp {
            raw: raw_str.to_string(),
            reason: "Empty timestamp string".to_string(),
        });
    }

    // Prohibit exponential notation or invalid float symbols
    if s.contains('e') || s.contains('E') || s.eq_ignore_ascii_case("nan") || s.contains("inf") {
        return Err(PaxSecurityError::InvalidTimestamp {
            raw: raw_str.to_string(),
            reason: "Scientific notation, NaN, or Infinity not permitted".to_string(),
        });
    }

    let is_negative = s.starts_with('-');
    let (secs_part, nanos_part) = match s.split_once('.') {
        Some((sec, frac)) => (sec, Some(frac)),
        None => (s, None),
    };

    let raw_secs: i64 = secs_part.parse().map_err(|e| {
        PaxSecurityError::InvalidTimestamp {
            raw: raw_str.to_string(),
            reason: format!("Failed to parse integer seconds: {e}"),
        }
    })?;

    let mut raw_nanos: u32 = 0;
    if let Some(frac) = nanos_part {
        if frac.is_empty() {
            raw_nanos = 0;
        } else {
            // Take up to 9 digits and pad with zeroes if shorter
            let clean_digits = frac.chars().take(9).collect::<String>();
            if !clean_digits.chars().all(|c| c.is_ascii_digit()) {
                return Err(PaxSecurityError::InvalidTimestamp {
                    raw: raw_str.to_string(),
                    reason: "Non-digit character in nanoseconds".to_string(),
                });
            }

            let parsed: u32 = clean_digits.parse().map_err(|e| {
                PaxSecurityError::InvalidTimestamp {
                    raw: raw_str.to_string(),
                    reason: format!("Failed to parse nanoseconds: {e}"),
                }
            })?;

            let multiplier = match clean_digits.len() {
                1 => 100_000_000,
                2 => 10_000_000,
                3 => 1_000_000,
                4 => 100_000,
                5 => 10_000,
                6 => 1_000,
                7 => 100,
                8 => 10,
                _ => 1,
            };
            raw_nanos = parsed.saturating_mul(multiplier);
        }
    }

    if raw_nanos >= 1_000_000_000 {
        raw_nanos = 999_999_999;
    }

    let mut was_clamped = false;
    let clamped_secs = if raw_secs < 0 {
        was_clamped = true;
        0u64
    } else if raw_secs > MAX_SAFE_PAX_SECONDS {
        was_clamped = true;
        MAX_SAFE_PAX_SECONDS as u64
    } else {
        raw_secs as u64
    };

    let clamped_nanos = if raw_secs < 0 && clamp_negative {
        0
    } else {
        raw_nanos
    };

    Ok(PaxTimestamp {
        raw_secs,
        raw_nanos,
        clamped_secs,
        clamped_nanos,
        was_negative: is_negative || raw_secs < 0,
        was_clamped,
    })
}

/// A parsed single record from a Tar Pax Extended Header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaxRecord {
    pub key: String,
    pub value: String,
    pub declared_length: usize,
}

/// Parses a byte slice of Tar Pax extended header lines.
///
/// POSIX.1-2001 Pax format: `"<length> <key>=<value>\n"`
pub fn parse_pax_extended_header(data: &[u8]) -> Result<Vec<PaxRecord>, PaxSecurityError> {
    let mut records = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if records.len() >= MAX_PAX_RECORDS_PER_BLOCK {
            return Err(PaxSecurityError::RecordQuotaExceeded {
                count: records.len(),
                max_allowed: MAX_PAX_RECORDS_PER_BLOCK,
            });
        }

        // Find next newline
        let remaining = &data[pos..];
        let nl_offset = match remaining.iter().position(|&b| b == b'\n') {
            Some(idx) => idx,
            None => {
                // If trailing bytes are all zeroes (standard Tar padding), terminate cleanly
                if remaining.iter().all(|&b| b == 0) {
                    break;
                }
                return Err(PaxSecurityError::MalformedRecord {
                    reason: "Pax record missing terminating newline".to_string(),
                });
            }
        };

        let line_len = nl_offset + 1;
        if line_len > MAX_PAX_RECORD_SIZE {
            return Err(PaxSecurityError::RecordTooLarge {
                size: line_len,
                max_allowed: MAX_PAX_RECORD_SIZE,
            });
        }

        let line_bytes = &remaining[..line_len];
        let line_str = std::str::from_utf8(line_bytes).map_err(|e| {
            PaxSecurityError::InvalidUtf8 {
                reason: e.to_string(),
            }
        })?;

        // Format: "<length> <key>=<value>\n"
        let trimmed_line = line_str.trim_end_matches('\n');
        let (len_str, rest) = match trimmed_line.split_once(' ') {
            Some((l, r)) => (l, r),
            None => {
                return Err(PaxSecurityError::MalformedRecord {
                    reason: "Pax record missing space delimiter between length and key".to_string(),
                });
            }
        };

        let declared_len: usize = len_str.parse().map_err(|e| {
            PaxSecurityError::MalformedRecord {
                reason: format!("Invalid record length digits: {e}"),
            }
        })?;

        if declared_len != line_len {
            return Err(PaxSecurityError::RecordLengthMismatch {
                declared: declared_len,
                actual: line_len,
            });
        }

        let (key, value) = match rest.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => {
                return Err(PaxSecurityError::MalformedRecord {
                    reason: "Pax record missing '=' delimiter between key and value".to_string(),
                });
            }
        };

        records.push(PaxRecord {
            key,
            value,
            declared_length: declared_len,
        });

        pos += line_len;
    }

    Ok(records)
}

/// Security auditor and configurable circuit breaker for 7z Varints and Tar Pax headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarintPaxSecurityGuard {
    pub max_varint_bytes: usize,
    pub clamp_negative_timestamps: bool,
    pub max_pax_record_size: usize,
    pub max_pax_records_count: usize,
}

impl Default for VarintPaxSecurityGuard {
    fn default() -> Self {
        Self {
            max_varint_bytes: MAX_7Z_VARINT_BYTES,
            clamp_negative_timestamps: true,
            max_pax_record_size: MAX_PAX_RECORD_SIZE,
            max_pax_records_count: MAX_PAX_RECORDS_PER_BLOCK,
        }
    }
}

impl VarintPaxSecurityGuard {
    /// Creates a new security guard with custom limits.
    pub fn new(max_varint_bytes: usize, clamp_negative_timestamps: bool) -> Self {
        Self {
            max_varint_bytes: if max_varint_bytes == 0 {
                1
            } else {
                max_varint_bytes
            },
            clamp_negative_timestamps,
            max_pax_record_size: MAX_PAX_RECORD_SIZE,
            max_pax_records_count: MAX_PAX_RECORDS_PER_BLOCK,
        }
    }

    /// Decodes a 7z varint validating against configured limits.
    #[inline]
    pub fn decode_varint(&self, bytes: &[u8]) -> Result<(u64, usize), VarintSecurityError> {
        let (val, consumed) = decode_7z_varint(bytes)?;
        if consumed > self.max_varint_bytes {
            return Err(VarintSecurityError::ByteLengthExceeded {
                bytes_read: consumed,
                max_allowed: self.max_varint_bytes,
            });
        }
        Ok((val, consumed))
    }

    /// Sanitizes and bounds-checks a Pax timestamp string.
    #[inline]
    pub fn sanitize_timestamp(&self, raw: &str) -> Result<PaxTimestamp, PaxSecurityError> {
        parse_pax_timestamp(raw, self.clamp_negative_timestamps)
    }

    /// Parses and sanitizes a complete Pax extended header block.
    #[inline]
    pub fn parse_pax_block(&self, data: &[u8]) -> Result<Vec<PaxRecord>, PaxSecurityError> {
        parse_pax_extended_header(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_7z_varint_codec_roundtrip_boundaries() {
        let test_values = [
            0u64,
            1,
            127,
            128,
            0x1234,
            0xFFFF,
            1 << 32,
            1 << 62,
            u64::MAX - 1,
            u64::MAX,
        ];

        for &val in &test_values {
            let mut buf = Vec::new();
            encode_7z_varint(val, &mut buf);
            let (decoded, consumed) = decode_7z_varint(&buf).unwrap();
            assert_eq!(decoded, val, "Roundtrip failed for {val:#x}");
            assert_eq!(consumed, buf.len());
            assert!(consumed <= MAX_7Z_VARINT_BYTES);
        }
    }

    #[test]
    fn test_7z_varint_truncation_detection() {
        // Varint with high bit set indicating follow-up bytes, but empty payload
        let truncated = [0x80u8];
        let err = decode_7z_varint(&truncated).unwrap_err();
        match err {
            VarintSecurityError::TruncatedInput { .. } => {}
            _ => panic!("Expected TruncatedInput error, got: {:?}", err),
        }
    }

    #[test]
    fn test_pax_negative_timestamp_clamping() {
        // Pre-1970 date: -3600.500 (1 hour before Unix epoch)
        let ts = parse_pax_timestamp("-3600.500", true).unwrap();
        assert!(ts.was_negative);
        assert!(ts.was_clamped);
        assert_eq!(ts.raw_secs, -3600);
        assert_eq!(ts.clamped_secs, 0);
        assert_eq!(ts.clamped_nanos, 0);

        // SystemTime must succeed without panicking
        let sys_time = ts.to_system_time();
        assert_eq!(sys_time, UNIX_EPOCH);
    }

    #[test]
    fn test_pax_nanosecond_precision_and_normalization() {
        // 1700000000.123456789 (9-digit nanos)
        let ts1 = parse_pax_timestamp("1700000000.123456789", true).unwrap();
        assert_eq!(ts1.raw_secs, 1700000000);
        assert_eq!(ts1.raw_nanos, 123456789);
        assert_eq!(ts1.clamped_secs, 1700000000);
        assert_eq!(ts1.clamped_nanos, 123456789);

        // Short nanos: .5 -> 500,000,000 ns
        let ts2 = parse_pax_timestamp("1700000000.5", true).unwrap();
        assert_eq!(ts2.raw_nanos, 500_000_000);

        // Extra precision beyond 9 digits: truncated safely
        let ts3 = parse_pax_timestamp("1700000000.123456789999999", true).unwrap();
        assert_eq!(ts3.raw_nanos, 123456789);
    }

    #[test]
    fn test_pax_malformed_record_rejection() {
        // Missing space
        let bad1 = b"25mtime=1700000000\n";
        assert!(parse_pax_extended_header(bad1).is_err());

        // Length mismatch (declared 30, actual is 21)
        let bad2 = b"30 mtime=1700000000\n";
        assert!(parse_pax_extended_header(bad2).is_err());

        // Missing newline
        let bad3 = b"20 mtime=1700000000";
        assert!(parse_pax_extended_header(bad3).is_err());
    }

    #[test]
    fn test_pax_valid_multi_record_header() {
        let header = b"24 mtime=1700000000.500\n29 path=deep/nested/file.txt\n";
        let records = parse_pax_extended_header(header).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].key, "mtime");
        assert_eq!(records[0].value, "1700000000.500");
        assert_eq!(records[1].key, "path");
        assert_eq!(records[1].value, "deep/nested/file.txt");
    }

    #[test]
    fn test_varint_pax_security_guard_facade() {
        let guard = VarintPaxSecurityGuard::default();

        // 1. Valid Varint
        let mut buf = Vec::new();
        encode_7z_varint(1234567, &mut buf);
        let (val, consumed) = guard.decode_varint(&buf).unwrap();
        assert_eq!(val, 1234567);
        assert_eq!(consumed, buf.len());

        // 2. Custom guard with strict 2-byte limit
        let strict_guard = VarintPaxSecurityGuard::new(2, true);
        let mut large_buf = Vec::new();
        encode_7z_varint(1u64 << 30, &mut large_buf); // requires 5 bytes
        let err = strict_guard.decode_varint(&large_buf).unwrap_err();
        match err {
            VarintSecurityError::ByteLengthExceeded { bytes_read, max_allowed } => {
                assert_eq!(bytes_read, 5);
                assert_eq!(max_allowed, 2);
            }
            _ => panic!("Expected ByteLengthExceeded error, got: {:?}", err),
        }

        // 3. Sanitized timestamp
        let ts = guard.sanitize_timestamp("1700000000.999").unwrap();
        assert_eq!(ts.clamped_secs, 1700000000);
        assert_eq!(ts.clamped_nanos, 999_000_000);
    }
}
