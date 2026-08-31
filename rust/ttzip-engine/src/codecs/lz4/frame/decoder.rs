// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deterministic 15-stage streaming LZ4 Frame decompressor and ring micro-buffer dictionary.
//!
//! Conforms strictly to the official LZ4 Framing Format specification (v1.6.2+):
//! - 15-stage `dStage` deterministic state machine (`GetFrameHeader`, `StoreFrameHeader`, `Init`,
//!   `GetBlockHeader`, `StoreBlockHeader`, `CopyDirect`, `GetBlockChecksum`, `GetCBlock`,
//!   `StoreCBlock`, `FlushOut`, `GetSuffix`, `StoreSuffix`, `GetSFrameSize`, `StoreSFrameSize`,
//!   `SkipSkippable`).
//! - Re-entrancy safe `std::io::Read` stream decoding supporting arbitrary micro-buffers (1B..4MB).
//! - Automatic concatenated multi-frame continuation (`cat f1.lz4 f2.lz4 > multi.lz4`).
//! - Skippable metadata frame filtering and payload bypass.
//! - 64KB sliding dictionary window maintenance with resident memory strictly $\le 5\text{MB}$.

use crate::checksum::{xxh32, Xxh32Hasher};
use crate::codecs::lz4::constants::{
    is_lz4_frame_magic, is_lz4_legacy_magic, is_lz4_skippable_magic, FrameDescriptor,
};
use std::hash::Hasher;
use std::io::{self, Read};

// MARK: - Native C LZ4 Dictionary Decompression Binding

extern "C" {
    fn LZ4_decompress_safe_usingDict(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        dst_capacity: libc::c_int,
        dict_start: *const libc::c_char,
        dict_size: libc::c_int,
    ) -> libc::c_int;
}

// MARK: - 15-Stage Decompression State Machine

/// Deterministic 15-stage LZ4 Frame decoding states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DStage {
    /// Reading 4-byte frame magic number or encountering stream EOF.
    #[default]
    GetFrameHeader = 0,
    /// Buffering frame descriptor bytes (FLG, BD, optional ContentSize, DictID, HC).
    StoreFrameHeader = 1,
    /// Initializing frame context, block sizes, and checksum hashers.
    Init = 2,
    /// Transitioning to read next block header.
    GetBlockHeader = 3,
    /// Buffering 4-byte block size header or detecting EndMark.
    StoreBlockHeader = 4,
    /// Streaming direct uncompressed payload bytes.
    CopyDirect = 5,
    /// Reading and verifying 4-byte block XXH32 checksum.
    GetBlockChecksum = 6,
    /// Reading compressed block payload.
    GetCBlock = 7,
    /// Buffering compressed block payload chunks.
    StoreCBlock = 8,
    /// Delivering decompressed payload to caller and sliding dictionary.
    FlushOut = 9,
    /// Transitioning to read 4-byte content checksum suffix.
    GetSuffix = 10,
    /// Buffering and verifying 4-byte content XXH32 checksum suffix.
    StoreSuffix = 11,
    /// Transitioning to read 4-byte skippable frame payload length.
    GetSFrameSize = 12,
    /// Buffering 4-byte skippable frame payload length.
    StoreSFrameSize = 13,
    /// Skipping skippable metadata frame payload bytes.
    SkipSkippable = 14,
}

// MARK: - Lz4FrameDecoder

/// Streaming LZ4 Frame decompressor implementing `std::io::Read`.
pub struct Lz4FrameDecoder<R: Read> {
    reader: R,
    stage: DStage,
    descriptor: Option<FrameDescriptor>,
    content_hasher: Option<Xxh32Hasher>,
    content_size_accum: u64,
    block_max_size: usize,
    dict_history: Vec<u8>,
    tmp_in: Vec<u8>,
    tmp_in_target: usize,
    decomp_buf: Vec<u8>,
    decomp_pos: usize,
    curr_block_size: usize,
    curr_block_uncompressed: bool,
    curr_block_checksum: u32,
    skippable_remaining: usize,
    frames_decoded: usize,
    stream_ended: bool,
}

