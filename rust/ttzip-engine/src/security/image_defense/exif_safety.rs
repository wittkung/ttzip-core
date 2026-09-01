// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! EXIF metadata safety guard: recursion depth, tag entry quotas, and circular loop circuit breaker.

use super::{
    ImageDefenseError, DEFAULT_MAX_EXIF_ENTRIES, DEFAULT_MAX_EXIF_RECURSION_DEPTH,
};

/// Summary metrics from a safe EXIF header inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExifInspectionSummary {
    pub tag_count: usize,
    pub max_depth: usize,
    pub ifd_chain_count: usize,
    pub is_little_endian: bool,
}

/// Defensive scanner validating EXIF TIFF structure against recursion bombs and loop pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExifSafetyGuard {
    pub max_depth: usize,
    pub max_entries: usize,
}

impl Default for ExifSafetyGuard {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_EXIF_RECURSION_DEPTH,
            max_entries: DEFAULT_MAX_EXIF_ENTRIES,
        }
    }
}

struct IfdTraversalState<'a> {
    data: &'a [u8],
    is_le: bool,
    visited_offsets: [usize; 64],
    visited_count: usize,
    total_tag_count: usize,
    max_depth_reached: usize,
    ifd_chain_count: usize,
}

impl ExifSafetyGuard {
    /// Creates a new EXIF safety guard with custom recursion and entry quotas.
    pub const fn new(max_depth: usize, max_entries: usize) -> Self {
        Self {
            max_depth,
            max_entries,
        }
    }

    /// Validates raw EXIF metadata bytes against circular loops, out-of-bounds offsets, and nesting limits.
    pub fn inspect(&self, raw_data: &[u8]) -> Result<ExifInspectionSummary, ImageDefenseError> {
        let tiff_data = if raw_data.starts_with(b"Exif\0\0") && raw_data.len() >= 14 {
            &raw_data[6..]
        } else {
            raw_data
        };

        if tiff_data.len() < 8 {
            return Err(ImageDefenseError::ExifMalformed {
                reason: "EXIF TIFF header shorter than 8 bytes".to_string(),
            });
        }

        let is_le = match &tiff_data[0..2] {
            b"II" => true,
            b"MM" => false,
            _ => {
                return Err(ImageDefenseError::ExifMalformed {
                    reason: "Invalid EXIF endian signature".to_string(),
                });
            }
        };

        let magic = if is_le {
            u16::from_le_bytes([tiff_data[2], tiff_data[3]])
        } else {
            u16::from_be_bytes([tiff_data[2], tiff_data[3]])
        };

        if magic != 42 && magic != 43 {
            return Err(ImageDefenseError::ExifMalformed {
                reason: format!("Invalid TIFF magic identifier: {magic}"),
            });
        }

        let first_ifd_off = if is_le {
            u32::from_le_bytes([tiff_data[4], tiff_data[5], tiff_data[6], tiff_data[7]]) as usize
        } else {
            u32::from_be_bytes([tiff_data[4], tiff_data[5], tiff_data[6], tiff_data[7]]) as usize
        };

        let mut state = IfdTraversalState {
            data: tiff_data,
            is_le,
            visited_offsets: [0usize; 64],
            visited_count: 0,
            total_tag_count: 0,
            max_depth_reached: 0,
            ifd_chain_count: 0,
        };

        self.traverse_ifd(first_ifd_off, 0, &mut state)?;

        Ok(ExifInspectionSummary {
            tag_count: state.total_tag_count,
            max_depth: state.max_depth_reached,
            ifd_chain_count: state.ifd_chain_count,
            is_little_endian: is_le,
        })
    }

