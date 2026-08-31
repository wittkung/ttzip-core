// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-precision Zopfli iterative Squeeze optimization state machine.
//!
//! # Algorithmic Architecture
//!
//! 1. **Expectation-Maximization (EM) Refinement**:
//!    - Evaluates minimum-cost paths on the input buffer DAG using the active cost model.
//!    - Tallies symbol frequencies from the optimal LZ77 parse.
//!    - Computes exact length-limited ($\le 15$ bits) canonical Huffman code lengths.
//!    - Evaluates exact bit cost including dynamic Huffman header overhead and payload.
//!
//! 2. **Simulated Annealing & Stochastic Perturbation**:
//!    - When iteration stagnation is detected ($\ge 2$ iterations without cost improvement),
//!      a temperature-decaying pseudo-random perturbation is injected into the symbol cost model:
//!      $$C'(s) = C(s) \times \left(1.0 + T_k \cdot \phi(s, k)\right)$$
//!    - Allows escaping sub-optimal local minima and discovering superior global match configurations.

use super::shortest_path::{
    get_dist_slot, ZopfliCostModel, ZopfliShortestPathMatcher, ZopfliToken, END_OF_BLOCK_SYM,
    EXTRA_LENGTH_BITS, EXTRA_OFFSET_BITS, FIRST_LEN_SYM, LENGTH_SLOT_MAP, NUM_DIST_SYMS,
    NUM_LITLEN_SYMS,
};
use crate::codecs::libdeflate::huffman::{
    compute_num_explicit_precode_lens, compute_precode_items, deflate_make_huffman_code,
    DEFLATE_EXTRA_PRECODE_BITS, DEFLATE_NUM_PRECODE_SYMS,
};

// MARK: - Configuration Options

/// Zopfli compression tuning parameters and iteration limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZopfliOptions {
    /// Number of Squeeze optimization passes (default: 15).
    pub num_iterations: usize,
    /// Maximum number of recursive block splits (default: 15).
    pub max_block_splits: usize,
    /// Maximum search chain depth in the matchfinder (default: 1024).
    pub max_chain: usize,
}

impl Default for ZopfliOptions {
    fn default() -> Self {
        Self {
            num_iterations: 15,
            max_block_splits: 15,
            max_chain: 1024,
        }
    }
}

impl ZopfliOptions {
    /// Fast preset (5 iterations, lower chain depth).
    pub fn fast() -> Self {
        Self {
            num_iterations: 5,
            max_block_splits: 5,
            max_chain: 256,
        }
    }

    /// Maximum compression preset (30 iterations, 4096 chain depth).
    pub fn maximum() -> Self {
        Self {
            num_iterations: 30,
            max_block_splits: 30,
            max_chain: 4096,
        }
    }
}

// MARK: - Block Statistics & Huffman Output

/// Optimized block statistics including tokens, tree lengths, codewords, and total bit cost.
#[derive(Debug, Clone)]
pub struct BlockStats {
    /// Optimal parsed LZ77 token sequence.
    pub tokens: Vec<ZopfliToken>,
    /// Codeword bit lengths for literal/length alphabet (0..287).
    pub litlen_lens: [u8; NUM_LITLEN_SYMS],
    /// Codeword bit lengths for distance alphabet (0..31).
    pub dist_lens: [u8; NUM_DIST_SYMS],
    /// Canonical Huffman codewords for literal/length alphabet.
    pub litlen_codes: [u32; NUM_LITLEN_SYMS],
    /// Canonical Huffman codewords for distance alphabet.
    pub dist_codes: [u32; NUM_DIST_SYMS],
    /// Number of used literal/length symbols (HLIT + 257).
    pub num_litlen_syms: usize,
    /// Number of used distance symbols (HDIST + 1).
    pub num_dist_syms: usize,
    /// Total block bit cost (header + precode + data + EOB).
    pub total_bits: f64,
}

// MARK: - Exact Bit Cost Evaluation

