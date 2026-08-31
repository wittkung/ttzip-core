// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 extensible output function (XOF) and root node finalization.
//!
//! Provides the [`Output`] intermediate representation representing a completed chunk
//! or parent node prior to root decision, and the [`OutputReader`] stream cursor
//! providing $O(1)$ random seeking and streaming byte extraction.

use zeroize::{Zeroize, ZeroizeOnDrop};
use super::compress::{
    compress_in_place_mut, compress_xof, le_bytes_from_words_32,
};
use super::constants::{BLOCK_LEN, ROOT};

/// An intermediate node output descriptor capturing state prior to chaining value or root expansion.
///
/// In the BLAKE3 specification, every chunk or parent node produces an `Output` struct
/// which can either be truncated to a 32-byte chaining value for internal tree hashing,
/// or finalized with the `ROOT` domain flag to produce an arbitrary-length XOF stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Zeroize)]
pub struct Output {
    /// 256-bit chaining value input to this node.
    pub input_chaining_value: [u32; 8],
    /// Up to 64 bytes of payload message data in the final block.
    pub block: [u8; 64],
    /// Number of valid payload bytes within `block` (0..=64).
    pub block_len: u8,
    /// Chunk index counter or total block sequence offset.
    pub counter: u64,
    /// Domain separation and chunk position flags.
    pub flags: u8,
}

impl Output {
    /// Creates a new `Output` descriptor.
    #[inline]
    pub const fn new(
        input_chaining_value: [u32; 8],
        block: [u8; 64],
        block_len: u8,
        counter: u64,
        flags: u8,
    ) -> Self {
        Self {
            input_chaining_value,
            block,
            block_len,
            counter,
            flags,
        }
    }

    /// Extracts the 32-byte (256-bit) intermediate chaining value (CV) for non-root parent compression.
    ///
    /// Computes the first 8 words of the compression function without the `ROOT` flag.
    #[inline]
    pub fn chaining_value(&self) -> [u8; 32] {
        let mut cv = self.input_chaining_value;
        compress_in_place_mut(
            &mut cv,
            &self.block,
            self.block_len,
            self.counter,
            self.flags,
        );
        le_bytes_from_words_32(&cv)
    }

    /// Extracts the 8-word intermediate chaining value as native `[u32; 8]` words.
    #[inline]
    pub fn chaining_value_words(&self) -> [u32; 8] {
        let mut cv = self.input_chaining_value;
        compress_in_place_mut(
            &mut cv,
            &self.block,
            self.block_len,
            self.counter,
            self.flags,
        );
        cv
    }

    /// Extracts the canonical 32-byte root hash digest by applying the `ROOT` domain flag with counter 0.
    #[inline]
    pub fn root_hash(&self) -> [u8; 32] {
        let mut cv = self.input_chaining_value;
        compress_in_place_mut(
            &mut cv,
            &self.block,
            self.block_len,
            0,
            self.flags | ROOT,
        );
        le_bytes_from_words_32(&cv)
    }

    /// Generates a full 64-byte (512-bit) XOF output block at the specified `output_block_counter`.
    ///
    /// The `ROOT` flag is combined with existing flags, and all 16 state words after permutation
    /// are XORed and converted to little-endian bytes.
    #[inline]
    pub fn root_output_block(&self, output_block_counter: u64) -> [u8; 64] {
        compress_xof(
            &self.input_chaining_value,
            &self.block,
            self.block_len,
            output_block_counter,
            self.flags | ROOT,
        )
    }
}

/// An incremental, seekable reader stream for BLAKE3 extensible output (XOF).
///
/// Implements arbitrary byte extraction up to $2^{64}-1$ bytes with $O(1)$ random seeking,
/// internal 64-byte block caching, and [`std::io::Read`] / [`std::io::Seek`] trait support.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct OutputReader {
    /// Underlying output node descriptor.
    pub output: Output,
    /// 64-byte single-block cache.
    pub buf: [u8; 64],
    /// Current read offset within `buf` (0..=64). 64 indicates buffer is exhausted.
    pub buf_pos: u8,
    /// Next output block counter to generate from the compression function.
    pub block_counter: u64,
}