    #[inline]
    fn read_u16(data: &[u8], off: usize, is_le: bool) -> Option<u16> {
        let b = data.get(off..off + 2)?;
        Some(if is_le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }

    #[inline]
    fn read_u32(data: &[u8], off: usize, is_le: bool) -> Option<u32> {
        let b = data.get(off..off + 4)?;
        Some(if is_le {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        })
    }

    fn traverse_ifd(
        &self,
        ifd_offset: usize,
        depth: usize,
        state: &mut IfdTraversalState<'_>,
    ) -> Result<(), ImageDefenseError> {
        if ifd_offset == 0 {
            return Ok(());
        }

        if depth > self.max_depth {
            return Err(ImageDefenseError::ExifRecursionLimitExceeded {
                depth,
                max_depth: self.max_depth,
            });
        }

        if depth > state.max_depth_reached {
            state.max_depth_reached = depth;
        }

        if ifd_offset >= state.data.len() || ifd_offset.checked_add(2).is_none() {
            return Err(ImageDefenseError::ExifMalformed {
                reason: format!("IFD offset 0x{ifd_offset:X} points outside TIFF boundary"),
            });
        }

        // Detect circular cycle
        for &offset in &state.visited_offsets[..state.visited_count] {
            if offset == ifd_offset {
                return Err(ImageDefenseError::ExifCycleDetected {
                    offset: ifd_offset,
                });
            }
        }

        if state.visited_count < state.visited_offsets.len() {
            state.visited_offsets[state.visited_count] = ifd_offset;
            state.visited_count += 1;
        }
        state.ifd_chain_count += 1;

        let num_entries = match Self::read_u16(state.data, ifd_offset, state.is_le) {
            Some(n) => n as usize,
            None => {
                return Err(ImageDefenseError::ExifMalformed {
                    reason: "Failed to read IFD entry count".to_string(),
                });
            }
        };

        let mut sub_ifd_offsets = [0usize; 8];
        let mut sub_ifd_count = 0;

        let mut entry_off = ifd_offset + 2;
        for _ in 0..num_entries {
            if entry_off + 12 > state.data.len() {
                return Err(ImageDefenseError::ExifMalformed {
                    reason: "IFD directory entry truncated before end of stream".to_string(),
                });
            }

            state.total_tag_count += 1;
            if state.total_tag_count > self.max_entries {
                return Err(ImageDefenseError::ExifTagCountExceeded {
                    count: state.total_tag_count,
                    max_count: self.max_entries,
                });
            }

            let tag = Self::read_u16(state.data, entry_off, state.is_le).unwrap_or(0);
            let ftype = Self::read_u16(state.data, entry_off + 2, state.is_le).unwrap_or(0);
            let count = Self::read_u32(state.data, entry_off + 4, state.is_le).unwrap_or(0) as usize;
            let val_or_off = Self::read_u32(state.data, entry_off + 8, state.is_le).unwrap_or(0) as usize;

            let type_size = match ftype {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => 1,
            };

            let total_val_bytes = count.saturating_mul(type_size);
            if total_val_bytes > 4 && val_or_off.saturating_add(total_val_bytes) > state.data.len() {
                return Err(ImageDefenseError::ExifMalformed {
                    reason: format!(
                        "Tag 0x{tag:04X} data offset 0x{val_or_off:X} (+{total_val_bytes} bytes) overflows TIFF length"
                    ),
                });
            }

            // Check for SubIFD pointers (0x8769: ExifIFD, 0x8825: GPSInfo, 0xA005: InteroperabilityIFD, 0x014A: SubIFDs)
            if (tag == 0x8769 || tag == 0x8825 || tag == 0xA005 || tag == 0x014A)
                && val_or_off > 0
                && val_or_off < state.data.len()
                && sub_ifd_count < sub_ifd_offsets.len()
            {
                sub_ifd_offsets[sub_ifd_count] = val_or_off;
                sub_ifd_count += 1;
            }

            entry_off += 12;
        }

        // Next IFD offset in linear chain
        let next_ifd_off = Self::read_u32(state.data, entry_off, state.is_le).unwrap_or(0) as usize;
        if next_ifd_off != 0 && next_ifd_off < state.data.len() {
            self.traverse_ifd(next_ifd_off, depth, state)?;
        }

        // Traverse child SubIFDs at depth + 1
        for &sub_off in &sub_ifd_offsets[..sub_ifd_count] {
            if sub_off != 0 && sub_off < state.data.len() {
                self.traverse_ifd(sub_off, depth + 1, state)?;
            }
        }

        Ok(())
    }
}
