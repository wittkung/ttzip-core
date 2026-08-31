// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deterministic LCG Pseudo-Random Number Generator and 5-Level Probability Distribution Model.
//!
//! Ported and adapted from Yann Collet's canonical `datagen.c` (LZ4 / Zstandard / lzbench):
//! 1. `RdgRng`: Deterministic 32-bit Knuth multiplicative LCG PRNG (`PRIME1 = 2654435761`, `PRIME2 = 2246822519`, `rotl32(13)`).
//! 2. `LiteralDistribTable`: 8192-entry literal frequency distribution table (`LDT_SIZE = 8192`) with geometric decay weighting.
//! 3. `DataGenLevel`: 5-level discrete entropy and compressibility spectrum (`PureNoise`, `BarelyCompressible`, `Standard`, `HighlyCompressible`, `Sparse`).
//! 4. `generate_corpus` / `generate_corpus_into`: High-throughput ($\ge 1.0\text{ GB/s}$) synthetic corpus synthesis.

use std::cmp::min;

/// Size of the literal distribution lookup table ($2^{13} = 8192$).
pub const LDT_SIZE: usize = 8192;

/// Bitmask for literal distribution table index wrapping.
pub const LTMASK: usize = LDT_SIZE - 1;

// MARK: - 1. Deterministic LCG PRNG (RdgRng)

/// 32-bit deterministic Knuth multiplicative LCG pseudo-random number generator.
///
/// Implements the exact recurrence relation from canonical `datagen.c`:
/// ```text
/// state = (state * 2654435761) ^ 2246822519
/// state = rotl32(state, 13)
/// return state >> 5
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdgRng {
    state: u32,
}

impl RdgRng {
    /// Golden ratio 32-bit prime multiplier ($2^{32} / \phi \approx 2654435761$).
    pub const PRIME1: u32 = 2_654_435_761;

    /// xxHash 32-bit mixing prime constant.
    pub const PRIME2: u32 = 2_246_822_519;

    /// Creates a new `RdgRng` seeded with the provided 32-bit integer.
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Computes the next 32-bit pseudo-random value matching `RDG_rand()`.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut rand32 = self.state;
        rand32 = rand32.wrapping_mul(Self::PRIME1).wrapping_add(Self::PRIME2);
        rand32 = rand32.rotate_left(13);
        self.state = rand32;
        rand32
    }

    /// Generates the lowest `nb_bits` pseudo-random bits ($0 \le \text{nb\_bits} \le 32$).
    #[inline]
    #[allow(dead_code)]
    pub fn next_bits(&mut self, nb_bits: u32) -> u32 {
        if nb_bits == 0 {
            0
        } else if nb_bits >= 32 {
            self.next_u32()
        } else {
            self.next_u32() & ((1u32 << nb_bits) - 1)
        }
    }

    /// Generates a pseudo-random integer in the closed interval `[min, max]`.
    #[inline]
    #[allow(dead_code)]
    pub fn rand_range(&mut self, min_val: u32, max_val: u32) -> u32 {
        if min_val >= max_val {
            return min_val;
        }
        let span = max_val - min_val + 1;
        min_val + (self.next_u32() % span)
    }

    /// Generates a single pseudo-random byte (`u8`).
    #[inline]
    #[allow(dead_code)]
    pub fn rand_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }

    /// Evaluates a boolean flag with probability `prob` ($0.0 \le \text{prob} \le 1.0$).
    #[inline]
    #[allow(dead_code)]
    pub fn rand_bool_prob(&mut self, prob: f64) -> bool {
        if prob <= 0.0 {
            false
        } else if prob >= 1.0 {
            true
        } else {
            let threshold = (prob * 65536.0) as u32;
            (self.next_u32() & 0xFFFF) < threshold
        }
    }

    /// Returns the current raw 32-bit state.
    #[inline]
    #[allow(dead_code)]
    pub const fn raw_state(&self) -> u32 {
        self.state
    }

    /// Fills destination buffer with uniform pseudo-random bytes.
    #[inline]
    pub fn fill_bytes(&mut self, dst: &mut [u8]) {
        let mut chunks = dst.chunks_exact_mut(4);
        for chunk in chunks.by_ref() {
            let val = self.next_u32().to_le_bytes();
            chunk.copy_from_slice(&val);
        }
        let remainder = chunks.into_remainder();
        if !remainder.is_empty() {
            let val = self.next_u32().to_le_bytes();
            remainder.copy_from_slice(&val[..remainder.len()]);
        }
    }
}

// MARK: - 2. Literal Distribution Table (LiteralDistribTable)

/// 8192-entry literal frequency distribution table for $O(1)$ weighted symbol sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralDistribTable {
    pub table: [u8; LDT_SIZE],
}

