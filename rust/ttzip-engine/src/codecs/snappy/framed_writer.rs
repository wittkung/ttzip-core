// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust streaming chunked compressor conforming to the Snappy framing format (.sz).
//!
//! Encapsulates a 64KB uncompressed staging buffer and an L1-cache-resident hash table.
//! Emits 10-byte stream identifiers (`0xFF`), compressed frames (`0x00`), and uncompressed fallback
//! frames (`0x01`) guarded by Castagnoli CRC-32C checksums with zero heap reallocations per chunk.

use crate::codecs::snappy::crc::{crc32c, mask_crc32c};
use crate::codecs::snappy::error::SnappyError;
use crate::codecs::snappy::frame::{SNAPPY_MAX_CHUNK_SIZE, SNAPPY_STREAM_IDENTIFIER};
use crate::codecs::snappy::hash_table::SnappyHashTable;
use crate::codecs::snappy::raw_encoder::{max_compressed_len, raw_compress_fragment, write_varint};
use std::io::Write;

/// Streaming Snappy framing encoder implementing `std::io::Write`.
///
/// Buffers incoming uncompressed data up to 64KB (`SNAPPY_MAX_CHUNK_SIZE`). Once full,
/// or upon explicit `flush()` / `finish()`, compresses the chunk using `raw_compress_fragment`.
/// If compressed output achieves >= 12.5% reduction (`compressed_len < uncompressed_len - (uncompressed_len >> 3)`),
/// emits a compressed chunk (`0x00`); otherwise, falls back to emitting an uncompressed chunk (`0x01`).
pub struct SnappyFramedWriter<W: Write> {
    /// Underlying writer destination. Wrapped in `Option` to enable safe ownership transfer upon `finish()`.
    writer: Option<W>,
    /// Staging buffer for uncompressed bytes (capacity up to 64KB).
    chunk_uncompressed_buf: Vec<u8>,
    /// Scratch buffer for raw compressed output.
    chunk_compressed_buf: Vec<u8>,
    /// Tracks whether the initial 10-byte stream identifier chunk (`0xFF`) has been emitted.
    header_emitted: bool,
    /// L1-resident 14-bit hash table reused across chunk compression cycles.
    table: SnappyHashTable,
}

impl<W: Write> SnappyFramedWriter<W> {
    /// Creates a new `SnappyFramedWriter` wrapping the destination `writer`.
    ///
    /// Pre-allocates a 64KB uncompressed buffer, a compressed scratch buffer, and an L1 hash table.
    pub fn new(writer: W) -> Self {
        Self {
            writer: Some(writer),
            chunk_uncompressed_buf: Vec::with_capacity(SNAPPY_MAX_CHUNK_SIZE),
            chunk_compressed_buf: vec![0u8; max_compressed_len(SNAPPY_MAX_CHUNK_SIZE)],
            header_emitted: false,
            table: SnappyHashTable::new(),
        }
    }

    /// Returns a reference to the underlying writer if not yet finished.
    pub fn get_ref(&self) -> Option<&W> {
        self.writer.as_ref()
    }

    /// Returns a mutable reference to the underlying writer if not yet finished.
    pub fn get_mut(&mut self) -> Option<&mut W> {
        self.writer.as_mut()
    }

    /// Ensures that the mandatory 10-byte Snappy stream identifier chunk (`0xFF`) has been written.
    #[inline]
    fn ensure_header_emitted(&mut self) -> Result<(), SnappyError> {
        if !self.header_emitted {
            let writer = self.writer.as_mut().ok_or_else(|| {
                SnappyError::InvalidParam("Underlying writer already consumed".to_string())
            })?;
            writer.write_all(&SNAPPY_STREAM_IDENTIFIER).map_err(|e| {
                SnappyError::DecompressionFailed(format!("Failed to write stream identifier: {e}"))
            })?;
            self.header_emitted = true;
        }
        Ok(())
    }

