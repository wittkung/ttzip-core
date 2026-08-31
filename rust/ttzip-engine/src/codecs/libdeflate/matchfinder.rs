// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, ultra-high-throughput HT (Hash Table) and HC (Hash Chains) dual-hash
//! Lempel-Ziv LZ77 matchfinders for DEFLATE compression.
//!
//! # Architecture & Algorithm
//!
//! 1. **`HtMatchfinder` (Inline 2-Slot Hash Table)**:
//!    - Stores 2-entry hash chains directly inside a 128KB hash table ($2^{15} = 32768$ buckets).
//!    - Optimized for ultra-fast greedy compression (Levels 1..3).
//!    - Uses 4-byte prefix matching and 4-byte tail pruning prior to extension.
//!
//! 2. **`HcMatchfinder` (Dual 3-Byte Direct Table + 4-Byte Hash Chains)**:
//!    - Direct-mapped 3-byte hash table ($2^{15} = 32768$ slots) for instant length-3 matches.
//!    - 4-byte hash table ($2^{16} = 65536$ heads) with `next_tab` linked list traversal.
//!    - Configurable `max_search_depth` to trade compression ratio for throughput.
//!
//! 3. **SWAR 64-Bit Match Extension (`lz_extend`)**:
//!    - Employs 64-bit word XOR comparison with trailing zeros count for fast 8-byte step extension.
//!
//! 4. **Zero-Branch Window Rebasing (`rebase`)**:
//!    - Realigns position indices across 32KB window boundaries using signed saturation arithmetic.

use crate::utils::lz_extend as lz_extend_slices;

// MARK: - Constants

/// RFC 1951 Deflate sliding window size (32 KB = 32768 bytes).
pub const WINDOW_SIZE: usize = 32768;

/// Alias for RFC 1951 Deflate sliding window size.
pub const MATCHFINDER_WINDOW_SIZE: usize = WINDOW_SIZE;

/// Hash order for HT matchfinder (15 bits = 32768 buckets).
pub const HT_HASH_ORDER: usize = 15;

/// Number of hash buckets in HT matchfinder table (32768).
pub const HT_HASH_SIZE: usize = 1 << HT_HASH_ORDER;

/// Number of slots per HT hash bucket (2 entries).
pub const HT_BUCKET_SIZE: usize = 2;

/// Hash order for HC length-3 direct matchfinder table (15 bits = 32768 buckets).
pub const HC_HASH3_ORDER: usize = 15;

/// Number of buckets in HC length-3 table (32768).
pub const HC_HASH3_SIZE: usize = 1 << HC_HASH3_ORDER;

/// Hash order for HC length-4+ hash chain head table (16 bits = 65536 buckets).
pub const HC_HASH4_ORDER: usize = 16;

/// Number of buckets in HC length-4+ table (65536).
pub const HC_HASH4_SIZE: usize = 1 << HC_HASH4_ORDER;

/// Alias for HT hash order.
pub const HT_MATCHFINDER_HASH_ORDER: usize = HT_HASH_ORDER;

/// Alias for HC hash3 order.
pub const HC_MATCHFINDER_HASH3_ORDER: usize = HC_HASH3_ORDER;

/// Alias for HC hash4 order.
pub const HC_MATCHFINDER_HASH4_ORDER: usize = HC_HASH4_ORDER;

/// Sentinel initialization value indicating an empty or out-of-window entry (-32768).
pub const MATCHFINDER_INITVAL: i16 = -32768;

/// Minimum match length supported by DEFLATE format.
pub const MIN_MATCH_LEN: usize = 3;

/// Maximum match length supported by DEFLATE format.
pub const MAX_MATCH_LEN: usize = 258;

/// Minimum match length for HT matchfinder (4 bytes).
pub const HT_MIN_MATCH_LEN: usize = 4;

/// Minimum match length for HC matchfinder (3 bytes).
pub const HC_MIN_MATCH_LEN: usize = 3;