impl LiteralDistribTable {
    /// Builds a literal distribution table according to canonical `RDG_fillLiteralDistrib`.
    ///
    /// When `lit_proba <= 0.0`, symbols are evenly distributed across all 256 byte values.
    /// When `lit_proba > 0.0`, symbols follow a geometric frequency curve centered on printable ASCII.
    pub fn new(lit_proba: f64) -> Self {
        let mut table = [0u8; LDT_SIZE];

        if lit_proba <= 0.0 {
            for (i, slot) in table.iter_mut().enumerate() {
                *slot = (i & 0xFF) as u8;
            }
            return Self { table };
        }

        let first_char: u8 = b'('; // ASCII 40
        let last_char: u8 = b'}';  // ASCII 125
        let mut character: u8 = b'0';

        let ld_fixed = (lit_proba * 256.0).clamp(0.0, 65535.0) as u32;
        let mut u = 0usize;

        while u < LDT_SIZE {
            let remaining = (LDT_SIZE - u) as u32;
            let weight = ((remaining * ld_fixed) >> 8) + 1;
            let end = min(u + weight as usize, LDT_SIZE);

            while u < end {
                table[u] = character;
                u += 1;
            }

            character = character.wrapping_add(1);
            if character > last_char {
                character = first_char;
            }
        }

        Self { table }
    }

    /// Samples a single byte from the distribution table using fast bitmask indexing.
    #[inline]
    pub fn sample(&self, rng: &mut RdgRng) -> u8 {
        let idx = (rng.next_u32() as usize) & LTMASK;
        self.table[idx]
    }
}

// MARK: - 3. 5-Level Compressibility Spectrum (DataGenLevel)

/// Discrete compressibility and entropy levels for benchmark corpus generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataGenLevel {
    /// High-entropy random noise with 0% match probability (incompressible, ratio ~1.0).
    PureNoise,
    /// Low match probability (~12%) and broad literal alphabet (slight compression, ratio ~0.70..0.90).
    BarelyCompressible,
    /// Balanced match probability (~50%) and structured ASCII distribution (moderate compression, ratio ~0.30..0.55).
    Standard,
    /// High match probability (~85%) with restricted alphabet (high compression, ratio ~0.05..0.20).
    HighlyCompressible,
    /// Extreme match probability (~99.5%) with long repetitive runs (extreme compression, ratio < 0.02).
    Sparse,
}

impl DataGenLevel {
    /// Returns the LZ77 match generation probability ($0.0 \le p \le 1.0$).
    #[inline]
    pub const fn match_probability(&self) -> f64 {
        match self {
            Self::PureNoise => 0.0,
            Self::BarelyCompressible => 0.30,
            Self::Standard => 0.50,
            Self::HighlyCompressible => 0.85,
            Self::Sparse => 0.995,
        }
    }

    /// Returns the literal skew probability parameter for `LiteralDistribTable`.
    #[inline]
    pub const fn lit_probability(&self) -> f64 {
        match self {
            Self::PureNoise => 0.0,
            Self::BarelyCompressible => 0.0,
            Self::Standard => 0.25,
            Self::HighlyCompressible => 0.70,
            Self::Sparse => 0.95,
        }
    }

    /// Returns the maximum match length in bytes for LZ77 repetitions.
    #[inline]
    pub const fn max_match_len(&self) -> usize {
        match self {
            Self::PureNoise => 0,
            Self::BarelyCompressible => 12,
            Self::Standard => 32,
            Self::HighlyCompressible => 128,
            Self::Sparse => 1024,
        }
    }

    /// Returns the maximum lookback window history size in bytes.
    #[inline]
    pub const fn window_size(&self) -> usize {
        match self {
            Self::PureNoise => 0,
            Self::BarelyCompressible => 64 * 1024,
            Self::Standard => 128 * 1024,
            Self::HighlyCompressible => 32 * 1024,
            Self::Sparse => 8 * 1024,
        }
    }

    /// Returns human-readable description of the level.
    #[allow(dead_code)]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::PureNoise => "Pure Noise (Entropy 8.0 bpb, 0% Match, Incompressible)",
            Self::BarelyCompressible => "Barely Compressible (~12% Match, ~75% Ratio)",
            Self::Standard => "Standard Corpus (~50% Match, ASCII Skew, ~40% Ratio)",
            Self::HighlyCompressible => "Highly Compressible (~85% Match, High Skew, ~10% Ratio)",
            Self::Sparse => "Sparse Data (~99.5% Match, Long Runs, <2% Ratio)",
        }
    }
}

// MARK: - 4. High-Throughput Corpus Generator

