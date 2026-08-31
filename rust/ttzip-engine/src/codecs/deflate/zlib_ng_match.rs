// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Modern zlib-ng style Deflate matchfinder and sliding window engine.
//!
//! Provides:
//! - 32 KB sliding window matching with standard RFC 1951 bounds (`MIN_MATCH = 3`, `MAX_MATCH = 258`).
//! - Dual-level hash tables (`head` for bucket entry and `prev` for chain linking).
//! - Vectorized saturating subtraction `slide_hash` for fast window advancement.
//! - 64-bit unaligned word reading with `trailing_zeros` bit-manipulation for byte mismatch detection.
//! - Multi-level Early-Out heuristic matching with candidate endpoint filtering.

use std::cmp::min;

/// RFC 1951 minimum match length.
pub const MIN_MATCH: usize = 3;

/// RFC 1951 maximum match length.
pub const MAX_MATCH: usize = 258;

/// RFC 1951 Deflate sliding window size in bytes (32 KB).
pub const WINDOW_SIZE: usize = 32768;

/// RFC 1951 Deflate sliding window bitmask.
pub const WINDOW_MASK: usize = WINDOW_SIZE - 1;

/// Hash table size (32768 entries, 15-bit address space).
pub const HASH_SIZE: usize = 32768;

/// Hash table index bitmask.
pub const HASH_MASK: usize = HASH_SIZE - 1;

/// Hash shift parameter for 3-byte rolling hash.
pub const HASH_SHIFT: usize = 5;

/// Multiplicative hash constant for 4-byte / 3-byte hashing.
const HASH_PRIME: u32 = 0x1e35a7bd;

/// Matchfinder search configuration and tuning parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatcherConfig {
    /// Maximum search chain depth in the `prev` table.
    pub max_chain: usize,
    /// Nice match length threshold for early-out termination.
    pub nice_length: usize,
    /// Good match length threshold to reduce search depth.
    pub good_length: usize,
    /// Maximum lazy search evaluation length.
    pub max_lazy: usize,
}

impl MatcherConfig {
    /// Creates configuration preset for standard compression levels 0..=9.
    pub fn for_level(level: u32) -> Self {
        match level {
            0 => Self {
                max_chain: 0,
                nice_length: 0,
                good_length: 0,
                max_lazy: 0,
            },
            1 => Self {
                max_chain: 4,
                nice_length: 8,
                good_length: 4,
                max_lazy: 4,
            },
            2 => Self {
                max_chain: 8,
                nice_length: 16,
                good_length: 8,
                max_lazy: 6,
            },
            3 => Self {
                max_chain: 32,
                nice_length: 32,
                good_length: 16,
                max_lazy: 12,
            },
            4 => Self {
                max_chain: 64,
                nice_length: 64,
                good_length: 16,
                max_lazy: 24,
            },
            5 => Self {
                max_chain: 128,
                nice_length: 128,
                good_length: 32,
                max_lazy: 32,
            },
            6 => Self {
                max_chain: 256,
                nice_length: 128,
                good_length: 32,
                max_lazy: 128,
            },
            7 => Self {
                max_chain: 512,
                nice_length: 258,
                good_length: 64,
                max_lazy: 256,
            },
            8 => Self {
                max_chain: 1024,
                nice_length: 258,
                good_length: 128,
                max_lazy: 258,
            },
            _ => Self {
                max_chain: 4096,
                nice_length: 258,
                good_length: 258,
                max_lazy: 258,
            },
        }
    }
}

/// Represents an LZ77 match reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// Match length in bytes (`3..=258`).
    pub length: u16,
    /// Backward distance in bytes (`1..=32768`).
    pub distance: u16,
}

/// Deflate stream token produced by the matcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflateToken {
    /// Unmatched literal byte.
    Literal(u8),
    /// LZ77 copy match reference.
    Match(Match),
}

