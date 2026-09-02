// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ISOBMFF Atom / Matroska EBML / RIFF Box Depth Guard and 64-bit Largesize Sanitizer.
//!
//! Enforces deterministic parsing safety against nested container depth bombs (depth <= 16),
//! 64-bit largesize arithmetic overflow exploits, truncated box sizes, and out-of-bounds offsets.

use super::{VideoDefenseError, DEFAULT_MAX_ATOM_DEPTH};

/// Known ISOBMFF / QuickTime container atom four-character codes that can contain child atoms.
const CONTAINER_BOX_TYPES: &[[u8; 4]] = &[
    *b"moov", *b"trak", *b"mdia", *b"minf", *b"dinf", *b"stbl", *b"mvex", *b"moof", *b"traf",
    *b"mfra", *b"udta", *b"edts", *b"sinf", *b"schi", *b"clip", *b"matt", *b"kmat",
];

/// Information about an active box frame in the parsing stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomFrame {
    /// 4-byte box/atom type identifier (e.g., `*b"moov"`).
    pub box_type: [u8; 4],
    /// Absolute start offset of the box header in the stream.
    pub start_offset: u64,
    /// Total byte size of the box including header and payload.
    pub total_size: u64,
    /// Header size in bytes (8 for standard 32-bit, 16 for 64-bit largesize).
    pub header_size: u8,
    /// Stack depth of this atom (1-indexed).
    pub depth: usize,
}

impl AtomFrame {
    /// Returns the ASCII string representation of the 4-byte box type.
    pub fn box_type_str(&self) -> String {
        format_fourcc(&self.box_type)
    }

    /// End offset in the stream (exclusive).
    #[inline]
    pub fn end_offset(&self) -> u64 {
        self.start_offset.saturating_add(self.total_size)
    }
}

/// Parsed box header with validated size and offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedBoxHeader {
    /// 4-byte box type identifier.
    pub box_type: [u8; 4],
    /// Total box size in bytes.
    pub total_size: u64,
    /// Header size in bytes (8 or 16).
    pub header_size: u8,
    /// Whether this box uses 64-bit largesize.
    pub is_largesize: bool,
}

/// Summary report of container atom hierarchy scanning.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AtomInspectionSummary {
    /// Total number of atoms/boxes parsed across all levels.
    pub total_boxes: usize,
    /// Maximum nesting depth reached during traversal.
    pub max_depth_reached: usize,
    /// Top-level box four-character codes found in the container.
    pub top_level_boxes: Vec<[u8; 4]>,
}

/// Guard preventing atom nesting recursion bombs and arithmetic overflow in video containers.
#[derive(Debug, Clone)]
pub struct AtomDepthGuard {
    max_depth: usize,
    stack: Vec<AtomFrame>,
    total_boxes_scanned: usize,
    max_depth_reached: usize,
}

impl Default for AtomDepthGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomDepthGuard {
    /// Creates a new guard with default maximum depth of 16.
    pub const fn new() -> Self {
        Self {
            max_depth: DEFAULT_MAX_ATOM_DEPTH,
            stack: Vec::new(),
            total_boxes_scanned: 0,
            max_depth_reached: 0,
        }
    }

