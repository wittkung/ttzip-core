// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-compression Zopfli Dynamic Programming (DP) matching and Squeeze optimization microkernel.
//!
//! # Architecture & Pipeline
//!
//! 1. **Directed Acyclic Graph (DAG) Matching ([`shortest_path`])**:
//!    - Shannon self-information bit cost model with Laplace smoothing.
//!    - Multi-length candidate relaxation on topological DAG structure.
//!
//! 2. **Iterative Squeeze State Machine ([`squeeze`])**:
//!    - Expectation-Maximization iterative tree length re-estimation.
//!    - Simulated annealing stochastic perturbation to escape local minima.
//!
//! 3. **Entropy Variance Block Splitter ([`block_split`])**:
//!    - Dynamic block header penalty balancing (~250 bits).
//!    - 9-point equidistant search with local Golden-Section parabolic refinement.
//!
//! 4. **Multi-Format Container Encoder ([`encoder`])**:
//!    - RFC 1951 Raw DEFLATE, RFC 1950 Zlib, and RFC 1952 Gzip stream generation.

pub mod block_split;
pub mod encoder;
pub mod shortest_path;
pub mod squeeze;

#[cfg(test)]
mod tests;

pub use block_split::{
    estimate_entropy_cost, CumulativeHistogram, ZopfliBlockSplitter, DYNAMIC_HEADER_COST_BITS,
    MIN_BLOCK_SIZE, SPLIT_GAIN_THRESHOLD_BITS,
};
pub use encoder::{
    zopfli_compress, zopfli_compress_deflate, zopfli_compress_gzip, zopfli_compress_zlib,
    ZopfliEncoder, ZopfliFormat,
};
pub use shortest_path::{
    get_dist_slot, ZopfliCostModel, ZopfliHash, ZopfliMatchCache, ZopfliShortestPathMatcher,
    ZopfliToken, END_OF_BLOCK_SYM, EXTRA_LENGTH_BITS, EXTRA_OFFSET_BITS, FIRST_LEN_SYM,
    LENGTH_BASE, LENGTH_SLOT_MAP, NUM_DIST_SYMS, NUM_LITLEN_SYMS, OFFSET_BASE,
};
pub use squeeze::{
    calculate_dynamic_block_bit_cost, BlockStats, ZopfliOptions, ZopfliSqueeze,
};
