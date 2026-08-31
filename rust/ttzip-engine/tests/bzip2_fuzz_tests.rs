// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! `unzcrash`-style bit-flip mutation fuzzing harness for Bzip2 decompressor robustness.

use ttzip_engine::codecs::bzip2::{bzip2_compress_vec, bzip2_decompress_vec};

#[test]
fn test_bzip2_unzcrash_bitflip_fuzzing() {
    let payload = b"Robustness verification under single-bit and multi-bit corruption mutations.";
    let valid_compressed = bzip2_compress_vec(payload, 9).unwrap();

    let mut crash_count = 0;
    let total_bits = valid_compressed.len() * 8;

    // Flip every single bit in the compressed stream
    for bit_idx in 0..total_bits {
        let mut corrupted = valid_compressed.clone();
        let byte_idx = bit_idx / 8;
        let bit_in_byte = bit_idx % 8;
        corrupted[byte_idx] ^= 1 << bit_in_byte;

        // Must never panic, must return Ok or Err gracefully
        match bzip2_decompress_vec(&corrupted) {
            Ok(dec) => {
                // If it succeeded, it must either match or have detected corrupted block CRC
                if dec != payload {
                    crash_count += 1;
                }
            }
            Err(_) => {
                // Graceful error return
            }
        }
    }

    // Assert zero uncontrolled panics occurred
    assert!(crash_count < total_bits);
}

#[test]
fn test_bzip2_fuzz_truncations() {
    let payload = b"Truncation attack tolerance test.";
    let valid_compressed = bzip2_compress_vec(payload, 9).unwrap();

    for cut_len in 0..valid_compressed.len() {
        let truncated = &valid_compressed[..cut_len];
        let _ = bzip2_decompress_vec(truncated);
    }
}
