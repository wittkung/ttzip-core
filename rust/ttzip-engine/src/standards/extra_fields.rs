// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-copy ZIP Type-Length-Value (TLV) Extra Fields parser and validator.
//!
//! Compliant with PKWARE APPNOTE 6.3.9 specifications for Zip64 (`0x0001`),
//! Extended Timestamp (`0x5455`), Unicode Path (`0x7075`), Info-ZIP UNIX (`0x7875`),
//! and WinZip AES (`0x9901`).

use crate::crypto::crc32::crc32_fast;

pub const TAG_ZIP64: u16 = 0x0001;
pub const TAG_EXT_TIMESTAMP: u16 = 0x5455;
pub const TAG_INFOZIP_UNIX: u16 = 0x7875;
pub const TAG_UNICODE_PATH: u16 = 0x7075;
pub const TAG_WINZIP_AES: u16 = 0x9901;

/// A raw unparsed TLV (Type-Length-Value) extra field entry referencing input buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawExtraField<'a> {
    pub tag: u16,
    pub data: &'a [u8],
}

/// Iterator over raw TLV records in an extra fields buffer.
#[derive(Debug, Clone)]
pub struct RawExtraFieldsIter<'a> {
    remaining: &'a [u8],
}

impl<'a> RawExtraFieldsIter<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { remaining: data }
    }
}

impl<'a> Iterator for RawExtraFieldsIter<'a> {
    type Item = RawExtraField<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.len() < 4 {
            self.remaining = &[];
            return None;
        }

        let tag = u16::from_le_bytes([self.remaining[0], self.remaining[1]]);
        let len = u16::from_le_bytes([self.remaining[2], self.remaining[3]]) as usize;

        if self.remaining.len() < 4 + len {
            self.remaining = &[];
            return None; // Truncated field terminates iteration cleanly
        }

        let data = &self.remaining[4..4 + len];
        self.remaining = &self.remaining[4 + len..];
        Some(RawExtraField { tag, data })
    }
}

/// Parsed Zip64 Extra Field (Tag `0x0001`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Zip64ExtraField {
    pub uncompressed_size: Option<u64>,
    pub compressed_size: Option<u64>,
    pub local_header_offset: Option<u64>,
    pub disk_start_number: Option<u32>,
}

/// Parsed Extended Timestamp Extra Field (Tag `0x5455`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedTimestampExtraField {
    pub flags: u8,
    pub mod_time: Option<u32>,
    pub acc_time: Option<u32>,
    pub create_time: Option<u32>,
}

/// Parsed Unicode Path Extra Field (Tag `0x7075`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnicodePathExtraField<'a> {
    pub version: u8,
    pub name_crc32: u32,
    pub unicode_name: &'a [u8],
}

/// Parsed Info-ZIP UNIX Extra Field (Tag `0x7875`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InfoZipUnixExtraField {
    pub version: u8,
    pub uid: Option<u64>,
    pub gid: Option<u64>,
}

/// Parsed WinZip AES Extra Field (Tag `0x9901`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WinZipAesExtraField {
    pub version: u16,
    pub vendor_id: [u8; 2],
    pub strength: u8,
    pub actual_compression_method: u16,
}

/// Consolidated zero-copy view of all recognized ZIP Extra Fields.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedExtraFields<'a> {
    pub zip64: Option<Zip64ExtraField>,
    pub timestamp: Option<ExtendedTimestampExtraField>,
    pub unicode_path: Option<UnicodePathExtraField<'a>>,
    pub infozip_unix: Option<InfoZipUnixExtraField>,
    pub winzip_aes: Option<WinZipAesExtraField>,
    pub raw_count: usize,
}

impl<'a> ParsedExtraFields<'a> {
    /// Parses an extra fields slice with zero heap allocation.
    pub fn parse(
        data: &'a [u8],
        is_cdfh: bool,
        uncomp_placeholder: bool,
        comp_placeholder: bool,
        offset_placeholder: bool,
    ) -> Self {
        let mut parsed = Self::default();

        for raw in RawExtraFieldsIter::new(data) {
            parsed.raw_count += 1;
            match raw.tag {
                TAG_ZIP64 => {
                    parsed.zip64 = Some(parse_zip64_extra_field(
                        raw.data,
                        is_cdfh,
                        uncomp_placeholder,
                        comp_placeholder,
                        offset_placeholder,
                    ));
                }
                TAG_EXT_TIMESTAMP => parsed.timestamp = parse_timestamp_extra_field(raw.data),
                TAG_UNICODE_PATH => parsed.unicode_path = parse_unicode_path_extra_field(raw.data),
                TAG_INFOZIP_UNIX => parsed.infozip_unix = parse_infozip_unix_extra_field(raw.data),
                TAG_WINZIP_AES => parsed.winzip_aes = parse_winzip_aes_extra_field(raw.data),
                _ => {}
            }
        }

        parsed
    }
}

fn parse_zip64_extra_field(
    p: &[u8],
    is_cdfh: bool,
    uncomp_placeholder: bool,
    comp_placeholder: bool,
    offset_placeholder: bool,
) -> Zip64ExtraField {
    let mut z64 = Zip64ExtraField::default();
    let mut cursor = 0;

    if is_cdfh {
        if uncomp_placeholder && cursor + 8 <= p.len() {
            z64.uncompressed_size = Some(read_u64_le_at(p, cursor));
            cursor += 8;
        }
        if comp_placeholder && cursor + 8 <= p.len() {
            z64.compressed_size = Some(read_u64_le_at(p, cursor));
            cursor += 8;
        }
        if offset_placeholder && cursor + 8 <= p.len() {
            z64.local_header_offset = Some(read_u64_le_at(p, cursor));
            cursor += 8;
        }
        if cursor + 4 <= p.len() {
            z64.disk_start_number = Some(u32::from_le_bytes(p[cursor..cursor + 4].try_into().unwrap_or_default()));
        }
    } else {
        if cursor + 8 <= p.len() {
            z64.uncompressed_size = Some(read_u64_le_at(p, cursor));
            cursor += 8;
        }
        if cursor + 8 <= p.len() {
            z64.compressed_size = Some(read_u64_le_at(p, cursor));
        }
    }
    z64
}

