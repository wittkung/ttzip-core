// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 8-Dimensional Hyperparameter Space Manhattan Hill-Climbing Searcher (`ttzip-paramgrill`).
//!
//! Explores and optimizes the multi-dimensional Zstandard compression tuning space:
//! - Dimension 1: `window_log` ($10 \le W \le 31$)
//! - Dimension 2: `chain_log` ($6 \le C \le 30$)
//! - Dimension 3: `hash_log` ($6 \le H \le 30$)
//! - Dimension 4: `search_log` ($1 \le S \le 26$)
//! - Dimension 5: `min_match` ($3 \le M \le 7$)
//! - Dimension 6: `target_length` ($0 \le T \le 999$)
//! - Dimension 7: `strategy` ($1 \le \text{Strat} \le 9$, `ZSTD_fast` .. `ZSTD_btultra2`)
//! - Dimension 8: `chunk_size` ($64\text{KB} \le \text{Chunk} \le 64\text{MB}$)
//!
//! Features:
//! - **Manhattan Distance 1 Exploration**: Perturbs individual parameters along coordinate axes.
//! - **XXH64 Memoization Deduplication Cache**: 64-bit fast hashing to avoid redundant benchmark runs.
//! - **Multi-Objective 2D Pareto Frontier**: Computes the non-dominated Pareto front (Speed MB/s vs
//!   Compression Ratio) under user-specified throughput and space savings constraints.

use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::codecs::zstd::types::{ZstdCParameter, ZSTD_compressBound};
use crate::codecs::zstd::{ZstdCCtx, ZstdDCtx};
use crate::types::TTZipStatus;

// MARK: - 1. Discrete Chunk Size Lookup Table

pub const VALID_CHUNK_SIZES: [usize; 11] = [
    64 * 1024,        // 64 KB
    128 * 1024,       // 128 KB
    256 * 1024,       // 256 KB
    512 * 1024,       // 512 KB
    1024 * 1024,      // 1 MB
    2 * 1024 * 1024,  // 2 MB
    4 * 1024 * 1024,  // 4 MB
    8 * 1024 * 1024,  // 8 MB
    16 * 1024 * 1024, // 16 MB
    32 * 1024 * 1024, // 32 MB
    64 * 1024 * 1024, // 64 MB
];

// MARK: - 2. 8-Dimensional Hyperparameter Vector

/// Complete 8-dimensional compression tuning vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HyperParamVector {
    /// Log2 of sliding dictionary window size (10..=31).
    pub window_log: u32,
    /// Log2 of hash table chain length (6..=30).
    pub chain_log: u32,
    /// Log2 of primary hash table size (6..=30).
    pub hash_log: u32,
    /// Log2 of search length per match candidate (1..=26).
    pub search_log: u32,
    /// Minimum match length required to accept a match (3..=7).
    pub min_match: u32,
    /// Target length for match finding (0..=999).
    pub target_length: u32,
    /// Compression search strategy (1 = fast .. 9 = btultra2).
    pub strategy: u32,
    /// Streaming input chunk block size in bytes (64KB..=64MB).
    pub chunk_size: usize,
}

impl Default for HyperParamVector {
    fn default() -> Self {
        Self {
            window_log: 19,
            chain_log: 16,
            hash_log: 17,
            search_log: 1,
            min_match: 4,
            target_length: 0,
            strategy: 1, // ZSTD_fast
            chunk_size: 1024 * 1024,
        }
    }
}