    /// Creates a new guard with custom maximum depth threshold.
    pub const fn with_max_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            stack: Vec::new(),
            total_boxes_scanned: 0,
            max_depth_reached: 0,
        }
    }

    /// Returns the maximum allowed atom depth.
    #[inline]
    pub const fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Returns the current nesting depth.
    #[inline]
    pub fn current_depth(&self) -> usize {
        self.stack.len()
    }

    /// Returns the total number of boxes scanned so far.
    #[inline]
    pub const fn total_boxes_scanned(&self) -> usize {
        self.total_boxes_scanned
    }

    /// Returns the maximum depth reached during scanning.
    #[inline]
    pub const fn max_depth_reached(&self) -> usize {
        self.max_depth_reached
    }

    /// Resets the internal stack and counters.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.total_boxes_scanned = 0;
        self.max_depth_reached = 0;
    }

    /// Parses a standard 8-byte or 16-byte ISOBMFF box header from bytes at the given offset.
    pub fn parse_box_header(
        data: &[u8],
        current_offset: u64,
        total_len: Option<u64>,
    ) -> Result<ParsedBoxHeader, VideoDefenseError> {
        if data.len() < 8 {
            return Err(VideoDefenseError::MalformedContainerHeader {
                reason: format!(
                    "Insufficient bytes ({}) for standard 8-byte box header",
                    data.len()
                ),
            });
        }

        let raw_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64;
        let mut box_type = [0u8; 4];
        box_type.copy_from_slice(&data[4..8]);
        let box_type_str = format_fourcc(&box_type);

        let (total_size, header_size, is_largesize) = if raw_size == 1 {
            // 64-bit largesize
            if data.len() < 16 {
                return Err(VideoDefenseError::MalformedContainerHeader {
                    reason: format!(
                        "Insufficient bytes ({}) for 16-byte 64-bit largesize header in box '{box_type_str}'",
                        data.len()
                    ),
                });
            }
            let largesize = u64::from_be_bytes([
                data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
            ]);

            if largesize < 16 {
                return Err(VideoDefenseError::AtomInvalidSize {
                    box_type: box_type_str,
                    size: largesize,
                    min_required: 16,
                });
            }

            // Checked addition overflow test
            if current_offset.checked_add(largesize).is_none() || largesize > (i64::MAX as u64) {
                return Err(VideoDefenseError::AtomLargesizeOverflow {
                    box_type: box_type_str,
                    offset: current_offset,
                    declared_size: largesize,
                });
            }

            (largesize, 16u8, true)
        } else if raw_size == 0 {
            // Box extends to end of stream
            let remaining = match total_len {
                Some(total) => {
                    if current_offset > total {
                        return Err(VideoDefenseError::AtomOutOfBoundsOffset {
                            box_type: box_type_str,
                            offset: current_offset,
                            size: 0,
                            stream_len: total,
                        });
                    }
                    total - current_offset
                }
                None => 0,
            };
            (remaining, 8u8, false)
        } else if raw_size < 8 {
            return Err(VideoDefenseError::AtomInvalidSize {
                box_type: box_type_str,
                size: raw_size,
                min_required: 8,
            });
        } else {
            (raw_size, 8u8, false)
        };

        // Validate boundary against total stream length if known
        if let Some(stream_len) = total_len {
            if total_size > 0 {
                let end_offset = match current_offset.checked_add(total_size) {
                    Some(end) => end,
                    None => {
                        return Err(VideoDefenseError::AtomLargesizeOverflow {
                            box_type: box_type_str,
                            offset: current_offset,
                            declared_size: total_size,
                        });
                    }
                };

                if end_offset > stream_len {
                    return Err(VideoDefenseError::AtomOutOfBoundsOffset {
                        box_type: box_type_str,
                        offset: current_offset,
                        size: total_size,
                        stream_len,
                    });
                }
            }
        }

        Ok(ParsedBoxHeader {
            box_type,
            total_size,
            header_size,
            is_largesize,
        })
    }

    /// Pushes a new atom frame onto the depth stack, validating maximum depth constraints.
    pub fn push_box(
        &mut self,
        box_type: [u8; 4],
        box_size: u64,
        offset: u64,
        total_len: Option<u64>,
    ) -> Result<AtomFrame, VideoDefenseError> {
        let new_depth = self.stack.len() + 1;
        if new_depth > self.max_depth {
            return Err(VideoDefenseError::AtomDepthLimitExceeded {
                depth: new_depth,
                max_depth: self.max_depth,
            });
        }

        let box_type_str = format_fourcc(&box_type);

        // Validate that child box does not exceed parent box boundary
        if let Some(parent) = self.stack.last() {
            if parent.total_size > 0 {
                let parent_end = parent.end_offset();
                let child_end = match offset.checked_add(box_size) {
                    Some(end) => end,
                    None => {
                        return Err(VideoDefenseError::AtomLargesizeOverflow {
                            box_type: box_type_str,
                            offset,
                            declared_size: box_size,
                        });
                    }
                };

                if child_end > parent_end {
                    return Err(VideoDefenseError::AtomOutOfBoundsOffset {
                        box_type: box_type_str,
                        offset,
                        size: box_size,
                        stream_len: parent_end,
                    });
                }
            }
        }

        if let Some(stream_len) = total_len {
            let child_end = match offset.checked_add(box_size) {
                Some(end) => end,
                None => {
                    return Err(VideoDefenseError::AtomLargesizeOverflow {
                        box_type: box_type_str,
                        offset,
                        declared_size: box_size,
                    });
                }
            };
            if child_end > stream_len {
                return Err(VideoDefenseError::AtomOutOfBoundsOffset {
                    box_type: box_type_str,
                    offset,
                    size: box_size,
                    stream_len,
                });
            }
        }

        let header_size = if box_size > (u32::MAX as u64) { 16 } else { 8 };
        let frame = AtomFrame {
            box_type,
            start_offset: offset,
            total_size: box_size,
            header_size,
            depth: new_depth,
        };

        self.stack.push(frame.clone());
        self.total_boxes_scanned = self.total_boxes_scanned.saturating_add(1);
        if new_depth > self.max_depth_reached {
            self.max_depth_reached = new_depth;
        }

        Ok(frame)
    }

    /// Pops the topmost atom frame from the depth stack.
    pub fn pop_box(&mut self) -> Option<AtomFrame> {
        self.stack.pop()
    }

    /// Checks if a given 4-character code is a container box capable of holding child atoms.
    pub fn is_container_box(box_type: &[u8; 4]) -> bool {
        CONTAINER_BOX_TYPES.contains(box_type)
    }

    /// Iteratively scans container atom hierarchies across raw buffer bytes.
    pub fn scan_container_atoms(
        &mut self,
        data: &[u8],
    ) -> Result<AtomInspectionSummary, VideoDefenseError> {
        self.reset();
        let total_len = data.len() as u64;
        let mut top_level_boxes = Vec::new();

        self.scan_recursive(data, 0, total_len, &mut top_level_boxes)?;

        Ok(AtomInspectionSummary {
            total_boxes: self.total_boxes_scanned,
            max_depth_reached: self.max_depth_reached,
            top_level_boxes,
        })
    }

    fn scan_recursive(
        &mut self,
        data: &[u8],
        start_offset: u64,
        end_offset: u64,
        top_level: &mut Vec<[u8; 4]>,
    ) -> Result<(), VideoDefenseError> {
        let mut cursor = start_offset;
        let total_stream_len = data.len() as u64;

        while cursor + 8 <= end_offset {
            let slice_start = cursor as usize;
            let slice_end = (end_offset as usize).min(data.len());
            let header = Self::parse_box_header(
                &data[slice_start..slice_end],
                cursor,
                Some(total_stream_len),
            )?;

            let is_top = self.stack.is_empty();
            if is_top {
                top_level.push(header.box_type);
            }

            let box_size = if header.total_size == 0 {
                end_offset - cursor
            } else {
                header.total_size
            };

            let _frame = self.push_box(header.box_type, box_size, cursor, Some(total_stream_len))?;

            if Self::is_container_box(&header.box_type) {
                let payload_start = cursor + (header.header_size as u64);
                let payload_end = cursor + box_size;
                if payload_start < payload_end && payload_end <= end_offset {
                    self.scan_recursive(data, payload_start, payload_end, top_level)?;
                }
            }

            self.pop_box();
            cursor = match cursor.checked_add(box_size) {
                Some(next) => next,
                None => {
                    return Err(VideoDefenseError::AtomLargesizeOverflow {
                        box_type: format_fourcc(&header.box_type),
                        offset: cursor,
                        declared_size: box_size,
                    });
                }
            };
        }

        Ok(())
    }
}

