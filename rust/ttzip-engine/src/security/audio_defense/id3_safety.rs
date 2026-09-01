// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ID3 Tag Safety Guard and Syncsafe Integer Parser.
//!
//! Validates ID3v2 container headers, enforces 7-bit syncsafe integer guarantees,
//! protects against integer wrap-around and oversized tag bombs (<= 32MB quota),
//! and performs memory-safe in-place two-pointer desynchronization with zero out-of-bounds risk.

use super::{AudioDefenseError, DEFAULT_MAX_ID3_TAG_SIZE};

/// Header metadata and structural layout discovered during ID3 tag inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id3InspectionSummary {
    /// ID3v2 major version number (e.g. 2 for ID3v2.2, 3 for ID3v2.3, 4 for ID3v2.4).
    pub version_major: u8,
    /// ID3v2 revision number.
    pub version_revision: u8,
    /// Raw flag byte.
    pub flags: u8,
    /// Unsynchronisation flag enabled across all frames.
    pub unsynchronized: bool,
    /// Extended header present flag.
    pub extended_header: bool,
    /// Experimental flag set.
    pub experimental: bool,
    /// Footer present flag (ID3v2.4).
    pub has_footer: bool,
    /// Payload body size in bytes (excluding 10-byte header and optional 10-byte footer).
    pub tag_body_size: usize,
    /// Total tag size on disk including header and footer.
    pub total_tag_size: usize,
}

/// Defensive parser and validator for ID3 metadata containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id3TagSafetyGuard {
    max_tag_size: usize,
}

impl Default for Id3TagSafetyGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Id3TagSafetyGuard {
    /// Creates a guard with default 32 MiB memory quota limit.
    pub const fn new() -> Self {
        Self {
            max_tag_size: DEFAULT_MAX_ID3_TAG_SIZE,
        }
    }

    /// Creates a guard with custom tag size quota limit.
    pub const fn with_max_tag_size(max_tag_size: usize) -> Self {
        Self { max_tag_size }
    }

    /// Parses a 4-byte 7-bit Syncsafe integer according to ID3v2 specifications.
    ///
    /// Each byte in a syncsafe integer MUST have the most significant bit (MSB / bit 7) set to 0.
    /// Returns `Err(AudioDefenseError::Id3InvalidSyncsafe)` if any byte has bit 7 set.
    pub fn parse_syncsafe_u32(bytes: [u8; 4]) -> Result<u32, AudioDefenseError> {
        for (i, &b) in bytes.iter().enumerate() {
            if (b & 0x80) != 0 {
                return Err(AudioDefenseError::Id3InvalidSyncsafe {
                    reason: format!("Byte {i} (0x{b:02X}) has MSB bit 7 set in syncsafe integer"),
                });
            }
        }

        let val = ((bytes[0] as u32) << 21)
            | ((bytes[1] as u32) << 14)
            | ((bytes[2] as u32) << 7)
            | (bytes[3] as u32);

        Ok(val)
    }

    /// Encodes a 28-bit unsigned integer into 4-byte 7-bit syncsafe representation.
    pub fn encode_syncsafe_u32(val: u32) -> Result<[u8; 4], AudioDefenseError> {
        if val > 0x0FFF_FFFF {
            return Err(AudioDefenseError::Id3InvalidSyncsafe {
                reason: format!("Value 0x{val:08X} exceeds 28-bit syncsafe capacity (max 0x0FFFFFFF)"),
            });
        }

        let b0 = ((val >> 21) & 0x7F) as u8;
        let b1 = ((val >> 14) & 0x7F) as u8;
        let b2 = ((val >> 7) & 0x7F) as u8;
        let b3 = (val & 0x7F) as u8;

        Ok([b0, b1, b2, b3])
    }

    /// Parses a syncsafe integer directly to `usize`.
    pub fn parse_syncsafe_usize(bytes: [u8; 4]) -> Result<usize, AudioDefenseError> {
        let val = Self::parse_syncsafe_u32(bytes)?;
        Ok(val as usize)
    }

