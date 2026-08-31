// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-precision Near-Optimal Dynamic Programming (DP) parser with EM iterative refinement.
//!
//! # Architecture & Algorithm
//!
//! 1. **Fixed-Point Cost Model (`BIT_COST = 16`)**:
//!    - Costs are scaled by $16$ to maintain sub-bit fractional precision in integer arithmetic
//!      without floating-point non-determinism or overhead.
//!
//! 2. **Backward Dynamic Programming (`find_min_cost_path`)**:
//!    - Evaluates the optimal literal/match choice by moving backwards from $N$ down to $0$.
//!    - Node state `OptimumNode` packs `cost_to_end` and `item` (offset/length token).
//!
//! 3. **Expectation-Maximization (EM) Iterative Optimization (2~4 passes)**:
//!    - Pass 1: Computes initial minimum cost path with uniform / empirical entropy costs.
//!    - Pass 2..4: Tallies symbol frequencies, builds optimal length-limited ($\le 15$ bits)
//!      Huffman code lengths via Package-Merge, updates costs, and re-evaluates min-cost paths
//!      until convergence.

use super::bt_matchfinder::{BtMatchfinder, LzMatch, DEFLATE_MAX_MATCH_LEN, DEFLATE_MIN_MATCH_LEN};

// MARK: - Constants

/// Scaling factor for fixed-point bit cost representations ($1.0 \text{ bit} = 16$).
pub const BIT_COST: u32 = 16;

/// Number of literal symbols in Deflate (0..=255).
pub const DEFLATE_NUM_LITERALS: usize = 256;

/// End of block symbol index in litlen alphabet.
pub const DEFLATE_END_OF_BLOCK: usize = 256;

/// First length symbol index in litlen alphabet.
pub const DEFLATE_FIRST_LEN_SYM: usize = 257;

/// Total number of litlen alphabet symbols (288).
pub const DEFLATE_NUM_LITLEN_SYMS: usize = 288;

/// Total number of distance/offset alphabet symbols (32).
pub const DEFLATE_NUM_OFFSET_SYMS: usize = 32;

/// Maximum allowable Huffman codeword length in Deflate.
pub const MAX_HUFFMAN_CODE_LEN: usize = 15;

/// Number of extra bits for each length slot (slots 0..=28).
pub const EXTRA_LENGTH_BITS: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Number of extra bits for each offset slot (slots 0..=29).
pub const EXTRA_OFFSET_BITS: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

/// Precomputed mapping: match length ($0..=258$) -> length slot ($0..=28$).
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

// MARK: - Data Types

/// Item in the optimal parsed sequence (literal byte or backward match reference).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceItem {
    /// Uncompressed literal byte.
    Literal(u8),
    /// Backward match reference with length (3..=258) and distance offset (1..=32768).
    Match {
        /// Matched length.
        length: u16,
        /// Backward distance.
        offset: u16,
    },
}

/// Dynamic programming node representing an offset in the input buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptimumNode {
    /// Cumulative minimum cost to reach the end of the block from this position.
    pub cost_to_end: u32,
    /// Bit-packed item: low 9 bits = length (1 for literal), high bits = offset or literal byte.
    pub item: u32,
}

impl OptimumNode {
    /// Mask for extracting the length field (bits 0..=8).
    pub const LEN_MASK: u32 = 0x1FF;
    /// Shift amount for extracting the offset/literal field (bits 9..=31).
    pub const OFFSET_SHIFT: u32 = 9;

    /// Packs a literal byte into an item representation (`length = 1`).
    #[inline(always)]
    pub fn pack_literal(lit: u8) -> u32 {
        ((lit as u32) << Self::OFFSET_SHIFT) | 1
    }

    /// Packs a match reference into an item representation.
    #[inline(always)]
    pub fn pack_match(length: u16, offset: u16) -> u32 {
        ((offset as u32) << Self::OFFSET_SHIFT) | ((length as u32) & Self::LEN_MASK)
    }

    /// Returns `true` if this node selects a literal item.
    #[inline(always)]
    pub fn is_literal(&self) -> bool {
        (self.item & Self::LEN_MASK) == 1
    }