    /// Compresses and flushes the currently buffered uncompressed chunk.
    ///
    /// Emits either a `0x00` (compressed) or `0x01` (uncompressed fallback) chunk with
    /// masked Castagnoli CRC-32C verification header, then resets internal uncompressed buffer.
    pub fn flush_chunk(&mut self) -> Result<(), SnappyError> {
        let uncompressed_len = self.chunk_uncompressed_buf.len();
        if uncompressed_len == 0 {
            return Ok(());
        }

        self.ensure_header_emitted()?;

        // Calculate Castagnoli CRC-32C over uncompressed chunk and apply Snappy framing mask
        let masked_crc = mask_crc32c(crc32c(&self.chunk_uncompressed_buf));

        // Ensure compressed scratch buffer has sufficient bound
        let max_out = max_compressed_len(uncompressed_len);
        if self.chunk_compressed_buf.len() < max_out {
            self.chunk_compressed_buf.resize(max_out, 0);
        }

        // Write uncompressed length LEB128 varint header for raw Snappy block payload
        let varint_len = write_varint(uncompressed_len, &mut self.chunk_compressed_buf);

        let compressed_fragment_len = raw_compress_fragment(
            &self.chunk_uncompressed_buf,
            &mut self.chunk_compressed_buf[varint_len..],
            &mut self.table,
        )?;
        let compressed_len = varint_len + compressed_fragment_len;

        // Compressed chunk threshold per specification: must achieve >= 12.5% compression (size < 0.875 * uncompressed_len)
        let threshold = uncompressed_len.saturating_sub(uncompressed_len >> 3);
        let writer = self.writer.as_mut().ok_or_else(|| {
            SnappyError::InvalidParam("Underlying writer already consumed".to_string())
        })?;

        if compressed_len < threshold {
            // Chunk type 0x00: Compressed data
            let chunk_len = 4 + compressed_len;
            let mut header = [0u8; 8];
            header[0] = 0x00;
            header[1] = (chunk_len & 0xFF) as u8;
            header[2] = ((chunk_len >> 8) & 0xFF) as u8;
            header[3] = ((chunk_len >> 16) & 0xFF) as u8;
            header[4..8].copy_from_slice(&masked_crc.to_le_bytes());

            writer.write_all(&header).map_err(|e| {
                SnappyError::DecompressionFailed(format!("Failed to write compressed chunk header: {e}"))
            })?;
            writer.write_all(&self.chunk_compressed_buf[..compressed_len]).map_err(|e| {
                SnappyError::DecompressionFailed(format!("Failed to write compressed chunk payload: {e}"))
            })?;
        } else {
            // Chunk type 0x01: Uncompressed data fallback
            let chunk_len = 4 + uncompressed_len;
            let mut header = [0u8; 8];
            header[0] = 0x01;
            header[1] = (chunk_len & 0xFF) as u8;
            header[2] = ((chunk_len >> 8) & 0xFF) as u8;
            header[3] = ((chunk_len >> 16) & 0xFF) as u8;
            header[4..8].copy_from_slice(&masked_crc.to_le_bytes());

            writer.write_all(&header).map_err(|e| {
                SnappyError::DecompressionFailed(format!("Failed to write uncompressed chunk header: {e}"))
            })?;
            writer.write_all(&self.chunk_uncompressed_buf).map_err(|e| {
                SnappyError::DecompressionFailed(format!("Failed to write uncompressed chunk payload: {e}"))
            })?;
        }

        self.chunk_uncompressed_buf.clear();
        Ok(())
    }

    /// Flushes any pending chunk data, flushes the underlying writer, and returns the inner writer.
    pub fn finish(mut self) -> Result<W, SnappyError> {
        self.ensure_header_emitted()?;
        if !self.chunk_uncompressed_buf.is_empty() {
            self.flush_chunk()?;
        }
        let mut writer = self.writer.take().ok_or_else(|| {
            SnappyError::InvalidParam("Underlying writer already consumed".to_string())
        })?;
        writer.flush().map_err(|e| {
            SnappyError::DecompressionFailed(format!("Failed to flush underlying writer: {e}"))
        })?;
        Ok(writer)
    }

    /// Consumes the wrapper and returns the inner writer after finishing compression.
    pub fn into_inner(self) -> Result<W, SnappyError> {
        self.finish()
    }
}

impl<W: Write> Write for SnappyFramedWriter<W> {
    fn write(&mut self, mut buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.ensure_header_emitted()
            .map_err(std::io::Error::other)?;

        let total_written = buf.len();
        while !buf.is_empty() {
            let available = SNAPPY_MAX_CHUNK_SIZE.saturating_sub(self.chunk_uncompressed_buf.len());
            if available == 0 {
                self.flush_chunk()
                    .map_err(std::io::Error::other)?;
                continue;
            }
            let to_copy = buf.len().min(available);
            self.chunk_uncompressed_buf.extend_from_slice(&buf[..to_copy]);
            buf = &buf[to_copy..];

            if self.chunk_uncompressed_buf.len() >= SNAPPY_MAX_CHUNK_SIZE {
                self.flush_chunk()
                    .map_err(std::io::Error::other)?;
            }
        }
        Ok(total_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ensure_header_emitted()
            .map_err(std::io::Error::other)?;
        if !self.chunk_uncompressed_buf.is_empty() {
            self.flush_chunk()
                .map_err(std::io::Error::other)?;
        }
        if let Some(writer) = self.writer.as_mut() {
            writer.flush()?;
        }
        Ok(())
    }
}

impl<W: Write> Drop for SnappyFramedWriter<W> {
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = self.ensure_header_emitted();
            if !self.chunk_uncompressed_buf.is_empty() {
                let _ = self.flush_chunk();
            }
            if let Some(writer) = self.writer.as_mut() {
                let _ = writer.flush();
            }
        }
    }
}
