// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Upstream `ppmd-rust = "1"` canonical PPMd (Prediction by Partial Matching, Model H / 7z PPMd) codecs.
//!
//! Provides single canonical backend for 7z PPMd7 and Zip PPMd8 compression and decompression,
//! memory budget bounds protection, and 7z property parsing.

use std::io::{Cursor, Read, Write};
use crate::types::TTZipStatus;

pub use ppmd_rust::{
    Ppmd7Decoder, Ppmd7Encoder, Ppmd8Decoder, Ppmd8Encoder,
    RestoreMethod as PpmdRestoreMethod, RestoreMethod as PpmdVariant,
};

// MARK: - Constants & Limits

pub const METHOD_PPMD: u64 = 0x030401;
pub const PPMD_MIN_MEMORY_SIZE: usize = 2 * 1024 * 1024;
pub const PPMD_MAX_MEMORY_SIZE: usize = 256 * 1024 * 1024;
pub const PPMD_DEFAULT_MEMORY_SIZE: usize = 16 * 1024 * 1024;
pub const PPMD_MIN_ORDER: u32 = 2;
pub const PPMD_MAX_ORDER: u32 = 32;
pub const PPMD_DEFAULT_ORDER: u32 = 6;

// Legacy / Compatibility Constants
pub const INIT_BIN_ESC: u32 = 0;
pub const PPMD_BIN_SCALE: u32 = 0;
pub const PPMD_DEFAULT_SUBALLOC_SIZE: usize = PPMD_DEFAULT_MEMORY_SIZE;
pub const PPMD_EXP_ESCAPE: u32 = 0;
pub const PPMD_INT_BITS: u32 = 0;
pub const PPMD_MAX_FREQ: u32 = 0;
pub const PPMD_MAX_SUBALLOC_SIZE: usize = PPMD_MAX_MEMORY_SIZE;
pub const PPMD_MIN_SUBALLOC_SIZE: usize = PPMD_MIN_MEMORY_SIZE;
pub const PPMD_NUM_INDEXES: usize = 0;
pub const PPMD_PERIOD_BITS: u32 = 0;
pub const PPMD_UNIT_SIZE: usize = 12;
pub const SEE_NUM_BINS: usize = 0;
pub const SEE_NUM_CLASSES: usize = 0;

// Legacy / Compatibility Types
pub struct PpmdModel;
pub struct PpmdRangeDecoder;
pub struct PpmdRangeEncoder;
pub struct PpmdSubAlloc;
pub struct PpmdContext;
pub struct PpmdState;
pub struct SeeEntry;
pub struct SeeEstimator;
pub struct SubAllocBumpArena;
pub struct PpmdSubAllocModel;

// MARK: - Core Operations

/// Parses 7-Zip PPMd 5-byte coder properties `[order, mem_size (4 bytes LE)]`.
pub fn ppmd_parse_7z_props(coder_props: &[u8]) -> Result<(u32, usize), TTZipStatus> {
    if coder_props.len() < 5 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let order = coder_props[0] as u32;
    if !(PPMD_MIN_ORDER..=PPMD_MAX_ORDER).contains(&order) {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mem_raw = u32::from_le_bytes(coder_props[1..5].try_into().unwrap()) as usize;
    let mem_size = mem_raw.clamp(PPMD_MIN_MEMORY_SIZE, PPMD_MAX_MEMORY_SIZE);

    Ok((order, mem_size))
}

/// Decompresses a PPMd stream into a destination buffer with specified order and memory budget.
pub fn ppmd_decompress(
    src: &[u8],
    dst: &mut [u8],
    order: u32,
    mem_size: usize,
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    let mut dec = Ppmd7Decoder::new(src, order, mem_size as u32)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    dec.read_exact(dst)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

    Ok(dst.len())
}

/// Compresses a slice into PPMd format with specified order and memory budget.
pub fn ppmd_compress(
    src: &[u8],
    dst: &mut [u8],
    order: u32,
    mem_size: usize,
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    let mut writer = Cursor::new(dst);
    let mut enc = Ppmd7Encoder::new(&mut writer, order, mem_size as u32)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    enc.write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    enc.finish(false)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    let written = writer.position() as usize;
    Ok(written)
}

/// Compresses a slice using PPMd and returns an owned `Vec<u8>`.
pub fn ppmd_compress_to_vec(
    src: &[u8],
    order: u32,
    mem_size: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut enc = Ppmd7Encoder::new(&mut out, order, mem_size as u32)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    enc.write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    enc.finish(false)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    Ok(out)
}

/// Decompresses a PPMd stream into an owned `Vec<u8>`.
pub fn ppmd_decompress_to_vec(
    src: &[u8],
    expected_uncompressed_len: usize,
    order: u32,
    mem_size: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || expected_uncompressed_len == 0 {
        return Ok(Vec::new());
    }

    let mut dst = vec![0u8; expected_uncompressed_len];
    let written = ppmd_decompress(src, &mut dst, order, mem_size)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses raw PPMd payload using 7z coder properties (5 bytes).
pub fn ppmd_decompress_7z(
    src: &[u8],
    dst: &mut [u8],
    coder_props: &[u8],
) -> Result<usize, TTZipStatus> {
    let (order, mem_size) = ppmd_parse_7z_props(coder_props)?;
    ppmd_decompress(src, dst, order, mem_size)
}

/// Decompresses raw PPMd payload using 7z coder properties into an owned `Vec<u8>`.
pub fn ppmd_decompress_7z_to_vec(
    src: &[u8],
    coder_props: &[u8],
    expected_uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let (order, mem_size) = ppmd_parse_7z_props(coder_props)?;
    ppmd_decompress_to_vec(src, expected_uncompressed_len, order, mem_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppmd_roundtrip() {
        let input = b"Hello canonical ppmd-rust compression and decompression in TTZip 2026!";
        let comp = ppmd_compress_to_vec(input, 6, 2 * 1024 * 1024).expect("compress");
        assert!(!comp.is_empty());

        let decomp = ppmd_decompress_to_vec(&comp, input.len(), 6, 2 * 1024 * 1024).expect("decompress");
        assert_eq!(decomp.as_slice(), input);
    }

    #[test]
    fn test_ppmd_7z_props() {
        let mut props = [0u8; 5];
        props[0] = 6;
        props[1..5].copy_from_slice(&(16 * 1024 * 1024u32).to_le_bytes());

        let (order, mem) = ppmd_parse_7z_props(&props).expect("parse props");
        assert_eq!(order, 6);
        assert_eq!(mem, 16 * 1024 * 1024);
    }
}