impl<R: Read> Lz4FrameDecoder<R> {
    /// Creates a new `Lz4FrameDecoder` wrapping a `Read` stream.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            stage: DStage::GetFrameHeader,
            descriptor: None,
            content_hasher: None,
            content_size_accum: 0,
            block_max_size: 64 * 1024,
            dict_history: Vec::new(),
            tmp_in: Vec::with_capacity(4096),
            tmp_in_target: 4,
            decomp_buf: Vec::new(),
            decomp_pos: 0,
            curr_block_size: 0,
            curr_block_uncompressed: false,
            curr_block_checksum: 0,
            skippable_remaining: 0,
            frames_decoded: 0,
            stream_ended: false,
        }
    }

    /// Returns the current decompression state stage.
    #[inline]
    pub fn stage(&self) -> DStage {
        self.stage
    }

    /// Returns the number of complete frames decoded so far.
    #[inline]
    pub fn frames_decoded(&self) -> usize {
        self.frames_decoded
    }

    /// Returns the parsed `FrameDescriptor` of the current active frame, if any.
    #[inline]
    pub fn current_descriptor(&self) -> Option<&FrameDescriptor> {
        self.descriptor.as_ref()
    }

    /// Unwraps and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Returns a mutable reference to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Reads up to `tmp_in_target` bytes into `tmp_in` from the underlying reader.
    fn read_tmp_in(&mut self) -> io::Result<()> {
        while self.tmp_in.len() < self.tmp_in_target {
            let needed = self.tmp_in_target - self.tmp_in.len();
            let mut stack_buf = [0u8; 4096];
            let to_read = needed.min(stack_buf.len());
            let n = self.reader.read(&mut stack_buf[..to_read])?;
            if n == 0 {
                break;
            }
            self.tmp_in.extend_from_slice(&stack_buf[..n]);
        }
        Ok(())
    }

    /// Updates 64KB sliding dictionary history (`LZ4F_updateDict`).
    fn update_dict(dict: &mut Vec<u8>, block_decompressed: &[u8]) {
        const MAX_DICT_SIZE: usize = 65536;
        if block_decompressed.len() >= MAX_DICT_SIZE {
            dict.clear();
            dict.extend_from_slice(&block_decompressed[block_decompressed.len() - MAX_DICT_SIZE..]);
        } else {
            let available_space = MAX_DICT_SIZE.saturating_sub(block_decompressed.len());
            if dict.len() > available_space {
                let overflow = dict.len() - available_space;
                dict.drain(0..overflow);
            }
            dict.extend_from_slice(block_decompressed);
        }
    }
}

