// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` multi-block streaming reader and stream decompression engine.
//!
//! Implements a streaming `std::io::Read` adapter for LZFSE container streams (`bvx-`, `bvxn`, `bvx1`, `bvx2`, `bvx$`),
//! pure Safe Rust block-by-block stream decompression, and structural stream validation.

use super::block::{
    parse_block_header, BvxMagic, LzfseBlockHeader, LzfseFreqTables,
    LZFSE_ENCODE_D_STATES, LZFSE_ENCODE_D_SYMBOLS, LZFSE_ENCODE_LITERAL_STATES,
    LZFSE_ENCODE_LITERAL_SYMBOLS, LZFSE_ENCODE_L_STATES, LZFSE_ENCODE_L_SYMBOLS,
    LZFSE_ENCODE_M_STATES, LZFSE_ENCODE_M_SYMBOLS,
};
use super::fse::{fse_init_decoder_table_packed, fse_init_value_decoder_table, FseValueDecoderEntry};
use super::fse_decoder::{
    decode_literals_4way, decode_lmd_stream, FseInStream, FseLmdState, FseLmdTables,
};
use super::lzvn_decoder::lzvn_decompress_raw;
use super::tables::{
    D_BASE_VALUE, D_EXTRA_BITS, L_BASE_VALUE, L_EXTRA_BITS, M_BASE_VALUE, M_EXTRA_BITS,
};
use crate::types::TTZipStatus;
use std::io::{self, Read};

/// Maximum uncompressed block size in Apple LZFSE container specification (256KB).
pub const LZFSE_MAX_BLOCK_SIZE: usize = 256 * 1024;

// MARK: - Single Block Decompression Helper

fn decompress_v1_or_v2_block(
    header: &LzfseBlockHeader,
    header_len: usize,
    block_slice: &[u8],
    cached_tables: &mut Option<LzfseFreqTables>,
    out: &mut Vec<u8>,
) -> Result<usize, TTZipStatus> {
    if let Some(tables) = header.freq_tables.clone() {
        *cached_tables = Some(tables);
    }
    let tables = match cached_tables.as_ref() {
        Some(t) => t,
        None => return Err(TTZipStatus::ErrCorruptHeader),
    };

    let lit_payload_len = header.n_literal_payload_bytes as usize;
    let lmd_payload_len = header.n_lmd_payload_bytes as usize;
    let total_block_len = header_len + lit_payload_len + lmd_payload_len;

    if block_slice.len() < total_block_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let lit_slice = &block_slice[..header_len + lit_payload_len];
    let lmd_slice = &block_slice[..total_block_len];

    let mut lit_table = [0i32; LZFSE_ENCODE_LITERAL_STATES];
    let mut l_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_L_STATES];
    let mut m_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_M_STATES];
    let mut d_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_D_STATES];

    fse_init_decoder_table_packed(
        LZFSE_ENCODE_LITERAL_STATES,
        LZFSE_ENCODE_LITERAL_SYMBOLS,
        &tables.literal_freq,
        &mut lit_table,
    )?;
    fse_init_value_decoder_table(
        LZFSE_ENCODE_L_STATES,
        LZFSE_ENCODE_L_SYMBOLS,
        &tables.l_freq,
        &L_BASE_VALUE,
        &L_EXTRA_BITS,
        &mut l_table,
    )?;
    fse_init_value_decoder_table(
        LZFSE_ENCODE_M_STATES,
        LZFSE_ENCODE_M_SYMBOLS,
        &tables.m_freq,
        &M_BASE_VALUE,
        &M_EXTRA_BITS,
        &mut m_table,
    )?;
    fse_init_value_decoder_table(
        LZFSE_ENCODE_D_STATES,
        LZFSE_ENCODE_D_SYMBOLS,
        &tables.d_freq,
        &D_BASE_VALUE,
        &D_EXTRA_BITS,
        &mut d_table,
    )?;

    // 1. Decode literals using 4-way interleaved FSE
    let mut lit_stream = FseInStream::init(header.literal_bits, lit_slice)?;
    let mut literals = vec![0u8; header.n_literals as usize];
    let mut lit_states = header.literal_state;
    decode_literals_4way(&mut lit_stream, &lit_table, &mut lit_states, &mut literals)?;

    // 2. Decode LMD stream directly into cumulative output
    let mut lmd_stream = FseInStream::init(header.lmd_bits, lmd_slice)?;
    let tables = FseLmdTables {
        l_table: &l_table,
        m_table: &m_table,
        d_table: &d_table,
    };
    let mut state = FseLmdState {
        l_state: header.l_state,
        m_state: header.m_state,
        d_state: header.d_state,
    };

    let raw_len = header.n_raw_bytes as usize;
    let written = decode_lmd_stream(
        &mut lmd_stream,
        &tables,
        &mut state,
        header.n_matches as usize,
        &literals,
        out,
        raw_len,
    )?;

    Ok(written)
}