/// Multiplicative hash constant for 32-bit sequence hashing.
pub const LZ_HASH_MULTIPLIER: u32 = 0x1E35_A7BD;

// MARK: - Hash & Helper Functions

/// Computes multiplicative hash for a 32-bit sequence prefix.
#[inline(always)]
pub fn lz_hash(seq: u32, num_bits: u32) -> usize {
    ((seq.wrapping_mul(LZ_HASH_MULTIPLIER)) >> (32 - num_bits)) as usize
}

/// Subtracts 32768 from `pos` with signed saturation to -32768 (`MATCHFINDER_INITVAL`).
///
/// Branchless bitwise equivalent for 32768-byte sliding windows:
/// - If `pos >= 0` (0..32767): `pos - 32768` (results in -32768..-1).
/// - If `pos < 0`: saturates to -32768.
#[inline(always)]
pub fn rebase_pos(pos: i16) -> i16 {
    (0x8000u16 | ((pos as u16) & !((pos >> 15) as u16))) as i16
}

/// Extends a match starting at `cur_pos` and `match_pos` up to `max_len` bytes.
///
/// Uses SWAR 64-bit word XOR comparison with `trailing_zeros` count for high throughput.
#[inline(always)]
pub fn lz_extend(
    in_data: &[u8],
    cur_pos: usize,
    match_pos: usize,
    start_len: usize,
    max_len: usize,
) -> usize {
    let cur_bound = in_data.len().saturating_sub(cur_pos).min(max_len);
    let match_bound = in_data.len().saturating_sub(match_pos).min(max_len);
    let max_possible = cur_bound.min(match_bound);
    if start_len >= max_possible {
        return max_possible;
    }
    lz_extend_slices(
        &in_data[cur_pos..cur_pos + max_possible],
        &in_data[match_pos..match_pos + max_possible],
        start_len,
    )
}

// MARK: - HtMatchfinder

/// Fast Hash Table (HT) 2-slot inline matchfinder for high-throughput Deflate compression.
#[derive(Clone)]
pub struct HtMatchfinder {
    /// 128KB hash table containing 2 inline slots per bucket ($32768 \times 2 \times 2$ bytes).
    pub hash_tab: [[i16; HT_BUCKET_SIZE]; HT_HASH_SIZE],
}

impl Default for HtMatchfinder {
    fn default() -> Self {
        Self::new()
    }
}

impl HtMatchfinder {
    /// Creates a newly initialized HT matchfinder with all slots set to out-of-window.
    pub fn new() -> Self {
        Self {
            hash_tab: [[MATCHFINDER_INITVAL; HT_BUCKET_SIZE]; HT_HASH_SIZE],
        }
    }

    /// Creates a heap-allocated `Box<HtMatchfinder>`.
    pub fn new_boxed() -> Box<Self> {
        Box::new(Self::new())
    }

    /// Resets all hash table entries to the initial out-of-window sentinel value.
    pub fn reset(&mut self) {
        self.hash_tab = [[MATCHFINDER_INITVAL; HT_BUCKET_SIZE]; HT_HASH_SIZE];
    }

    /// Realigns position indices across 32KB window boundaries using branchless signed saturation.
    pub fn rebase(&mut self) {
        for bucket in self.hash_tab.iter_mut() {
            bucket[0] = rebase_pos(bucket[0]);
            bucket[1] = rebase_pos(bucket[1]);
        }
    }

    /// Updates the hash table with the sequence at `cur_pos` without searching.
    #[inline(always)]
    pub fn update(&mut self, seq: u32, cur_pos: usize) {
        let cur_pos_i16 = (cur_pos & (WINDOW_SIZE - 1)) as i16;
        let hash = lz_hash(seq, HT_HASH_ORDER as u32);
        self.hash_tab[hash][1] = self.hash_tab[hash][0];
        self.hash_tab[hash][0] = cur_pos_i16;
    }

