// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 3: PalmDOC Binary Stream Decompression & MOBI EXTH Arithmetic Guard.
//!
//! Enforces deterministic sliding window lookback distance verification,
//! 4KB single-record buffer upper bounds, and checked arithmetic for MOBI EXTH headers.

use super::{
    EbookDefenseError, MOBI_EXTH_MAX_RECORDS, PALMDOC_MAX_RECORD_SIZE,
};

/// Guard providing safe PalmDOC LZ77 decompression and MOBI EXTH header parsing.
#[derive(Debug, Default, Clone, Copy)]
pub struct PalmDocDecompressGuard;

impl PalmDocDecompressGuard {
    /// Decompresses a single PalmDOC LZ77 record (max 4,096 bytes) with strict bounds and backreference checks.
    pub fn decompress_record(input: &[u8]) -> Result<Vec<u8>, EbookDefenseError> {
        let mut output = Vec::with_capacity(PALMDOC_MAX_RECORD_SIZE);
        let mut in_pos = 0;

        while in_pos < input.len() {
            let byte = input[in_pos];
            in_pos += 1;

            if byte == 0x00 {
                // Literal null byte
                if output.len() >= PALMDOC_MAX_RECORD_SIZE {
                    return Err(EbookDefenseError::RecordBufferOverflow {
                        attempted_len: output.len() + 1,
                        limit: PALMDOC_MAX_RECORD_SIZE,
                    });
                }
                output.push(0x00);
            } else if byte <= 0x08 {
                // Copy 1 to 8 literals
                let count = byte as usize;
                if in_pos + count > input.len() {
                    return Err(EbookDefenseError::UnexpectedEof);
                }
                if output.len() + count > PALMDOC_MAX_RECORD_SIZE {
                    return Err(EbookDefenseError::RecordBufferOverflow {
                        attempted_len: output.len() + count,
                        limit: PALMDOC_MAX_RECORD_SIZE,
                    });
                }
                output.extend_from_slice(&input[in_pos..in_pos + count]);
                in_pos += count;
            } else if byte <= 0x7F {
                // Literal single byte
                if output.len() >= PALMDOC_MAX_RECORD_SIZE {
                    return Err(EbookDefenseError::RecordBufferOverflow {
                        attempted_len: output.len() + 1,
                        limit: PALMDOC_MAX_RECORD_SIZE,
                    });
                }
                output.push(byte);
            } else if byte <= 0xBF {
                // 2-byte distance/length backreference pair
                if in_pos >= input.len() {
                    return Err(EbookDefenseError::UnexpectedEof);
                }
                let next_byte = input[in_pos];
                in_pos += 1;

                let pair = ((byte as u16) << 8) | (next_byte as u16);
                let distance = ((pair >> 3) & 0x07FF) as usize;
                let length = ((pair & 0x0007) as usize) + 3;

                if distance == 0 || distance > output.len() {
                    return Err(EbookDefenseError::IllegalBackreferenceDistance {
                        distance,
                        current_len: output.len(),
                    });
                }

                if output.len() + length > PALMDOC_MAX_RECORD_SIZE {
                    return Err(EbookDefenseError::RecordBufferOverflow {
                        attempted_len: output.len() + length,
                        limit: PALMDOC_MAX_RECORD_SIZE,
                    });
                }

                let start = output.len() - distance;
                for i in 0..length {
                    let b = output[start + i];
                    output.push(b);
                }
            } else {
                // 0xC0..=0xFF: Space character + (byte ^ 0x80)
                if output.len() + 2 > PALMDOC_MAX_RECORD_SIZE {
                    return Err(EbookDefenseError::RecordBufferOverflow {
                        attempted_len: output.len() + 2,
                        limit: PALMDOC_MAX_RECORD_SIZE,
                    });
                }
                output.push(b' ');
                output.push(byte ^ 0x80);
            }
        }

        Ok(output)
    }

