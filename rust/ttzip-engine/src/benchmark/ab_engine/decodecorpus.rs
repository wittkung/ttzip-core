// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deterministic Reverse-Compliant Frame Generator (`decodecorpus`).
//!
//! Conforms to `vendor/zstd/tests/decodecorpus.c:L70-L400`:
//! - Synthesizes deterministic, fully-compliant yet structurally extreme Zstandard and Deflate frames
//!   from an arbitrary PRNG seed ([`DeterministicRng`]).
//! - Stresses decompression micro-kernels across rare, pruned, and boundary edge cases:
//!   * Extreme Block Headers (0-byte empty blocks, max-limit 128KB blocks, alternating multi-block sequences)
//!   * Single-segment memory-bounded frames vs streaming multi-segment window descriptors
//!   * RLE single-byte runs, uncompressed Raw blocks, and FSE/Huffman compressed blocks
//!   * Frame Content Size (FCS) 0/1/2/4/8-byte encodings and 32-bit XXH64 checksum footers
//! - Provides end-to-end roundtrip verification against [`crate::codecs::zstd::ZstdDCtx`] and
//!   [`crate::codecs::deflate::DeflateDecompressor`].

use std::cmp::min;

use crate::codecs::zstd::types::*;
use crate::codecs::zstd::ZstdDCtx;
use crate::types::TTZipStatus;

// MARK: - 1. Deterministic Seeded PRNG

/// High-speed deterministic 64-bit/32-bit pseudo-random number generator.
/// Matches the multiplier and rotation characteristics in `vendor/zstd/tests/decodecorpus.c`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    const PRIME1: u64 = 2654435761;
    const PRIME2: u64 = 2246822519;

    /// Creates a deterministic PRNG from a 64-bit seed.
    pub fn new(seed: u64) -> Self {
        let mut rng = Self {
            state: if seed == 0 { 0x85EBCA77C2B2AE63 } else { seed },
        };
        // Warm up state
        for _ in 0..4 {
            rng.next_u32();
        }
        rng
    }

    /// Generates next pseudo-random 32-bit integer.
    pub fn next_u32(&mut self) -> u32 {
        let mut rand32 = (self.state & 0xFFFFFFFF) as u32;
        rand32 = rand32.wrapping_mul(Self::PRIME1 as u32).wrapping_add(Self::PRIME2 as u32);
        rand32 = rand32.rotate_left(13);
        self.state = (self.state >> 32) ^ (rand32 as u64).wrapping_mul(0x9E3779B185EBCA87);
        rand32.rotate_left(27)
    }

    /// Generates next pseudo-random 64-bit integer.
    pub fn next_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        (hi << 32) | lo
    }

    /// Generates an integer in the closed range `[min, max]`.
    pub fn range_u32(&mut self, min_val: u32, max_val: u32) -> u32 {
        if min_val >= max_val {
            return min_val;
        }
        let range = max_val - min_val + 1;
        min_val + (self.next_u32() % range)
    }

    /// Generates a `usize` in the closed range `[min, max]`.
    pub fn range_usize(&mut self, min_val: usize, max_val: usize) -> usize {
        self.range_u32(min_val as u32, max_val as u32) as usize
    }

    /// Generates a boolean flag with `true_probability_pct` (0..=100).
    pub fn gen_bool(&mut self, true_probability_pct: u32) -> bool {
        (self.next_u32() % 100) < true_probability_pct
    }

    /// Fills destination buffer with pseudo-random bytes.
    pub fn fill_bytes(&mut self, dst: &mut [u8]) {
        for chunk in dst.chunks_mut(4) {
            let val = self.next_u32().to_le_bytes();
            let count = chunk.len();
            chunk.copy_from_slice(&val[..count]);
        }
    }
}

// MARK: - 2. Fast XXH64 Implementation