    /// Advances the matchfinder across multiple bytes without searching for matches.
    pub fn skip_bytes(&mut self, in_data: &[u8], cur_pos: usize, count: usize) {
        if cur_pos + count + 4 > in_data.len() {
            return;
        }
        for i in 0..count {
            let pos = cur_pos + i;
            let seq = u32::from_le_bytes(in_data[pos..pos + 4].try_into().unwrap());
            self.update(seq, pos);
        }
    }

    /// Finds the longest match at `cur_pos` in `in_data`.
    ///
    /// Returns `(length, offset)` where `offset` is relative to `cur_pos`.
    /// Returns `(0, 0)` if no match of length $\ge 4$ was found.
    pub fn longest_match(
        &mut self,
        in_data: &[u8],
        cur_pos: usize,
        max_len: usize,
        nice_len: usize,
    ) -> (usize, usize) {
        if cur_pos + 4 > in_data.len() || max_len < HT_MIN_MATCH_LEN {
            return (0, 0);
        }

        let cur_pos_i16 = (cur_pos & (WINDOW_SIZE - 1)) as i16;
        let cutoff = (cur_pos_i16 as i32) - (WINDOW_SIZE as i32);
        let max_len = max_len.min(in_data.len() - cur_pos).min(MAX_MATCH_LEN);
        let nice_len = nice_len.min(max_len);

        let seq = u32::from_le_bytes(in_data[cur_pos..cur_pos + 4].try_into().unwrap());
        let hash = lz_hash(seq, HT_HASH_ORDER as u32);

        let prev_node0 = self.hash_tab[hash][0];
        self.hash_tab[hash][0] = cur_pos_i16;

        if (prev_node0 as i32) <= cutoff {
            return (0, 0);
        }

        let prev_node1 = self.hash_tab[hash][1];
        self.hash_tab[hash][1] = prev_node0;

        let mut best_len = 0;
        let mut best_offset = 0;

        // Slot 0 candidate evaluation
        let offset0 = (cur_pos_i16 as i32 - prev_node0 as i32) as usize;
        if cur_pos >= offset0 && offset0 > 0 {
            let match_pos0 = cur_pos - offset0;
            let seq0 = u32::from_le_bytes(in_data[match_pos0..match_pos0 + 4].try_into().unwrap());
            if seq0 == seq {
                best_len = lz_extend(in_data, cur_pos, match_pos0, 4, max_len);
                best_offset = offset0;

                if (prev_node1 as i32) <= cutoff || best_len >= nice_len {
                    return (best_len, best_offset);
                }

                // Slot 1 candidate evaluation with 4-byte prefix + tail pruning
                let offset1 = (cur_pos_i16 as i32 - prev_node1 as i32) as usize;
                if cur_pos >= offset1 && offset1 > 0 {
                    let match_pos1 = cur_pos - offset1;
                    if cur_pos + best_len < in_data.len() {
                        let seq1 = u32::from_le_bytes(
                            in_data[match_pos1..match_pos1 + 4].try_into().unwrap(),
                        );
                        let tail_cur = u32::from_le_bytes(
                            in_data[cur_pos + best_len - 3..cur_pos + best_len + 1]
                                .try_into()
                                .unwrap(),
                        );
                        let tail_cand = u32::from_le_bytes(
                            in_data[match_pos1 + best_len - 3..match_pos1 + best_len + 1]
                                .try_into()
                                .unwrap(),
                        );
                        if seq1 == seq && tail_cand == tail_cur {
                            let len1 = lz_extend(in_data, cur_pos, match_pos1, 4, max_len);
                            if len1 > best_len {
                                best_len = len1;
                                best_offset = offset1;
                            }
                        }
                    } else {
                        let seq1 = u32::from_le_bytes(
                            in_data[match_pos1..match_pos1 + 4].try_into().unwrap(),
                        );
                        if seq1 == seq {
                            let len1 = lz_extend(in_data, cur_pos, match_pos1, 4, max_len);
                            if len1 > best_len {
                                best_len = len1;
                                best_offset = offset1;
                            }
                        }
                    }
                }
                return (best_len, best_offset);
            }
        }

        // Slot 0 did not match prefix: evaluate Slot 1
        if (prev_node1 as i32) > cutoff {
            let offset1 = (cur_pos_i16 as i32 - prev_node1 as i32) as usize;
            if cur_pos >= offset1 && offset1 > 0 {
                let match_pos1 = cur_pos - offset1;
                let seq1 = u32::from_le_bytes(in_data[match_pos1..match_pos1 + 4].try_into().unwrap());
                if seq1 == seq {
                    best_len = lz_extend(in_data, cur_pos, match_pos1, 4, max_len);
                    best_offset = offset1;
                }
            }
        }

        (best_len, best_offset)
    }
}

