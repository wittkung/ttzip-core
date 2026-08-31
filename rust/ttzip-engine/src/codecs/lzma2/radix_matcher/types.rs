// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Radix Matcher Data Types, Bitpack Encodings, and Architectural Constants.

/// Number of entries in the 16-bit radix bucket table ($2^{16} = 65,536$).
pub const RADIX16_TABLE_SIZE: usize = 65536;

/// Number of entries in the 8-bit radix sub-bucket table ($2^8 = 256$).
pub const RADIX8_TABLE_SIZE: usize = 256;

/// Maximum list size where pairwise brute-force comparison is faster than recursive bucketing.
pub const MAX_BRUTE_FORCE_LIST_SIZE: usize = 5;

/// Threshold for consecutive repeating bytes before triggering RLE folding.
pub const MAX_REPEAT: usize = 24;

/// Sentinel value indicating the end of a match chain or an uninitialized link.
pub const RADIX_NULL_LINK: u32 = 0xFFFF_FFFF;

/// Number of bits used to store the link index in bitpack mode (up to 64 MB dictionary).
pub const RADIX_LINK_BITS: u32 = 26;

/// Bitmask for extracting the 26-bit link index from a packed match table entry.
pub const RADIX_LINK_MASK: u32 = (1 << RADIX_LINK_BITS) - 1;

/// Maximum match length representable in the 6-bit depth field of a packed 32-bit word (63 bytes).
pub const RADIX_MAX_LENGTH: u32 = (1 << (32 - RADIX_LINK_BITS)) - 1;

/// Bitmask for the lower 24 bits of next index in [`RadixBuildMatch`].
pub const BUFFER_LINK_MASK: u32 = 0x00FF_FFFF;

/// Compact 12-byte match building node with 4-byte character L1 prefetch cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct RadixBuildMatch {
    /// Original byte position in the input data buffer.
    pub from: u32,
    /// 4-byte prefetch character cache stored in native/little-endian format.
    pub src_u32: u32,
    /// Next match buffer index (lower 24 bits) packed with match depth (upper 8 bits).
    pub next_and_depth: u32,
}

impl RadixBuildMatch {
    /// Creates a new match building node.
    #[inline(always)]
    pub const fn new(from: u32, src_u32: u32, next: u32, depth: u32) -> Self {
        Self {
            from,
            src_u32,
            next_and_depth: (next & BUFFER_LINK_MASK) | ((depth & 0xFF) << 24),
        }
    }

    /// Returns the next node index in the match buffer.
    #[inline(always)]
    pub const fn next_index(&self) -> usize {
        (self.next_and_depth & BUFFER_LINK_MASK) as usize
    }

    /// Returns the current match depth (0..255).
    #[inline(always)]
    pub const fn depth(&self) -> u32 {
        self.next_and_depth >> 24
    }

    /// Sets the next node index and match depth.
    #[inline(always)]
    pub fn set_next_and_depth(&mut self, next: u32, depth: u32) {
        self.next_and_depth = (next & BUFFER_LINK_MASK) | ((depth & 0xFF) << 24);
    }

    /// Retrieves a cached byte at the given 0..3 slot offset.
    #[inline(always)]
    pub fn byte_at(&self, slot: usize) -> u8 {
        (self.src_u32 >> ((slot & 3) * 8)) as u8
    }

    /// Reads up to 4 bytes from `data` at `offset` into `src_u32`.
    #[inline(always)]
    pub fn load_src_u32(&mut self, data: &[u8], offset: usize) {
        if offset + 4 <= data.len() {
            let chunk: [u8; 4] = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            self.src_u32 = u32::from_le_bytes(chunk);
        } else if offset < data.len() {
            let mut chunk = [0u8; 4];
            let avail = data.len() - offset;
            chunk[..avail].copy_from_slice(&data[offset..offset + avail]);
            self.src_u32 = u32::from_le_bytes(chunk);
        } else {
            self.src_u32 = 0;
        }
    }
}

/// Helper structure for 8-bit or 16-bit bucket linked-list tail tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadixListTail {
    pub prev_index: u32,
    pub list_count: u32,
}

impl Default for RadixListTail {
    #[inline(always)]
    fn default() -> Self {
        Self {
            prev_index: RADIX_NULL_LINK,
            list_count: 0,
        }
    }
}

/// Match entry containing reference position link and match length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchEntry {
    /// Position of the matching prior string.
    pub link: usize,
    /// Number of matching bytes found.
    pub length: usize,
}