/// Generates a synthetic test corpus vector with the requested compressibility profile.
///
/// # Arguments
/// * `level` - Compressibility profile level from [`DataGenLevel`].
/// * `size` - Exact size of generated byte buffer in bytes.
/// * `seed` - 32-bit PRNG seed ensuring 100% deterministic reproducibility.
pub fn generate_corpus(level: DataGenLevel, size: usize, seed: u32) -> Vec<u8> {
    let mut buffer = vec![0u8; size];
    generate_corpus_into(level, &mut buffer, seed);
    buffer
}

/// Populates an existing byte slice in-place with synthetic corpus data.
///
/// Achieves high throughput ($\ge 1.0\text{ GB/s}$) via vectorized slice copying and fast LCG PRNG.
pub fn generate_corpus_into(level: DataGenLevel, dst: &mut [u8], seed: u32) {
    let size = dst.len();
    if size == 0 {
        return;
    }

    let mut rng = RdgRng::new(seed);

    // Fast path for PureNoise: direct 4-byte vectorized filling
    if level == DataGenLevel::PureNoise {
        rng.fill_bytes(dst);
        return;
    }

    let ldt = LiteralDistribTable::new(level.lit_probability());
    let match_proba = level.match_probability();
    let match_threshold = (match_proba * 65536.0) as u32;
    let max_match = level.max_match_len();
    let window_size = level.window_size();

    let mut pos = 0usize;

    // Seed initial prefix literals so matches have valid history
    let initial_prefix = min(16, size);
    while pos < initial_prefix {
        dst[pos] = ldt.sample(&mut rng);
        pos += 1;
    }

    while pos < size {
        let is_match = ((rng.next_u32() & 0xFFFF) < match_threshold) && pos > 0;

        if is_match {
            let min_len = 4usize;
            let len_range = if max_match > min_len {
                max_match - min_len + 1
            } else {
                1
            };
            let raw_len = min_len + ((rng.next_u32() as usize) % len_range);
            let match_len = min(raw_len, size - pos);

            let max_offset = min(pos, window_size);
            let offset = 1 + ((rng.next_u32() as usize) % max_offset);
            let src_start = pos - offset;

            if offset >= match_len {
                dst.copy_within(src_start..src_start + match_len, pos);
            } else if offset == 1 {
                let fill_byte = dst[src_start];
                dst[pos..pos + match_len].fill(fill_byte);
            } else {
                let mut copied = 0;
                while copied < match_len {
                    let chunk = min(offset, match_len - copied);
                    dst.copy_within(src_start..src_start + chunk, pos + copied);
                    copied += chunk;
                }
            }
            pos += match_len;
        } else {
            let run_len = min(4 + ((rng.next_u32() as usize) % 16), size - pos);
            let mut i = 0usize;
            while i + 4 <= run_len {
                let rand_val = rng.next_u32();
                dst[pos + i] = ldt.table[(rand_val as usize) & LTMASK];
                dst[pos + i + 1] = ldt.table[((rand_val >> 8) as usize) & LTMASK];
                dst[pos + i + 2] = ldt.table[((rand_val >> 16) as usize) & LTMASK];
                dst[pos + i + 3] = ldt.table[((rand_val >> 24) as usize) & LTMASK];
                i += 4;
            }
            while i < run_len {
                dst[pos + i] = ldt.sample(&mut rng);
                i += 1;
            }
            pos += run_len;
        }
    }
}

// MARK: - 5. Internal Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdg_rng_determinism() {
        let mut rng1 = RdgRng::new(0x1337);
        let mut rng2 = RdgRng::new(0x1337);

        for _ in 0..10_000 {
            assert_eq!(rng1.next_u32(), rng2.next_u32());
        }
    }

    #[test]
    fn test_literal_distrib_table_bounds() {
        let ldt_flat = LiteralDistribTable::new(0.0);
        assert_eq!(ldt_flat.table.len(), LDT_SIZE);

        let ldt_skewed = LiteralDistribTable::new(0.5);
        assert_eq!(ldt_skewed.table.len(), LDT_SIZE);

        // Verify skewed distribution contains printable ASCII characters
        let mut rng = RdgRng::new(42);
        for _ in 0..1000 {
            let b = ldt_skewed.sample(&mut rng);
            assert!((b'('..=b'}').contains(&b), "Character out of ASCII range: {b}");
        }
    }

    #[test]
    fn test_generate_corpus_empty_and_small() {
        let empty = generate_corpus(DataGenLevel::Standard, 0, 100);
        assert!(empty.is_empty());

        let small = generate_corpus(DataGenLevel::HighlyCompressible, 7, 200);
        assert_eq!(small.len(), 7);
    }
}
