// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ICC color profile poisoning and multi-dimensional CLUT memory explosion defense guard.

use super::{
    ImageDefenseError, DEFAULT_MAX_ICC_CLUT_MEMORY, DEFAULT_MAX_ICC_PROFILE_SIZE,
};

/// Summary metrics from a safe ICC profile inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IccInspectionSummary {
    pub profile_size: usize,
    pub tag_count: usize,
    pub max_clut_memory_bytes: usize,
    pub color_space: String,
    pub pcs: String,
}

/// Guard preventing ICC profile memory bombs, corrupted tag offsets, and poisoned LUT tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IccProfileGuard {
    pub max_profile_size: usize,
    pub max_clut_memory: usize,
}

impl Default for IccProfileGuard {
    fn default() -> Self {
        Self {
            max_profile_size: DEFAULT_MAX_ICC_PROFILE_SIZE,
            max_clut_memory: DEFAULT_MAX_ICC_CLUT_MEMORY,
        }
    }
}

impl IccProfileGuard {
    /// Creates an ICC profile guard with custom size and CLUT memory limits.
    pub const fn new(max_profile_size: usize, max_clut_memory: usize) -> Self {
        Self {
            max_profile_size,
            max_clut_memory,
        }
    }

    /// Validates raw ICC profile payload against size quotas, tag bounds, and CLUT explosion.
    pub fn inspect(&self, data: &[u8]) -> Result<IccInspectionSummary, ImageDefenseError> {
        if data.len() < 128 {
            return Err(ImageDefenseError::TruncatedStream {
                expected_len: 128,
                actual_len: data.len(),
            });
        }

        if data.len() > self.max_profile_size {
            return Err(ImageDefenseError::IccProfileSizeExceeded {
                size: data.len(),
                max_size: self.max_profile_size,
            });
        }

        // ICC Profile Header (128 bytes)
        let declared_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if declared_size > data.len() || declared_size < 128 {
            return Err(ImageDefenseError::IccMalformed {
                reason: format!(
                    "Declared ICC profile size {declared_size} exceeds actual buffer {}",
                    data.len()
                ),
            });
        }

        // Magic signature 'acsp' at offset 36..40
        if &data[36..40] != b"acsp" {
            return Err(ImageDefenseError::IccMalformed {
                reason: "Missing standard 'acsp' ICC profile magic signature".to_string(),
            });
        }

        let color_space = String::from_utf8_lossy(&data[16..20]).to_string();
        let pcs = String::from_utf8_lossy(&data[20..24]).to_string();

        // Tag table starts at byte 128 if present
        let tag_count = if data.len() >= 132 {
            u32::from_be_bytes([data[128], data[129], data[130], data[131]]) as usize
        } else {
            0
        };
        if tag_count > 256 {
            return Err(ImageDefenseError::IccMalformed {
                reason: format!("Excessive tag count {tag_count} in ICC profile"),
            });
        }

        let mut max_clut_bytes = 0usize;
        let mut tag_offset = 132;

        for _ in 0..tag_count {
            if tag_offset + 12 > data.len() {
                return Err(ImageDefenseError::IccMalformed {
                    reason: "ICC tag table truncated before declared count".to_string(),
                });
            }

            let tag_sig = &data[tag_offset..tag_offset + 4];
            let element_offset = u32::from_be_bytes([
                data[tag_offset + 4],
                data[tag_offset + 5],
                data[tag_offset + 6],
                data[tag_offset + 7],
            ]) as usize;
            let element_size = u32::from_be_bytes([
                data[tag_offset + 8],
                data[tag_offset + 9],
                data[tag_offset + 10],
                data[tag_offset + 11],
            ]) as usize;

            let tag_end = match element_offset.checked_add(element_size) {
                Some(end) => end,
                None => {
                    return Err(ImageDefenseError::IccMalformed {
                        reason: "ICC tag offset arithmetic overflow".to_string(),
                    });
                }
            };

            if tag_end > data.len() {
                return Err(ImageDefenseError::IccMalformed {
                    reason: format!(
                        "Tag '{}' at 0x{element_offset:X} (+{element_size} bytes) overflows profile buffer",
                        String::from_utf8_lossy(tag_sig)
                    ),
                });
            }

            // Inspect potential CLUT tags
            let is_lut_tag = matches!(
                tag_sig,
                b"A2B0"
                    | b"A2B1"
                    | b"A2B2"
                    | b"B2A0"
                    | b"B2A1"
                    | b"B2A2"
                    | b"mAB "
                    | b"mBA "
                    | b"D2B0"
                    | b"D2B1"
                    | b"D2B2"
                    | b"B2D0"
                    | b"B2D1"
                    | b"B2D2"
            );

            if is_lut_tag && element_size >= 8 {
                let clut_mem = Self::calculate_clut_memory(data, element_offset, element_size)?;
                if clut_mem > max_clut_bytes {
                    max_clut_bytes = clut_mem;
                }
                if clut_mem > self.max_clut_memory {
                    return Err(ImageDefenseError::IccClutMemoryExceeded {
                        bytes: clut_mem,
                        max_bytes: self.max_clut_memory,
                    });
                }
            }

            tag_offset += 12;
        }

        Ok(IccInspectionSummary {
            profile_size: declared_size,
            tag_count,
            max_clut_memory_bytes: max_clut_bytes,
            color_space,
            pcs,
        })
    }

