// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pixel Bomb decompression fuse and zero-allocation dimension inspection guard.

use super::{
    ImageDefenseError, DEFAULT_MAX_IMAGE_DIMENSION, DEFAULT_MAX_IMAGE_EXPANSION_RATIO,
    DEFAULT_MAX_UNCOMPRESSED_MEMORY,
};

/// Dimensions and channel configuration extracted from image headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub bit_depth: u8,
}

/// Guard preventing decompression bombs, oversized canvases, and extreme memory expansion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelBombGuard {
    pub max_dimension: u32,
    pub max_uncompressed_memory: usize,
    pub max_expansion_ratio: f64,
}

impl Default for PixelBombGuard {
    fn default() -> Self {
        Self {
            max_dimension: DEFAULT_MAX_IMAGE_DIMENSION,
            max_uncompressed_memory: DEFAULT_MAX_UNCOMPRESSED_MEMORY,
            max_expansion_ratio: DEFAULT_MAX_IMAGE_EXPANSION_RATIO,
        }
    }
}

impl PixelBombGuard {
    /// Creates a guard with specified limits.
    pub const fn new(
        max_dimension: u32,
        max_uncompressed_memory: usize,
        max_expansion_ratio: f64,
    ) -> Self {
        Self {
            max_dimension,
            max_uncompressed_memory,
            max_expansion_ratio,
        }
    }

    /// Fast zero-allocation probe for image dimensions and safety validation.
    pub fn inspect_and_validate(&self, data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        let dims = Self::inspect_dimensions(data)?;
        self.validate(dims.width, dims.height, dims.channels as usize, data.len())?;
        Ok(dims)
    }

    /// Validates pre-computed dimensions and channel configuration against safety thresholds.
    pub fn validate(
        &self,
        width: u32,
        height: u32,
        channels: usize,
        input_len: usize,
    ) -> Result<(), ImageDefenseError> {
        if width > self.max_dimension {
            return Err(ImageDefenseError::DimensionLimitExceeded {
                dim: width,
                max_dim: self.max_dimension,
                axis: "width",
            });
        }
        if height > self.max_dimension {
            return Err(ImageDefenseError::DimensionLimitExceeded {
                dim: height,
                max_dim: self.max_dimension,
                axis: "height",
            });
        }

        let ch = channels.max(1);
        let pixel_count = (width as usize).saturating_mul(height as usize);
        let uncompressed_bytes = pixel_count.saturating_mul(ch);

        let input_size = input_len.max(1);
        let ratio = uncompressed_bytes as f64 / input_size as f64;

        if uncompressed_bytes > self.max_uncompressed_memory || ratio > self.max_expansion_ratio {
            return Err(ImageDefenseError::PixelBombDetected {
                width,
                height,
                uncompressed_bytes,
                max_bytes: self.max_uncompressed_memory,
                ratio,
                max_ratio: self.max_expansion_ratio,
            });
        }

        Ok(())
    }