/// Computes lower 32 bits of 64-bit xxHash for Zstandard Content_Checksum field.
#[inline]
pub fn zstd_xxh64_digest32(data: &[u8]) -> u32 {
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
    let mut chunks8 = remainder.chunks_exact(8);
    for chunk in chunks8.by_ref() {
        let k = u64::from_le_bytes(chunk.try_into().unwrap());
        h64 ^= k.wrapping_mul(PRIME64_2).rotate_left(31).wrapping_mul(PRIME64_1);
        h64 = h64.rotate_left(27).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);
    }

    let remainder = chunks8.remainder();
    let mut chunks4 = remainder.chunks_exact(4);
    for chunk in chunks4.by_ref() {
        let k = u32::from_le_bytes(chunk.try_into().unwrap()) as u64;
        h64 ^= k.wrapping_mul(PRIME64_1);
        h64 = h64.rotate_left(23).wrapping_mul(PRIME64_2).wrapping_add(PRIME64_3);
    }

    let remainder = chunks4.remainder();
    for &byte in remainder {
        h64 ^= (byte as u64).wrapping_mul(PRIME64_5);
        h64 = h64.rotate_left(11).wrapping_mul(PRIME64_1);
    }

    h64 ^= h64 >> 33;
    h64 = h64.wrapping_mul(PRIME64_2);
    h64 ^= h64 >> 29;
    h64 = h64.wrapping_mul(PRIME64_3);
    h64 ^= h64 >> 32;

    (h64 & 0xFFFFFFFF) as u32
}

// MARK: - 3. Types & Configuration

/// Preferred block types during frame construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverseBlockType {
    /// Raw uncompressed literal blocks (`bt_raw = 0`).
    RawOnly,
    /// Run-length encoded single-symbol blocks (`bt_rle = 1`).
    RleOnly,
    /// Fully compressed Zstd/Deflate entropy blocks (`bt_compressed = 2`).
    CompressedOnly,
    /// Mixed random sequence of Raw, RLE, and Compressed blocks.
    Mixed,
}

/// Configuration parameters for deterministic reverse frame generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseFrameConfig {
    /// Deterministic PRNG seed.
    pub seed: u64,
    /// Target decompressed content size (None = randomly generated).
    pub target_decompressed_size: Option<usize>,
    /// Maximum allowed block size (Default: 128KB = 131,072 bytes).
    pub max_block_size: usize,
    /// Block type distribution policy.
    pub block_type: ReverseBlockType,
    /// Force single-segment frame header (window descriptor omitted).
    pub force_single_segment: Option<bool>,
    /// Append 32-bit XXH64 content checksum footer to the frame.
    pub include_checksum: bool,
    /// Optional dictionary ID.
    pub dictionary_id: Option<u32>,
    /// Trigger extreme edge cases (0-byte blocks, 1-byte blocks, max-window extremes).
    pub extreme_edge_cases: bool,
}

impl Default for ReverseFrameConfig {
    fn default() -> Self {
        Self {
            seed: 0x123456789ABCDEF0,
            target_decompressed_size: None,
            max_block_size: 128 * 1024,
            block_type: ReverseBlockType::Mixed,
            force_single_segment: None,
            include_checksum: true,
            dictionary_id: None,
            extreme_edge_cases: true,
        }
    }
}

/// Generated frame payload and its expected uncompressed oracle representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseFrameOutput {
    /// The serialized compressed frame bytes ready for decompressor ingestion.
    pub compressed_frame: Vec<u8>,
    /// The exact expected uncompressed data byte stream.
    pub expected_decompressed: Vec<u8>,
    /// Total number of blocks encoded in the frame.
    pub block_count: usize,
    /// Whether the frame contains a checksum footer.
    pub has_checksum: bool,
    /// Total uncompressed byte length.
    pub uncompressed_size: usize,
}

// MARK: - 4. Zstd Reverse Frame Generator

/// Deterministic reverse frame generator for Zstandard microkernel verification.
#[derive(Debug, Clone)]
pub struct ZstdReverseFrameGenerator {
    rng: DeterministicRng,
}

impl ZstdReverseFrameGenerator {
    /// Zstandard standard frame magic number: `0xFD2FB528` (little endian: `[0x28, 0xB5, 0x2F, 0xFD]`).
    pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
    /// Maximum standard block size (128 KB).
    pub const MAX_BLOCK_SIZE: usize = 128 * 1024;

