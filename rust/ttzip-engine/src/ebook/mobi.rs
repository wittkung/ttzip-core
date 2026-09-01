// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! MOBI and AZW3 Palm Database (PDB) format parser, PalmDOC LZ77 decompressor, and EXTH metadata engine.
//!
//! Provides pure Safe Rust decoding for PalmDOC Header, MOBI Header, EXTH Header, and 4-way variable
//! length sliding-window LZ77 stream decompression.

use std::collections::HashMap;
use crate::ebook::{EbookError, EbookResult};

/// Palm Database record descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdbRecordEntry {
    /// Byte offset from start of file.
    pub offset: u32,
    /// Record attributes bitmask.
    pub attributes: u8,
    /// Unique record identification number.
    pub unique_id: u32,
}

/// PalmDOC header layout (16 bytes) at the start of Record 0.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PalmDocHeader {
    /// Compression type: 1 = None, 2 = PalmDOC LZ77, 17480 = HUFF/CDIC.
    pub compression: u16,
    /// Total uncompressed length of text payload across all text records.
    pub text_length: u32,
    /// Number of sequential text records (records 1..=record_count).
    pub record_count: u16,
    /// Maximum uncompressed size of a single text record (typically 4096 bytes).
    pub record_size: u16,
    /// Current reading position bookmark.
    pub current_position: u32,
}

/// Parsed MOBI header information.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MobiHeaderInfo {
    /// MOBI header length in bytes.
    pub header_length: u32,
    /// MOBI file type (e.g. 2 = Book).
    pub mobi_type: u32,
    /// Character encoding: 1252 = CP1252, 65001 = UTF-8.
    pub text_encoding: u32,
    /// Unique file identifier.
    pub unique_id: u32,
    /// MOBI format version (e.g. 4, 6, 8 for KF8/AZW3).
    pub file_version: u32,
    /// Byte offset to full book title inside Record 0.
    pub full_name_offset: u32,
    /// Length of full book title in bytes.
    pub full_name_length: u32,
    /// Bitmask indicating optional headers (bit 6 = EXTH present).
    pub exth_flags: u32,
    /// First image record index in PDB record table.
    pub first_image_index: u32,
    /// Trailing flags bitmask for multi-byte indexing.
    pub extra_data_flags: u16,
}

/// A parsed record from the EXTH header extension table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobiExthRecord {
    /// Tag identifier (e.g., 100 = Author, 201 = CoverOffset).
    pub tag: u32,
    /// Binary data payload for this tag.
    pub data: Vec<u8>,
}

/// Decoder for MOBI, AZW3, and PalmDOC container structures.
pub struct EbookMobiDecoder<'a> {
    data: &'a [u8],
    records: Vec<PdbRecordEntry>,
    palmdoc: PalmDocHeader,
    mobi: MobiHeaderInfo,
    exth: HashMap<u32, Vec<u8>>,
    full_name: Option<String>,
}

