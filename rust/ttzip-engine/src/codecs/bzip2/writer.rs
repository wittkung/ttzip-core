// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Rust streaming Bzip2 writer implementing `std::io::Write`.

use std::io::{self, Write};
use super::block::{encode_bzip2_block, BitWriter, BZIP2_EOS_MAGIC};
use super::crc::Bzip2CombinedCrc;

/// Streaming Bzip2 compressor wrapping any `Write` sink.
pub struct Bzip2Writer<W: Write> {
    inner: Option<W>,
    level: u8,
    block_size_limit: usize,
    current_block: Vec<u8>,
    writer: BitWriter,
    combined_crc: Bzip2CombinedCrc,
    header_written: bool,
}

impl<W: Write> Bzip2Writer<W> {
    /// Creates a new streaming Bzip2 writer with compression level (1..=9).
    pub fn new(inner: W, level: u32) -> Self {
        let lvl = level.clamp(1, 9) as u8;
        let block_size_limit = (lvl as usize) * 100_000;

        Self {
            inner: Some(inner),
            level: lvl,
            block_size_limit,
            current_block: Vec::with_capacity(block_size_limit),
            writer: BitWriter::new(),
            combined_crc: Bzip2CombinedCrc::new(),
            header_written: false,
        }
    }

    fn write_header(&mut self) {
        if !self.header_written {
            self.writer.write_bits(b'B' as u32, 8);
            self.writer.write_bits(b'Z' as u32, 8);
            self.writer.write_bits(b'h' as u32, 8);
            self.writer.write_bits((b'0' + self.level) as u32, 8);
            self.header_written = true;
        }
    }

    fn flush_block(&mut self) -> io::Result<()> {
        if self.current_block.is_empty() {
            return Ok(());
        }

        self.write_header();
        encode_bzip2_block(&self.current_block, &mut self.writer, &mut self.combined_crc)
            .map_err(|e| io::Error::other(format!("Block encode error: {:?}", e)))?;
        self.current_block.clear();
        Ok(())
    }

    /// Finishes compression, emits stream trailer, flushes to underlying sink, and returns the sink.
    pub fn finish(mut self) -> io::Result<W> {
        self.flush_block()?;
        self.write_header();

        // Emit Stream Trailer (48-bit sqrt(pi) magic + combined CRC)
        for &b in &BZIP2_EOS_MAGIC {
            self.writer.write_bits(b as u32, 8);
        }
        self.writer.write_bits(self.combined_crc.finalize(), 32);
        self.writer.flush_to_byte_boundary();

        let mut inner = self.inner.take().unwrap();
        inner.write_all(&self.writer.buf)?;
        inner.flush()?;
        Ok(inner)
    }
}

impl<W: Write> Write for Bzip2Writer<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < buf.len() {
            let space = self.block_size_limit - self.current_block.len();
            let to_add = space.min(buf.len() - written);
            self.current_block.extend_from_slice(&buf[written..written + to_add]);
            written += to_add;

            if self.current_block.len() >= self.block_size_limit {
                self.flush_block()?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