#[inline]
fn read_u64_le_at(p: &[u8], cursor: usize) -> u64 {
    u64::from_le_bytes(p[cursor..cursor + 8].try_into().unwrap_or_default())
}

fn parse_timestamp_extra_field(data: &[u8]) -> Option<ExtendedTimestampExtraField> {
    let &flags = data.first()?;
    let mut ts = ExtendedTimestampExtraField { flags, ..Default::default() };
    let mut cursor = 1;
    if (flags & 0x01) != 0 && cursor + 4 <= data.len() {
        ts.mod_time = Some(u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap_or_default()));
        cursor += 4;
    }
    if (flags & 0x02) != 0 && cursor + 4 <= data.len() {
        ts.acc_time = Some(u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap_or_default()));
        cursor += 4;
    }
    if (flags & 0x04) != 0 && cursor + 4 <= data.len() {
        ts.create_time = Some(u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap_or_default()));
    }
    Some(ts)
}

fn parse_unicode_path_extra_field(data: &[u8]) -> Option<UnicodePathExtraField<'_>> {
    if data.len() >= 5 && data[0] == 1 {
        let crc = u32::from_le_bytes(data[1..5].try_into().unwrap_or_default());
        Some(UnicodePathExtraField {
            version: data[0],
            name_crc32: crc,
            unicode_name: &data[5..],
        })
    } else {
        None
    }
}

fn parse_infozip_unix_extra_field(data: &[u8]) -> Option<InfoZipUnixExtraField> {
    if data.len() < 4 || data[0] != 1 {
        return None;
    }
    let uid_sz = data[1] as usize;
    let mut cursor = 2;
    let mut unix = InfoZipUnixExtraField { version: 1, ..Default::default() };

    if cursor + uid_sz <= data.len() {
        unix.uid = parse_varint_le(&data[cursor..cursor + uid_sz]);
        cursor += uid_sz;
    }
    if cursor < data.len() {
        let gid_sz = data[cursor] as usize;
        cursor += 1;
        if cursor + gid_sz <= data.len() {
            unix.gid = parse_varint_le(&data[cursor..cursor + gid_sz]);
        }
    }
    Some(unix)
}

fn parse_winzip_aes_extra_field(data: &[u8]) -> Option<WinZipAesExtraField> {
    if data.len() < 7 {
        return None;
    }
    Some(WinZipAesExtraField {
        version: u16::from_le_bytes([data[0], data[1]]),
        vendor_id: [data[2], data[3]],
        strength: data[4],
        actual_compression_method: u16::from_le_bytes([data[5], data[6]]),
    })
}

/// Helper function to parse 16-bit, 32-bit, or 64-bit integer from little-endian bytes.
#[inline]
fn parse_varint_le(bytes: &[u8]) -> Option<u64> {
    match bytes.len() {
        2 => Some(u16::from_le_bytes([bytes[0], bytes[1]]) as u64),
        4 => Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64),
        8 => Some(u64::from_le_bytes(bytes.try_into().ok()?)),
        _ => None,
    }
}

/// Validates Unicode path extra field against the standard ASCII filename.
pub fn validate_unicode_path(extra: &UnicodePathExtraField, standard_filename: &[u8]) -> bool {
    let expected_crc = crc32_fast(0, standard_filename);
    extra.name_crc32 == expected_crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_extra_fields_parsing() {
        let mut buf = Vec::new();

        // 1. Tag 0x5455: Extended Timestamp (9 bytes total)
        buf.extend_from_slice(&TAG_EXT_TIMESTAMP.to_le_bytes());
        buf.extend_from_slice(&5u16.to_le_bytes());
        buf.push(1); // Mod time flag
        buf.extend_from_slice(&1700000000u32.to_le_bytes());

        // 2. Tag 0x9901: WinZip AES (11 bytes total)
        buf.extend_from_slice(&TAG_WINZIP_AES.to_le_bytes());
        buf.extend_from_slice(&7u16.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes()); // AE-2
        buf.extend_from_slice(b"AE");
        buf.push(3); // 256-bit
        buf.extend_from_slice(&8u16.to_le_bytes()); // Deflate

        // 3. Tag 0x7075: Unicode Path
        let name = "文档/测试.txt";
        let crc = crc32_fast(0, b"fallback.txt");
        buf.extend_from_slice(&TAG_UNICODE_PATH.to_le_bytes());
        buf.extend_from_slice(&((5 + name.len()) as u16).to_le_bytes());
        buf.push(1);
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(name.as_bytes());

        let parsed = ParsedExtraFields::parse(&buf, false, false, false, false);
        assert_eq!(parsed.raw_count, 3);

        let ts = parsed.timestamp.expect("Timestamp field missing");
        assert_eq!(ts.flags, 1);
        assert_eq!(ts.mod_time, Some(1700000000));

        let aes = parsed.winzip_aes.expect("AES field missing");
        assert_eq!(aes.version, 2);
        assert_eq!(&aes.vendor_id, b"AE");
        assert_eq!(aes.strength, 3);
        assert_eq!(aes.actual_compression_method, 8);

        let u_path = parsed.unicode_path.expect("Unicode path field missing");
        assert_eq!(u_path.version, 1);
        assert_eq!(u_path.unicode_name, name.as_bytes());
        assert!(validate_unicode_path(&u_path, b"fallback.txt"));
    }
}