/// Evaluates exact bit cost for encoding dynamic Huffman trees and data stream.
pub fn calculate_dynamic_block_bit_cost(
    tokens: &[ZopfliToken],
    litlen_lens: &[u8; NUM_LITLEN_SYMS],
    dist_lens: &[u8; NUM_DIST_SYMS],
) -> (f64, usize, usize) {
    // 1. Determine HLIT and HDIST
    let mut num_litlen = NUM_LITLEN_SYMS;
    while num_litlen > 257 && litlen_lens[num_litlen - 1] == 0 {
        num_litlen -= 1;
    }

    let mut num_dist = NUM_DIST_SYMS;
    while num_dist > 1 && dist_lens[num_dist - 1] == 0 {
        num_dist -= 1;
    }

    // 2. Precode RLE items
    let mut combined_lens = Vec::with_capacity(num_litlen + num_dist);
    combined_lens.extend_from_slice(&litlen_lens[..num_litlen]);
    combined_lens.extend_from_slice(&dist_lens[..num_dist]);

    let mut precode_freqs = [0u32; DEFLATE_NUM_PRECODE_SYMS];
    let mut precode_items = Vec::with_capacity(combined_lens.len());
    compute_precode_items(&combined_lens, &mut precode_freqs, &mut precode_items);

    let mut precode_lens = [0u8; DEFLATE_NUM_PRECODE_SYMS];
    let mut precode_codes = [0u32; DEFLATE_NUM_PRECODE_SYMS];
    deflate_make_huffman_code(
        DEFLATE_NUM_PRECODE_SYMS,
        7,
        &precode_freqs,
        &mut precode_lens,
        &mut precode_codes,
    );

    let num_explicit_precode = compute_num_explicit_precode_lens(&precode_lens);

    // 3. Dynamic header cost in bits:
    // - 3 bits block header (BFINAL=1, BTYPE=10)
    // - 5 bits HLIT
    // - 5 bits HDIST
    // - 4 bits HCLEN
    // - num_explicit_precode * 3 bits
    let mut header_bits = 3.0 + 5.0 + 5.0 + 4.0 + (num_explicit_precode as f64 * 3.0);

    // - Precode items cost
    for &item in &precode_items {
        let sym = (item & 0x1F) as usize;
        let extra_bits = DEFLATE_EXTRA_PRECODE_BITS[sym] as f64;
        header_bits += (precode_lens[sym] as f64) + extra_bits;
    }

    // 4. Data payload cost in bits
    let mut data_bits = 0.0;
    for token in tokens {
        match *token {
            ZopfliToken::Literal(lit) => {
                data_bits += litlen_lens[lit as usize] as f64;
            }
            ZopfliToken::Match { length, distance } => {
                let lslot = LENGTH_SLOT_MAP[length as usize] as usize;
                let dslot = get_dist_slot(distance);

                data_bits += (litlen_lens[FIRST_LEN_SYM + lslot] as f64)
                    + (EXTRA_LENGTH_BITS[lslot] as f64)
                    + (dist_lens[dslot] as f64)
                    + (EXTRA_OFFSET_BITS[dslot] as f64);
            }
        }
    }

    // End of block symbol cost
    data_bits += litlen_lens[END_OF_BLOCK_SYM] as f64;

    (header_bits + data_bits, num_litlen, num_dist)
}

// MARK: - Simulated Annealing Perturbation

