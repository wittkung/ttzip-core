// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance Binary Tree (BT) matchfinder inspired by `libdeflate`.
//!
//! # Algorithmic Architecture
//!
//! The matchfinder maintains a dual-hash indexing structure:
//! 1. **3-byte Hash Table (`hash3_tab`)**: 2-way associative array of $2^{16} = 65536$ slots
//!    for fast length-3 match lookups.
//! 2. **4-byte Hash Table (`hash4_tab`)**: $2^{16} = 65536$ slots storing binary tree roots
//!    for length-4+ sequence matching.
//! 3. **Binary Tree Topology (`child_tab`)**: Stores left and right children for $2 \times 32768 = 65536$
//!    nodes within the 32 KB sliding window.
//!
//! At each step, a single top-down tree traversal searches for matches while simultaneously
//! re-rooting the binary tree at the current position. Common prefix lengths (`best_lt_len` and
//! `best_gt_len`) are tracked to prune redundant byte comparisons.

// MARK: - Constants

/// Sliding window order (15 bits -> 32 KB).
pub const MATCHFINDER_WINDOW_ORDER: usize = 15;

/// Sliding window size in bytes (32768 bytes).
pub const MATCHFINDER_WINDOW_SIZE: usize = 1 << MATCHFINDER_WINDOW_ORDER;

/// Hash order for 3-byte prefix lookup table (16 bits -> 65536 entries).
pub const BT_HASH3_ORDER: usize = 16;

/// Number of entries in 3-byte hash table (65536).
pub const BT_HASH3_SIZE: usize = 1 << BT_HASH3_ORDER;

/// Number of ways in 3-byte associative hash table.
pub const BT_HASH3_WAYS: usize = 2;

/// Hash order for 4-byte prefix lookup table (16 bits -> 65536 entries).
pub const BT_HASH4_ORDER: usize = 16;

/// Number of entries in 4-byte hash table (65536).
pub const BT_HASH4_SIZE: usize = 1 << BT_HASH4_ORDER;

/// Sentinel initialization value indicating an empty/out-of-window node (-32768).
pub const MATCHFINDER_INITVAL: i16 = -32768;

/// Minimum required input bytes remaining to compute lookahead hashes.
pub const BT_REQUIRED_NBYTES: usize = 5;

/// Minimum Deflate match length.
pub const DEFLATE_MIN_MATCH_LEN: usize = 3;

/// Maximum Deflate match length.
pub const DEFLATE_MAX_MATCH_LEN: usize = 258;

/// Default maximum search depth for binary tree traversal.
pub const DEFAULT_MAX_SEARCH_DEPTH: usize = 32;

/// Default nice match length threshold.
pub const DEFAULT_NICE_MATCH_LEN: usize = 32;

// MARK: - Data Types

/// Representation of an LZ match with length and backward offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LzMatch {
    /// Number of matching bytes.
    pub length: u16,
    /// Distance back from current position (1..=32768).
    pub offset: u16,
}

/// Binary tree matchfinder state for high-ratio Deflate compression.
pub struct BtMatchfinder {
    /// 2-way hash table for 3-byte matches (`[position; 2]`).
    pub hash3_tab: Box<[[i16; BT_HASH3_WAYS]; BT_HASH3_SIZE]>,
    /// Root pointers of binary trees for 4-byte+ matches.
    pub hash4_tab: Box<[i16; BT_HASH4_SIZE]>,
    /// Left and right child references for tree nodes within the sliding window.
    pub child_tab: Box<[i16; 2 * MATCHFINDER_WINDOW_SIZE]>,
}

// MARK: - Matchfinder Implementation

impl BtMatchfinder {
    /// Creates a newly initialized binary tree matchfinder on the heap.
    pub fn new() -> Self {
        let mut mf = Self {
            hash3_tab: vec![[MATCHFINDER_INITVAL; BT_HASH3_WAYS]; BT_HASH3_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| panic!("Failed to allocate hash3_tab")),
            hash4_tab: vec![MATCHFINDER_INITVAL; BT_HASH4_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| panic!("Failed to allocate hash4_tab")),
            child_tab: vec![MATCHFINDER_INITVAL; 2 * MATCHFINDER_WINDOW_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| panic!("Failed to allocate child_tab")),
        };
        mf.reset();
        mf
    }

    /// Resets all internal tables to the initial sentinel state.
    #[inline]
    pub fn reset(&mut self) {
        self.hash3_tab
            .fill([MATCHFINDER_INITVAL; BT_HASH3_WAYS]);
        self.hash4_tab.fill(MATCHFINDER_INITVAL);
        self.child_tab.fill(MATCHFINDER_INITVAL);
    }

