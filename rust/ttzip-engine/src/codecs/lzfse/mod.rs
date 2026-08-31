// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` and `LZVN` hardware/native block codecs and pure Safe Rust 4-Way FSE engine.
//!
//! Provides thread-private 2MB scratch buffer pooling for zero-allocation LZFSE operations,
//! pure Rust 4-Way associative hash matching, and ultra-high-speed LZVN block encoding/decoding.

pub mod block;
pub mod encoder;
pub mod freq_tables;
pub mod fse;
pub mod fse_decoder;
pub mod fsm;
pub mod lzvn_decoder;
pub mod lzvn_encoder;
pub mod reader;
pub mod tables;
pub mod writer;

pub use block::{
    decode_v1_freq_value, decode_v2_freq_tables, emit_block_header_v2, encode_v1_freq_value,
    encode_v2_freq_tables, parse_block_header, BvxMagic, LzfseBlockHeader, LzfseFreqTables,
    LZFSE_FREQ_TOTAL_SYMBOLS, LZFSE_V2_HEADER_FIXED_SIZE,
};
pub use encoder::{
    apply_d_prev_filter, find_matches_4way, lzfse_compress_pure_rust, lzfse_encode_block,
    split_lmd_matches, FseOutStream, LmdTriplet, LzfseHistorySet, LzfseMatchTable, LzfseRawMatch,
};
pub use fse::{
    fse_check_freq, fse_init_decoder_table, fse_init_decoder_table_packed, fse_init_encoder_table,
    fse_init_value_decoder_table, fse_normalize_freq, lzfse_encode_v1_freq_table, FseDecoderEntry,
    FseEncoderEntry, FseValueDecoderEntry,
};
pub use fse_decoder::{decode_literals_4way, decode_lmd_stream, FseInStream, FseLmdState, FseLmdTables};
pub use fsm::{LzfseBlockFsm, LzfseFsmState, LzfseFsmStep, LzfseParsedBlock};
pub use lzvn_decoder::{
    lzvn_decompress, lzvn_decompress_pure_rust, lzvn_decompress_raw, lzvn_decompress_to_vec,
    lzvn_decompress_to_vec_pure_rust, lzvn_validate, LzvnDecoder, LzvnOpcodeKind, LZVN_OPCODE_TABLE,
};
pub use lzvn_encoder::{lzvn_compress, lzvn_compress_bound, lzvn_compress_raw, lzvn_compress_to_vec};
pub use reader::{lzfse_decompress_stream, lzfse_validate, LzfseReader, LZFSE_MAX_BLOCK_SIZE};
pub use tables::*;
pub use writer::{
    lzfse_compress_stream, LzfseWriter, DEFAULT_LZVN_THRESHOLD, LZFSE_BLOCK_CHUNK_SIZE,
    LZFSE_CHUNK_SIZE,
};

use crate::types::TTZipStatus;
use std::cell::RefCell;

// MARK: - Native C LZFSE Scratch & Bindings

extern "C" {
    fn lzfse_encode_scratch_size() -> libc::size_t;
    fn lzfse_encode_buffer(
        dst_buffer: *mut u8,
        dst_size: libc::size_t,
        src_buffer: *const u8,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;

    fn lzfse_decode_scratch_size() -> libc::size_t;
    fn lzfse_decode_buffer(
        dst_buffer: *mut u8,
        dst_size: libc::size_t,
        src_buffer: *const u8,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;
}

const LZFSE_SCRATCH_MIN_CAPACITY: usize = 2 * 1024 * 1024; // 2MB Scratch buffer

thread_local! {
    static LZFSE_SCRATCH_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_lzfse_scratch<F, R>(min_size: usize, f: F) -> R
where
    F: FnOnce(*mut libc::c_void) -> R,
{
    LZFSE_SCRATCH_BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        let target_size = min_size.max(LZFSE_SCRATCH_MIN_CAPACITY);
        if buf.len() < target_size {
            buf.resize(target_size, 0);
        }
        f(buf.as_mut_ptr() as *mut libc::c_void)
    })
}

// MARK: - Safe Buffer Bound Calculations

/// Computes worst-case output buffer size for LZFSE block compression.
#[inline]
pub fn lzfse_compress_bound(src_size: usize) -> usize {
    src_size.saturating_add(4096)
}

// MARK: - LZFSE Core APIs

/// Compresses a buffer with Apple LZFSE using thread-private 2MB scratch buffer.
pub fn lzfse_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let scratch_size = unsafe { lzfse_encode_scratch_size() };
    let written = with_lzfse_scratch(scratch_size, |scratch| unsafe {
        lzfse_encode_buffer(
            dst.as_mut_ptr(),
            dst.len(),
            src.as_ptr(),
            src.len(),
            scratch,
        )
    });

    if written == 0 && !src.is_empty() {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written)
    }
}

/// Decompresses an Apple LZFSE buffer using thread-private 2MB scratch buffer.
pub fn lzfse_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let scratch_size = unsafe { lzfse_decode_scratch_size() };
    let written = with_lzfse_scratch(scratch_size, |scratch| unsafe {
        lzfse_decode_buffer(
            dst.as_mut_ptr(),
            dst.len(),
            src.as_ptr(),
            src.len(),
            scratch,
        )
    });

    if written == 0 && !src.is_empty() {
        Err(TTZipStatus::ErrCorruptHeader)
    } else {
        Ok(written)
    }
}

/// Compresses a raw memory buffer using Apple LZFSE format into a newly allocated `Vec<u8>`.
pub fn lzfse_compress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lzfse_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzfse_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

/// Decompresses an Apple LZFSE buffer with pre-known uncompressed length into a newly allocated `Vec<u8>`.
pub fn lzfse_decompress_raw(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || uncompressed_len == 0 {
        return Ok(Vec::new());
    }
    let out = lzfse_decompress_stream(src)?;
    if out.len() != uncompressed_len {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(out)
}

/// Compatibility alias for `lzfse_compress_raw`.
#[inline]
pub fn lzfse_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    lzfse_compress_raw(src)
}

/// Compatibility alias for `lzfse_decompress_raw`.
#[inline]
pub fn lzfse_decompress_to_vec(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    lzfse_decompress_raw(src, uncompressed_len)
}