impl<'a> EbookMobiDecoder<'a> {
    /// Parses a MOBI / AZW3 file from a raw byte slice.
    pub fn parse(data: &'a [u8]) -> EbookResult<Self> {
        if data.len() < 78 {
            return Err(EbookError::Mobi("Payload smaller than PDB header (78 bytes)".to_string()));
        }

        // PDB Type & Creator check: bytes 60..64 and 64..68
        let db_type = &data[60..64];
        let db_creator = &data[64..68];
        let is_mobi_pdb = (db_type == b"BOOK" || db_type == b"TEXt")
            && (db_creator == b"MOBI" || db_creator == b"REAd");

        if !is_mobi_pdb {
            // Also check Record 0 directly for "MOBI" identifier if PDB creator was non-standard
            let num_records = read_u16_be(data, 76) as usize;
            if num_records == 0 || data.len() < 78 + num_records * 8 {
                return Err(EbookError::UnsupportedFormat("Not a valid Palm Database (PDB)".to_string()));
            }
        }

        let num_records = read_u16_be(data, 76) as usize;
        let record_table_end = 78 + num_records * 8;
        if data.len() < record_table_end {
            return Err(EbookError::Mobi("Truncated PDB record table".to_string()));
        }

        let mut records = Vec::with_capacity(num_records);
        for i in 0..num_records {
            let offset = 78 + i * 8;
            let rec_offset = read_u32_be(data, offset);
            let attributes = data[offset + 4];
            let unique_id = ((data[offset + 5] as u32) << 16)
                | ((data[offset + 6] as u32) << 8)
                | (data[offset + 7] as u32);
            records.push(PdbRecordEntry {
                offset: rec_offset,
                attributes,
                unique_id,
            });
        }

        if records.is_empty() {
            return Err(EbookError::Mobi("PDB has 0 records".to_string()));
        }

        // Parse Record 0
        let rec0 = get_record_slice(data, &records, 0)
            .ok_or_else(|| EbookError::Mobi("Missing Record 0 in PDB".to_string()))?;

        if rec0.len() < 16 {
            return Err(EbookError::Mobi("Record 0 smaller than PalmDOC header (16 bytes)".to_string()));
        }

        let palmdoc = PalmDocHeader {
            compression: read_u16_be(rec0, 0),
            text_length: read_u32_be(rec0, 4),
            record_count: read_u16_be(rec0, 8),
            record_size: read_u16_be(rec0, 10),
            current_position: read_u32_be(rec0, 12),
        };

        let mut mobi = MobiHeaderInfo::default();
        let mut exth_map = HashMap::new();
        let mut full_name = None;

        if rec0.len() >= 40 && &rec0[16..20] == b"MOBI" {
            let header_len = read_u32_be(rec0, 20);
            let mobi_type = read_u32_be(rec0, 24);
            let text_encoding = read_u32_be(rec0, 28);
            let unique_id = read_u32_be(rec0, 32);
            let file_version = read_u32_be(rec0, 36);

            let full_name_offset = if rec0.len() >= 88 {
                read_u32_be(rec0, 84)
            } else {
                0
            };
            let full_name_length = if rec0.len() >= 92 {
                read_u32_be(rec0, 88)
            } else {
                0
            };

            let first_image_index = if rec0.len() >= 112 {
                read_u32_be(rec0, 108)
            } else {
                0
            };

            let exth_flags = if rec0.len() >= 132 {
                read_u32_be(rec0, 128)
            } else {
                0
            };

            let extra_data_flags = if rec0.len() >= 244 {
                read_u16_be(rec0, 242)
            } else {
                0
            };

            mobi = MobiHeaderInfo {
                header_length: header_len,
                mobi_type,
                text_encoding,
                unique_id,
                file_version,
                full_name_offset,
                full_name_length,
                exth_flags,
                first_image_index,
                extra_data_flags,
            };

            // Parse Full Name if present
            if full_name_offset > 0 && full_name_length > 0 {
                let start = full_name_offset as usize;
                let end = start + full_name_length as usize;
                if end <= rec0.len() {
                    let title_slice = &rec0[start..end];
                    if let Ok(s) = std::str::from_utf8(title_slice) {
                        full_name = Some(s.trim().to_string());
                    } else {
                        let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(title_slice);
                        full_name = Some(cow.trim().to_string());
                    }
                }
            }

            // Parse EXTH Header if present
            if (exth_flags & 0x40) != 0 {
                let exth_start = 16 + header_len as usize;
                if exth_start + 12 <= rec0.len() && &rec0[exth_start..exth_start + 4] == b"EXTH" {
                    let exth_len = read_u32_be(rec0, exth_start + 4) as usize;
                    let exth_count = read_u32_be(rec0, exth_start + 8) as usize;
                    let mut curr = exth_start + 12;
                    let exth_end = (exth_start + exth_len).min(rec0.len());

                    for _ in 0..exth_count {
                        if curr + 8 > exth_end {
                            break;
                        }
                        let tag = read_u32_be(rec0, curr);
                        let rec_len = read_u32_be(rec0, curr + 4) as usize;
                        if rec_len < 8 || curr + rec_len > exth_end {
                            break;
                        }
                        let tag_data = rec0[curr + 8..curr + rec_len].to_vec();
                        exth_map.insert(tag, tag_data);
                        curr += rec_len;
                    }
                }
            }
        }

        Ok(Self {
            data,
            records,
            palmdoc,
            mobi,
            exth: exth_map,
            full_name,
        })
    }

    /// Returns the parsed PalmDOC header.
    #[inline]
    pub fn palmdoc(&self) -> &PalmDocHeader {
        &self.palmdoc
    }