// MARK: - HcMatchfinder

/// Hash Chains (HC) matchfinder with 3-byte direct-map table and 4-byte linked lists.
#[derive(Clone)]
pub struct HcMatchfinder {
    /// Direct-mapped hash table for finding length-3 matches (32768 entries, 64KB).
    pub hash3_tab: [i16; HC_HASH3_SIZE],
    /// Linked list head table for finding length-4+ matches (65536 entries, 128KB).
    pub hash4_tab: [i16; HC_HASH4_SIZE],
    /// Next-node link pointers for 4-byte hash chains (32768 entries, 64KB).
    pub next_tab: [i16; WINDOW_SIZE],
}

impl Default for HcMatchfinder {
    fn default() -> Self {
        Self::new()
    }
}

impl HcMatchfinder {
    /// Creates a newly initialized HC matchfinder with all tables set to out-of-window.
    pub fn new() -> Self {
        Self {
            hash3_tab: [MATCHFINDER_INITVAL; HC_HASH3_SIZE],
            hash4_tab: [MATCHFINDER_INITVAL; HC_HASH4_SIZE],
            next_tab: [MATCHFINDER_INITVAL; WINDOW_SIZE],
        }
    }

    /// Creates a heap-allocated `Box<HcMatchfinder>`.
    pub fn new_boxed() -> Box<Self> {
        Box::new(Self::new())
    }

    /// Resets all hash and next-pointer tables to the initial out-of-window sentinel value.
    pub fn reset(&mut self) {
        self.hash3_tab = [MATCHFINDER_INITVAL; HC_HASH3_SIZE];
        self.hash4_tab = [MATCHFINDER_INITVAL; HC_HASH4_SIZE];
        self.next_tab = [MATCHFINDER_INITVAL; WINDOW_SIZE];
    }

    /// Realigns position indices across 32KB window boundaries using branchless signed saturation.
    pub fn rebase(&mut self) {
        for entry in self.hash3_tab.iter_mut() {
            *entry = rebase_pos(*entry);
        }
        for entry in self.hash4_tab.iter_mut() {
            *entry = rebase_pos(*entry);
        }
        for entry in self.next_tab.iter_mut() {
            *entry = rebase_pos(*entry);
        }
    }

    /// Updates hash tables with sequence at `cur_pos` without searching.
    #[inline(always)]
    pub fn update(&mut self, seq4: u32, cur_pos: usize) {
        let cur_pos_i16 = (cur_pos & (WINDOW_SIZE - 1)) as i16;
        let hash3 = lz_hash(seq4 & 0x00FF_FFFF, HC_HASH3_ORDER as u32);
        let hash4 = lz_hash(seq4, HC_HASH4_ORDER as u32);

        self.hash3_tab[hash3] = cur_pos_i16;
        self.next_tab[cur_pos & (WINDOW_SIZE - 1)] = self.hash4_tab[hash4];
        self.hash4_tab[hash4] = cur_pos_i16;
    }

    /// Advances the matchfinder across multiple bytes without searching for matches.
    pub fn skip_bytes(&mut self, in_data: &[u8], cur_pos: usize, count: usize) {
        if cur_pos + count + 4 > in_data.len() {
            return;
        }
        for i in 0..count {
            let pos = cur_pos + i;
            let seq4 = u32::from_le_bytes(in_data[pos..pos + 4].try_into().unwrap());
            self.update(seq4, pos);
        }
    }