    /// Compresses uncompressed textual data into a PalmDOC LZ77 record.
    pub fn compress_record(input: &[u8]) -> Result<Vec<u8>, EbookDefenseError> {
        if input.len() > PALMDOC_MAX_RECORD_SIZE {
            return Err(EbookDefenseError::RecordBufferOverflow {
                attempted_len: input.len(),
                limit: PALMDOC_MAX_RECORD_SIZE,
            });
        }

        let mut output = Vec::with_capacity(input.len() + 32);
        let mut pos = 0;

        while pos < input.len() {
            // Check for space + char shortcut (0xC0..=0xFF)
            if input[pos] == b' ' && pos + 1 < input.len() {
                let next_ch = input[pos + 1];
                if (0x40..=0x7F).contains(&next_ch) {
                    output.push(next_ch ^ 0x80);
                    pos += 2;
                    continue;
                }
            }

            // Check for LZ77 lookback match (min 3, max 10 bytes, max 2047 distance)
            let mut best_len = 0;
            let mut best_dist = 0;

            let max_lookback = pos.min(2047);
            let search_start = pos - max_lookback;

            for start in search_start..pos {
                let mut match_len = 0;
                while pos + match_len < input.len()
                    && match_len < 10
                    && input[start + match_len] == input[pos + match_len]
                {
                    match_len += 1;
                }

                if match_len >= 3 && match_len > best_len {
                    best_len = match_len;
                    best_dist = pos - start;
                    if best_len == 10 {
                        break;
                    }
                }
            }

            if best_len >= 3 {
                let distance = best_dist as u16;
                let length = (best_len - 3) as u16;
                let pair = 0x8000 | (distance << 3) | length;
                output.push((pair >> 8) as u8);
                output.push((pair & 0xFF) as u8);
                pos += best_len;
                continue;
            }

            // Literal byte encoding
            let byte = input[pos];
            if byte == 0x00 || (0x09..=0x7F).contains(&byte) {
                output.push(byte);
                pos += 1;
            } else {
                // Collect literals (1 to 8 bytes)
                let mut lit_end = pos;
                while lit_end < input.len()
                    && lit_end - pos < 8
                    && (input[lit_end] != 0x00 && !(0x09..=0x7F).contains(&input[lit_end]))
                {
                    lit_end += 1;
                }
                let count = lit_end - pos;
                if count > 0 {
                    output.push(count as u8);
                    output.extend_from_slice(&input[pos..lit_end]);
                    pos = lit_end;
                } else {
                    output.push(byte);
                    pos += 1;
                }
            }
        }

        Ok(output)
    }

    /// Parses MOBI EXTH header metadata records using checked arithmetic to prevent integer wrapping.
    pub fn parse_mobi_exth_records(data: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, EbookDefenseError> {
        if data.len() < 12 {
            return Ok(Vec::new());
        }

        // Magic "EXTH"
        if &data[0..4] != b"EXTH" {
            return Ok(Vec::new());
        }

        let header_len = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let record_count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;

        if header_len > data.len() {
            return Err(EbookDefenseError::ExthRecordOutOfBounds {
                record_len: header_len,
                remaining_bytes: data.len(),
            });
        }

        if record_count > MOBI_EXTH_MAX_RECORDS {
            return Err(EbookDefenseError::CorruptedBitstream(format!(
                "EXTH record count {} exceeds maximum allowable limit {}",
                record_count, MOBI_EXTH_MAX_RECORDS
            )));
        }

        let mut records = Vec::with_capacity(record_count.min(128));
        let mut offset = 12usize;

        for _ in 0..record_count {
            let next_header_end = offset
                .checked_add(8)
                .ok_or(EbookDefenseError::ExthIntegerOverflow)?;
            if next_header_end > header_len {
                break;
            }

            let rec_type = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let rec_len = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if rec_len < 8 {
                return Err(EbookDefenseError::CorruptedBitstream(
                    "EXTH record length smaller than 8-byte header".to_string(),
                ));
            }

            let next_offset = offset
                .checked_add(rec_len)
                .ok_or(EbookDefenseError::ExthIntegerOverflow)?;
            if next_offset > header_len {
                return Err(EbookDefenseError::ExthRecordOutOfBounds {
                    record_len: rec_len,
                    remaining_bytes: header_len.saturating_sub(offset),
                });
            }

            let payload = data[offset + 8..next_offset].to_vec();
            records.push((rec_type, payload));
            offset = next_offset;
        }

        Ok(records)
    }
}