    /// Slides the sliding window by `MATCHFINDER_WINDOW_SIZE` bytes, rebasing positions.
    #[inline]
    pub fn slide_window(&mut self) {
        for entry in self.hash3_tab.iter_mut() {
            for val in entry.iter_mut() {
                *val = rebase_val(*val);
            }
        }
        for val in self.hash4_tab.iter_mut() {
            *val = rebase_val(*val);
        }
        for val in self.child_tab.iter_mut() {
            *val = rebase_val(*val);
        }
    }

    /// Searches for matches starting at `cur_pos` and updates the binary tree.
    ///
    /// The recorded matches in `matches` are strictly monotonically increasing in length.
    pub fn get_matches(
        &mut self,
        in_base: &[u8],
        cur_pos: usize,
        max_len: usize,
        nice_len: usize,
        max_search_depth: usize,
        matches: &mut Vec<LzMatch>,
    ) {
        matches.clear();
        if cur_pos + BT_REQUIRED_NBYTES > in_base.len() {
            return;
        }

        let max_len = max_len.min(in_base.len() - cur_pos);
        if max_len < DEFLATE_MIN_MATCH_LEN {
            return;
        }

        let nice_len = nice_len.min(max_len);
        let in_next = &in_base[cur_pos..];
        let cutoff = (cur_pos as isize) - (MATCHFINDER_WINDOW_SIZE as isize);

        let seq3 = load_u24_le(in_next);
        let hash3 = lz_hash(seq3, BT_HASH3_ORDER);
        let seq4 = load_u32_le(in_next);
        let hash4 = lz_hash(seq4, BT_HASH4_ORDER);

        // 1. Probe 3-byte hash table
        let cur_node_3 = self.hash3_tab[hash3][0] as isize;
        self.hash3_tab[hash3][0] = cur_pos as i16;
        let cur_node_3_2 = self.hash3_tab[hash3][1] as isize;
        self.hash3_tab[hash3][1] = cur_node_3 as i16;

        let mut best_len = 2usize;

        if cur_node_3 > cutoff {
            let offset = cur_pos - (cur_node_3 as usize);
            if in_base[cur_node_3 as usize..cur_node_3 as usize + 3] == in_next[..3] {
                matches.push(LzMatch {
                    length: 3,
                    offset: offset as u16,
                });
                best_len = 3;
            }
        } else if cur_node_3_2 > cutoff {
            let offset = cur_pos - (cur_node_3_2 as usize);
            if in_base[cur_node_3_2 as usize..cur_node_3_2 as usize + 3] == in_next[..3] {
                matches.push(LzMatch {
                    length: 3,
                    offset: offset as u16,
                });
                best_len = 3;
            }
        }

        // 2. Probe 4-byte hash table and traverse binary tree
        let mut cur_node = self.hash4_tab[hash4] as isize;
        self.hash4_tab[hash4] = cur_pos as i16;

        let mut pending_lt_slot = 2 * (cur_pos & (MATCHFINDER_WINDOW_SIZE - 1));
        let mut pending_gt_slot = pending_lt_slot + 1;

        if cur_node <= cutoff {
            self.child_tab[pending_lt_slot] = MATCHFINDER_INITVAL;
            self.child_tab[pending_gt_slot] = MATCHFINDER_INITVAL;
            return;
        }

        let mut best_lt_len = 0usize;
        let mut best_gt_len = 0usize;
        let mut len = 0usize;
        let mut depth_remaining = max_search_depth;

        loop {
            let node_pos = cur_node as usize;
            let match_ptr = &in_base[node_pos..];
            let offset = cur_pos - node_pos;

            // Match extension with common prefix optimization
            while len < max_len && match_ptr[len] == in_next[len] {
                len += 1;
            }

            if len > best_len {
                best_len = len;
                matches.push(LzMatch {
                    length: len as u16,
                    offset: offset as u16,
                });

                if len >= nice_len {
                    let node_slot = 2 * (node_pos & (MATCHFINDER_WINDOW_SIZE - 1));
                    self.child_tab[pending_lt_slot] = self.child_tab[node_slot];
                    self.child_tab[pending_gt_slot] = self.child_tab[node_slot + 1];
                    return;
                }
            }

            if len < max_len && match_ptr[len] < in_next[len] {
                self.child_tab[pending_lt_slot] = cur_node as i16;
                pending_lt_slot = 2 * (node_pos & (MATCHFINDER_WINDOW_SIZE - 1)) + 1;
                cur_node = self.child_tab[pending_lt_slot] as isize;
                best_lt_len = len;
                if best_gt_len < len {
                    len = best_gt_len;
                }
            } else {
                self.child_tab[pending_gt_slot] = cur_node as i16;
                pending_gt_slot = 2 * (node_pos & (MATCHFINDER_WINDOW_SIZE - 1));
                cur_node = self.child_tab[pending_gt_slot] as isize;
                best_gt_len = len;
                if best_lt_len < len {
                    len = best_lt_len;
                }
            }

            depth_remaining -= 1;
            if cur_node <= cutoff || depth_remaining == 0 {
                self.child_tab[pending_lt_slot] = MATCHFINDER_INITVAL;
                self.child_tab[pending_gt_slot] = MATCHFINDER_INITVAL;
                return;
            }
        }
    }