    /// Inspects the 10-byte ID3v2 header and validates structural integrity and size quotas.
    pub fn inspect_header(&self, data: &[u8]) -> Result<Id3InspectionSummary, AudioDefenseError> {
        if data.len() < 10 {
            return Err(AudioDefenseError::Id3Malformed {
                reason: format!("Header too short: expected >= 10 bytes, found {}", data.len()),
            });
        }

        if !data.starts_with(b"ID3") {
            return Err(AudioDefenseError::Id3Malformed {
                reason: "Missing 'ID3' magic signature".to_string(),
            });
        }

        let version_major = data[3];
        let version_revision = data[4];
        let flags = data[5];

        if !(2..=4).contains(&version_major) {
            return Err(AudioDefenseError::Id3Malformed {
                reason: format!("Unsupported ID3v2 major version: 2.{version_major}"),
            });
        }

        let unsynchronized = (flags & 0x80) != 0;
        let extended_header = (flags & 0x40) != 0;
        let experimental = (flags & 0x20) != 0;
        let has_footer = (version_major == 4) && ((flags & 0x10) != 0);

        let syncsafe_bytes = [data[6], data[7], data[8], data[9]];
        let tag_body_size = Self::parse_syncsafe_usize(syncsafe_bytes)?;

        if tag_body_size > self.max_tag_size {
            return Err(AudioDefenseError::Id3TagSizeExceeded {
                size: tag_body_size,
                max_size: self.max_tag_size,
            });
        }

        let mut total_tag_size = 10usize.saturating_add(tag_body_size);
        if has_footer {
            total_tag_size = total_tag_size.saturating_add(10);
        }

        if total_tag_size > self.max_tag_size {
            return Err(AudioDefenseError::Id3TagSizeExceeded {
                size: total_tag_size,
                max_size: self.max_tag_size,
            });
        }

        Ok(Id3InspectionSummary {
            version_major,
            version_revision,
            flags,
            unsynchronized,
            extended_header,
            experimental,
            has_footer,
            tag_body_size,
            total_tag_size,
        })
    }

    /// Performs zero-allocation, memory-safe in-place two-pointer desynchronization on a byte buffer.
    ///
    /// Removes false MPEG audio frame synchronization sequences introduced by ID3 unsynchronisation:
    /// any byte pair `0xFF 0x00` in the unsynchronized stream is replaced by `0xFF`.
    /// Returns the compacted byte length of the desynchronized buffer.
    pub fn desynchronize_in_place(buf: &mut Vec<u8>) -> usize {
        let len = buf.len();
        if len < 2 {
            return len;
        }

        let mut read_idx = 0usize;
        let mut write_idx = 0usize;

        while read_idx < len {
            if read_idx + 1 < len && buf[read_idx] == 0xFF && buf[read_idx + 1] == 0x00 {
                // Emit 0xFF and discard the 0x00 padding byte
                buf[write_idx] = 0xFF;
                write_idx += 1;
                read_idx += 2;
            } else {
                buf[write_idx] = buf[read_idx];
                write_idx += 1;
                read_idx += 1;
            }
        }

        buf.truncate(write_idx);
        write_idx
    }

    /// Inspects and sanitizes a complete ID3v2 tag buffer, unpacking desynchronization if enabled.
    pub fn inspect_and_sanitize_tag_payload(
        &self,
        data: &[u8],
    ) -> Result<Vec<u8>, AudioDefenseError> {
        let summary = self.inspect_header(data)?;

        if data.len() < summary.total_tag_size {
            return Err(AudioDefenseError::Id3Malformed {
                reason: format!(
                    "Incomplete tag payload: expected {} bytes, found {}",
                    summary.total_tag_size,
                    data.len()
                ),
            });
        }

        let body = &data[10..10 + summary.tag_body_size];
        let mut sanitized = body.to_vec();

        if summary.unsynchronized {
            Self::desynchronize_in_place(&mut sanitized);
        }

        Ok(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syncsafe_u32_parsing_and_encoding_roundtrip() {
        // 0 -> [0, 0, 0, 0]
        assert_eq!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0, 0, 0]).unwrap(), 0);
        assert_eq!(Id3TagSafetyGuard::encode_syncsafe_u32(0).unwrap(), [0, 0, 0, 0]);

