// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 256KB Dual-Table Architecture with zero-heap-allocation relative delta linked list.
//!
//! Provides the primary data structures for LZ4HC sequence lookup:
//! - 32K-entry `hash_table` (128 KB): stores latest 1-based positions.
//! - 64K-entry `chain_table` (128 KB): stores backward relative deltas.
//!
//! Enables zero-allocation circular buffer matching over the full 64KB LZ4 window.

use crate::codecs::lz4::hash::lz4_hash4;

/// Hash table address bit width (15 bits -> 32,768 entries).
pub const LZ4HC_HASH_LOG: u32 = 15;

/// Number of entries in the LZ4HC primary hash table (32,768 entries = 128 KB).
pub const LZ4HC_HASH_SIZE: usize = 1 << LZ4HC_HASH_LOG;

/// Number of entries in the LZ4HC chain table (65,536 entries = 128 KB).
pub const LZ4HC_CHAIN_SIZE: usize = 65536;

/// Maximum backward reference distance (64 KB - 1 byte = 65,535).
pub const LZ4HC_MAX_DISTANCE: usize = 65535;

/// Minimum match length required by LZ4 block format.
pub const MIN_MATCH: usize = 4;

/// Last literals limit: last 5 bytes of a block must be emitted as raw literals.
pub const LAST_LITERALS: usize = 5;

/// Candidate match representation with distance offset and match length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lz4Match {
    /// Backward distance (1..=65535).
    pub offset: u16,
    /// Match length in bytes (>= 4).
    pub length: u32,
}

/// 256KB Dual-Table Architecture with zero-heap-allocation relative delta linked list.
/// - `hash_table`: 32,768 entries of 32-bit 1-based positions (128 KB).
/// - `chain_table`: 65,536 entries of 16-bit relative position deltas (128 KB).
#[repr(C)]
pub struct Lz4HcDualTable {
    /// Primary 32K-entry hash table storing latest 1-based buffer positions.
    pub hash_table: [u32; LZ4HC_HASH_SIZE],
    /// Secondary 64K-entry circular chain table storing backward deltas.
    pub chain_table: [u16; LZ4HC_CHAIN_SIZE],
}

impl Lz4HcDualTable {
    /// Allocates a new 256KB dual-table on the heap with zeroed memory.
    pub fn new() -> Box<Self> {
        let layout = std::alloc::Layout::new::<Self>();
        unsafe {
            let ptr = std::alloc::alloc_zeroed(layout) as *mut Self;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            Box::from_raw(ptr)
        }
    }

    /// Resets all hash and chain entries for reusing across blocks.
    #[inline]
    pub fn reset(&mut self) {
        self.hash_table.fill(0);
        self.chain_table.fill(0xFFFF);
    }

    /// Inserts a position into the hash table without performing match search.
    #[inline]
    pub fn insert_pos(&mut self, src: &[u8], ip: usize) {
        if ip + MIN_MATCH <= src.len().saturating_sub(LAST_LITERALS) {
            let mut seq_bytes = [0u8; 4];
            seq_bytes.copy_from_slice(&src[ip..ip + 4]);
            let seq = u32::from_le_bytes(seq_bytes);
            let hash = lz4_hash4(seq, LZ4HC_HASH_LOG) as usize;
            let prev = self.hash_table[hash] as usize;

            self.hash_table[hash] = (ip + 1) as u32;
            let delta = if prev > 0 && (ip + 1) > prev && ((ip + 1) - prev) <= LZ4HC_MAX_DISTANCE {
                ((ip + 1) - prev) as u16
            } else {
                0xFFFF
            };
            self.chain_table[ip & 0xFFFF] = delta;
        }
    }

    /// Inserts position `ip` and traverses the relative delta chain to find matches.
    pub fn insert_and_find_matches(
        &mut self,
        src: &[u8],
        ip: usize,
        max_depth: usize,
        favor_dec_speed: bool,
        matches: &mut [Lz4Match],
    ) -> usize {
        let limit = src.len().saturating_sub(LAST_LITERALS);
        if ip + MIN_MATCH > limit {
            return 0;
        }
        let max_match_len = limit - ip;
        if max_match_len < MIN_MATCH {
            return 0;
        }

        let mut seq_bytes = [0u8; 4];
        seq_bytes.copy_from_slice(&src[ip..ip + 4]);
        let seq = u32::from_le_bytes(seq_bytes);
        let hash = lz4_hash4(seq, LZ4HC_HASH_LOG) as usize;

        let prev_pos_1based = self.hash_table[hash] as usize;
        self.hash_table[hash] = (ip + 1) as u32;
        let delta = if prev_pos_1based > 0
            && (ip + 1) > prev_pos_1based
            && ((ip + 1) - prev_pos_1based) <= LZ4HC_MAX_DISTANCE
        {
            ((ip + 1) - prev_pos_1based) as u16
        } else {
            0xFFFF
        };
        self.chain_table[ip & 0xFFFF] = delta;

        if prev_pos_1based == 0 {
            return 0;
        }



        let mut match_pos_1based = prev_pos_1based;
        let mut depth = 0;
        let mut match_count = 0;
        let mut longest = 0;

        while match_pos_1based > 0 && depth < max_depth {
            let match_idx = match_pos_1based - 1;
            if match_idx >= ip {
                let d = self.chain_table[match_idx & 0xFFFF] as usize;
                if d == 0xFFFF || d == 0 || d >= match_pos_1based {
                    break;
                }
                match_pos_1based -= d;
                depth += 1;
                continue;
            }

            let dist = ip - match_idx;
            if dist == 0 || dist > LZ4HC_MAX_DISTANCE {
                break;
            }

            let skip_small_offset = favor_dec_speed && dist < 8;

            if !skip_small_offset && src[match_idx..match_idx + 4] == src[ip..ip + 4] {
                let mut len = 4;
                while len < max_match_len && src[match_idx + len] == src[ip + len] {
                    len += 1;
                }

                let valid = if favor_dec_speed && dist < 8 {
                    len >= 18
                } else {
                    true
                };

                if valid && len > longest {
                    longest = len;
                    if match_count < matches.len() {
                        matches[match_count] = Lz4Match {
                            offset: dist as u16,
                            length: len as u32,
                        };
                        match_count += 1;
                    }
                    if len == max_match_len {
                        break;
                    }
                }
            }

            let d = self.chain_table[match_idx & 0xFFFF] as usize;
            if d == 0xFFFF || d == 0 || d >= match_pos_1based {
                break;
            }
            match_pos_1based -= d;
            depth += 1;
        }

        match_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_table_creation_reset() {
        let mut table = Lz4HcDualTable::new();
        assert_eq!(table.hash_table[0], 0);
        table.reset();
        assert_eq!(table.hash_table[0], 0);
        assert_eq!(table.chain_table[0], 0xFFFF);
    }

    #[test]
    fn test_dual_table_match_finding() {
        let data = b"abcdefgh12345678abcdefgh12345678padding";
        let mut table = Lz4HcDualTable::new();
        table.reset();

        table.insert_pos(data, 0);
        table.insert_pos(data, 1);

        let mut matches = [Lz4Match::default(); 4];
        let count = table.insert_and_find_matches(data, 16, 64, false, &mut matches);
        assert!(count > 0);
        assert_eq!(matches[0].offset, 16);
        assert!(matches[0].length >= 16);
    }
}