impl HyperParamVector {
    /// Creates a baseline preset corresponding to standard compression levels (1, 3, 7, 9, 15, 19).
    pub fn preset(level: u32) -> Self {
        match level {
            1 => Self {
                window_log: 19,
                chain_log: 14,
                hash_log: 15,
                search_log: 1,
                min_match: 5,
                target_length: 0,
                strategy: 1,
                chunk_size: 1024 * 1024,
            },
            3 => Self {
                window_log: 20,
                chain_log: 16,
                hash_log: 17,
                search_log: 2,
                min_match: 4,
                target_length: 0,
                strategy: 2, // ZSTD_dfast
                chunk_size: 1024 * 1024,
            },
            7 => Self {
                window_log: 21,
                chain_log: 18,
                hash_log: 18,
                search_log: 4,
                min_match: 4,
                target_length: 8,
                strategy: 4, // ZSTD_lazy
                chunk_size: 2 * 1024 * 1024,
            },
            9 => Self {
                window_log: 22,
                chain_log: 20,
                hash_log: 19,
                search_log: 6,
                min_match: 4,
                target_length: 16,
                strategy: 6, // ZSTD_btlazy2
                chunk_size: 4 * 1024 * 1024,
            },
            15 => Self {
                window_log: 23,
                chain_log: 22,
                hash_log: 20,
                search_log: 8,
                min_match: 3,
                target_length: 32,
                strategy: 7, // ZSTD_btopt
                chunk_size: 8 * 1024 * 1024,
            },
            _ => Self {
                window_log: 24,
                chain_log: 24,
                hash_log: 22,
                search_log: 12,
                min_match: 3,
                target_length: 64,
                strategy: 9, // ZSTD_btultra2
                chunk_size: 16 * 1024 * 1024,
            },
        }
    }

    /// Clamps all vector coordinates to their strict hardware and mathematical boundaries.
    pub fn clamp_to_bounds(&mut self) {
        self.window_log = self.window_log.clamp(10, 31);
        self.chain_log = self.chain_log.clamp(6, 30);
        self.hash_log = self.hash_log.clamp(6, 30);
        self.search_log = self.search_log.clamp(1, 26);
        self.min_match = self.min_match.clamp(3, 7);
        self.target_length = self.target_length.clamp(0, 999);
        self.strategy = self.strategy.clamp(1, 9);

        // Find nearest valid chunk size
        let mut best_chunk = VALID_CHUNK_SIZES[0];
        let mut min_diff = usize::MAX;
        for &sz in &VALID_CHUNK_SIZES {
            let diff = self.chunk_size.abs_diff(sz);
            if diff < min_diff {
                min_diff = diff;
                best_chunk = sz;
            }
        }
        self.chunk_size = best_chunk;
    }

    /// Generates all immediate Manhattan distance 1 neighbors in the 8-dimensional hypergrid.
    pub fn manhattan_neighbors(&self) -> Vec<HyperParamVector> {
        let mut neighbors = Vec::with_capacity(16);

        // 1. window_log (+1, -1)
        if self.window_log < 31 {
            let mut n = *self;
            n.window_log += 1;
            neighbors.push(n);
        }
        if self.window_log > 10 {
            let mut n = *self;
            n.window_log -= 1;
            neighbors.push(n);
        }

        // 2. chain_log (+1, -1)
        if self.chain_log < 30 {
            let mut n = *self;
            n.chain_log += 1;
            neighbors.push(n);
        }
        if self.chain_log > 6 {
            let mut n = *self;
            n.chain_log -= 1;
            neighbors.push(n);
        }

        // 3. hash_log (+1, -1)
        if self.hash_log < 30 {
            let mut n = *self;
            n.hash_log += 1;
            neighbors.push(n);
        }
        if self.hash_log > 6 {
            let mut n = *self;
            n.hash_log -= 1;
            neighbors.push(n);
        }

        // 4. search_log (+1, -1)
        if self.search_log < 26 {
            let mut n = *self;
            n.search_log += 1;
            neighbors.push(n);
        }
        if self.search_log > 1 {
            let mut n = *self;
            n.search_log -= 1;
            neighbors.push(n);
        }

        // 5. min_match (+1, -1)
        if self.min_match < 7 {
            let mut n = *self;
            n.min_match += 1;
            neighbors.push(n);
        }
        if self.min_match > 3 {
            let mut n = *self;
            n.min_match -= 1;
            neighbors.push(n);
        }

        // 6. target_length (+8, -8 or boundary)
        if self.target_length + 8 <= 999 {
            let mut n = *self;
            n.target_length += 8;
            neighbors.push(n);
        }
        if self.target_length >= 8 {
            let mut n = *self;
            n.target_length -= 8;
            neighbors.push(n);
        }

        // 7. strategy (+1, -1)
        if self.strategy < 9 {
            let mut n = *self;
            n.strategy += 1;
            neighbors.push(n);
        }
        if self.strategy > 1 {
            let mut n = *self;
            n.strategy -= 1;
            neighbors.push(n);
        }

        // 8. chunk_size (step up, step down in VALID_CHUNK_SIZES)
        let cur_idx = VALID_CHUNK_SIZES
            .iter()
            .position(|&s| s == self.chunk_size)
            .unwrap_or(4);

        if cur_idx + 1 < VALID_CHUNK_SIZES.len() {
            let mut n = *self;
            n.chunk_size = VALID_CHUNK_SIZES[cur_idx + 1];
            neighbors.push(n);
        }
        if cur_idx > 0 {
            let mut n = *self;
            n.chunk_size = VALID_CHUNK_SIZES[cur_idx - 1];
            neighbors.push(n);
        }

        neighbors
    }