    /// Unpacks the item as a `SequenceItem`.
    #[inline(always)]
    pub fn unpack(&self) -> SequenceItem {
        let len = (self.item & Self::LEN_MASK) as u16;
        let off = (self.item >> Self::OFFSET_SHIFT) as u16;
        if len == 1 {
            SequenceItem::Literal(off as u8)
        } else {
            SequenceItem::Match {
                length: len,
                offset: off,
            }
        }
    }
}

/// Cost model storing fixed-point costs ($16 \times \text{bits}$) for all Deflate symbols.
#[derive(Debug, Clone)]
pub struct CostModel {
    /// Cost to output each literal symbol (0..=255).
    pub literal: [u32; DEFLATE_NUM_LITERALS],
    /// Cost to output each match length (0..=258, entries 3..=258 used).
    pub length: [u32; DEFLATE_MAX_MATCH_LEN + 1],
    /// Cost to output each offset slot (0..=29).
    pub offset_slot: [u32; 30],
}

impl CostModel {
    /// Creates a default cost model based on standard Deflate entropy assumptions.
    pub fn default_uniform() -> Self {
        let mut model = Self {
            literal: [8 * BIT_COST; DEFLATE_NUM_LITERALS],
            length: [0; DEFLATE_MAX_MATCH_LEN + 1],
            offset_slot: [0; 30],
        };

        for len in DEFLATE_MIN_MATCH_LEN..=DEFLATE_MAX_MATCH_LEN {
            let slot = LENGTH_SLOT_MAP[len] as usize;
            let extra = EXTRA_LENGTH_BITS[slot] as u32;
            model.length[len] = (6 + extra) * BIT_COST;
        }

        for slot in 0..30 {
            let extra = EXTRA_OFFSET_BITS[slot] as u32;
            model.offset_slot[slot] = (5 + extra) * BIT_COST;
        }

        model
    }

    /// Updates the cost model from computed Huffman codeword lengths.
    pub fn update_from_lens(
        &mut self,
        litlen_lens: &[u8; DEFLATE_NUM_LITLEN_SYMS],
        offset_lens: &[u8; DEFLATE_NUM_OFFSET_SYMS],
    ) {
        for i in 0..DEFLATE_NUM_LITERALS {
            let bits = if litlen_lens[i] > 0 {
                litlen_lens[i] as u32
            } else {
                12
            };
            self.literal[i] = bits * BIT_COST;
        }

        for len in DEFLATE_MIN_MATCH_LEN..=DEFLATE_MAX_MATCH_LEN {
            let slot = LENGTH_SLOT_MAP[len] as usize;
            let sym = DEFLATE_FIRST_LEN_SYM + slot;
            let bits = if litlen_lens[sym] > 0 {
                litlen_lens[sym] as u32
            } else {
                12
            };
            let extra = EXTRA_LENGTH_BITS[slot] as u32;
            self.length[len] = (bits + extra) * BIT_COST;
        }

        for slot in 0..30 {
            let bits = if offset_lens[slot] > 0 {
                offset_lens[slot] as u32
            } else {
                8
            };
            let extra = EXTRA_OFFSET_BITS[slot] as u32;
            self.offset_slot[slot] = (bits + extra) * BIT_COST;
        }
    }

    /// Maps match offset ($1..=32768$) to offset slot index ($0..=29$) in $O(1)$.
    #[inline(always)]
    pub fn offset_slot(offset: u16) -> usize {
        let off = (offset.saturating_sub(1)) as u32;
        if off < 4 {
            off as usize
        } else {
            let msb = 31 - off.leading_zeros();
            ((msb << 1) | ((off >> (msb - 1)) & 1)) as usize
        }
    }
}

// MARK: - Dynamic Programming Search