/// Compares two byte slices up to `max_len` using 64-bit unaligned word reads.
///
/// Returns the number of contiguous identical bytes from the start of each slice.
#[inline(always)]
pub fn match_length_fast(slice_a: &[u8], slice_b: &[u8], max_len: usize) -> usize {
    let limit = min(max_len, min(slice_a.len(), slice_b.len()));
    let mut len = 0;

    // Fast path: 8-byte chunk comparison via 64-bit little-endian loads
    while len + 8 <= limit {
        // Safe unaligned 8-byte load
        let mut buf_a = [0u8; 8];
        let mut buf_b = [0u8; 8];
        buf_a.copy_from_slice(&slice_a[len..len + 8]);
        buf_b.copy_from_slice(&slice_b[len..len + 8]);

        let val_a = u64::from_le_bytes(buf_a);
        let val_b = u64::from_le_bytes(buf_b);
        let diff = val_a ^ val_b;

        if diff != 0 {
            let matched_bytes = (diff.trailing_zeros() >> 3) as usize;
            return len + matched_bytes;
        }
        len += 8;
    }

    // Scalar fallback for remaining trailing bytes (< 8 bytes)
    while len < limit && slice_a[len] == slice_b[len] {
        len += 1;
    }

    len
}

/// Computes a 15-bit hash index from 3-byte prefix.
#[inline(always)]
pub fn compute_hash3(b0: u8, b1: u8, b2: u8) -> usize {
    let raw = (b0 as u32) | ((b1 as u32) << 8) | ((b2 as u32) << 16);
    let hash = raw.wrapping_mul(HASH_PRIME);
    ((hash >> 17) ^ hash) as usize & HASH_MASK
}

/// Modern zlib-ng style Deflate matchfinder.
pub struct ZlibNgMatcher {
    /// Primary hash bucket table mapping hash keys to head window positions.
    head: Vec<u16>,
    /// Link table storing backward match positions within the sliding window.
    prev: Vec<u16>,
    /// Sliding window buffer.
    window: Vec<u8>,
    /// Current position in the window buffer.
    window_pos: usize,
    /// Active matcher configuration parameters.
    config: MatcherConfig,
}

impl ZlibNgMatcher {
    /// Creates a new `ZlibNgMatcher` with specified configuration.
    pub fn new(config: MatcherConfig) -> Self {
        Self {
            head: vec![0u16; HASH_SIZE],
            prev: vec![0u16; WINDOW_SIZE],
            window: vec![0u8; WINDOW_SIZE * 2],
            window_pos: 0,
            config,
        }
    }

    /// Creates a new matcher for a standard compression level (0..=9).
    pub fn with_level(level: u32) -> Self {
        Self::new(MatcherConfig::for_level(level))
    }

    /// Resets all internal tables and window state.
    pub fn reset(&mut self) {
        self.head.fill(0);
        self.prev.fill(0);
        self.window.fill(0);
        self.window_pos = 0;
    }

    /// Returns the active matcher configuration.
    #[inline]
    pub fn config(&self) -> MatcherConfig {
        self.config
    }

    /// Updates the matcher configuration.
    #[inline]
    pub fn set_config(&mut self, config: MatcherConfig) {
        self.config = config;
    }

    /// Performs vectorized saturating subtraction across head and prev tables.
    ///
    /// Clears any indices that have slid past the 32 KB sliding window.
    #[inline]
    pub fn slide_hash(&mut self) {
        let wsize = WINDOW_SIZE as u16;

        // Auto-vectorized slice transformation via saturating subtraction
        for h in self.head.iter_mut() {
            *h = h.saturating_sub(wsize);
        }
        for p in self.prev.iter_mut() {
            *p = p.saturating_sub(wsize);
        }
    }

