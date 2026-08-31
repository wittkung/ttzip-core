// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZ4 High Compression (LZ4HC) with 256KB Dual-Table Relative Linked List and DP Optimal Parser.
//!
//! Exposes:
//! - `Lz4HcDualTable`: 256KB zero-allocation hash & circular relative delta chain table
//! - `price_literals` & `price_sequence`: exact bit-cost pricing functions
//! - `Lz4HcOptimalParser`: multi-tier evaluation (Fast, Lazy-2, Lazy-3, and 4096-window DP Optimal)
//! - `lz4_compress_hc_opt` and convenience functional wrappers

mod dual_table;
mod parser;
mod price;

pub use dual_table::{
    Lz4HcDualTable, Lz4Match, LAST_LITERALS, LZ4HC_CHAIN_SIZE, LZ4HC_HASH_LOG, LZ4HC_HASH_SIZE,
    LZ4HC_MAX_DISTANCE, MIN_MATCH,
};
pub use parser::{
    Lz4HcOptimalParser, Lz4HcParams, Lz4HcStrategy, Lz4OptimalNode, LZ4_OPT_NUM,
    MAX_SEARCH_DEPTH_TABLE, MF_LIMIT,
};
pub use price::{price_literals, price_sequence, price_sequence_speed};


use crate::codecs::lz4::block::lz4_compress_bound;
use crate::types::TTZipStatus;

/// Compresses a buffer using the LZ4HC 256KB dual-table optimal parser.
pub fn lz4_compress_hc_opt(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    let mut parser = Lz4HcOptimalParser::new(level);
    parser.compress_block(src, dst)
}

/// Compresses a buffer using the LZ4HC optimal parser with explicit custom parameters.
pub fn lz4_compress_hc_opt_custom(
    src: &[u8],
    dst: &mut [u8],
    params: &Lz4HcParams,
) -> Result<usize, TTZipStatus> {
    let mut parser = Lz4HcOptimalParser::with_params(*params);
    parser.compress_block(src, dst)
}

/// Compresses a buffer into a newly allocated `Vec<u8>` using the LZ4HC optimal parser.
pub fn lz4_compress_hc_opt_to_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lz4_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lz4_compress_hc_opt(src, &mut out, level)?;
    out.truncate(written);
    Ok(out)
}

/// Compresses a buffer into a newly allocated `Vec<u8>` using custom LZ4HC parameters.
pub fn lz4_compress_hc_opt_custom_to_vec(
    src: &[u8],
    params: &Lz4HcParams,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lz4_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lz4_compress_hc_opt_custom(src, &mut out, params)?;
    out.truncate(written);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::lz4::block::lz4_decompress;

    #[test]
    fn test_optimal_parser_roundtrip_all_levels() {
        let payload = b"The quick brown fox jumps over the lazy dog. Repeat repeatedly with high entropy and redundancies across the full span of LZ4 HC compression!";
        let mut repeated = Vec::new();
        for _ in 0..20 {
            repeated.extend_from_slice(payload);
        }

        for level in 1..=12 {
            let comp = lz4_compress_hc_opt_to_vec(&repeated, level).expect("compress hc opt");
            assert!(!comp.is_empty());
            assert!(comp.len() < repeated.len());

            let mut decomp = vec![0u8; repeated.len()];
            let d_len = lz4_decompress(&comp, &mut decomp).expect("lz4 decompress");
            assert_eq!(d_len, repeated.len());
            assert_eq!(&decomp[..d_len], repeated.as_slice());
        }
    }

    #[test]
    fn test_favor_dec_speed_mode() {
        let payload = b"Pattern payload test for favor_dec_speed mode verification in TTZip LZ4HC optimal parser.";
        let mut repeated = Vec::new();
        for _ in 0..30 {
            repeated.extend_from_slice(payload);
        }

        let params = Lz4HcParams::for_level(9).with_favor_dec_speed(true);
        let comp = lz4_compress_hc_opt_custom_to_vec(&repeated, &params).expect("favor speed comp");
        assert!(!comp.is_empty());

        let mut decomp = vec![0u8; repeated.len()];
        let d_len = lz4_decompress(&comp, &mut decomp).expect("lz4 decompress");
        assert_eq!(d_len, repeated.len());
        assert_eq!(&decomp[..d_len], repeated.as_slice());
    }
}
