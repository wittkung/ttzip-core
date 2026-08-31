// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-precision Zopfli Directed Acyclic Graph (DAG) shortest path LZ77 matcher.
//!
//! # Algorithmic Architecture
//!
//! 1. **DAG State Graph Formulation**:
//!    - Each input byte position $i \in [0, N]$ forms a graph node.
//!    - Forward edges represent either an uncompressed literal ($i \to i+1$) or an LZ77
//!      match reference ($i \to i + \text{len}$) with backward distance $\text{dist}$.
//!
//! 2. **Shannon Self-Information Cost Model**:
//!    - Edge weight $\text{Cost}(e) = \text{LengthCost}(\text{len}) + \text{DistCost}(\text{dist}) + \text{ExtraBits}$.
//!    - Self-information $I(s) = -\log_2(P(s)) = \log_2(N_{\text{total}}) - \log_2(\text{freq}(s))$.
//!    - Laplace smoothing applied to unobserved symbols to avoid infinite path penalties.
//!
//! 3. **Topological Single-Source Shortest Path (SSSP)**:
//!    - Since the graph is strictly a forward DAG, single-pass relaxation visits nodes
//!      $i = 0 \dots N-1$ in $O(V + E)$ topological order.
//!    - Multi-length candidate evaluation tests all sub-lengths $3..=\text{max\_len}$ to find
//!      globally optimal non-greedy path configurations.

use std::cmp::min;

// MARK: - RFC 1951 Deflate Constants

/// RFC 1951 Deflate sliding window size in bytes (32 KB).
pub const WINDOW_SIZE: usize = 32768;

/// RFC 1951 Deflate sliding window bitmask.
pub const WINDOW_MASK: usize = WINDOW_SIZE - 1;

/// RFC 1951 minimum match length.
pub const MIN_MATCH: usize = 3;

/// RFC 1951 maximum match length.
pub const MAX_MATCH: usize = 258;

/// Number of literal / length alphabet symbols (0..=285).
pub const NUM_LITLEN_SYMS: usize = 288;

/// Number of distance / offset alphabet symbols (0..=29).
pub const NUM_DIST_SYMS: usize = 32;

/// Symbol index representing End of Block (EOB = 256).
pub const END_OF_BLOCK_SYM: usize = 256;

/// First match length symbol index (257).
pub const FIRST_LEN_SYM: usize = 257;

/// Hash table size (16-bit address space = 65536 entries).
pub const HASH_SIZE: usize = 65536;

/// Hash table index bitmask.
pub const HASH_MASK: usize = HASH_SIZE - 1;

/// Multiplicative hash constant for 3-byte prefix hashing.
const HASH_PRIME: u32 = 0x1e35a7bd;

// MARK: - RFC 1951 Lookup Tables

/// RFC 1951 extra bits for match length slots (slots 0..=28).
pub const EXTRA_LENGTH_BITS: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// RFC 1951 length base values (slots 0..=28).
pub const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];

/// RFC 1951 extra bits for distance slots (slots 0..=29).
pub const EXTRA_OFFSET_BITS: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

/// RFC 1951 distance base values (slots 0..=29).
pub const OFFSET_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// Precomputed mapping: match length (0..=258) -> length slot (0..=28).
pub const LENGTH_SLOT_MAP: [u8; 259] = [
    0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 12, 12, 13, 13, 13, 13,
    14, 14, 14, 14, 15, 15, 15, 15, 16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 17, 17, 17,
    17, 18, 18, 18, 18, 18, 18, 18, 18, 19, 19, 19, 19, 19, 19, 19, 19, 20, 20, 20, 20, 20, 20,
    20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21,
    21, 21, 21, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 23, 23, 23, 23,
    23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
    24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25,
    25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25,
    25, 25, 25, 25, 25, 25, 25, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26,
    26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 27, 27, 27, 27, 27, 27, 27,
    27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27,
    27, 28,
];

/// Returns the RFC 1951 distance slot index (0..=29) for backward distance `dist` (1..=32768).
#[inline(always)]
pub fn get_dist_slot(dist: u16) -> usize {
    debug_assert!((1..=32768).contains(&dist));
    let d = dist as usize;
    if d <= 256 {
        match d {
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            5..=6 => 4,
            7..=8 => 5,
            9..=12 => 6,
            13..=16 => 7,
            17..=24 => 8,
            25..=32 => 9,
            33..=48 => 10,
            49..=64 => 11,
            65..=96 => 12,
            97..=128 => 13,
            129..=192 => 14,
            _ => 15,
        }
    } else if d <= 1024 {
        if d <= 384 {
            16
        } else if d <= 512 {
            17
        } else if d <= 768 {
            18
        } else {
            19
        }
    } else if d <= 4096 {
        if d <= 1536 {
            20
        } else if d <= 2048 {
            21
        } else if d <= 3072 {
            22
        } else {
            23
        }
    } else if d <= 8192 {
        if d <= 6144 {
            24
        } else {
            25
        }
    } else if d <= 16384 {
        if d <= 12288 {
            26
        } else {
            27
        }
    } else if d <= 24576 {
        28
    } else {
        29
    }
}

