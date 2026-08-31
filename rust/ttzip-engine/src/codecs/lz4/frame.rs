// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance LZ4 Framing Format codec, 15-stage streaming decoder, and encoder.
//!
//! Conforms strictly to the official LZ4 Framing Format specification (v1.6.2+):
//! - Deterministic 15-stage `dStage` streaming decoder (`Lz4FrameDecoder`)
//! - Micro-buffering sliding dictionary window with $\le 5\text{MB}$ resident memory
//! - Streaming multi-frame block compressor (`Lz4FrameEncoder`)
//! - Concatenated multi-frame auto-continuation and skippable metadata frame bypass
//! - Convenient zero-allocation in-memory frame compression and validation helpers

pub mod decoder;
pub mod encoder;

pub use decoder::*;
pub use encoder::*;

use crate::codecs::lz4::constants::FrameDescriptor;
use crate::types::TTZipStatus;
use std::io::{Read, Write};

// MARK: - Convenient In-Memory Helpers

/// Compresses an in-memory slice into standard LZ4 Frame format in `dst`.
pub fn lz4_frame_compress(
    src: &[u8],
    dst: &mut [u8],
    desc: Option<&FrameDescriptor>,
    level: i32,
) -> Result<usize, TTZipStatus> {
    let mut cursor = std::io::Cursor::new(dst);
    let mut descriptor = desc.cloned().unwrap_or_default();
    if descriptor.content_size.is_none() {
        descriptor.content_size = Some(src.len() as u64);
    }
    let mut encoder = Lz4FrameEncoder::with_options(&mut cursor, descriptor, level)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    encoder
        .write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    encoder
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(cursor.position() as usize)
}

/// Compresses an in-memory slice into standard LZ4 Frame format returning `Vec<u8>`.
pub fn lz4_frame_compress_to_vec(
    src: &[u8],
    desc: Option<&FrameDescriptor>,
    level: i32,
) -> Result<Vec<u8>, TTZipStatus> {
    let mut out = Vec::with_capacity(src.len() + 128);
    let mut descriptor = desc.cloned().unwrap_or_default();
    if descriptor.content_size.is_none() {
        descriptor.content_size = Some(src.len() as u64);
    }
    let mut encoder = Lz4FrameEncoder::with_options(&mut out, descriptor, level)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    encoder
        .write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    encoder
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(out)
}

/// Decompresses an LZ4 Frame buffer into pre-allocated destination buffer.
pub fn lz4_frame_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut decoder = Lz4FrameDecoder::new(src);
    let mut total = 0;
    while total < dst.len() {
        match decoder.read(&mut dst[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => return Err(TTZipStatus::ErrExtractionFailed),
        }
    }
    Ok(total)
}

/// Decompresses an LZ4 Frame buffer into `Vec<u8>` with optional max size limit.
pub fn lz4_frame_decompress_to_vec(src: &[u8], max_allowed: usize) -> Result<Vec<u8>, TTZipStatus> {
    let mut decoder = Lz4FrameDecoder::new(src);
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if out.len() + n > max_allowed {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                out.extend_from_slice(&buf[..n]);
            }
            Err(_) => return Err(TTZipStatus::ErrExtractionFailed),
        }
    }
    Ok(out)
}

/// Validates integrity of an LZ4 Frame buffer without full memory allocation.
pub fn lz4_frame_validate(src: &[u8]) -> bool {
    let mut cursor = std::io::Cursor::new(src);
    lz4_frame_validate_reader(&mut cursor)
}

/// Validates integrity of an LZ4 Frame stream from a `Read` source.
pub fn lz4_frame_validate_reader<R: Read>(reader: &mut R) -> bool {
    let mut decoder = Lz4FrameDecoder::new(reader);
    let mut stack_buf = [0u8; 8192];
    loop {
        match decoder.read(&mut stack_buf) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
}
