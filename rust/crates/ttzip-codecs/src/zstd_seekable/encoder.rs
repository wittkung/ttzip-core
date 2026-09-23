// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Seekable Zstandard table encoding and streaming writer.

use super::types::{
    SeekTableEntry, SeekableError, SEEKABLE_FOOTER_SIZE, SEEKABLE_MAGIC_NUMBER,
    SEEK_TABLE_FLAG_CHECKSUM, SKIPPABLE_HEADER_SIZE, SKIPPABLE_MAGIC_NUMBER,
};
use crate::codecs::zstd::{zstd_compress, zstd_compress_bound};
use std::io::{self, Write};

/// Seek Table encoder that records frames and serializes the trailing Skippable Frame index.
#[derive(Debug, Clone, Default)]
pub struct SeekTableEncoder {
    entries: Vec<SeekTableEntry>,
    include_checksums: bool,
    total_c_size: u64,
    total_d_size: u64,
}

impl SeekTableEncoder {
    /// Creates a new `SeekTableEncoder`.
    pub fn new(include_checksums: bool) -> Self {
        Self {
            entries: Vec::new(),
            include_checksums,
            total_c_size: 0,
            total_d_size: 0,
        }
    }

    /// Records a newly written compressed frame into the seek table index.
    pub fn add_frame(
        &mut self,
        c_size: u32,
        d_size: u32,
        checksum: Option<u32>,
    ) -> Result<(), SeekableError> {
        let chk = checksum.unwrap_or(0);
        self.entries.push(SeekTableEntry {
            c_size,
            d_size,
            checksum: chk,
        });
        self.total_c_size = self.total_c_size.saturating_add(c_size as u64);
        self.total_d_size = self.total_d_size.saturating_add(d_size as u64);
        Ok(())
    }

    /// Returns the number of indexed frames.
    #[inline]
    pub fn frame_count(&self) -> usize {
        self.entries.len()
    }

    /// Total compressed payload size across all frames.
    #[inline]
    pub fn total_compressed_size(&self) -> u64 {
        self.total_c_size
    }

    /// Total uncompressed payload size across all frames.
    #[inline]
    pub fn total_decompressed_size(&self) -> u64 {
        self.total_d_size
    }

    /// Computes the exact byte length of the encoded Seek Table (including skippable header).
    #[inline]
    pub fn encoded_table_size(&self) -> usize {
        let entry_size = if self.include_checksums { 12 } else { 8 };
        SKIPPABLE_HEADER_SIZE + (self.entries.len() * entry_size) + SEEKABLE_FOOTER_SIZE
    }

    /// Serializes the entire Seek Table into a byte buffer (header + entries + footer).
    pub fn serialize_to_vec(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_table_size());
        let _ = self.write_seek_table(&mut buf);
        buf
    }

    /// Writes the Skippable Frame Seek Table to the provided output sink.
    pub fn write_seek_table<W: Write>(&self, writer: &mut W) -> Result<usize, SeekableError> {
        let entry_size = if self.include_checksums { 12 } else { 8 };
        let payload_size = (self.entries.len() * entry_size) + SEEKABLE_FOOTER_SIZE;

        // 1. Skippable Frame Header (8 bytes)
        writer.write_all(&SKIPPABLE_MAGIC_NUMBER.to_le_bytes())?;
        writer.write_all(&(payload_size as u32).to_le_bytes())?;

        // 2. Entries
        for entry in &self.entries {
            writer.write_all(&entry.c_size.to_le_bytes())?;
            writer.write_all(&entry.d_size.to_le_bytes())?;
            if self.include_checksums {
                writer.write_all(&entry.checksum.to_le_bytes())?;
            }
        }

        // 3. Footer (9 bytes)
        let descriptor = if self.include_checksums {
            SEEK_TABLE_FLAG_CHECKSUM
        } else {
            0
        };
        writer.write_all(&(self.entries.len() as u32).to_le_bytes())?;
        writer.write_all(&[descriptor])?;
        writer.write_all(&SEEKABLE_MAGIC_NUMBER.to_le_bytes())?;

        Ok(SKIPPABLE_HEADER_SIZE + payload_size)
    }
}

/// High-level seekable archive compressor that chunks uncompressed input into frames
/// and automatically appends the Skippable Frame Seek Table.
pub struct ZstdSeekableWriter<W: Write> {
    writer: W,
    encoder: SeekTableEncoder,
    chunk_size: usize,
    compression_level: i32,
    buffer: Vec<u8>,
}

impl<W: Write> ZstdSeekableWriter<W> {
    /// Creates a new `ZstdSeekableWriter` with specified frame chunk size and compression level.
    pub fn new(
        writer: W,
        chunk_size: usize,
        compression_level: i32,
        include_checksums: bool,
    ) -> Self {
        Self {
            writer,
            encoder: SeekTableEncoder::new(include_checksums),
            chunk_size: chunk_size.max(4096),
            compression_level,
            buffer: Vec::with_capacity(chunk_size),
        }
    }

    fn flush_chunk(&mut self) -> Result<(), SeekableError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let d_size = self.buffer.len();
        let bound = zstd_compress_bound(d_size);
        let mut comp_buf = vec![0u8; bound];

        let c_size = zstd_compress(&self.buffer, &mut comp_buf, self.compression_level)
            .map_err(|e| io::Error::other(format!("{:?}", e)))?;

        self.writer.write_all(&comp_buf[..c_size])?;

        let checksum = if self.encoder.include_checksums {
            Some(crc32fast::hash(&self.buffer))
        } else {
            None
        };

        self.encoder.add_frame(c_size as u32, d_size as u32, checksum)?;
        self.buffer.clear();
        Ok(())
    }

    /// Finalizes the archive by flushing any pending uncompressed data and writing the Seek Table.
    pub fn finish(mut self) -> Result<(W, usize), SeekableError> {
        self.flush_chunk()?;
        let table_bytes = self.encoder.write_seek_table(&mut self.writer)?;
        self.writer.flush()?;
        Ok((self.writer, table_bytes))
    }
}

impl<W: Write> Write for ZstdSeekableWriter<W> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let total_written = buf.len();
        while !buf.is_empty() {
            let space = self.chunk_size.saturating_sub(self.buffer.len());
            let take = space.min(buf.len());
            self.buffer.extend_from_slice(&buf[..take]);
            buf = &buf[take..];

            if self.buffer.len() >= self.chunk_size {
                self.flush_chunk()
                    .map_err(|e| io::Error::other(e.to_string()))?;
            }
        }
        Ok(total_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