// MARK: - Token Representation

/// Token emitted during LZ77 shortest path parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZopfliToken {
    /// Uncompressed literal byte.
    Literal(u8),
    /// Backward match reference with length (3..=258) and distance (1..=32768).
    Match {
        /// Matched length.
        length: u16,
        /// Backward distance.
        distance: u16,
    },
}

// MARK: - Shannon Cost Model

/// Cost model providing exact or statistical bit costs for literals and matches.
#[derive(Debug, Clone)]
pub struct ZopfliCostModel {
    /// Bit costs for literal/length symbols (0..287).
    pub litlen_costs: [f64; NUM_LITLEN_SYMS],
    /// Bit costs for distance symbols (0..31).
    pub dist_costs: [f64; NUM_DIST_SYMS],
}

impl Default for ZopfliCostModel {
    fn default() -> Self {
        Self::uniform()
    }
}

impl ZopfliCostModel {
    /// Creates a uniform baseline cost model.
    pub fn uniform() -> Self {
        let mut model = Self {
            litlen_costs: [8.5; NUM_LITLEN_SYMS],
            dist_costs: [6.0; NUM_DIST_SYMS],
        };
        // End-of-block symbol penalty
        model.litlen_costs[END_OF_BLOCK_SYM] = 10.0;
        model
    }

    /// Builds a cost model based on Shannon self-information from symbol frequency tallies.
    ///
    /// $I(s) = -\log_2(P(s)) = \log_2(N) - \log_2(f_s)$
    pub fn from_shannon_frequencies(
        litlen_freqs: &[u32; NUM_LITLEN_SYMS],
        dist_freqs: &[u32; NUM_DIST_SYMS],
    ) -> Self {
        let mut model = Self {
            litlen_costs: [0.0; NUM_LITLEN_SYMS],
            dist_costs: [0.0; NUM_DIST_SYMS],
        };

        let litlen_total: u32 = litlen_freqs.iter().sum();
        let dist_total: u32 = dist_freqs.iter().sum();

        let log2_litlen_total = if litlen_total > 0 {
            (litlen_total as f64).log2()
        } else {
            8.0
        };

        for (sym, &freq) in litlen_freqs.iter().enumerate() {
            if freq > 0 {
                model.litlen_costs[sym] = log2_litlen_total - (freq as f64).log2();
            } else {
                // Laplace pseudo-count penalty for unobserved symbols
                model.litlen_costs[sym] = log2_litlen_total + 2.5;
            }
        }

        let log2_dist_total = if dist_total > 0 {
            (dist_total as f64).log2()
        } else {
            5.0
        };

        for (sym, &freq) in dist_freqs.iter().enumerate() {
            if freq > 0 {
                model.dist_costs[sym] = log2_dist_total - (freq as f64).log2();
            } else {
                model.dist_costs[sym] = log2_dist_total + 2.0;
            }
        }

        model
    }

    /// Builds a cost model directly from computed canonical Huffman codeword lengths.
    pub fn from_huffman_lengths(
        litlen_lens: &[u8; NUM_LITLEN_SYMS],
        dist_lens: &[u8; NUM_DIST_SYMS],
    ) -> Self {
        let mut model = Self {
            litlen_costs: [0.0; NUM_LITLEN_SYMS],
            dist_costs: [0.0; NUM_DIST_SYMS],
        };

        for (sym, &len) in litlen_lens.iter().enumerate() {
            model.litlen_costs[sym] = if len > 0 { len as f64 } else { 15.0 };
        }

        for (sym, &len) in dist_lens.iter().enumerate() {
            model.dist_costs[sym] = if len > 0 { len as f64 } else { 15.0 };
        }

        model
    }

    /// Evaluates bit cost for an uncompressed literal byte.
    #[inline(always)]
    pub fn literal_cost(&self, lit: u8) -> f64 {
        self.litlen_costs[lit as usize]
    }

    /// Evaluates bit cost for an LZ77 match reference `(distance, length)`.
    #[inline(always)]
    pub fn match_cost(&self, dist: u16, len: u16) -> f64 {
        debug_assert!((3..=258).contains(&len));
        debug_assert!((1..=32768).contains(&dist));

        let lslot = LENGTH_SLOT_MAP[len as usize] as usize;
        let dslot = get_dist_slot(dist);

        let lcost = self.litlen_costs[FIRST_LEN_SYM + lslot] + EXTRA_LENGTH_BITS[lslot] as f64;
        let dcost = self.dist_costs[dslot] + EXTRA_OFFSET_BITS[dslot] as f64;

        lcost + dcost
    }
}