impl<R: Read> Read for Lz4FrameDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            // 1. Deliver unconsumed decompressed data
            if self.decomp_pos < self.decomp_buf.len() {
                let avail = self.decomp_buf.len() - self.decomp_pos;
                let to_copy = avail.min(buf.len());
                buf[..to_copy].copy_from_slice(
                    &self.decomp_buf[self.decomp_pos..self.decomp_pos + to_copy],
                );
                self.decomp_pos += to_copy;
                return Ok(to_copy);
            }

            // Drain decomp_buf if fully consumed
            if !self.decomp_buf.is_empty() {
                self.decomp_buf.clear();
                self.decomp_pos = 0;
            }

            if self.stream_ended {
                return Ok(0);
            }

            // Step the 15-stage state machine
            match self.stage {
                DStage::GetFrameHeader => {
                    self.tmp_in_target = 4;
                    self.read_tmp_in()?;
                    if self.tmp_in.is_empty() {
                        self.stream_ended = true;
                        return Ok(0);
                    }
                    if self.tmp_in.len() < 4 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated LZ4 magic number",
                        ));
                    }
                    let magic = u32::from_le_bytes([
                        self.tmp_in[0],
                        self.tmp_in[1],
                        self.tmp_in[2],
                        self.tmp_in[3],
                    ]);
                    self.tmp_in.clear();

                    if is_lz4_frame_magic(magic) {
                        self.stage = DStage::StoreFrameHeader;
                        self.tmp_in_target = 3;
                    } else if is_lz4_skippable_magic(magic) {
                        self.stage = DStage::GetSFrameSize;
                        self.tmp_in_target = 4;
                    } else if is_lz4_legacy_magic(magic) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "legacy LZ4 format unsupported in frame decoder",
                        ));
                    } else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("invalid LZ4 frame magic: 0x{:08X}", magic),
                        ));
                    }
                }

                DStage::StoreFrameHeader => {
                    if self.tmp_in.len() < 2 {
                        self.tmp_in_target = 2;
                        self.read_tmp_in()?;
                        if self.tmp_in.len() < 2 {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "truncated frame header descriptor",
                            ));
                        }
                    }

                    let flg = self.tmp_in[0];
                    let has_content_size = ((flg >> 3) & 0x01) != 0;
                    let has_dict_id = (flg & 0x01) != 0;
                    let mut required_len = 2usize;
                    if has_content_size {
                        required_len += 8;
                    }
                    if has_dict_id {
                        required_len += 4;
                    }
                    required_len += 1;

                    self.tmp_in_target = required_len;
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < required_len {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated LZ4 frame header",
                        ));
                    }

                    let (desc, _consumed) = FrameDescriptor::parse(&self.tmp_in).map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("header parse error: {:?}", e),
                        )
                    })?;
                    self.descriptor = Some(desc);
                    self.tmp_in.clear();
                    self.stage = DStage::Init;
                }

                DStage::Init => {
                    let desc = self.descriptor.as_ref().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing descriptor in init")
                    })?;
                    self.block_max_size = desc.block_max_size.max_bytes();
                    if desc.content_checksum {
                        self.content_hasher = Some(Xxh32Hasher::new());
                    } else {
                        self.content_hasher = None;
                    }
                    self.content_size_accum = 0;
                    if desc.block_independence.is_independent() {
                        self.dict_history.clear();
                    }
                    self.stage = DStage::GetBlockHeader;
                    self.tmp_in_target = 4;
                }

                DStage::GetBlockHeader => {
                    self.tmp_in_target = 4;
                    self.tmp_in.clear();
                    self.stage = DStage::StoreBlockHeader;
                }

                DStage::StoreBlockHeader => {
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < 4 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated block header",
                        ));
                    }
                    let raw_header = u32::from_le_bytes([
                        self.tmp_in[0],
                        self.tmp_in[1],
                        self.tmp_in[2],
                        self.tmp_in[3],
                    ]);
                    self.tmp_in.clear();

                    if raw_header == 0 {
                        let desc = self.descriptor.as_ref().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing descriptor at endmark",
                            )
                        })?;
                        if let Some(expected_size) = desc.content_size {
                            if self.content_size_accum != expected_size {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "content size mismatch: expected {}, actual {}",
                                        expected_size, self.content_size_accum
                                    ),
                                ));
                            }
                        }
                        if desc.content_checksum {
                            self.stage = DStage::GetSuffix;
                            self.tmp_in_target = 4;
                        } else {
                            self.frames_decoded += 1;
                            self.descriptor = None;
                            self.stage = DStage::GetFrameHeader;
                        }
                    } else {
                        let is_uncompressed = (raw_header & 0x8000_0000) != 0;
                        let block_size = (raw_header & 0x7FFF_FFFF) as usize;
                        if block_size > self.block_max_size {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "block size {} exceeds max block size {}",
                                    block_size, self.block_max_size
                                ),
                            ));
                        }
                        self.curr_block_size = block_size;
                        self.curr_block_uncompressed = is_uncompressed;
                        self.tmp_in_target = block_size;

                        if is_uncompressed {
                            self.stage = DStage::CopyDirect;
                        } else {
                            self.stage = DStage::StoreCBlock;
                        }
                    }
                }

                DStage::CopyDirect => {
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < self.curr_block_size {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated raw block payload",
                        ));
                    }
                    let desc = self.descriptor.as_ref().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing descriptor")
                    })?;
                    if desc.block_checksum {
                        self.curr_block_checksum = xxh32(&self.tmp_in, 0);
                    }
                    if let Some(hasher) = &mut self.content_hasher {
                        hasher.write(&self.tmp_in);
                    }
                    self.content_size_accum = self
                        .content_size_accum
                        .saturating_add(self.tmp_in.len() as u64);

                    if !desc.block_independence.is_independent() {
                        Self::update_dict(&mut self.dict_history, &self.tmp_in);
                    }

                    self.decomp_buf.clear();
                    self.decomp_buf.extend_from_slice(&self.tmp_in);
                    self.decomp_pos = 0;
                    self.tmp_in.clear();

                    if desc.block_checksum {
                        self.stage = DStage::GetBlockChecksum;
                        self.tmp_in_target = 4;
                    } else {
                        self.stage = DStage::FlushOut;
                    }
                }

                DStage::StoreCBlock | DStage::GetCBlock => {
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < self.curr_block_size {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated compressed block payload",
                        ));
                    }
                    let desc = self.descriptor.as_ref().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing descriptor")
                    })?;
                    if desc.block_checksum {
                        self.curr_block_checksum = xxh32(&self.tmp_in, 0);
                    }

                    let mut decompressed = vec![0u8; self.block_max_size];
                    let (dict_ptr, dict_len) = if desc.block_independence.is_independent()
                        || self.dict_history.is_empty()
                    {
                        (std::ptr::null(), 0 as libc::c_int)
                    } else {
                        (
                            self.dict_history.as_ptr() as *const libc::c_char,
                            self.dict_history.len() as libc::c_int,
                        )
                    };

                    let res = unsafe {
                        LZ4_decompress_safe_usingDict(
                            self.tmp_in.as_ptr() as *const libc::c_char,
                            decompressed.as_mut_ptr() as *mut libc::c_char,
                            self.tmp_in.len() as libc::c_int,
                            decompressed.len() as libc::c_int,
                            dict_ptr,
                            dict_len,
                        )
                    };
                    if res < 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "lz4 decompression failed: code {} (block size {}, dict len {})",
                                res,
                                self.tmp_in.len(),
                                dict_len
                            ),
                        ));
                    }
                    let written = res as usize;

                    decompressed.truncate(written);
                    if let Some(hasher) = &mut self.content_hasher {
                        hasher.write(&decompressed);
                    }
                    self.content_size_accum =
                        self.content_size_accum.saturating_add(written as u64);

                    if !desc.block_independence.is_independent() {
                        Self::update_dict(&mut self.dict_history, &decompressed);
                    }

                    self.decomp_buf = decompressed;
                    self.decomp_pos = 0;
                    self.tmp_in.clear();

                    if desc.block_checksum {
                        self.stage = DStage::GetBlockChecksum;
                        self.tmp_in_target = 4;
                    } else {
                        self.stage = DStage::FlushOut;
                    }
                }

                DStage::GetBlockChecksum => {
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < 4 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated block checksum",
                        ));
                    }
                    let expected_bc = u32::from_le_bytes([
                        self.tmp_in[0],
                        self.tmp_in[1],
                        self.tmp_in[2],
                        self.tmp_in[3],
                    ]);
                    self.tmp_in.clear();

                    if expected_bc != self.curr_block_checksum {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "block checksum mismatch: expected 0x{:08X}, actual 0x{:08X}",
                                expected_bc, self.curr_block_checksum
                            ),
                        ));
                    }

                    self.stage = DStage::FlushOut;
                }

                DStage::FlushOut => {
                    self.stage = DStage::GetBlockHeader;
                }

                DStage::GetSuffix => {
                    self.tmp_in_target = 4;
                    self.stage = DStage::StoreSuffix;
                }

                DStage::StoreSuffix => {
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < 4 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated content checksum",
                        ));
                    }
                    let expected_cc = u32::from_le_bytes([
                        self.tmp_in[0],
                        self.tmp_in[1],
                        self.tmp_in[2],
                        self.tmp_in[3],
                    ]);
                    self.tmp_in.clear();

                    if let Some(hasher) = &self.content_hasher {
                        let actual_cc = hasher.digest();
                        if expected_cc != actual_cc {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "content checksum mismatch: expected 0x{:08X}, actual 0x{:08X}",
                                    expected_cc, actual_cc
                                ),
                            ));
                        }
                    }

                    self.frames_decoded += 1;
                    self.descriptor = None;
                    self.stage = DStage::GetFrameHeader;
                }

                DStage::GetSFrameSize => {
                    self.tmp_in_target = 4;
                    self.stage = DStage::StoreSFrameSize;
                }

                DStage::StoreSFrameSize => {
                    self.read_tmp_in()?;
                    if self.tmp_in.len() < 4 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "truncated skippable frame size",
                        ));
                    }
                    let s_size = u32::from_le_bytes([
                        self.tmp_in[0],
                        self.tmp_in[1],
                        self.tmp_in[2],
                        self.tmp_in[3],
                    ]) as usize;
                    self.tmp_in.clear();
                    self.skippable_remaining = s_size;
                    self.stage = DStage::SkipSkippable;
                }

                DStage::SkipSkippable => {
                    let mut discard = [0u8; 4096];
                    while self.skippable_remaining > 0 {
                        let to_read = self.skippable_remaining.min(discard.len());
                        let n = self.reader.read(&mut discard[..to_read])?;
                        if n == 0 {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "truncated skippable payload",
                            ));
                        }
                        self.skippable_remaining -= n;
                    }
                    self.stage = DStage::GetFrameHeader;
                }
            }
        }
    }
}