    /// Advances the matchfinder by one byte without recording matches.
    pub fn skip_byte(
        &mut self,
        in_base: &[u8],
        cur_pos: usize,
        nice_len: usize,
        max_search_depth: usize,
    ) {
        if cur_pos + BT_REQUIRED_NBYTES > in_base.len() {
            return;
        }

        let max_len = nice_len.min(in_base.len() - cur_pos);
        let in_next = &in_base[cur_pos..];
        let cutoff = (cur_pos as isize) - (MATCHFINDER_WINDOW_SIZE as isize);

        let seq3 = load_u24_le(in_next);
        let hash3 = lz_hash(seq3, BT_HASH3_ORDER);
        let seq4 = load_u32_le(in_next);
        let hash4 = lz_hash(seq4, BT_HASH4_ORDER);

        let cur_node_3 = self.hash3_tab[hash3][0];
        self.hash3_tab[hash3][0] = cur_pos as i16;
        self.hash3_tab[hash3][1] = cur_node_3;

        let mut cur_node = self.hash4_tab[hash4] as isize;
        self.hash4_tab[hash4] = cur_pos as i16;

        let mut pending_lt_slot = 2 * (cur_pos & (MATCHFINDER_WINDOW_SIZE - 1));
        let mut pending_gt_slot = pending_lt_slot + 1;

        if cur_node <= cutoff {
            self.child_tab[pending_lt_slot] = MATCHFINDER_INITVAL;
            self.child_tab[pending_gt_slot] = MATCHFINDER_INITVAL;
            return;
        }

        let mut best_lt_len = 0usize;
        let mut best_gt_len = 0usize;
        let mut len = 0usize;
        let mut depth_remaining = max_search_depth;

        loop {
            let node_pos = cur_node as usize;
            let match_ptr = &in_base[node_pos..];

            while len < max_len && match_ptr[len] == in_next[len] {
                len += 1;
            }

            if len >= nice_len {
                let node_slot = 2 * (node_pos & (MATCHFINDER_WINDOW_SIZE - 1));
                self.child_tab[pending_lt_slot] = self.child_tab[node_slot];
                self.child_tab[pending_gt_slot] = self.child_tab[node_slot + 1];
                return;
            }

            if len < max_len && match_ptr[len] < in_next[len] {
                self.child_tab[pending_lt_slot] = cur_node as i16;
                pending_lt_slot = 2 * (node_pos & (MATCHFINDER_WINDOW_SIZE - 1)) + 1;
                cur_node = self.child_tab[pending_lt_slot] as isize;
                best_lt_len = len;
                if best_gt_len < len {
                    len = best_gt_len;
                }
            } else {
                self.child_tab[pending_gt_slot] = cur_node as i16;
                pending_gt_slot = 2 * (node_pos & (MATCHFINDER_WINDOW_SIZE - 1));
                cur_node = self.child_tab[pending_gt_slot] as isize;
                best_gt_len = len;
                if best_lt_len < len {
                    len = best_lt_len;
                }
            }

            depth_remaining -= 1;
            if cur_node <= cutoff || depth_remaining == 0 {
                self.child_tab[pending_lt_slot] = MATCHFINDER_INITVAL;
                self.child_tab[pending_gt_slot] = MATCHFINDER_INITVAL;
                return;
            }
        }
    }
}

impl Default for BtMatchfinder {
    fn default() -> Self {
        Self::new()
    }
}

// MARK: - Helper Functions

/// Multiplicative hash function for sequence prefixes.
#[inline(always)]
pub fn lz_hash(seq: u32, num_bits: usize) -> usize {
    ((seq.wrapping_mul(0x1E35_A7BD)) >> (32 - num_bits)) as usize
}

/// Loads 3 bytes in little-endian order as `u32`.
#[inline(always)]
pub fn load_u24_le(buf: &[u8]) -> u32 {
    (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16)
}

/// Loads 4 bytes in little-endian order as `u32`.
#[inline(always)]
pub fn load_u32_le(buf: &[u8]) -> u32 {
    u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])
}

/// Signed saturating rebase helper for sliding window.
#[inline(always)]
fn rebase_val(val: i16) -> i16 {
    if val >= 0 {
        let rebased = (val as i32) - (MATCHFINDER_WINDOW_SIZE as i32);
        rebased.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    } else {
        MATCHFINDER_INITVAL
    }
}