// MARK: - Sliding Window Hash Table

/// 3-byte rolling hash table for Zopfli sliding window search.
pub struct ZopfliHash {
    head: Vec<i32>,
    prev: Vec<u16>,
}

impl Default for ZopfliHash {
    fn default() -> Self {
        Self::new()
    }
}

impl ZopfliHash {
    /// Creates a newly initialized Zopfli hash table.
    pub fn new() -> Self {
        Self {
            head: vec![-1; HASH_SIZE],
            prev: vec![0; WINDOW_SIZE],
        }
    }

    /// Resets all internal tables.
    pub fn reset(&mut self) {
        self.head.fill(-1);
        self.prev.fill(0);
    }

    /// Computes 16-bit hash index from 3-byte sequence.
    #[inline(always)]
    pub fn hash3(b0: u8, b1: u8, b2: u8) -> usize {
        let raw = (b0 as u32) | ((b1 as u32) << 8) | ((b2 as u32) << 16);
        let hash = raw.wrapping_mul(HASH_PRIME);
        ((hash >> 16) ^ hash) as usize & HASH_MASK
    }

    /// Updates the hash table with the byte at `pos`.
    #[inline(always)]
    pub fn update(&mut self, data: &[u8], pos: usize) {
        if pos + 2 < data.len() {
            let h = Self::hash3(data[pos], data[pos + 1], data[pos + 2]);
            let old_head = self.head[h];
            self.prev[pos & WINDOW_MASK] = if old_head >= 0 {
                (pos - (old_head as usize)).min(WINDOW_SIZE) as u16
            } else {
                0
            };
            self.head[h] = pos as i32;
        }
    }

    /// Finds all non-dominated candidate matches at `pos`.
    ///
    /// Stores the minimum backward distance for each match length $3..=\text{max\_len}$
    /// in `sublen[3..=258]`. Returns the maximum length found.
    pub fn find_longest_matches(
        &mut self,
        data: &[u8],
        pos: usize,
        max_chain: usize,
        sublen: &mut [u16; 259],
    ) -> u16 {
        sublen.fill(0);
        if pos + MIN_MATCH > data.len() {
            return 0;
        }

        let max_len_limit = min(MAX_MATCH, data.len() - pos);
        let h = Self::hash3(data[pos], data[pos + 1], data[pos + 2]);
        let mut match_pos = self.head[h];

        // Insert current position into hash chain
        self.prev[pos & WINDOW_MASK] = if match_pos >= 0 {
            (pos - (match_pos as usize)).min(WINDOW_SIZE) as u16
        } else {
            0
        };
        self.head[h] = pos as i32;

        let mut longest_len = 0u16;
        let mut chain_len = 0;

        while match_pos >= 0 && chain_len < max_chain {
            let mp = match_pos as usize;
            if pos <= mp || pos - mp > WINDOW_SIZE {
                break;
            }

            let dist = (pos - mp) as u16;

            // Early check of endpoint and first byte before full scan
            if longest_len >= 3 {
                let check_idx = longest_len as usize;
                if check_idx < max_len_limit && data[pos + check_idx] != data[mp + check_idx] {
                    let step = self.prev[mp & WINDOW_MASK] as usize;
                    if step == 0 {
                        break;
                    }
                    match_pos -= step as i32;
                    chain_len += 1;
                    continue;
                }
            }

            // Compare matching bytes
            let mut len = 0;
            while len < max_len_limit && data[pos + len] == data[mp + len] {
                len += 1;
            }

            if len >= MIN_MATCH && len as u16 > longest_len {
                // Update sublen for all newly covered lengths
                for l in (longest_len as usize + 1)..=len {
                    sublen[l] = dist;
                }
                longest_len = len as u16;
            }

            if longest_len as usize >= max_len_limit {
                break;
            }

            let step = self.prev[mp & WINDOW_MASK] as usize;
            if step == 0 {
                break;
            }
            match_pos -= step as i32;
            chain_len += 1;
        }

        longest_len
    }
}

// MARK: - Match Cache for Multi-Pass Squeeze

/// Cached match references for a block to eliminate redundant matchfinding across Squeeze passes.
pub struct ZopfliMatchCache {
    /// Cached (max_len, sublen) per position.
    cached_lengths: Vec<u16>,
    cached_sublens: Vec<[u16; 259]>,
    is_populated: bool,
}

impl Default for ZopfliMatchCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ZopfliMatchCache {
    /// Creates a new match cache.
    pub fn new() -> Self {
        Self {
            cached_lengths: Vec::new(),
            cached_sublens: Vec::new(),
            is_populated: false,
        }
    }