/// Helper formatting a 4-byte slice into printable ASCII or hex representation.
fn format_fourcc(fourcc: &[u8; 4]) -> String {
    if fourcc.iter().all(|&b| b.is_ascii_graphic() || b == b' ') {
        String::from_utf8_lossy(fourcc).to_string()
    } else {
        format!(
            "0x{:02X}{:02X}{:02X}{:02X}",
            fourcc[0], fourcc[1], fourcc[2], fourcc[3]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_box_header_parsing() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&24u32.to_be_bytes());
        buf.extend_from_slice(b"ftyp");
        buf.extend_from_slice(b"mp42\0\0\0\0mp42isom");

        let header = AtomDepthGuard::parse_box_header(&buf, 0, Some(24)).unwrap();
        assert_eq!(header.box_type, *b"ftyp");
        assert_eq!(header.total_size, 24);
        assert_eq!(header.header_size, 8);
        assert!(!header.is_largesize);
    }

    #[test]
    fn test_64bit_largesize_parsing() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_be_bytes()); // signals 64-bit largesize
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&1024u64.to_be_bytes()); // largesize = 1024
        buf.resize(1024, 0xAA);

        let header = AtomDepthGuard::parse_box_header(&buf, 0, Some(1024)).unwrap();
        assert_eq!(header.box_type, *b"mdat");
        assert_eq!(header.total_size, 1024);
        assert_eq!(header.header_size, 16);
        assert!(header.is_largesize);
    }

    #[test]
    fn test_invalid_largesize_underflow() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&8u64.to_be_bytes()); // Illegal < 16 for largesize

        let err = AtomDepthGuard::parse_box_header(&buf, 0, Some(100)).unwrap_err();
        match err {
            VideoDefenseError::AtomInvalidSize { size, min_required, .. } => {
                assert_eq!(size, 8);
                assert_eq!(min_required, 16);
            }
            _ => panic!("Expected AtomInvalidSize"),
        }
    }

    #[test]
    fn test_depth_overflow_fusing() {
        let mut guard = AtomDepthGuard::with_max_depth(3);
        assert!(guard.push_box(*b"moov", 1000, 0, Some(1000)).is_ok());
        assert!(guard.push_box(*b"trak", 800, 8, Some(1000)).is_ok());
        assert!(guard.push_box(*b"mdia", 600, 16, Some(1000)).is_ok());

        // 4th level exceeds max depth 3
        let err = guard.push_box(*b"minf", 400, 24, Some(1000)).unwrap_err();
        assert_eq!(
            err,
            VideoDefenseError::AtomDepthLimitExceeded {
                depth: 4,
                max_depth: 3
            }
        );
    }

    #[test]
    fn test_out_of_bounds_box_offset() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&500u32.to_be_bytes());
        buf.extend_from_slice(b"moov");

        // Stream len is only 100 bytes, but box declares 500
        let err = AtomDepthGuard::parse_box_header(&buf, 0, Some(100)).unwrap_err();
        match err {
            VideoDefenseError::AtomOutOfBoundsOffset { size, stream_len, .. } => {
                assert_eq!(size, 500);
                assert_eq!(stream_len, 100);
            }
            _ => panic!("Expected AtomOutOfBoundsOffset"),
        }
    }
}