// MARK: - Stream Decompression API

/// Decompresses an Apple LZFSE multi-block container stream into a newly allocated `Vec<u8>`.
///
/// Fully supports uncompressed (`bvx-`), LZVN (`bvxn`), and LZFSE (`bvx1`, `bvx2`) blocks
/// terminating cleanly on `bvx$`.
pub fn lzfse_decompress_stream(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let mut offset = 0;
    let mut out = Vec::new();
    let mut cached_tables: Option<LzfseFreqTables> = None;
    let mut eos_reached = false;

    while offset < src.len() {
        if src.len() - offset < 4 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let (header, header_len) = parse_block_header(&src[offset..])?;

        match header.magic {
            BvxMagic::EndOfStream => {
                eos_reached = true;
                break;
            }
            BvxMagic::RawUncompressed => {
                let raw_len = header.n_raw_bytes as usize;
                let payload_start = offset + header_len;
                let payload_end = payload_start + raw_len;
                if payload_end > src.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                out.extend_from_slice(&src[payload_start..payload_end]);
                offset = payload_end;
            }
            BvxMagic::CompressedLZVN => {
                let payload_len = header.n_payload_bytes as usize;
                let raw_len = header.n_raw_bytes as usize;
                let payload_start = offset + header_len;
                let payload_end = payload_start + payload_len;
                if payload_end > src.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let payload = &src[payload_start..payload_end];
                let decompressed = lzvn_decompress_raw(payload, raw_len)?;
                out.extend_from_slice(&decompressed);
                offset = payload_end;
            }
            BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                let lit_payload_len = header.n_literal_payload_bytes as usize;
                let lmd_payload_len = header.n_lmd_payload_bytes as usize;
                let total_payload = lit_payload_len + lmd_payload_len;
                let block_end = offset + header_len + total_payload;

                if block_end > src.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }

                let block_slice = &src[offset..block_end];
                decompress_v1_or_v2_block(
                    &header,
                    header_len,
                    block_slice,
                    &mut cached_tables,
                    &mut out,
                )?;
                offset = block_end;
            }
        }
    }

    if !eos_reached {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(out)
}


/// Validates whether `src` contains a structurally compliant LZFSE container stream.
///
/// Verifies block headers, payload size boundaries, frequency tables, and terminal `bvx$` magic.
pub fn lzfse_validate(src: &[u8]) -> bool {
    if src.len() < 4 {
        return false;
    }

    let mut offset = 0;
    while offset < src.len() {
        if src.len() - offset < 4 {
            return false;
        }

        let (header, header_len) = match parse_block_header(&src[offset..]) {
            Ok(res) => res,
            Err(_) => return false,
        };

        match header.magic {
            BvxMagic::EndOfStream => {
                offset += header_len;
                return offset == src.len();
            }
            BvxMagic::RawUncompressed => {
                let raw_len = header.n_raw_bytes as usize;
                offset += header_len + raw_len;
                if offset > src.len() {
                    return false;
                }
            }
            BvxMagic::CompressedLZVN => {
                let payload_len = header.n_payload_bytes as usize;
                offset += header_len + payload_len;
                if offset > src.len() {
                    return false;
                }
            }
            BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                let lit_len = header.n_literal_payload_bytes as usize;
                let lmd_len = header.n_lmd_payload_bytes as usize;
                offset += header_len + lit_len + lmd_len;
                if offset > src.len() {
                    return false;
                }
            }
        }
    }

    false
}

// MARK: - Streaming Read Adapter

/// Streaming `std::io::Read` adapter for decoding Apple LZFSE container streams on the fly.
pub struct LzfseReader<R: Read> {
    inner: R,
    in_buffer: Vec<u8>,
    in_pos: usize,
    out_buffer: Vec<u8>,
    out_pos: usize,
    cached_tables: Option<LzfseFreqTables>,
    eof: bool,
}