    /// Returns the parsed MOBI header.
    #[inline]
    pub fn mobi(&self) -> &MobiHeaderInfo {
        &self.mobi
    }

    /// Returns true if this e-book is AZW3 / KF8 format.
    #[inline]
    pub fn is_azw3(&self) -> bool {
        self.mobi.file_version >= 8 || self.exth.contains_key(&121) // 121 = KF8 Boundary Offset
    }

    /// Returns the book title extracted from EXTH 503, full_name, or PDB metadata.
    pub fn title(&self) -> Option<String> {
        if let Some(data) = self.exth.get(&503) {
            if let Ok(s) = std::str::from_utf8(data) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        if let Some(ref name) = self.full_name {
            if !name.is_empty() {
                return Some(name.clone());
            }
        }
        // Fallback to PDB database name (first 32 bytes)
        let name_bytes = &self.data[0..32.min(self.data.len())];
        let clean_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
        if clean_len > 0 {
            if let Ok(s) = std::str::from_utf8(&name_bytes[..clean_len]) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    /// Returns author(s) extracted from EXTH 100.
    pub fn authors(&self) -> Vec<String> {
        if let Some(data) = self.exth.get(&100) {
            if let Ok(s) = std::str::from_utf8(data) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return vec![trimmed.to_string()];
                }
            }
        }
        Vec::new()
    }

    /// Returns publisher extracted from EXTH 101.
    pub fn publisher(&self) -> Option<String> {
        self.get_exth_string(101)
    }

    /// Returns description extracted from EXTH 103.
    pub fn description(&self) -> Option<String> {
        self.get_exth_string(103)
    }

    /// Returns ISBN / ASIN identifier.
    pub fn asin_or_isbn(&self) -> Option<String> {
        self.get_exth_string(113).or_else(|| self.get_exth_string(104))
    }

    /// Returns publishing date from EXTH 106.
    pub fn publication_date(&self) -> Option<String> {
        self.get_exth_string(106)
    }

    /// Returns rights from EXTH 109.
    pub fn rights(&self) -> Option<String> {
        self.get_exth_string(109)
    }

    /// Extracts raw binary cover image if CoverOffset (EXTH 201) is defined.
    pub fn extract_cover_image(&self) -> Option<Vec<u8>> {
        let cover_offset_data = self.exth.get(&201)?;
        if cover_offset_data.len() < 4 {
            return None;
        }
        let cover_offset = read_u32_be(cover_offset_data, 0);
        let image_record_idx = (self.mobi.first_image_index + cover_offset) as usize;
        let record_slice = get_record_slice(self.data, &self.records, image_record_idx)?;
        Some(record_slice.to_vec())
    }

    /// Decompresses and extracts the full raw text content across all text records.
    pub fn extract_full_text(&self) -> EbookResult<String> {
        let num_text_records = self.palmdoc.record_count as usize;
        let mut uncompressed_all: Vec<u8> = Vec::with_capacity(
            (self.palmdoc.text_length as usize).min(16 * 1024 * 1024),
        );

        let max_record_size = if self.palmdoc.record_size > 0 {
            self.palmdoc.record_size as usize
        } else {
            4096
        };

        for i in 1..=num_text_records {
            let Some(rec) = get_record_slice(self.data, &self.records, i) else {
                break;
            };

            let decompressed = match self.palmdoc.compression {
                1 => rec.to_vec(),
                2 => decompress_palmdoc_record(rec, max_record_size)?,
                _ => return Err(EbookError::UnsupportedFormat(format!(
                    "Unsupported PalmDOC compression: {}",
                    self.palmdoc.compression
                ))),
            };

            let clean_slice = strip_mobi_trailing_bytes(&decompressed, self.mobi.extra_data_flags);
            uncompressed_all.extend_from_slice(clean_slice);
        }

        match self.mobi.text_encoding {
            65001 => {
                match std::str::from_utf8(&uncompressed_all) {
                    Ok(s) => Ok(s.to_string()),
                    Err(_) => {
                        let (cow, _, _) = encoding_rs::UTF_8.decode(&uncompressed_all);
                        Ok(cow.into_owned())
                    }
                }
            }
            _ => {
                let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(&uncompressed_all);
                Ok(cow.into_owned())
            }
        }
    }

    fn get_exth_string(&self, tag: u32) -> Option<String> {
        let data = self.exth.get(&tag)?;
        if let Ok(s) = std::str::from_utf8(data) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(data);
        let trimmed = cow.trim();
        if !trimmed.is_empty() {
            Some(trimmed.to_string())
        } else {
            None
        }
    }
}

/// PalmDOC LZ77 4-way variable-length sliding window decompressor.
///
/// Decompresses a single PalmDOC record (bounded to `max_out_len` to prevent memory blowup).
pub fn decompress_palmdoc_record(input: &[u8], max_out_len: usize) -> EbookResult<Vec<u8>> {
    let mut out = Vec::with_capacity(max_out_len.min(4096));
    let mut pos = 0;

    while pos < input.len() && out.len() < max_out_len {
        let b = input[pos];
        pos += 1;

        match b {
            // Case 1: Literal byte 0x00
            0x00 => {
                out.push(0x00);
            }
            // Case 2: 1 to 8 literal bytes follow
            0x01..=0x08 => {
                let count = b as usize;
                if pos + count > input.len() {
                    return Err(EbookError::PalmDocDecompress(
                        "Unexpected EOF while reading literal sequence".to_string(),
                    ));
                }
                out.extend_from_slice(&input[pos..pos + count]);
                pos += count;
            }
            // Case 3: Single literal byte
            0x09..=0x7F => {
                out.push(b);
            }
            // Case 4: Sliding window back-reference (2 bytes)
            0x80..=0xBF => {
                if pos >= input.len() {
                    return Err(EbookError::PalmDocDecompress(
                        "Unexpected EOF in LZ77 back-reference pair".to_string(),
                    ));
                }
                let b2 = input[pos];
                pos += 1;

                let distance = ((b as usize & 0x3F) << 3) | ((b2 as usize) >> 5);
                let length = (b2 as usize & 0x07) + 3;

                if distance == 0 || distance > out.len() {
                    return Err(EbookError::PalmDocDecompress(format!(
                        "Invalid LZ77 distance: {distance} (current buffer len: {})",
                        out.len()
                    )));
                }

                let start = out.len() - distance;
                for i in 0..length {
                    let byte = out[start + i];
                    out.push(byte);
                    if out.len() >= max_out_len {
                        break;
                    }
                }
            }
            // Case 5: Space + character (0xC0..=0xFF)
            0xC0..=0xFF => {
                out.push(b' ');
                out.push(b ^ 0x80);
            }
        }
    }

    Ok(out)
}

/// Strips MOBI trailing indexing and formatting bytes from an uncompressed record payload.
fn strip_mobi_trailing_bytes(data: &[u8], extra_flags: u16) -> &[u8] {
    if data.is_empty() || extra_flags == 0 {
        return data;
    }

    let mut end = data.len();
    let mut flags = extra_flags >> 1;

    while flags > 0 && end > 0 {
        if (flags & 1) != 0 {
            let trailing_len = get_trailing_size(&data[..end]);
            end = end.saturating_sub(trailing_len);
        }
        flags >>= 1;
    }

    if (extra_flags & 1) != 0 && end > 0 {
        let last_byte = data[end - 1] as usize & 3;
        end = end.saturating_sub(last_byte);
    }

    &data[..end]
}

/// Reads variable-length trailing size from the end of a byte slice.
fn get_trailing_size(data: &[u8]) -> usize {
    let mut size = 0usize;
    let len = data.len();
    for i in 0..4.min(len) {
        let b = data[len - 1 - i] as usize;
        size |= (b & 0x7F) << (i * 7);
        if (b & 0x80) != 0 {
            return size;
        }
    }
    size
}

/// Extracts a slice for record `index` safely from the Palm Database.
fn get_record_slice<'a>(
    data: &'a [u8],
    records: &[PdbRecordEntry],
    index: usize,
) -> Option<&'a [u8]> {
    let entry = records.get(index)?;
    let start = entry.offset as usize;
    if start >= data.len() {
        return None;
    }

    let end = if index + 1 < records.len() {
        (records[index + 1].offset as usize).min(data.len())
    } else {
        data.len()
    };

    if start <= end {
        Some(&data[start..end])
    } else {
        None
    }
}

#[inline]
fn read_u16_be(slice: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([slice[offset], slice[offset + 1]])
}

#[inline]
fn read_u32_be(slice: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
    ])
}