        // 127 -> [0, 0, 0, 0x7F]
        assert_eq!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0, 0, 0x7F]).unwrap(), 127);
        assert_eq!(Id3TagSafetyGuard::encode_syncsafe_u32(127).unwrap(), [0, 0, 0, 0x7F]);

        // 128 -> [0, 0, 1, 0]
        assert_eq!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0, 1, 0]).unwrap(), 128);
        assert_eq!(Id3TagSafetyGuard::encode_syncsafe_u32(128).unwrap(), [0, 0, 1, 0]);

        // Max 28-bit integer: 0x0FFF_FFFF -> [0x7F, 0x7F, 0x7F, 0x7F]
        let max_28 = 0x0FFF_FFFF;
        assert_eq!(
            Id3TagSafetyGuard::parse_syncsafe_u32([0x7F, 0x7F, 0x7F, 0x7F]).unwrap(),
            max_28
        );
        assert_eq!(
            Id3TagSafetyGuard::encode_syncsafe_u32(max_28).unwrap(),
            [0x7F, 0x7F, 0x7F, 0x7F]
        );

        // Arbitrary test vector: 257 (1 * 128 + 1 = 129 -> [0, 0, 2, 1])
        assert_eq!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0, 2, 1]).unwrap(), 257);
    }

    #[test]
    fn test_syncsafe_msb_violation_rejection() {
        // MSB in byte 0
        assert!(Id3TagSafetyGuard::parse_syncsafe_u32([0x80, 0, 0, 0]).is_err());
        // MSB in byte 1
        assert!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0x80, 0, 0]).is_err());
        // MSB in byte 2
        assert!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0, 0x80, 0]).is_err());
        // MSB in byte 3
        assert!(Id3TagSafetyGuard::parse_syncsafe_u32([0, 0, 0, 0x80]).is_err());
    }

    #[test]
    fn test_id3_header_inspection() {
        let guard = Id3TagSafetyGuard::new();

        // Valid ID3v2.3 header with 1024 bytes payload
        let mut header = vec![b'I', b'D', b'3', 3, 0, 0x80]; // Unsync flag set
        let size_bytes = Id3TagSafetyGuard::encode_syncsafe_u32(1024).unwrap();
        header.extend_from_slice(&size_bytes);

        let summary = guard.inspect_header(&header).unwrap();
        assert_eq!(summary.version_major, 3);
        assert_eq!(summary.version_revision, 0);
        assert!(summary.unsynchronized);
        assert!(!summary.extended_header);
        assert!(!summary.has_footer);
        assert_eq!(summary.tag_body_size, 1024);
        assert_eq!(summary.total_tag_size, 1034);
    }

    #[test]
    fn test_id3_header_quota_exceeded() {
        let guard = Id3TagSafetyGuard::with_max_tag_size(1024);

        let mut header = vec![b'I', b'D', b'3', 4, 0, 0];
        let size_bytes = Id3TagSafetyGuard::encode_syncsafe_u32(2048).unwrap();
        header.extend_from_slice(&size_bytes);

        let err = guard.inspect_header(&header).unwrap_err();
        assert!(matches!(err, AudioDefenseError::Id3TagSizeExceeded { .. }));
    }

    #[test]
    fn test_desynchronize_in_place() {
        // [0xFF, 0x00, 0x12, 0xFF, 0x00, 0x34, 0xFF] -> [0xFF, 0x12, 0xFF, 0x34, 0xFF]
        let mut buf = vec![0xFF, 0x00, 0x12, 0xFF, 0x00, 0x34, 0xFF];
        let new_len = Id3TagSafetyGuard::desynchronize_in_place(&mut buf);
        assert_eq!(new_len, 5);
        assert_eq!(buf, vec![0xFF, 0x12, 0xFF, 0x34, 0xFF]);

        // Consecutive sync escapes: [0xFF, 0x00, 0xFF, 0x00] -> [0xFF, 0xFF]
        let mut buf2 = vec![0xFF, 0x00, 0xFF, 0x00];
        let new_len2 = Id3TagSafetyGuard::desynchronize_in_place(&mut buf2);
        assert_eq!(new_len2, 2);
        assert_eq!(buf2, vec![0xFF, 0xFF]);

        // Trailing 0xFF without 0x00
        let mut buf3 = vec![0x10, 0x20, 0xFF];
        let new_len3 = Id3TagSafetyGuard::desynchronize_in_place(&mut buf3);
        assert_eq!(new_len3, 3);
        assert_eq!(buf3, vec![0x10, 0x20, 0xFF]);
    }
}