/// Finds the minimum-cost path through a block using backward dynamic programming.
pub fn find_min_cost_path(
    block: &[u8],
    matches_cache: &[Vec<LzMatch>],
    costs: &CostModel,
) -> Vec<SequenceItem> {
    let block_len = block.len();
    if block_len == 0 {
        return Vec::new();
    }

    let mut nodes = vec![OptimumNode::default(); block_len + 1];
    nodes[block_len].cost_to_end = 0;

    for i in (0..block_len).rev() {
        let literal = block[i];
        let mut best_cost = costs.literal[literal as usize] + nodes[i + 1].cost_to_end;
        let mut best_item = OptimumNode::pack_literal(literal);

        if i < matches_cache.len() {
            for m in &matches_cache[i] {
                let match_len = (m.length as usize).min(block_len - i);
                let offset = m.offset;
                let offset_slot = CostModel::offset_slot(offset);
                let offset_cost = costs.offset_slot[offset_slot];

                for len in DEFLATE_MIN_MATCH_LEN..=match_len {
                    let cost = offset_cost + costs.length[len] + nodes[i + len].cost_to_end;
                    if cost < best_cost {
                        best_cost = cost;
                        best_item = OptimumNode::pack_match(len as u16, offset);
                    }
                }
            }
        }

        nodes[i] = OptimumNode {
            cost_to_end: best_cost,
            item: best_item,
        };
    }

    // Reconstruct sequence forward from index 0
    let mut seq = Vec::with_capacity(block_len / 2 + 8);
    let mut pos = 0;
    while pos < block_len {
        let item = nodes[pos].unpack();
        match item {
            SequenceItem::Literal(_) => {
                seq.push(item);
                pos += 1;
            }
            SequenceItem::Match { length, .. } => {
                seq.push(item);
                pos += length as usize;
            }
        }
    }

    seq
}

// MARK: - EM Iterative Optimization

/// Performs 2~4 passes of Expectation-Maximization iterative optimization to converge
/// on the near-optimal parse and minimal compressed bit length.
pub fn optimize_parse_em(
    block: &[u8],
    matches_cache: &[Vec<LzMatch>],
    max_passes: usize,
) -> (Vec<SequenceItem>, u32) {
    let block_len = block.len();
    if block_len == 0 {
        return (Vec::new(), 0);
    }

    let mut cost_model = CostModel::default_uniform();
    let max_passes = max_passes.clamp(1, 4);

    let mut best_seq = Vec::new();
    let mut best_true_cost = u32::MAX;

    for _pass in 0..max_passes {
        let seq = find_min_cost_path(block, matches_cache, &cost_model);

        let mut litlen_freqs = [0u32; DEFLATE_NUM_LITLEN_SYMS];
        let mut offset_freqs = [0u32; DEFLATE_NUM_OFFSET_SYMS];

        for item in &seq {
            match *item {
                SequenceItem::Literal(lit) => {
                    litlen_freqs[lit as usize] += 1;
                }
                SequenceItem::Match { length, offset } => {
                    let slot = LENGTH_SLOT_MAP[length as usize] as usize;
                    litlen_freqs[DEFLATE_FIRST_LEN_SYM + slot] += 1;
                    let off_slot = CostModel::offset_slot(offset);
                    offset_freqs[off_slot] += 1;
                }
            }
        }
        litlen_freqs[DEFLATE_END_OF_BLOCK] += 1;

        let litlen_lens = compute_huffman_lengths(&litlen_freqs, MAX_HUFFMAN_CODE_LEN);
        let offset_lens = compute_huffman_lengths(&offset_freqs, MAX_HUFFMAN_CODE_LEN);

        let true_cost = compute_true_bits(&seq, &litlen_lens, &offset_lens);

        if true_cost < best_true_cost {
            best_true_cost = true_cost;
            best_seq = seq;
        } else {
            // Early stopping when cost does not improve
            break;
        }

        cost_model.update_from_lens(&litlen_lens, &offset_lens);
    }

    (best_seq, best_true_cost)
}

/// Helper function to build a pre-populated match cache for an entire input block.
pub fn build_matches_cache(
    mf: &mut BtMatchfinder,
    block: &[u8],
    nice_len: usize,
    max_search_depth: usize,
) -> Vec<Vec<LzMatch>> {
    let mut cache = Vec::with_capacity(block.len());
    let mut matches = Vec::with_capacity(16);

    for pos in 0..block.len() {
        mf.get_matches(
            block,
            pos,
            DEFLATE_MAX_MATCH_LEN,
            nice_len,
            max_search_depth,
            &mut matches,
        );
        cache.push(matches.clone());
    }

    cache
}

// MARK: - Package-Merge Length-Limited Huffman Builder