    /// Resets the cache for a new block of size `block_len`.
    pub fn reset(&mut self, block_len: usize) {
        self.cached_lengths.clear();
        self.cached_lengths.resize(block_len, 0);
        self.cached_sublens.clear();
        self.cached_sublens.resize(block_len, [0u16; 259]);
        self.is_populated = false;
    }

    /// Returns whether the cache is populated.
    #[inline(always)]
    pub fn is_populated(&self) -> bool {
        self.is_populated
    }

    /// Marks cache as populated.
    #[inline(always)]
    pub fn set_populated(&mut self, populated: bool) {
        self.is_populated = populated;
    }
}

// MARK: - Shortest Path Matcher

/// Zopfli DAG shortest path LZ77 matcher.
pub struct ZopfliShortestPathMatcher {
    hash: ZopfliHash,
    cache: ZopfliMatchCache,
    costs: Vec<f64>,
    from_pos: Vec<u32>,
    from_dist: Vec<u16>,
}

impl Default for ZopfliShortestPathMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ZopfliShortestPathMatcher {
    /// Creates a new shortest path matcher.
    pub fn new() -> Self {
        Self {
            hash: ZopfliHash::new(),
            cache: ZopfliMatchCache::new(),
            costs: Vec::new(),
            from_pos: Vec::new(),
            from_dist: Vec::new(),
        }
    }

    /// Warms up hash table up to `from` offset.
    pub fn warmup_hash(&mut self, data: &[u8], from: usize) {
        self.hash.reset();
        let start = from.saturating_sub(WINDOW_SIZE);
        for p in start..from {
            self.hash.update(data, p);
        }
    }

    /// Resets cache state for a new compression session.
    pub fn reset_cache(&mut self, block_len: usize) {
        self.cache.reset(block_len);
    }

    /// Finds the optimal sequence of tokens using DAG shortest path relaxation.
    pub fn find_shortest_path(
        &mut self,
        data: &[u8],
        from: usize,
        to: usize,
        cost_model: &ZopfliCostModel,
        max_chain: usize,
    ) -> Vec<ZopfliToken> {
        let block_len = to.saturating_sub(from);
        if block_len == 0 {
            return Vec::new();
        }

        // Initialize DP arrays
        self.costs.clear();
        self.costs.resize(block_len + 1, f64::INFINITY);
        self.costs[0] = 0.0;

        self.from_pos.clear();
        self.from_pos.resize(block_len + 1, 0);

        self.from_dist.clear();
        self.from_dist.resize(block_len + 1, 0);

        let mut sublen = [0u16; 259];
        let use_cache = self.cache.is_populated();

        // 1. Forward DAG relaxation
        for i in 0..block_len {
            let cur_cost = self.costs[i];
            if cur_cost == f64::INFINITY {
                continue;
            }

            let cur_global_pos = from + i;

            // Literal edge: i -> i + 1
            let lit = data[cur_global_pos];
            let lit_cost = cur_cost + cost_model.literal_cost(lit);
            if lit_cost < self.costs[i + 1] {
                self.costs[i + 1] = lit_cost;
                self.from_pos[i + 1] = i as u32;
                self.from_dist[i + 1] = 0;
            }

            // Match edges: i -> i + len
            let longest_len = if use_cache {
                let clen = self.cache.cached_lengths[i];
                sublen.copy_from_slice(&self.cache.cached_sublens[i]);
                clen
            } else {
                let clen = self.hash.find_longest_matches(
                    data,
                    cur_global_pos,
                    max_chain,
                    &mut sublen,
                );
                if i < self.cache.cached_lengths.len() {
                    self.cache.cached_lengths[i] = clen;
                    self.cache.cached_sublens[i].copy_from_slice(&sublen);
                }
                clen
            };

            if longest_len >= MIN_MATCH as u16 {
                let max_len = min(longest_len as usize, block_len - i);
                for len in MIN_MATCH..=max_len {
                    let dist = sublen[len];
                    if dist > 0 {
                        let m_cost = cur_cost + cost_model.match_cost(dist, len as u16);
                        if m_cost < self.costs[i + len] {
                            self.costs[i + len] = m_cost;
                            self.from_pos[i + len] = i as u32;
                            self.from_dist[i + len] = dist;
                        }
                    }
                }
            }
        }

        if !use_cache {
            self.cache.set_populated(true);
        }

        // 2. Backtrack to reconstruct shortest path
        let mut tokens = Vec::with_capacity(block_len / 2 + 1);
        let mut curr = block_len;

        while curr > 0 {
            let prev = self.from_pos[curr] as usize;
            let dist = self.from_dist[curr];
            let len = (curr - prev) as u16;

            if dist == 0 {
                tokens.push(ZopfliToken::Literal(data[from + prev]));
            } else {
                tokens.push(ZopfliToken::Match {
                    length: len,
                    distance: dist,
                });
            }
            curr = prev;
        }

        tokens.reverse();
        tokens
    }
}