impl<R: Read> LzfseReader<R> {
    /// Creates a new `LzfseReader` wrapping the underlying stream `inner`.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            in_buffer: Vec::with_capacity(64 * 1024),
            in_pos: 0,
            out_buffer: Vec::with_capacity(256 * 1024),
            out_pos: 0,
            cached_tables: None,
            eof: false,
        }
    }

    /// Unwraps and returns the inner reader.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns an immutable reference to the inner reader.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the inner reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    fn fill_in_buffer_min(&mut self, min_bytes: usize) -> io::Result<()> {
        let available = self.in_buffer.len() - self.in_pos;
        if available >= min_bytes {
            return Ok(());
        }

        // Compact buffer
        if self.in_pos > 0 {
            self.in_buffer.drain(0..self.in_pos);
            self.in_pos = 0;
        }

        let mut read_buf = [0u8; 32 * 1024];
        while self.in_buffer.len() < min_bytes {
            let n = self.inner.read(&mut read_buf)?;
            if n == 0 {
                break;
            }
            self.in_buffer.extend_from_slice(&read_buf[..n]);
        }
        Ok(())
    }

    fn decode_next_block(&mut self) -> io::Result<bool> {
        if self.eof {
            return Ok(false);
        }

        // Need at least 4 bytes to inspect magic
        self.fill_in_buffer_min(4)?;
        let available = self.in_buffer.len() - self.in_pos;
        if available == 0 {
            self.eof = true;
            return Ok(false);
        }
        if available < 4 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete LZFSE block header",
            ));
        }

        let slice = &self.in_buffer[self.in_pos..];
        let (header, header_len) = parse_block_header(slice).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("LZFSE Header error: {e:?}"))
        })?;

        match header.magic {
            BvxMagic::EndOfStream => {
                self.in_pos += header_len;
                // Check if another stream follows
                let _ = self.fill_in_buffer_min(4);
                let remaining = self.in_buffer.len() - self.in_pos;
                if remaining >= 4 {
                    let next_magic = u32::from_le_bytes(
                        self.in_buffer[self.in_pos..self.in_pos + 4]
                            .try_into()
                            .unwrap(),
                    );
                    if BvxMagic::from_u32(next_magic).is_some() {
                        self.cached_tables = None;
                        return self.decode_next_block();
                    }
                }
                self.eof = true;
                Ok(false)
            }
            BvxMagic::RawUncompressed => {
                let raw_len = header.n_raw_bytes as usize;
                let needed = header_len + raw_len;
                self.fill_in_buffer_min(needed)?;
                if self.in_buffer.len() - self.in_pos < needed {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Incomplete raw uncompressed block payload",
                    ));
                }
                let payload_start = self.in_pos + header_len;
                let payload_end = payload_start + raw_len;
                self.out_buffer.clear();
                self.out_buffer
                    .extend_from_slice(&self.in_buffer[payload_start..payload_end]);
                self.out_pos = 0;
                self.in_pos = payload_end;
                Ok(true)
            }
            BvxMagic::CompressedLZVN => {
                let payload_len = header.n_payload_bytes as usize;
                let raw_len = header.n_raw_bytes as usize;
                let needed = header_len + payload_len;
                self.fill_in_buffer_min(needed)?;
                if self.in_buffer.len() - self.in_pos < needed {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Incomplete LZVN block payload",
                    ));
                }
                let payload_start = self.in_pos + header_len;
                let payload_end = payload_start + payload_len;
                let decompressed = lzvn_decompress_raw(
                    &self.in_buffer[payload_start..payload_end],
                    raw_len,
                )
                .map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("LZVN decode error: {e:?}"))
                })?;
                self.out_buffer = decompressed;
                self.out_pos = 0;
                self.in_pos = payload_end;
                Ok(true)
            }
            BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                let lit_len = header.n_literal_payload_bytes as usize;
                let lmd_len = header.n_lmd_payload_bytes as usize;
                let needed = header_len + lit_len + lmd_len;
                self.fill_in_buffer_min(needed)?;
                if self.in_buffer.len() - self.in_pos < needed {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Incomplete LZFSE compressed block payload",
                    ));
                }
                let block_slice = &self.in_buffer[self.in_pos..self.in_pos + needed];
                let prev_out_len = self.out_buffer.len();
                decompress_v1_or_v2_block(
                    &header,
                    header_len,
                    block_slice,
                    &mut self.cached_tables,
                    &mut self.out_buffer,
                )
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("LZFSE block decode error: {e:?}"),
                    )
                })?;
                self.out_pos = prev_out_len;
                self.in_pos += needed;
                Ok(true)
            }
        }
    }
}

impl<R: Read> Read for LzfseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            let available = self.out_buffer.len() - self.out_pos;
            if available > 0 {
                let to_copy = available.min(buf.len());
                buf[..to_copy]
                    .copy_from_slice(&self.out_buffer[self.out_pos..self.out_pos + to_copy]);
                self.out_pos += to_copy;
                return Ok(to_copy);
            }

            if self.eof {
                return Ok(0);
            }

            // Maintain sliding 256KB ring history when buffer exceeds 512KB
            if self.out_buffer.len() > 512 * 1024 {
                let excess = self.out_buffer.len() - 256 * 1024;
                self.out_buffer.drain(0..excess);
                self.out_pos = self.out_buffer.len();
            }

            let has_more = self.decode_next_block()?;
            if !has_more {
                return Ok(0);
            }
        }
    }
}

