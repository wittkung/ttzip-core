// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` multi-block streaming writer and stream compression engine.
//!
//! Implements a streaming `std::io::Write` adapter for producing compliant LZFSE multi-block
//! container streams with automatic 256KB chunk slicing, multi-tiered format routing
//! (LZFSE `bvx2`, LZVN `bvxn`, and Raw `bvx-`), and clean `finish()` / `Drop` lifecycle semantics
//! terminating on `bvx$` end-of-stream markers.

use super::block::BvxMagic;
use super::encoder::{lzfse_encode_block, LzfseMatchTable};
use super::lzvn_encoder::{lzvn_compress, lzvn_compress_bound};
use crate::types::TTZipStatus;
use std::io::{self, Error, ErrorKind, Write};

/// Standard 256KB block chunk size for LZFSE container compression.
pub const LZFSE_BLOCK_CHUNK_SIZE: usize = 256 * 1024;

/// Standard chunk size alias.
pub const LZFSE_CHUNK_SIZE: usize = LZFSE_BLOCK_CHUNK_SIZE;

/// Default threshold in bytes below which LZVN compression is preferred over LZFSE.
pub const DEFAULT_LZVN_THRESHOLD: usize = 4096;

/// Compresses a buffer into a multi-block Apple LZFSE container stream terminating with `bvx$`.
///
/// Chunks payloads exceeding 256KB into independent LZFSE blocks.
pub fn lzfse_compress_stream(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let mut dst = Vec::with_capacity(src.len() / 2 + 1024);
    let mut writer = LzfseWriter::new(&mut dst);
    writer
        .write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    writer
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(dst)
}

/// Streaming `std::io::Write` adapter that compresses written byte streams into Apple LZFSE format.
pub struct LzfseWriter<W: Write> {
    inner: Option<W>,
    buffer: Vec<u8>,
    table: LzfseMatchTable,
    lzvn_threshold: usize,
    finished: bool,
}

impl<W: Write> LzfseWriter<W> {
    /// Creates a new `LzfseWriter` wrapping the underlying stream `inner` with default LZVN threshold (4096B).
    pub fn new(inner: W) -> Self {
        Self::with_lzvn_threshold(inner, DEFAULT_LZVN_THRESHOLD)
    }

    /// Creates a new `LzfseWriter` with a custom LZVN threshold limit.
    pub fn with_lzvn_threshold(inner: W, threshold: usize) -> Self {
        Self {
            inner: Some(inner),
            buffer: Vec::with_capacity(LZFSE_BLOCK_CHUNK_SIZE),
            table: LzfseMatchTable::new(),
            lzvn_threshold: threshold,
            finished: false,
        }
    }

    /// Flushes any buffered bytes as an LZFSE / LZVN / Raw block to the inner writer.
    fn flush_current_block(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| io::Error::other("Writer already closed"))?;

        let n = self.buffer.len();
        let mut block_bytes = Vec::with_capacity(n + 1024);

        if n < 8 {
            // Raw uncompressed block (bvx-)
            block_bytes.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
            block_bytes.extend_from_slice(&(n as u32).to_le_bytes());
            block_bytes.extend_from_slice(&self.buffer);
        } else if n < self.lzvn_threshold {
            // Attempt LZVN compression (bvxn)
            let bound = lzvn_compress_bound(n);
            let mut lzvn_out = vec![0u8; bound];
            match lzvn_compress(&self.buffer, &mut lzvn_out) {
                Ok(written) if written > 0 && written < n => {
                    block_bytes.extend_from_slice(&BvxMagic::CompressedLZVN.as_bytes());
                    block_bytes.extend_from_slice(&(n as u32).to_le_bytes());
                    block_bytes.extend_from_slice(&(written as u32).to_le_bytes());
                    block_bytes.extend_from_slice(&lzvn_out[..written]);
                }
                _ => {
                    // Fall back to Raw uncompressed block
                    block_bytes.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
                    block_bytes.extend_from_slice(&(n as u32).to_le_bytes());
                    block_bytes.extend_from_slice(&self.buffer);
                }
            }
        } else {
            // Encode LZFSE block (bvx2 or bvx-)
            self.table.reset();
            lzfse_encode_block(&self.buffer, &mut self.table, &mut block_bytes).map_err(|e| {
                io::Error::other(format!("LZFSE encode error: {e:?}"))
            })?;
        }

        inner.write_all(&block_bytes)?;
        self.buffer.clear();
        Ok(())
    }

    /// Finalizes the LZFSE container stream by emitting remaining blocks and the `bvx$` marker.
    pub fn finish(mut self) -> io::Result<W> {
        if !self.finished {
            self.flush_current_block()?;
            if let Some(inner) = self.inner.as_mut() {
                inner.write_all(&BvxMagic::EndOfStream.as_bytes())?;
                inner.flush()?;
            }
            self.finished = true;
        }

        self.inner
            .take()
            .ok_or_else(|| io::Error::other("Writer already closed"))
    }

    /// Returns an immutable reference to the inner writer.
    pub fn get_ref(&self) -> io::Result<&W> {
        self.inner
            .as_ref()
            .ok_or_else(|| io::Error::other("Writer already closed"))
    }

    /// Returns a mutable reference to the inner writer.
    pub fn get_mut(&mut self) -> io::Result<&mut W> {
        self.inner
            .as_mut()
            .ok_or_else(|| io::Error::other("Writer already closed"))
    }
}

impl<W: Write> Write for LzfseWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.finished {
            return Err(Error::new(
                ErrorKind::BrokenPipe,
                "cannot write to finished LzfseWriter",
            ));
        }

        let mut written = 0;
        while written < buf.len() {
            let space = LZFSE_BLOCK_CHUNK_SIZE.saturating_sub(self.buffer.len());
            let to_copy = space.min(buf.len() - written);

            self.buffer
                .extend_from_slice(&buf[written..written + to_copy]);
            written += to_copy;

            if self.buffer.len() >= LZFSE_BLOCK_CHUNK_SIZE {
                self.flush_current_block()?;
            }
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.flush_current_block()?;
        if let Some(inner) = self.inner.as_mut() {
            inner.flush()?;
        }
        Ok(())
    }
}

impl<W: Write> Drop for LzfseWriter<W> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.flush_current_block();
            if let Some(inner) = self.inner.as_mut() {
                let _ = inner.write_all(&BvxMagic::EndOfStream.as_bytes());
                let _ = inner.flush();
            }
            self.finished = true;
        }
    }
}