    /// Inserts a string at `pos` into the hash table, returning the previous head position.
    #[inline]
    pub fn insert_string(&mut self, pos: usize, data: &[u8]) -> u16 {
        if pos + MIN_MATCH > data.len() {
            return 0;
        }

        let hash = compute_hash3(data[pos], data[pos + 1], data[pos + 2]);
        let prev_head = self.head[hash];
        self.head[hash] = (pos + 1) as u16; // 1-based indexing
        self.prev[pos & WINDOW_MASK] = prev_head;
        prev_head
    }

    /// Searches the hash chain for the longest match at `pos`.
    ///
    /// Employs 64-bit unaligned comparison, endpoint early-out checking, and nice length early-out.
    pub fn find_longest_match(
        &self,
        pos: usize,
        data: &[u8],
        prev_match_len: u16,
    ) -> Option<Match> {
        if self.config.max_chain == 0 || pos + MIN_MATCH > data.len() {
            return None;
        }

        let hash = compute_hash3(data[pos], data[pos + 1], data[pos + 2]);
        let mut cur_match = self.head[hash];
        if cur_match == 0 {
            return None;
        }

        let mut best_len = (prev_match_len as usize).max(MIN_MATCH - 1);
        let mut best_dist = 0usize;
        let mut chain_length = self.config.max_chain;
        let nice_len = self.config.nice_length;
        let max_len = min(MAX_MATCH, data.len() - pos);

        if best_len >= self.config.good_length {
            chain_length >>= 2;
        }

        while cur_match != 0 && chain_length > 0 {
            chain_length -= 1;
            let match_pos = (cur_match - 1) as usize;

            if match_pos >= pos {
                cur_match = self.prev[match_pos & WINDOW_MASK];
                continue;
            }

            let dist = pos - match_pos;
            if dist > WINDOW_SIZE {
                break;
            }

            // Early-Out: Filter candidate by verifying match endpoint and current best length byte
            if pos + best_len < data.len()
                && match_pos + best_len < data.len()
                && (data[match_pos + best_len] != data[pos + best_len]
                    || (best_len > 0 && data[match_pos + best_len - 1] != data[pos + best_len - 1])
                    || data[match_pos] != data[pos])
            {
                cur_match = self.prev[match_pos & WINDOW_MASK];
                continue;
            }

            // 64-bit accelerated full comparison
            let len = match_length_fast(&data[match_pos..], &data[pos..], max_len);

            if len > best_len {
                best_len = len;
                best_dist = dist;

                // Nice length early-out cutoff
                if len >= nice_len || len >= max_len {
                    break;
                }
            }

            cur_match = self.prev[match_pos & WINDOW_MASK];
        }

        if best_len >= MIN_MATCH && best_dist > 0 && best_dist <= WINDOW_SIZE {
            Some(Match {
                length: best_len as u16,
                distance: best_dist as u16,
            })
        } else {
            None
        }
    }

    /// Tokenizes an input stream into literal bytes and LZ77 match tokens.
    pub fn tokenize_stream(&mut self, input: &[u8]) -> Vec<DeflateToken> {
        let mut tokens = Vec::with_capacity(input.len() / 2);
        let mut pos = 0;

        self.reset();

        while pos < input.len() {
            if pos + MIN_MATCH > input.len() {
                while pos < input.len() {
                    tokens.push(DeflateToken::Literal(input[pos]));
                    pos += 1;
                }
                break;
            }

            let prev_head = self.insert_string(pos, input);
            let mut matched = None;

            if prev_head != 0 && self.config.max_chain > 0 {
                matched = self.find_longest_match(pos, input, 0);
            }

            if let Some(m) = matched {
                let match_len = m.length as usize;
                tokens.push(DeflateToken::Match(m));

                // Insert intermediate strings for the matched span
                for k in 1..match_len {
                    if pos + k + MIN_MATCH <= input.len() {
                        self.insert_string(pos + k, input);
                    }
                }
                pos += match_len;
            } else {
                tokens.push(DeflateToken::Literal(input[pos]));
                pos += 1;
            }
        }

        tokens
    }
}