/// Computes optimal length-limited ($\le \text{max\_len}$) Huffman codeword lengths
/// using the Package-Merge algorithm ($O(N \cdot L)$).
pub fn compute_huffman_lengths<const N: usize>(freqs: &[u32; N], max_len: usize) -> [u8; N] {
    let mut active: Vec<(u32, usize)> = Vec::with_capacity(N);
    for (sym, &f) in freqs.iter().enumerate() {
        if f > 0 {
            active.push((f, sym));
        }
    }

    let mut lengths = [0u8; N];
    let num_active = active.len();
    if num_active == 0 {
        return lengths;
    }
    if num_active == 1 {
        lengths[active[0].1] = 1;
        return lengths;
    }

    active.sort_unstable_by_key(|&(f, _)| f);

    #[derive(Clone)]
    struct Item {
        weight: u64,
        leaves: Vec<usize>,
    }

    let mut current_level: Vec<Item> = active
        .iter()
        .map(|&(f, sym)| Item {
            weight: f as u64,
            leaves: vec![sym],
        })
        .collect();

    let mut all_levels: Vec<Vec<Item>> = Vec::with_capacity(max_len);

    for _level in 0..max_len {
        all_levels.push(current_level.clone());

        // Form packages by pairing adjacent elements
        let mut packages: Vec<Item> = Vec::with_capacity(current_level.len() / 2);
        let mut i = 0;
        while i + 1 < current_level.len() {
            let a = &current_level[i];
            let b = &current_level[i + 1];
            let mut leaves = Vec::with_capacity(a.leaves.len() + b.leaves.len());
            leaves.extend_from_slice(&a.leaves);
            leaves.extend_from_slice(&b.leaves);
            packages.push(Item {
                weight: a.weight + b.weight,
                leaves,
            });
            i += 2;
        }

        // Merge packages with base singleton items
        let mut next_level: Vec<Item> = Vec::with_capacity(active.len() + packages.len());
        let mut p_idx = 0;
        let mut a_idx = 0;

        while p_idx < packages.len() && a_idx < active.len() {
            if (packages[p_idx].weight) <= (active[a_idx].0 as u64) {
                next_level.push(packages[p_idx].clone());
                p_idx += 1;
            } else {
                next_level.push(Item {
                    weight: active[a_idx].0 as u64,
                    leaves: vec![active[a_idx].1],
                });
                a_idx += 1;
            }
        }
        while p_idx < packages.len() {
            next_level.push(packages[p_idx].clone());
            p_idx += 1;
        }
        while a_idx < active.len() {
            next_level.push(Item {
                weight: active[a_idx].0 as u64,
                leaves: vec![active[a_idx].1],
            });
            a_idx += 1;
        }

        current_level = next_level;
    }

    let needed = 2 * num_active - 2;
    let final_level = &all_levels[max_len - 1];
    let num_select = needed.min(final_level.len());

    for item in &final_level[..num_select] {
        for &sym in &item.leaves {
            lengths[sym] = lengths[sym].saturating_add(1);
        }
    }

    lengths
}

/// Computes the exact encoded bit count for a parsed sequence given codeword lengths.
fn compute_true_bits(
    seq: &[SequenceItem],
    litlen_lens: &[u8; DEFLATE_NUM_LITLEN_SYMS],
    offset_lens: &[u8; DEFLATE_NUM_OFFSET_SYMS],
) -> u32 {
    let mut total_bits = 0u32;

    for item in seq {
        match *item {
            SequenceItem::Literal(lit) => {
                total_bits += litlen_lens[lit as usize] as u32;
            }
            SequenceItem::Match { length, offset } => {
                let slot = LENGTH_SLOT_MAP[length as usize] as usize;
                let extra_len = EXTRA_LENGTH_BITS[slot] as u32;
                total_bits += (litlen_lens[DEFLATE_FIRST_LEN_SYM + slot] as u32) + extra_len;

                let off_slot = CostModel::offset_slot(offset);
                let extra_off = EXTRA_OFFSET_BITS[off_slot] as u32;
                total_bits += (offset_lens[off_slot] as u32) + extra_off;
            }
        }
    }

    // End-of-block symbol
    total_bits += litlen_lens[DEFLATE_END_OF_BLOCK] as u32;
    total_bits
}