impl OutputReader {
    /// Creates a new `OutputReader` initialized at stream position 0.
    #[inline]
    pub const fn new(output: Output) -> Self {
        Self {
            output,
            buf: [0u8; 64],
            buf_pos: 64,
            block_counter: 0,
        }
    }

    /// Returns a reference to the underlying [`Output`] descriptor.
    #[inline]
    pub const fn output(&self) -> &Output {
        &self.output
    }

    /// Returns the current logical byte offset within the output stream.
    #[inline]
    pub fn position(&self) -> u64 {
        if self.block_counter == 0 {
            0
        } else {
            (self.block_counter - 1)
                .wrapping_mul(BLOCK_LEN as u64)
                .wrapping_add(self.buf_pos as u64)
        }
    }

    /// Seeks to an absolute byte position within the infinite XOF stream in $O(1)$ time.
    ///
    /// If the target position falls within the currently buffered 64-byte block, the buffer
    /// is reused without invoking the compression function.
    pub fn seek(&mut self, position: u64) {
        let target_block_counter = position / (BLOCK_LEN as u64);
        let target_offset = (position % (BLOCK_LEN as u64)) as u8;

        if self.block_counter > 0 && target_block_counter == self.block_counter - 1 {
            self.buf_pos = target_offset;
        } else {
            self.buf = self.output.root_output_block(target_block_counter);
            self.block_counter = target_block_counter.wrapping_add(1);
            self.buf_pos = target_offset;
        }
    }

    /// Explicitly sets the current byte position in the stream (alias for [`Self::seek`]).
    #[inline]
    pub fn set_position(&mut self, position: u64) {
        self.seek(position);
    }

    /// Squeezes an arbitrary number of bytes from the XOF stream into `out`.
    ///
    /// This method exhausts the remaining cached bytes in `buf`, compresses full 64-byte
    /// blocks directly into the destination slice when available, and caches the remainder.
    pub fn fill(&mut self, mut out: &mut [u8]) {
        while !out.is_empty() {
            if self.buf_pos < (BLOCK_LEN as u8) {
                let available = ((BLOCK_LEN as u8) - self.buf_pos) as usize;
                let take = out.len().min(available);
                out[..take].copy_from_slice(&self.buf[self.buf_pos as usize..self.buf_pos as usize + take]);
                self.buf_pos += take as u8;
                out = &mut out[take..];
                if out.is_empty() {
                    return;
                }
            }

            // Buffer is exhausted (buf_pos == 64). Fast path for direct full-block compression.
            if out.len() >= BLOCK_LEN {
                let block = self.output.root_output_block(self.block_counter);
                self.block_counter = self.block_counter.wrapping_add(1);
                out[..BLOCK_LEN].copy_from_slice(&block);
                out = &mut out[BLOCK_LEN..];
            } else {
                self.buf = self.output.root_output_block(self.block_counter);
                self.block_counter = self.block_counter.wrapping_add(1);
                self.buf_pos = 0;
                let take = out.len();
                out[..take].copy_from_slice(&self.buf[..take]);
                self.buf_pos = take as u8;
                return;
            }
        }
    }
}

impl std::io::Read for OutputReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.fill(buf);
        Ok(buf.len())
    }
}

impl std::io::Seek for OutputReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let max_position = u64::MAX as i128;
        let current_pos = self.position() as i128;
        let target_position: i128 = match pos {
            std::io::SeekFrom::Start(offset) => offset as i128,
            std::io::SeekFrom::Current(offset) => current_pos + offset as i128,
            std::io::SeekFrom::End(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "seek from end is not supported on infinite BLAKE3 XOF stream",
                ));
            }
        };

        if target_position < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cannot seek before start of stream",
            ));
        }

        let clamped = target_position.min(max_position) as u64;
        OutputReader::seek(self, clamped);
        Ok(self.position())
    }
}