    /// Probes image stream header to extract dimensions without full decoding.
    pub fn inspect_dimensions(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 4 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 4,
                actual_len: data.len(),
            });
        }

        if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Self::probe_png(data);
        }
        if data.starts_with(&[0xFF, 0xD8]) {
            return Self::probe_jpeg(data);
        }
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
            return Self::probe_webp(data);
        }
        if data.starts_with(b"qoif") {
            return Self::probe_qoi(data);
        }
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return Self::probe_gif(data);
        }
        if data.starts_with(b"BM") {
            return Self::probe_bmp(data);
        }
        if data.starts_with(b"II\x2A\x00") || data.starts_with(b"MM\x00\x2A") {
            return Self::probe_tiff(data);
        }

        Err(ImageDefenseError::GeneralDefenseError(
            "Unrecognized or unsupported image container magic".to_string(),
        ))
    }

    fn probe_png(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 24 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 24,
                actual_len: data.len(),
            });
        }
        if &data[12..16] != b"IHDR" {
            return Err(ImageDefenseError::MalformedChunk {
                chunk_type: "IHDR".to_string(),
                offset: 12,
                reason: "First PNG chunk must be IHDR".to_string(),
            });
        }
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let bit_depth = data.get(24).copied().unwrap_or(8);
        let color_type = data.get(25).copied().unwrap_or(6);
        let channels = match color_type {
            0 => 1, // Grayscale
            2 => 3, // RGB
            3 => 3, // Indexed (expands to RGB/RGBA)
            4 => 2, // Grayscale + Alpha
            6 => 4, // RGBA
            _ => 4,
        };
        Ok(ImageDimensions {
            width,
            height,
            channels,
            bit_depth,
        })
    }

    fn probe_jpeg(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        let mut pos = 2;
        while pos + 4 <= data.len() {
            if data[pos] != 0xFF {
                pos += 1;
                continue;
            }
            let marker = data[pos + 1];
            pos += 2;

            if marker == 0xD9 || marker == 0xDA {
                // EOI or SOS marker
                break;
            }
            if marker == 0x00 || (0xD0..=0xD7).contains(&marker) {
                // Restart markers or byte-stuffed 0x00
                continue;
            }

            if pos + 2 > data.len() {
                break;
            }
            let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
            if len < 2 {
                return Err(ImageDefenseError::MalformedChunk {
                    chunk_type: format!("JPEG 0xFF{:02X}", marker),
                    offset: pos - 2,
                    reason: "Invalid marker segment length < 2".to_string(),
                });
            }

            // SOF markers: SOF0 (0xC0), SOF1 (0xC1), SOF2 (0xC2), SOF3 (0xC3),
            // SOF5..SOF7, SOF9..SOF11, SOF13..SOF15
            let is_sof = matches!(
                marker,
                0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
            );

            if is_sof && pos + 7 <= data.len() {
                let bit_depth = data[pos + 2];
                let height = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as u32;
                let width = u16::from_be_bytes([data[pos + 5], data[pos + 6]]) as u32;
                let channels = if pos + 8 <= data.len() {
                    data[pos + 7]
                } else {
                    3
                };
                return Ok(ImageDimensions {
                    width,
                    height,
                    channels: channels.max(1),
                    bit_depth,
                });
            }

            pos = pos.saturating_add(len);
        }

        Err(ImageDefenseError::GeneralDefenseError(
            "JPEG missing SOF segment".to_string(),
        ))
    }

    fn probe_webp(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 16 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 16,
                actual_len: data.len(),
            });
        }
        let chunk_fourcc = &data[12..16];
        if chunk_fourcc == b"VP8 " {
            if data.len() < 30 {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: 30,
                    actual_len: data.len(),
                });
            }
            // Lossy VP8 frame header tag
            let width = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
            let height = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
            return Ok(ImageDimensions {
                width,
                height,
                channels: 3,
                bit_depth: 8,
            });
        } else if chunk_fourcc == b"VP8L" {
            if data.len() < 25 {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: 25,
                    actual_len: data.len(),
                });
            }
            // Lossless VP8L 1-byte signature 0x2F followed by 4 dimension bytes
            let b0 = data[21] as u32;
            let b1 = data[22] as u32;
            let b2 = data[23] as u32;
            let b3 = data[24] as u32;
            let width = 1 + (((b1 & 0x3F) << 8) | b0);
            let height = 1 + (((b3 & 0x0F) << 10) | (b2 << 2) | ((b1 & 0xC0) >> 6));
            return Ok(ImageDimensions {
                width,
                height,
                channels: 4,
                bit_depth: 8,
            });
        } else if chunk_fourcc == b"VP8X" {
            if data.len() < 30 {
                return Err(ImageDefenseError::TruncatedStream {
                    expected_len: 30,
                    actual_len: data.len(),
                });
            }
            let w_bytes = [data[24], data[25], data[26], 0];
            let h_bytes = [data[27], data[28], data[29], 0];
            let width = 1 + u32::from_le_bytes(w_bytes);
            let height = 1 + u32::from_le_bytes(h_bytes);
            return Ok(ImageDimensions {
                width,
                height,
                channels: 4,
                bit_depth: 8,
            });
        }

        Err(ImageDefenseError::GeneralDefenseError(
            "Unsupported WebP chunk format".to_string(),
        ))
    }

    fn probe_qoi(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 14 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 14,
                actual_len: data.len(),
            });
        }
        let width = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let height = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let channels = data[12];
        Ok(ImageDimensions {
            width,
            height,
            channels: if channels == 3 || channels == 4 {
                channels
            } else {
                4
            },
            bit_depth: 8,
        })
    }

    fn probe_gif(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 10 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 10,
                actual_len: data.len(),
            });
        }
        let width = u16::from_le_bytes([data[6], data[7]]) as u32;
        let height = u16::from_le_bytes([data[8], data[9]]) as u32;
        Ok(ImageDimensions {
            width,
            height,
            channels: 4,
            bit_depth: 8,
        })
    }

    fn probe_bmp(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 26 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 26,
                actual_len: data.len(),
            });
        }
        let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]).unsigned_abs();
        let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).unsigned_abs();
        let bpp = if data.len() >= 30 {
            u16::from_le_bytes([data[28], data[29]])
        } else {
            24
        };
        let channels = if bpp == 32 { 4 } else { 3 };
        Ok(ImageDimensions {
            width,
            height,
            channels,
            bit_depth: 8,
        })
    }

    fn probe_tiff(data: &[u8]) -> Result<ImageDimensions, ImageDefenseError> {
        if data.len() < 8 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 8,
                actual_len: data.len(),
            });
        }
        let is_le = &data[0..2] == b"II";
        let read_u16 = |off: usize| -> Option<u16> {
            let b = data.get(off..off + 2)?;
            Some(if is_le {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            })
        };
        let read_u32 = |off: usize| -> Option<u32> {
            let b = data.get(off..off + 4)?;
            Some(if is_le {
                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
            } else {
                u32::from_be_bytes([b[0], b[1], b[2], b[3]])
            })
        };

        let ifd_off = read_u32(4).unwrap_or(8) as usize;
        let num_entries = read_u16(ifd_off).unwrap_or(0) as usize;
        let mut width = 0;
        let mut height = 0;

        let mut entry_off = ifd_off + 2;
        for _ in 0..num_entries.min(64) {
            if entry_off + 12 > data.len() {
                break;
            }
            let tag = read_u16(entry_off).unwrap_or(0);
            let ftype = read_u16(entry_off + 2).unwrap_or(0);
            let val = if ftype == 3 {
                read_u16(entry_off + 8).unwrap_or(0) as u32
            } else {
                read_u32(entry_off + 8).unwrap_or(0)
            };

            if tag == 0x0100 {
                width = val;
            } else if tag == 0x0101 {
                height = val;
            }
            entry_off += 12;
        }

        if width > 0 && height > 0 {
            Ok(ImageDimensions {
                width,
                height,
                channels: 4,
                bit_depth: 8,
            })
        } else {
            Err(ImageDefenseError::GeneralDefenseError(
                "TIFF missing ImageWidth/ImageLength tags".to_string(),
            ))
        }
    }
}