    /// Computes a deterministic 64-bit XXH64 hash of this 8D hyperparameter vector.
    pub fn compute_xxh64(&self) -> u64 {
        let mut raw = [0u8; 36];
        raw[0..4].copy_from_slice(&self.window_log.to_le_bytes());
        raw[4..8].copy_from_slice(&self.chain_log.to_le_bytes());
        raw[8..12].copy_from_slice(&self.hash_log.to_le_bytes());
        raw[12..16].copy_from_slice(&self.search_log.to_le_bytes());
        raw[16..20].copy_from_slice(&self.min_match.to_le_bytes());
        raw[20..24].copy_from_slice(&self.target_length.to_le_bytes());
        raw[24..28].copy_from_slice(&self.strategy.to_le_bytes());
        raw[28..36].copy_from_slice(&(self.chunk_size as u64).to_le_bytes());

        // Standard XXH64 calculation over 36 bytes
        compute_xxh64_bytes(&raw)
    }
}

// MARK: - 3. Evaluation Result & Constraints

/// Benchmark evaluation result for an individual hyperparameter vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamEvaluationResult {
    pub params: HyperParamVector,
    pub param_hash: u64,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub compression_ratio: f64,
    pub space_savings_pct: f64,
    pub compress_speed_mb_s: f64,
    pub decompress_speed_mb_s: f64,
    pub fitness_score: f64,
    pub is_pareto_optimal: bool,
}

/// Search constraints and termination criteria for the hill climbing algorithm.
#[derive(Debug, Clone)]
pub struct ParamGrillSearchConstraints {
    pub min_speed_mb_s: Option<f64>,
    pub min_compression_ratio: Option<f64>,
    pub max_evaluations: usize,
    pub max_stagnant_steps: usize,
    pub restarts: usize,
    pub alpha_speed_weight: f64,
    pub beta_ratio_weight: f64,
}

impl Default for ParamGrillSearchConstraints {
    fn default() -> Self {
        Self {
            min_speed_mb_s: None,
            min_compression_ratio: None,
            max_evaluations: 128,
            max_stagnant_steps: 12,
            restarts: 2,
            alpha_speed_weight: 0.5,
            beta_ratio_weight: 0.5,
        }
    }
}

/// Comprehensive search report output by `ParamGrillSearchEngine`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamGrillReport {
    pub total_evaluations: usize,
    pub cache_hits: usize,
    pub pareto_optimal_points: Vec<ParamEvaluationResult>,
    pub best_solution: Option<ParamEvaluationResult>,
    pub recommended_levels: HashMap<u32, HyperParamVector>,
    pub search_duration_millis: u64,
}

// MARK: - 4. ParamGrill Hill Climbing Engine

/// 8-Dimensional Hyperparameter Optimization Engine with Manhattan exploration and XXH64 cache.
pub struct ParamGrillSearchEngine {
    memo_cache: HashMap<u64, ParamEvaluationResult>,
    history: Vec<ParamEvaluationResult>,
    constraints: ParamGrillSearchConstraints,
    cache_hits: usize,
}

impl ParamGrillSearchEngine {
    /// Creates a new search engine instance with given constraints.
    pub fn new(constraints: ParamGrillSearchConstraints) -> Self {
        Self {
            memo_cache: HashMap::new(),
            history: Vec::new(),
            constraints,
            cache_hits: 0,
        }
    }