    /// Finds the longest match at `cur_pos` in `in_data` using 3-byte and 4-byte hash chains.
    ///
    /// Returns `(length, offset)` where `offset` is relative to `cur_pos`.
    /// Returns `(0, 0)` if no match of length $\ge 3$ was found.
    pub fn longest_match(
        &mut self,
        in_data: &[u8],
        cur_pos: usize,
        max_len: usize,
        nice_len: usize,
        max_search_depth: usize,
    ) -> (usize, usize) {
        if cur_pos + 4 > in_data.len() || max_len < HC_MIN_MATCH_LEN {
            return (0, 0);
        }

        let cur_pos_i16 = (cur_pos & (WINDOW_SIZE - 1)) as i16;
        let cutoff = (cur_pos_i16 as i32) - (WINDOW_SIZE as i32);
        let max_len = max_len.min(in_data.len() - cur_pos).min(MAX_MATCH_LEN);
        let nice_len = nice_len.min(max_len);
        let mut depth_remaining = max_search_depth.max(1);

        let seq4 = u32::from_le_bytes(in_data[cur_pos..cur_pos + 4].try_into().unwrap());
        let hash3 = lz_hash(seq4 & 0x00FF_FFFF, HC_HASH3_ORDER as u32);
        let hash4 = lz_hash(seq4, HC_HASH4_ORDER as u32);

        let cur_node3 = self.hash3_tab[hash3];
        let mut cur_node4 = self.hash4_tab[hash4];

        // Update hash tables with current sequence
        self.hash3_tab[hash3] = cur_pos_i16;
        self.hash4_tab[hash4] = cur_pos_i16;
        self.next_tab[cur_pos & (WINDOW_SIZE - 1)] = cur_node4;

        let mut best_len = 0;
        let mut best_offset = 0;

        // Step 1: Check length-3 match from direct-mapped hash3_tab
        if (cur_node3 as i32) > cutoff {
            let offset3 = (cur_pos_i16 as i32 - cur_node3 as i32) as usize;
            if cur_pos >= offset3 && offset3 > 0 {
                let match_pos3 = cur_pos - offset3;
                if in_data[match_pos3..match_pos3 + 3] == in_data[cur_pos..cur_pos + 3] {
                    best_len = 3;
                    best_offset = offset3;
                }
            }
        }

        // Step 2: Search length-4+ hash chain
        while (cur_node4 as i32) > cutoff && depth_remaining > 0 {
            depth_remaining -= 1;
            let offset4 = (cur_pos_i16 as i32 - cur_node4 as i32) as usize;
            if cur_pos < offset4 || offset4 == 0 {
                break;
            }
            let match_pos4 = cur_pos - offset4;

            if best_len < 4 {
                let cand_seq = u32::from_le_bytes(
                    in_data[match_pos4..match_pos4 + 4].try_into().unwrap(),
                );
                if cand_seq == seq4 {
                    let len = lz_extend(in_data, cur_pos, match_pos4, 4, max_len);
                    best_len = len;
                    best_offset = offset4;
                    if best_len >= nice_len {
                        break;
                    }
                }
            } else {
                // Match length >= 4 already exists: verify match at best_len first
                if match_pos4 + best_len < in_data.len()
                    && in_data[match_pos4 + best_len] == in_data[cur_pos + best_len]
                {
                    let cand_seq = u32::from_le_bytes(
                        in_data[match_pos4..match_pos4 + 4].try_into().unwrap(),
                    );
                    if cand_seq == seq4 {
                        let len = lz_extend(in_data, cur_pos, match_pos4, 4, max_len);
                        if len > best_len {
                            best_len = len;
                            best_offset = offset4;
                            if best_len >= nice_len {
                                break;
                            }
                        }
                    }
                }
            }

            cur_node4 = self.next_tab[(cur_node4 as u16 & (WINDOW_SIZE as u16 - 1)) as usize];
        }

        (best_len, best_offset)
    }
}