/// Deterministic pseudo-random perturbation generator for simulated annealing.
#[inline]
fn pseudo_annealing_noise(sym: usize, iteration: usize) -> f64 {
    // 64-bit splitmix-like deterministic hash
    let mut x = ((sym as u64) << 32) ^ (iteration as u64).wrapping_mul(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^= x >> 31;
    let fraction = ((x & 0xFFFF) as f64) / 65535.0; // 0.0 .. 1.0
    (fraction * 2.0) - 1.0 // -1.0 .. 1.0
}

// MARK: - Squeeze State Machine

/// Zopfli iterative Squeeze optimization engine.
pub struct ZopfliSqueeze {
    matcher: ZopfliShortestPathMatcher,
}

impl Default for ZopfliSqueeze {
    fn default() -> Self {
        Self::new()
    }
}

impl ZopfliSqueeze {
    /// Creates a new Squeeze optimization engine.
    pub fn new() -> Self {
        Self {
            matcher: ZopfliShortestPathMatcher::new(),
        }
    }

    /// Performs Squeeze iterative dynamic programming on input range `data[from..to]`.
    pub fn squeeze(
        &mut self,
        data: &[u8],
        from: usize,
        to: usize,
        options: &ZopfliOptions,
    ) -> BlockStats {
        let block_len = to.saturating_sub(from);
        if block_len == 0 {
            return BlockStats {
                tokens: Vec::new(),
                litlen_lens: [0; NUM_LITLEN_SYMS],
                dist_lens: [0; NUM_DIST_SYMS],
                litlen_codes: [0; NUM_LITLEN_SYMS],
                dist_codes: [0; NUM_DIST_SYMS],
                num_litlen_syms: 257,
                num_dist_syms: 1,
                total_bits: 0.0,
            };
        }

        self.matcher.warmup_hash(data, from);
        self.matcher.reset_cache(block_len);

        // 1. Initial cost model from byte frequencies (Shannon entropy)
        let mut lit_freqs = [0u32; NUM_LITLEN_SYMS];
        let mut dist_freqs = [0u32; NUM_DIST_SYMS];

        for &b in &data[from..to] {
            lit_freqs[b as usize] += 1;
        }
        lit_freqs[END_OF_BLOCK_SYM] = 1;
        dist_freqs.fill(1);

        let mut cost_model = ZopfliCostModel::from_shannon_frequencies(&lit_freqs, &dist_freqs);

        let mut best_stats: Option<BlockStats> = None;
        let mut stagnation_count = 0usize;

        // 2. Squeeze iteration loop
        for iter in 0..options.num_iterations {
            let tokens = self.matcher.find_shortest_path(
                data,
                from,
                to,
                &cost_model,
                options.max_chain,
            );

            // Tally symbol frequencies from optimal parse
            let mut cur_litlen_freqs = [0u32; NUM_LITLEN_SYMS];
            let mut cur_dist_freqs = [0u32; NUM_DIST_SYMS];

            for token in &tokens {
                match *token {
                    ZopfliToken::Literal(lit) => {
                        cur_litlen_freqs[lit as usize] += 1;
                    }
                    ZopfliToken::Match { length, distance } => {
                        let lslot = LENGTH_SLOT_MAP[length as usize] as usize;
                        let dslot = get_dist_slot(distance);
                        cur_litlen_freqs[FIRST_LEN_SYM + lslot] += 1;
                        cur_dist_freqs[dslot] += 1;
                    }
                }
            }
            cur_litlen_freqs[END_OF_BLOCK_SYM] += 1;

            // Guarantee at least one distance symbol frequency is non-zero
            let mut has_dist = false;
            for &f in cur_dist_freqs.iter() {
                if f > 0 {
                    has_dist = true;
                    break;
                }
            }
            if !has_dist {
                cur_dist_freqs[0] = 1;
            }

            // Generate canonical Huffman trees
            let mut litlen_lens = [0u8; NUM_LITLEN_SYMS];
            let mut litlen_codes = [0u32; NUM_LITLEN_SYMS];
            deflate_make_huffman_code(
                NUM_LITLEN_SYMS,
                15,
                &cur_litlen_freqs,
                &mut litlen_lens,
                &mut litlen_codes,
            );

            let mut dist_lens = [0u8; NUM_DIST_SYMS];
            let mut dist_codes = [0u32; NUM_DIST_SYMS];
            deflate_make_huffman_code(
                NUM_DIST_SYMS,
                15,
                &cur_dist_freqs,
                &mut dist_lens,
                &mut dist_codes,
            );

            // Ensure RFC 1951 valid distance tree
            let mut max_dlen = 0;
            for &l in dist_lens.iter() {
                if l > max_dlen {
                    max_dlen = l;
                }
            }
            if max_dlen == 0 {
                dist_lens[0] = 1;
            }

            // Evaluate exact bit cost
            let (total_bits, num_litlen, num_dist) =
                calculate_dynamic_block_bit_cost(&tokens, &litlen_lens, &dist_lens);

            let is_improvement = match &best_stats {
                Some(best) => total_bits < best.total_bits,
                None => true,
            };

            if is_improvement {
                best_stats = Some(BlockStats {
                    tokens,
                    litlen_lens,
                    dist_lens,
                    litlen_codes,
                    dist_codes,
                    num_litlen_syms: num_litlen,
                    num_dist_syms: num_dist,
                    total_bits,
                });
                stagnation_count = 0;
            } else {
                stagnation_count += 1;
            }

            // 3. Update cost model for next iteration with optional simulated annealing
            cost_model = ZopfliCostModel::from_huffman_lengths(&litlen_lens, &dist_lens);

            if stagnation_count >= 2 {
                // Annealing temperature decay: T_k = 0.15 / (1.0 + 0.1 * k)
                let temperature = 0.15 / (1.0 + 0.1 * (iter as f64));
                for sym in 0..NUM_LITLEN_SYMS {
                    let noise = pseudo_annealing_noise(sym, iter);
                    cost_model.litlen_costs[sym] *= 1.0 + temperature * noise;
                }
                for sym in 0..NUM_DIST_SYMS {
                    let noise = pseudo_annealing_noise(sym + 300, iter);
                    cost_model.dist_costs[sym] *= 1.0 + temperature * noise;
                }
            }
        }

        best_stats.expect("Squeeze iterations produced at least one block stats result")
    }
}