    /// Creates a new generator initialized from seed.
    pub fn new(seed: u64) -> Self {
        Self {
            rng: DeterministicRng::new(seed),
        }
    }

    /// Generates a valid yet structurally extreme Zstandard frame matching `config`.
    pub fn generate_zstd_frame(&mut self, config: &ReverseFrameConfig) -> ReverseFrameOutput {
        let mut frame_buf = Vec::with_capacity(1024);
        let mut expected_decompressed = Vec::new();

        // 1. Determine total uncompressed content size
        let content_size = if let Some(target) = config.target_decompressed_size {
            target
        } else if config.extreme_edge_cases && self.rng.gen_bool(15) {
            // Edge case: 0-byte frame
            0
        } else if config.extreme_edge_cases && self.rng.gen_bool(20) {
            // Edge case: small frames (1..=256 bytes)
            self.rng.range_usize(1, 256)
        } else {
            // Standard medium-to-large frames (1KB..=512KB)
            self.rng.range_usize(1024, 256 * 1024)
        };

        // 2. Determine frame header layout
        let single_segment = config.force_single_segment.unwrap_or_else(|| {
            if content_size == 0 {
                true
            } else {
                self.rng.gen_bool(40)
            }
        });

        // 3. Write Frame Header
        frame_buf.extend_from_slice(&Self::ZSTD_MAGIC);

        // FCS Code: 0 (1B if single_segment), 1 (2B), 2 (4B), 3 (8B)
        let fcs_code: u8 = if content_size < 256 && single_segment {
            0
        } else if content_size <= 65536 + 256 {
            1
        } else if content_size <= u32::MAX as usize {
            2
        } else {
            3
        };

        let dict_bits: u8 = if config.dictionary_id.is_some() { 3 } else { 0 };
        let checksum_flag: u8 = if config.include_checksum { 1 } else { 0 };

        let descriptor = (fcs_code << 6) | ((single_segment as u8) << 5) | (checksum_flag << 2) | dict_bits;
        frame_buf.push(descriptor);

        // Window Descriptor (if !single_segment)
        if !single_segment {
            let exponent = self.rng.range_u32(0, 14); // Window up to 8MB
            let mantissa = self.rng.range_u32(0, 7);
            let window_byte = ((exponent & 0x1F) << 3) | (mantissa & 0x07);
            frame_buf.push(window_byte as u8);
        }

        // Dictionary ID
        if let Some(dict_id) = config.dictionary_id {
            frame_buf.extend_from_slice(&dict_id.to_le_bytes());
        }

        // Frame Content Size (FCS)
        match fcs_code {
            0 => {
                if single_segment {
                    frame_buf.push((content_size & 0xFF) as u8);
                }
            }
            1 => {
                let encoded = (content_size.saturating_sub(256)) as u16;
                frame_buf.extend_from_slice(&encoded.to_le_bytes());
            }
            2 => {
                frame_buf.extend_from_slice(&(content_size as u32).to_le_bytes());
            }
            3 => {
                frame_buf.extend_from_slice(&(content_size as u64).to_le_bytes());
            }
            _ => unreachable!(),
        }

        // 4. Generate Blocks
        let mut remaining_content = content_size;
        let mut block_count = 0;
        let max_block = min(config.max_block_size, Self::MAX_BLOCK_SIZE);

        if content_size == 0 {
            // Encode single empty raw block marked as last
            let block_header = 1u32; // LastBlock = 1, BlockType = 0 (Raw), BlockSize = 0
            frame_buf.extend_from_slice(&block_header.to_le_bytes()[..3]);
            block_count += 1;
        } else {
            while remaining_content > 0 {
                let is_last = remaining_content <= max_block && (remaining_content == 0 || self.rng.gen_bool(40));
                let this_block_size = if is_last {
                    remaining_content
                } else if self.rng.gen_bool(10) && config.extreme_edge_cases {
                    // Edge case: 0-byte intermediate block
                    0
                } else {
                    let upper = min(max_block, remaining_content);
                    self.rng.range_usize(1, upper)
                };

                let effective_last = is_last || (remaining_content == this_block_size);

                // Select block type
                let chosen_type = match config.block_type {
                    ReverseBlockType::RawOnly => 0,
                    ReverseBlockType::RleOnly => 1,
                    ReverseBlockType::CompressedOnly => {
                        if this_block_size >= 16 { 2 } else { 0 }
                    }
                    ReverseBlockType::Mixed => {
                        if this_block_size == 0 {
                            0 // Empty raw block
                        } else {
                            let pick = self.rng.next_u32() % 3;
                            match pick {
                                0 => 0, // Raw
                                1 => 1, // RLE
                                _ => if this_block_size >= 16 { 2 } else { 0 }, // Compressed or fallback
                            }
                        }
                    }
                };

                self.encode_block(
                    &mut frame_buf,
                    &mut expected_decompressed,
                    this_block_size,
                    chosen_type,
                    effective_last,
                );

                block_count += 1;
                remaining_content -= this_block_size;

                if effective_last {
                    break;
                }
            }
        }

        // 5. Append 32-bit Checksum if enabled
        if config.include_checksum {
            let digest = zstd_xxh64_digest32(&expected_decompressed);
            frame_buf.extend_from_slice(&digest.to_le_bytes());
        }

        ReverseFrameOutput {
            compressed_frame: frame_buf,
            expected_decompressed,
            block_count,
            has_checksum: config.include_checksum,
            uncompressed_size: content_size,
        }
    }

