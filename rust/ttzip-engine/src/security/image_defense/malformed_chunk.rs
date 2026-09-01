// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed chunk isolation, stream sanitization, and self-healing recovery guard.

use crc32fast::Hasher;

use super::ImageDefenseError;

/// Report detailing chunk inspection and self-healing sanitization outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedChunkReport {
    pub chunk_count: usize,
    pub is_sanitized: bool,
    pub stripped_chunks: usize,
    pub recovered_bytes: usize,
}

/// Guard preventing crashes and memory corruption from malformed, truncated, or hostile chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MalformedChunkGuard;

impl MalformedChunkGuard {
    /// Inspects and verifies the integrity of chunks in an image container stream.
    pub fn inspect_and_validate(data: &[u8]) -> Result<SanitizedChunkReport, ImageDefenseError> {
        if data.is_empty() {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 1,
                actual_len: 0,
            });
        }

        if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Self::validate_png(data)
        } else if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
            Self::validate_webp(data)
        } else if data.starts_with(&[0xFF, 0xD8]) {
            Self::validate_jpeg(data)
        } else {
            // General format with minimal length check
            Ok(SanitizedChunkReport {
                chunk_count: 1,
                is_sanitized: false,
                stripped_chunks: 0,
                recovered_bytes: data.len(),
            })
        }
    }

    /// Performs self-healing sanitization on PNG streams, stripping corrupted ancillary chunks
    /// or adding a synthetic IEND if the stream was prematurely truncated after valid IDAT data.
    pub fn sanitize_png(data: &[u8]) -> Result<(Vec<u8>, SanitizedChunkReport), ImageDefenseError> {
        if data.len() < 8 || !data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Err(ImageDefenseError::GeneralDefenseError(
                "Invalid PNG signature for sanitization".to_string(),
            ));
        }

        let mut output = Vec::with_capacity(data.len() + 12);
        output.extend_from_slice(&data[..8]);

        let mut offset = 8;
        let mut chunk_count = 0;
        let mut stripped_chunks = 0;
        let mut seen_ihdr = false;
        let mut seen_idat = false;
        let mut seen_iend = false;

        while offset + 8 <= data.len() {
            let chunk_len = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            let chunk_type = &data[offset + 4..offset + 8];
            let chunk_type_str = String::from_utf8_lossy(chunk_type).to_string();

            // Total chunk block = 4 (length) + 4 (type) + chunk_len + 4 (crc)
            let total_chunk_len = match chunk_len.checked_add(12) {
                Some(len) => len,
                None => {
                    return Err(ImageDefenseError::MalformedChunk {
                        chunk_type: chunk_type_str,
                        offset,
                        reason: "Chunk length arithmetic overflow".to_string(),
                    });
                }
            };

            // First chunk must be IHDR
            if !seen_ihdr {
                if chunk_type != b"IHDR" {
                    return Err(ImageDefenseError::MalformedChunk {
                        chunk_type: chunk_type_str,
                        offset,
                        reason: "First PNG chunk must be IHDR".to_string(),
                    });
                }
                seen_ihdr = true;
            }

            // Check if chunk extends past EOF
            if offset + total_chunk_len > data.len() {
                // Critical chunks (IHDR, IDAT, PLTE) cannot be truncated
                let is_critical = chunk_type[0].is_ascii_uppercase();
                if is_critical && chunk_type != b"IEND" {
                    return Err(ImageDefenseError::TruncatedStream {
                        expected_len: offset + total_chunk_len,
                        actual_len: data.len(),
                    });
                }
                // Ancillary chunk truncated -> strip and stop
                stripped_chunks += 1;
                break;
            }

            // Verify CRC32
            let expected_crc = u32::from_be_bytes([
                data[offset + 8 + chunk_len],
                data[offset + 8 + chunk_len + 1],
                data[offset + 8 + chunk_len + 2],
                data[offset + 8 + chunk_len + 3],
            ]);
            let mut hasher = Hasher::new();
            hasher.update(&data[offset + 4..offset + 8 + chunk_len]);
            let computed_crc = hasher.finalize();

            if computed_crc != expected_crc {
                let is_critical = chunk_type[0].is_ascii_uppercase();
                if is_critical {
                    return Err(ImageDefenseError::MalformedChunk {
                        chunk_type: chunk_type_str,
                        offset,
                        reason: format!(
                            "Critical chunk CRC mismatch: expected 0x{expected_crc:08X}, computed 0x{computed_crc:08X}"
                        ),
                    });
                }
                // Non-critical ancillary chunk corrupted -> strip it safely
                stripped_chunks += 1;
                offset += total_chunk_len;
                continue;
            }

            if chunk_type == b"IDAT" {
                seen_idat = true;
            } else if chunk_type == b"IEND" {
                seen_iend = true;
                output.extend_from_slice(&data[offset..offset + total_chunk_len]);
                chunk_count += 1;
                break;
            }

            output.extend_from_slice(&data[offset..offset + total_chunk_len]);
            chunk_count += 1;
            offset += total_chunk_len;
        }

        if !seen_idat {
            return Err(ImageDefenseError::MalformedChunk {
                chunk_type: "IDAT".to_string(),
                offset,
                reason: "PNG stream contains no IDAT image data chunks".to_string(),
            });
        }

        // Self-healing: if stream ended prematurely without IEND, synthesize a valid IEND chunk
        if !seen_iend {
            let iend_chunk = [
                0x00, 0x00, 0x00, 0x00, // length 0
                b'I', b'E', b'N', b'D', // chunk type
                0xAE, 0x42, 0x60, 0x82, // CRC32
            ];
            output.extend_from_slice(&iend_chunk);
            chunk_count += 1;
        }

        let is_sanitized = stripped_chunks > 0 || !seen_iend;
        let recovered_len = output.len();

        Ok((
            output,
            SanitizedChunkReport {
                chunk_count,
                is_sanitized,
                stripped_chunks,
                recovered_bytes: recovered_len,
            },
        ))
    }

    fn validate_png(data: &[u8]) -> Result<SanitizedChunkReport, ImageDefenseError> {
        let mut offset = 8;
        let mut chunk_count = 0;
        let mut seen_ihdr = false;
        let mut seen_idat = false;
        let mut seen_iend = false;

        while offset + 8 <= data.len() {
            let chunk_len = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            let chunk_type = &data[offset + 4..offset + 8];
            let chunk_type_str = String::from_utf8_lossy(chunk_type).to_string();

            let total_chunk_len = match chunk_len.checked_add(12) {
                Some(len) => len,
                None => {
                    return Err(ImageDefenseError::MalformedChunk {
                        chunk_type: chunk_type_str,
                        offset,
                        reason: "Chunk length overflow".to_string(),
                    });
                }
            };

            if !seen_ihdr {
                if chunk_type != b"IHDR" {
                    return Err(ImageDefenseError::MalformedChunk {
                        chunk_type: chunk_type_str,
                        offset,
                        reason: "First PNG chunk must be IHDR".to_string(),
                    });
                }
                seen_ihdr = true;
            }

            if offset + total_chunk_len > data.len() {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: offset + total_chunk_len,
                    actual_len: data.len(),
                });
            }

            // CRC check
            let expected_crc = u32::from_be_bytes([
                data[offset + 8 + chunk_len],
                data[offset + 8 + chunk_len + 1],
                data[offset + 8 + chunk_len + 2],
                data[offset + 8 + chunk_len + 3],
            ]);
            let mut hasher = Hasher::new();
            hasher.update(&data[offset + 4..offset + 8 + chunk_len]);
            let computed_crc = hasher.finalize();

            if computed_crc != expected_crc {
                return Err(ImageDefenseError::MalformedChunk {
                    chunk_type: chunk_type_str,
                    offset,
                    reason: format!(
                        "CRC mismatch: expected 0x{expected_crc:08X}, computed 0x{computed_crc:08X}"
                    ),
                });
            }

            if chunk_type == b"IDAT" {
                seen_idat = true;
            } else if chunk_type == b"IEND" {
                seen_iend = true;
                chunk_count += 1;
                break;
            }

            chunk_count += 1;
            offset += total_chunk_len;
        }

        if !seen_ihdr || !seen_idat || !seen_iend {
            return Err(ImageDefenseError::MalformedChunk {
                chunk_type: "PNG".to_string(),
                offset,
                reason: format!(
                    "Incomplete PNG chunk sequence: ihdr={seen_ihdr}, idat={seen_idat}, iend={seen_iend}"
                ),
            });
        }

        Ok(SanitizedChunkReport {
            chunk_count,
            is_sanitized: false,
            stripped_chunks: 0,
            recovered_bytes: data.len(),
        })
    }

    fn validate_webp(data: &[u8]) -> Result<SanitizedChunkReport, ImageDefenseError> {
        if data.len() < 12 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 12,
                actual_len: data.len(),
            });
        }

        let riff_payload_len =
            u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let expected_total = riff_payload_len.saturating_add(8);

        if data.len() < expected_total {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: expected_total,
                actual_len: data.len(),
            });
        }

        let mut offset = 12;
        let mut chunk_count = 0;

        while offset + 8 <= expected_total.min(data.len()) {
            let chunk_fourcc = &data[offset..offset + 4];
            let chunk_fourcc_str = String::from_utf8_lossy(chunk_fourcc).to_string();
            let chunk_len = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            let padded_len = (chunk_len + 1) & !1;
            let total_chunk_size = match padded_len.checked_add(8) {
                Some(s) => s,
                None => {
                    return Err(ImageDefenseError::MalformedChunk {
                        chunk_type: chunk_fourcc_str,
                        offset,
                        reason: "WebP chunk length overflow".to_string(),
                    });
                }
            };

            if offset + total_chunk_size > data.len() {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: offset + total_chunk_size,
                    actual_len: data.len(),
                });
            }

            chunk_count += 1;
            offset += total_chunk_size;
        }

        Ok(SanitizedChunkReport {
            chunk_count,
            is_sanitized: false,
            stripped_chunks: 0,
            recovered_bytes: data.len(),
        })
    }

    fn validate_jpeg(data: &[u8]) -> Result<SanitizedChunkReport, ImageDefenseError> {
        let mut pos = 2;
        let mut chunk_count = 0;
        let mut seen_sof = false;

        while pos + 1 < data.len() {
            if data[pos] != 0xFF {
                pos += 1;
                continue;
            }
            let marker = data[pos + 1];
            pos += 2;

            if marker == 0xD9 {
                // EOI
                chunk_count += 1;
                break;
            }
            if marker == 0xDA {
                // SOS (Start of Scan) - scan until EOI
                chunk_count += 1;
                seen_sof = true;
                break;
            }
            if marker == 0x00 || (0xD0..=0xD7).contains(&marker) {
                // Restart markers
                continue;
            }

            if pos + 2 > data.len() {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: pos + 2,
                    actual_len: data.len(),
                });
            }

            let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
            if len < 2 {
                return Err(ImageDefenseError::MalformedChunk {
                    chunk_type: format!("JPEG 0xFF{marker:02X}"),
                    offset: pos - 2,
                    reason: "Marker segment length < 2".to_string(),
                });
            }

            let is_sof = matches!(
                marker,
                0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
            );
            if is_sof {
                seen_sof = true;
            }

            if pos + len > data.len() {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: pos + len,
                    actual_len: data.len(),
                });
            }

            chunk_count += 1;
            pos += len;
        }

        if !seen_sof {
            return Err(ImageDefenseError::MalformedChunk {
                chunk_type: "JPEG".to_string(),
                offset: pos,
                reason: "JPEG stream missing SOF or SOS segment".to_string(),
            });
        }

        Ok(SanitizedChunkReport {
            chunk_count,
            is_sanitized: false,
            stripped_chunks: 0,
            recovered_bytes: data.len(),
        })
    }
}