    /// Number of cache hits during search.
    #[inline]
    pub fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    /// Evaluates a single hyperparameter vector against target test corpus.
    pub fn evaluate(
        &mut self,
        params: &HyperParamVector,
        corpus: &[u8],
    ) -> Result<ParamEvaluationResult, TTZipStatus> {
        let hash = params.compute_xxh64();

        if let Some(cached) = self.memo_cache.get(&hash) {
            self.cache_hits += 1;
            return Ok(cached.clone());
        }

        if corpus.is_empty() {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let mut cctx = ZstdCCtx::new()?;
        cctx.set_parameter(ZstdCParameter::WindowLog, params.window_log as i32)?;
        cctx.set_parameter(ZstdCParameter::ChainLog, params.chain_log as i32)?;
        cctx.set_parameter(ZstdCParameter::HashLog, params.hash_log as i32)?;
        cctx.set_parameter(ZstdCParameter::SearchLog, params.search_log as i32)?;
        cctx.set_parameter(ZstdCParameter::MinMatch, params.min_match as i32)?;
        cctx.set_parameter(ZstdCParameter::TargetLength, params.target_length as i32)?;
        cctx.set_parameter(ZstdCParameter::Strategy, params.strategy as i32)?;

        let max_dst_size = unsafe { ZSTD_compressBound(corpus.len()) };
        let mut comp_buf = vec![0u8; max_dst_size];

        // 1. Measure compression speed
        let t_comp_start = Instant::now();
        let compressed_size = cctx.compress(corpus, &mut comp_buf, 0)?;
        let comp_duration = t_comp_start.elapsed();
        comp_buf.truncate(compressed_size);

        let comp_secs = comp_duration.as_secs_f64().max(1e-6);
        let comp_speed_mb_s = (corpus.len() as f64) / (1024.0 * 1024.0 * comp_secs);

        // 2. Measure decompression speed
        let mut dctx = ZstdDCtx::new()?;
        let mut decomp_buf = vec![0u8; corpus.len()];
        let t_decomp_start = Instant::now();
        let decomp_res = dctx.decompress(&comp_buf, &mut decomp_buf);
        let decomp_duration = t_decomp_start.elapsed();

        let decomp_secs = decomp_duration.as_secs_f64().max(1e-6);
        let decomp_speed_mb_s = (corpus.len() as f64) / (1024.0 * 1024.0 * decomp_secs);

        if decomp_res.is_err() || decomp_buf != corpus {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        let uncompressed_size = corpus.len();
        let compression_ratio = if compressed_size > 0 {
            (uncompressed_size as f64) / (compressed_size as f64)
        } else {
            1.0
        };
        let space_savings_pct = (1.0 - (compressed_size as f64 / uncompressed_size as f64)) * 100.0;

        // Multi-objective normalized fitness score
        let fitness_score = (self.constraints.alpha_speed_weight * (comp_speed_mb_s / 500.0))
            + (self.constraints.beta_ratio_weight * compression_ratio);

        let result = ParamEvaluationResult {
            params: *params,
            param_hash: hash,
            compressed_size,
            uncompressed_size,
            compression_ratio,
            space_savings_pct,
            compress_speed_mb_s: comp_speed_mb_s,
            decompress_speed_mb_s: decomp_speed_mb_s,
            fitness_score,
            is_pareto_optimal: false,
        };

        self.memo_cache.insert(hash, result.clone());
        self.history.push(result.clone());

        Ok(result)
    }

    /// Executes hill-climbing search across the 8-dimensional space starting from initial seeds.
    pub fn search(
        &mut self,
        initial_seeds: &[HyperParamVector],
        corpus: &[u8],
    ) -> Result<ParamGrillReport, TTZipStatus> {
        let t_start = Instant::now();
        let mut seeds = if initial_seeds.is_empty() {
            vec![
                HyperParamVector::preset(1),
                HyperParamVector::preset(3),
                HyperParamVector::preset(9),
                HyperParamVector::preset(19),
            ]
        } else {
            initial_seeds.to_vec()
        };

        let mut eval_count = 0;

        for seed in &mut seeds {
            seed.clamp_to_bounds();
            let mut current = self.evaluate(seed, corpus)?;
            eval_count += 1;
            let mut stagnant = 0;

            while eval_count < self.constraints.max_evaluations
                && stagnant < self.constraints.max_stagnant_steps
            {
                let neighbors = current.params.manhattan_neighbors();
                let mut best_neighbor: Option<ParamEvaluationResult> = None;

                for n in neighbors {
                    if eval_count >= self.constraints.max_evaluations {
                        break;
                    }
                    let res = self.evaluate(&n, corpus)?;
                    eval_count += 1;

                    // Check constraints
                    if let Some(min_speed) = self.constraints.min_speed_mb_s {
                        if res.compress_speed_mb_s < min_speed {
                            continue;
                        }
                    }
                    if let Some(min_ratio) = self.constraints.min_compression_ratio {
                        if res.compression_ratio < min_ratio {
                            continue;
                        }
                    }

                    if let Some(ref best) = best_neighbor {
                        if res.fitness_score > best.fitness_score {
                            best_neighbor = Some(res);
                        }
                    } else if res.fitness_score > current.fitness_score {
                        best_neighbor = Some(res);
                    }
                }

                if let Some(next) = best_neighbor {
                    if next.fitness_score > current.fitness_score + 1e-4 {
                        current = next;
                        stagnant = 0;
                    } else {
                        stagnant += 1;
                    }
                } else {
                    stagnant += 1;
                }
            }
        }

        // 3. Compute 2D Pareto non-dominated frontier
        let pareto_points = self.compute_pareto_frontier();

        // 4. Derive recommended levels
        let recommended_levels = self.derive_recommended_levels(&pareto_points);

        let best_solution = pareto_points
            .iter()
            .max_by(|a, b| a.fitness_score.partial_cmp(&b.fitness_score).unwrap())
            .cloned();

        let elapsed = t_start.elapsed().as_millis() as u64;

        Ok(ParamGrillReport {
            total_evaluations: self.history.len(),
            cache_hits: self.cache_hits,
            pareto_optimal_points: pareto_points,
            best_solution,
            recommended_levels,
            search_duration_millis: elapsed,
        })
    }

    fn compute_pareto_frontier(&mut self) -> Vec<ParamEvaluationResult> {
        let mut results = self.history.clone();
        if results.is_empty() {
            return Vec::new();
        }

        // Sort by speed descending, ratio descending
        results.sort_by(|a, b| {
            b.compress_speed_mb_s
                .partial_cmp(&a.compress_speed_mb_s)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut pareto_front: Vec<ParamEvaluationResult> = Vec::new();
        let mut max_ratio = -1.0;

        for item in results {
            if item.compression_ratio > max_ratio + 1e-6 {
                let mut p = item.clone();
                p.is_pareto_optimal = true;
                max_ratio = item.compression_ratio;
                pareto_front.push(p);
            }
        }

        pareto_front
    }

    fn derive_recommended_levels(
        &self,
        pareto_front: &[ParamEvaluationResult],
    ) -> HashMap<u32, HyperParamVector> {
        let mut rec = HashMap::new();
        if pareto_front.is_empty() {
            rec.insert(1, HyperParamVector::preset(1));
            rec.insert(3, HyperParamVector::preset(3));
            rec.insert(9, HyperParamVector::preset(9));
            rec.insert(19, HyperParamVector::preset(19));
            return rec;
        }

        // Fastest point on Pareto front -> Level 1
        if let Some(fastest) = pareto_front.first() {
            rec.insert(1, fastest.params);
        }

        // Highest ratio point on Pareto front -> Level 19
        if let Some(highest_ratio) = pareto_front.last() {
            rec.insert(19, highest_ratio.params);
        }

        // Median points -> Level 3, Level 9
        if pareto_front.len() >= 3 {
            let idx3 = pareto_front.len() / 3;
            let idx9 = (pareto_front.len() * 2) / 3;
            rec.insert(3, pareto_front[idx3].params);
            rec.insert(9, pareto_front[idx9].params);
        } else {
            rec.insert(3, HyperParamVector::preset(3));
            rec.insert(9, HyperParamVector::preset(9));
        }

        rec
    }
}

// MARK: - 5. XXH64 Hash Function

fn compute_xxh64_bytes(data: &[u8]) -> u64 {
    const PRIME64_1: u64 = 0x9E3779B185EBCA87;
    const PRIME64_2: u64 = 0xC2B2AE3D27D4EB4F;
    const PRIME64_3: u64 = 0x165667B19E3779F9;
    const PRIME64_4: u64 = 0x85EBCA77C2B2AE63;
    const PRIME64_5: u64 = 0x27D4EB2F165667C5;

    let len = data.len();
    let mut h64: u64;

    if len >= 32 {
        let mut v1 = PRIME64_1.wrapping_add(PRIME64_2);
        let mut v2 = PRIME64_2;
        let mut v3 = 0u64;
        let mut v4 = 0u64.wrapping_sub(PRIME64_1);

        let mut chunks = data.chunks_exact(32);
        for chunk in chunks.by_ref() {
            let mut sub = chunk.chunks_exact(8);
            let k1 = u64::from_le_bytes(sub.next().unwrap().try_into().unwrap());
            let k2 = u64::from_le_bytes(sub.next().unwrap().try_into().unwrap());
            let k3 = u64::from_le_bytes(sub.next().unwrap().try_into().unwrap());
            let k4 = u64::from_le_bytes(sub.next().unwrap().try_into().unwrap());

            v1 = v1.wrapping_add(k1.wrapping_mul(PRIME64_2)).rotate_left(31).wrapping_mul(PRIME64_1);
            v2 = v2.wrapping_add(k2.wrapping_mul(PRIME64_2)).rotate_left(31).wrapping_mul(PRIME64_1);
            v3 = v3.wrapping_add(k3.wrapping_mul(PRIME64_2)).rotate_left(31).wrapping_mul(PRIME64_1);
            v4 = v4.wrapping_add(k4.wrapping_mul(PRIME64_2)).rotate_left(31).wrapping_mul(PRIME64_1);
        }

        h64 = v1.rotate_left(1).wrapping_add(v2.rotate_left(7)).wrapping_add(v3.rotate_left(12)).wrapping_add(v4.rotate_left(18));

        v1 = v1.wrapping_mul(PRIME64_2).rotate_left(31).wrapping_mul(PRIME64_1);
        h64 = (h64 ^ v1).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        v2 = v2.wrapping_mul(PRIME64_2).rotate_left(31).wrapping_mul(PRIME64_1);
        h64 = (h64 ^ v2).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        v3 = v3.wrapping_mul(PRIME64_2).rotate_left(31).wrapping_mul(PRIME64_1);
        h64 = (h64 ^ v3).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);

        v4 = v4.wrapping_mul(PRIME64_2).rotate_left(31).wrapping_mul(PRIME64_1);
        h64 = (h64 ^ v4).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);
    } else {
        h64 = PRIME64_5;
    }

    h64 = h64.wrapping_add(len as u64);

    let remainder = &data[(len / 32) * 32..];
    let mut rem_chunks = remainder.chunks_exact(8);
    for chunk in rem_chunks.by_ref() {
        let k = u64::from_le_bytes(chunk.try_into().unwrap());
        h64 ^= k.wrapping_mul(PRIME64_2).rotate_left(31).wrapping_mul(PRIME64_1);
        h64 = h64.rotate_left(27).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);
    }

    let rem_tail = rem_chunks.remainder();
    let mut sub4 = rem_tail.chunks_exact(4);
    for chunk in sub4.by_ref() {
        let k = u32::from_le_bytes(chunk.try_into().unwrap()) as u64;
        h64 ^= k.wrapping_mul(PRIME64_1);
        h64 = h64.rotate_left(23).wrapping_mul(PRIME64_2).wrapping_add(PRIME64_3);
    }

    for &b in sub4.remainder() {
        h64 ^= (b as u64).wrapping_mul(PRIME64_5);
        h64 = h64.rotate_left(11).wrapping_mul(PRIME64_1);
    }

    h64 ^= h64 >> 33;
    h64 = h64.wrapping_mul(PRIME64_2);
    h64 ^= h64 >> 29;
    h64 = h64.wrapping_mul(PRIME64_3);
    h64 ^= h64 >> 32;

    h64
}