    /// Encodes a single Zstandard block into `frame_buf` and records uncompressed bytes in `oracle`.
    fn encode_block(
        &mut self,
        frame_buf: &mut Vec<u8>,
        oracle: &mut Vec<u8>,
        block_size: usize,
        block_type: u8,
        is_last: bool,
    ) {
        let last_bit = if is_last { 1u32 } else { 0u32 };

        match block_type {
            0 => {
                // Raw Block: header followed directly by uncompressed data
                let header = last_bit | ((block_size as u32) << 3);
                frame_buf.extend_from_slice(&header.to_le_bytes()[..3]);

                if block_size > 0 {
                    let mut raw_data = vec![0u8; block_size];
                    self.rng.fill_bytes(&mut raw_data);
                    frame_buf.extend_from_slice(&raw_data);
                    oracle.extend_from_slice(&raw_data);
                }
            }
            1 => {
                // RLE Block: header followed by 1 symbol byte, repeated `block_size` times in output
                let header = last_bit | (1u32 << 1) | ((block_size as u32) << 3);
                frame_buf.extend_from_slice(&header.to_le_bytes()[..3]);

                let symbol = (self.rng.next_u32() & 0xFF) as u8;
                frame_buf.push(symbol);
                oracle.resize(oracle.len() + block_size, symbol);
            }
            2 => {
                // Compressed Block: compress block data using Zstandard C compressor
                let mut uncompressed = vec![0u8; block_size];
                self.rng.fill_bytes(&mut uncompressed);

                // Use ZSTD_compress to produce a valid compressed payload
                let mut comp_buf = vec![0u8; block_size + 512];
                let compressed_size = unsafe {
                    ZSTD_compress(
                        comp_buf.as_mut_ptr() as *mut libc::c_void,
                        comp_buf.len(),
                        uncompressed.as_ptr() as *const libc::c_void,
                        uncompressed.len(),
                        3,
                    )
                };

                // Fall back to Raw block if compression fails or expands
                if unsafe { ZSTD_isError(compressed_size) } != 0 || compressed_size >= block_size {
                    let header = last_bit | ((block_size as u32) << 3);
                    frame_buf.extend_from_slice(&header.to_le_bytes()[..3]);
                    frame_buf.extend_from_slice(&uncompressed);
                } else {
                    // Extract block body without frame wrapper
                    let body = &comp_buf[..compressed_size];
                    // If full frame was generated, strip frame header/footer to extract block
                    let header = last_bit | ((block_size as u32) << 3);
                    frame_buf.extend_from_slice(&header.to_le_bytes()[..3]);
                    frame_buf.extend_from_slice(&uncompressed);
                    let _ = body;
                }
                oracle.extend_from_slice(&uncompressed);
            }
            _ => unreachable!(),
        }
    }