    fn calculate_clut_memory(
        data: &[u8],
        tag_offset: usize,
        tag_size: usize,
    ) -> Result<usize, ImageDefenseError> {
        let tag_data = match data.get(tag_offset..tag_offset + tag_size) {
            Some(s) => s,
            None => return Ok(0),
        };

        if tag_data.len() < 8 {
            return Ok(0);
        }

        let type_sig = &tag_data[0..4];

        if type_sig == b"mAB " || type_sig == b"mBA " {
            if tag_data.len() < 32 {
                return Ok(0);
            }
            let input_channels = tag_data[8] as usize;
            let output_channels = tag_data[9] as usize;
            let clut_offset =
                u32::from_be_bytes([tag_data[24], tag_data[25], tag_data[26], tag_data[27]])
                    as usize;

            if clut_offset > 0 && clut_offset + 20 <= tag_data.len() {
                let clut_header = &tag_data[clut_offset..];
                let mut total_points = 1usize;
                let num_dims = input_channels.min(16);
                for i in 0..num_dims {
                    let grid_points = clut_header.get(i).copied().unwrap_or(0) as usize;
                    if grid_points > 0 {
                        total_points = total_points.saturating_mul(grid_points);
                    }
                }
                let precision = clut_header.get(16).copied().unwrap_or(1) as usize;
                let bytes_per_sample = if precision == 2 { 2 } else { 1 };
                let memory_needed = total_points
                    .saturating_mul(output_channels.max(1))
                    .saturating_mul(bytes_per_sample);
                return Ok(memory_needed);
            }
        } else if type_sig == b"mft1" {
            // lut8Type: 48 bytes header
            if tag_data.len() >= 48 {
                let in_chan = tag_data[8] as usize;
                let out_chan = tag_data[9] as usize;
                let grid_pts = tag_data[10] as usize;
                let mut total_pts = 1usize;
                for _ in 0..in_chan.min(8) {
                    total_pts = total_pts.saturating_mul(grid_pts);
                }
                return Ok(total_pts.saturating_mul(out_chan.max(1)));
            }
        } else if type_sig == b"mft2" {
            // lut16Type: 48 bytes header + 2-byte samples
            if tag_data.len() >= 48 {
                let in_chan = tag_data[8] as usize;
                let out_chan = tag_data[9] as usize;
                let grid_pts = tag_data[10] as usize;
                let mut total_pts = 1usize;
                for _ in 0..in_chan.min(8) {
                    total_pts = total_pts.saturating_mul(grid_pts);
                }
                return Ok(total_pts.saturating_mul(out_chan.max(1)).saturating_mul(2));
            }
        }

        Ok(0)
    }
}