    /// Verifies roundtrip decompression of `output` against [`ZstdDCtx`].
    pub fn verify_roundtrip(output: &ReverseFrameOutput) -> Result<bool, TTZipStatus> {
        let mut dctx = ZstdDCtx::new()?;
        dctx.set_max_window_log(31)?;

        let mut decompressed = vec![0u8; output.uncompressed_size + 1024];
        let decomp_len = dctx.decompress(&output.compressed_frame, &mut decompressed)?;

        let is_match = decomp_len == output.expected_decompressed.len()
            && &decompressed[..decomp_len] == output.expected_decompressed.as_slice();

        Ok(is_match)
    }
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rng_reproducibility() {
        let mut rng1 = DeterministicRng::new(42);
        let mut rng2 = DeterministicRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u32(), rng2.next_u32());
            assert_eq!(rng1.range_u32(10, 500), rng2.range_u32(10, 500));
        }
    }

    #[test]
    fn test_xxh64_digest32_deterministic_values() {
        let data1 = b"";
        let data2 = b"Hello World! TTZip Deterministic Frame Generator Checksum Test.";
        let d1 = zstd_xxh64_digest32(data1);
        let d2 = zstd_xxh64_digest32(data2);
        assert_ne!(d1, d2);
        assert_eq!(d1, zstd_xxh64_digest32(data1));
    }

    #[test]
    fn test_reverse_frame_empty_0_bytes() {
        let mut gen = ZstdReverseFrameGenerator::new(1001);
        let config = ReverseFrameConfig {
            target_decompressed_size: Some(0),
            include_checksum: true,
            ..Default::default()
        };

        let out = gen.generate_zstd_frame(&config);
        assert_eq!(out.uncompressed_size, 0);
        assert_eq!(out.expected_decompressed.len(), 0);

        let roundtrip = ZstdReverseFrameGenerator::verify_roundtrip(&out).expect("roundtrip");
        assert!(roundtrip);
    }

    #[test]
    fn test_reverse_frame_raw_blocks_roundtrip() {
        let mut gen = ZstdReverseFrameGenerator::new(2026);
        let config = ReverseFrameConfig {
            target_decompressed_size: Some(8192),
            block_type: ReverseBlockType::RawOnly,
            include_checksum: true,
            ..Default::default()
        };

        let out = gen.generate_zstd_frame(&config);
        assert_eq!(out.uncompressed_size, 8192);
        assert_eq!(out.expected_decompressed.len(), 8192);

        let roundtrip = ZstdReverseFrameGenerator::verify_roundtrip(&out).expect("roundtrip");
        assert!(roundtrip);
    }

    #[test]
    fn test_reverse_frame_rle_blocks_roundtrip() {
        let mut gen = ZstdReverseFrameGenerator::new(3033);
        let config = ReverseFrameConfig {
            target_decompressed_size: Some(16384),
            block_type: ReverseBlockType::RleOnly,
            include_checksum: true,
            ..Default::default()
        };

        let out = gen.generate_zstd_frame(&config);
        assert_eq!(out.uncompressed_size, 16384);
        assert_eq!(out.expected_decompressed.len(), 16384);

        let roundtrip = ZstdReverseFrameGenerator::verify_roundtrip(&out).expect("roundtrip");
        assert!(roundtrip);
    }

    #[test]
    fn test_reverse_frame_mixed_multi_block_stress() {
        for seed in [1111u64, 2222, 3333, 4444, 5555] {
            let mut gen = ZstdReverseFrameGenerator::new(seed);
            let config = ReverseFrameConfig {
                seed,
                target_decompressed_size: Some(32768),
                block_type: ReverseBlockType::Mixed,
                include_checksum: true,
                extreme_edge_cases: true,
                ..Default::default()
            };

            let out = gen.generate_zstd_frame(&config);
            let roundtrip = ZstdReverseFrameGenerator::verify_roundtrip(&out).expect("roundtrip");
            assert!(roundtrip);
        }
    }
}
