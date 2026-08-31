// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Seekable Zstandard Format (`ZstdSeekable`) & Skippable Frame Index Table.
//!
//! Conforms to the official Zstandard Seekable Format Specification:
//! - **Magic Number**: `SEEKABLE_MAGIC_NUMBER = 0x8F92EAB1`
//! - **Skippable Frame Magic**: `SKIPPABLE_MAGIC_NUMBER = 0x184D2A5E`
//! - **Footer**: 9 bytes fixed (`Number_Of_Frames: u32`, `Descriptor: u8`, `Seekable_Magic: u32`)
//! - **Entries**: `c_size: u32`, `d_size: u32`, optional `checksum: u32` (when Descriptor bit 7 is set)
//! - **Random Access**: $\mathcal{O}(\log N)$ binary search prefix indexing for sub-millisecond point seeks.

use crate::codecs::zstd::{zstd_compress, zstd_compress_bound, zstd_decompress};
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Zstandard Seekable Format Magic Number (0x8F92EAB1).
pub const SEEKABLE_MAGIC_NUMBER: u32 = 0x8F92EAB1;

/// Zstandard Skippable Frame Magic Number for Seek Table (0x184D2A5E).
pub const SKIPPABLE_MAGIC_NUMBER: u32 = 0x184D2A5E;

/// Fixed byte size of the Seek Table footer (4 + 1 + 4 = 9 bytes).
pub const SEEKABLE_FOOTER_SIZE: usize = 9;

/// Fixed byte size of the Skippable Frame header (4 + 4 = 8 bytes).
pub const SKIPPABLE_HEADER_SIZE: usize = 8;

/// Descriptor flag: Bit 7 indicates that a 4-byte checksum is present for every frame entry.
pub const SEEK_TABLE_FLAG_CHECKSUM: u8 = 0x80;

/// Default frame chunk size for seekable archive writers (256 KB).
pub const DEFAULT_SEEKABLE_FRAME_SIZE: usize = 256 * 1024;

/// Errors returned during Seek Table generation, parsing, or random-access decompression.
#[derive(Debug, thiserror::Error)]
pub enum SeekableError {
    /// The input archive is smaller than the minimum seek table size.
    #[error("Archive size ({0} bytes) is too small to contain a valid seek table")]
    ArchiveTooSmall(usize),

    /// The Seekable magic number in footer does not match 0x8F92EAB1.
    #[error("Invalid seekable magic number: expected 0x{expected:08X}, found 0x{found:08X}")]
    InvalidSeekableMagic { expected: u32, found: u32 },

    /// The Skippable Frame magic number does not match 0x184D2A5E.
    #[error("Invalid skippable frame magic number: expected 0x{expected:08X}, found 0x{found:08X}")]
    InvalidSkippableMagic { expected: u32, found: u32 },

    /// Skippable frame length declared in header does not match calculated seek table payload size.
    #[error("Mismatched seek table frame size: expected {expected} bytes, declared {declared} bytes")]
    MismatchedFrameSize { expected: u32, declared: u32 },

    /// Frame index is out of bounds.
    #[error("Frame index {index} out of bounds (total frames: {total})")]
    FrameIndexOutOfBounds { index: usize, total: usize },

    /// Decompressed byte offset is beyond the total uncompressed length.
    #[error("Decompressed offset {offset} out of bounds (total decompressed size: {total})")]
    OffsetOutOfBounds { offset: u64, total: u64 },

    /// Decompression failure in the underlying Zstd engine.
    #[error("Decompression failed for frame {frame_index}: {reason}")]
    DecompressionFailed { frame_index: usize, reason: String },

    /// Checksum mismatch for decompressed frame data.
    #[error("Checksum mismatch for frame {frame_index}: expected 0x{expected:08X}, calculated 0x{calculated:08X}")]
    ChecksumMismatch {
        frame_index: usize,
        expected: u32,
        calculated: u32,
    },

    /// Destination buffer is too small for uncompressed output.
    #[error("Destination buffer too small: required {required} bytes, provided {provided} bytes")]
    BufferTooSmall { required: usize, provided: usize },

    /// Underlying I/O error during stream reading/seeking.
    #[error("I/O error during seekable archive operation: {0}")]
    Io(#[from] io::Error),
}

/// Metadata entry describing a single compressed Zstandard frame in the Seek Table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekTableEntry {
    /// Compressed frame size in bytes.
    pub c_size: u32,
    /// Decompressed data size in bytes.
    pub d_size: u32,
    /// Optional 32-bit checksum (CRC-32 / XXH32) of the uncompressed data.
    pub checksum: u32,
}

/// Resolved random-access coordinates for a specific frame in the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekFrameInfo {
    /// 0-indexed frame index.
    pub frame_index: usize,
    /// Byte offset where this frame begins in the compressed archive stream.
    pub c_offset: u64,
    /// Compressed size in bytes.
    pub c_size: u32,
    /// Byte offset of this frame's first uncompressed byte in the logical stream.
    pub d_offset: u64,
    /// Decompressed size in bytes.
    pub d_size: u32,
    /// Optional 32-bit checksum.
    pub checksum: Option<u32>,
}

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

/// Seek Table decoder providing $\mathcal{O}(\log N)$ binary search and random-access frame reads.
#[derive(Debug, Clone)]
pub struct SeekTableDecoder {
    entries: Vec<SeekTableEntry>,
    c_offsets: Vec<u64>,
    d_offsets: Vec<u64>,
    total_c_size: u64,
    total_d_size: u64,
    include_checksums: bool,
    seek_table_start_offset: u64,
}

impl SeekTableDecoder {
    /// Parses the Seek Table from an in-memory archive byte slice.
    pub fn parse_from_slice(archive: &[u8]) -> Result<Self, SeekableError> {
        let mut cursor = io::Cursor::new(archive);
        Self::parse_from_reader(&mut cursor)
    }

    /// Parses the Seek Table from the tail of a seekable reader stream.
    pub fn parse_from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self, SeekableError> {
        let stream_len = reader.seek(SeekFrom::End(0))?;
        let min_required = SKIPPABLE_HEADER_SIZE + SEEKABLE_FOOTER_SIZE;
        if stream_len < min_required as u64 {
            return Err(SeekableError::ArchiveTooSmall(stream_len as usize));
        }

        // 1. Read trailing 9-byte footer
        reader.seek(SeekFrom::End(-(SEEKABLE_FOOTER_SIZE as i64)))?;
        let mut footer_buf = [0u8; SEEKABLE_FOOTER_SIZE];
        reader.read_exact(&mut footer_buf)?;

        let num_frames = u32::from_le_bytes(footer_buf[0..4].try_into().unwrap()) as usize;
        let descriptor = footer_buf[4];
        let magic = u32::from_le_bytes(footer_buf[5..9].try_into().unwrap());

        if magic != SEEKABLE_MAGIC_NUMBER {
            return Err(SeekableError::InvalidSeekableMagic {
                expected: SEEKABLE_MAGIC_NUMBER,
                found: magic,
            });
        }

        let include_checksums = (descriptor & SEEK_TABLE_FLAG_CHECKSUM) != 0;
        let entry_size = if include_checksums { 12 } else { 8 };
        let entries_total_bytes = num_frames
            .checked_mul(entry_size)
            .ok_or(SeekableError::ArchiveTooSmall(0))?;
        let payload_size = entries_total_bytes + SEEKABLE_FOOTER_SIZE;
        let total_seek_frame_size = SKIPPABLE_HEADER_SIZE + payload_size;

        if stream_len < total_seek_frame_size as u64 {
            return Err(SeekableError::ArchiveTooSmall(stream_len as usize));
        }

        let seek_table_start_offset = stream_len - total_seek_frame_size as u64;

        // 2. Read and validate 8-byte Skippable Frame Header
        reader.seek(SeekFrom::Start(seek_table_start_offset))?;
        let mut header_buf = [0u8; SKIPPABLE_HEADER_SIZE];
        reader.read_exact(&mut header_buf)?;

        let skippable_magic = u32::from_le_bytes(header_buf[0..4].try_into().unwrap());
        let declared_frame_size = u32::from_le_bytes(header_buf[4..8].try_into().unwrap());

        if skippable_magic != SKIPPABLE_MAGIC_NUMBER {
            return Err(SeekableError::InvalidSkippableMagic {
                expected: SKIPPABLE_MAGIC_NUMBER,
                found: skippable_magic,
            });
        }

        if declared_frame_size != payload_size as u32 {
            return Err(SeekableError::MismatchedFrameSize {
                expected: payload_size as u32,
                declared: declared_frame_size,
            });
        }

        // 3. Read Entry Table
        let mut raw_entries = vec![0u8; entries_total_bytes];
        reader.read_exact(&mut raw_entries)?;

        let mut entries = Vec::with_capacity(num_frames);
        let mut c_offsets = Vec::with_capacity(num_frames);
        let mut d_offsets = Vec::with_capacity(num_frames);

        let mut cur_c: u64 = 0;
        let mut cur_d: u64 = 0;

        for i in 0..num_frames {
            let offset = i * entry_size;
            let c_sz = u32::from_le_bytes(raw_entries[offset..offset + 4].try_into().unwrap());
            let d_sz = u32::from_le_bytes(raw_entries[offset + 4..offset + 8].try_into().unwrap());
            let checksum = if include_checksums {
                u32::from_le_bytes(raw_entries[offset + 8..offset + 12].try_into().unwrap())
            } else {
                0
            };

            c_offsets.push(cur_c);
            d_offsets.push(cur_d);
            cur_c = cur_c.saturating_add(c_sz as u64);
            cur_d = cur_d.saturating_add(d_sz as u64);

            entries.push(SeekTableEntry {
                c_size: c_sz,
                d_size: d_sz,
                checksum,
            });
        }

        Ok(Self {
            entries,
            c_offsets,
            d_offsets,
            total_c_size: cur_c,
            total_d_size: cur_d,
            include_checksums,
            seek_table_start_offset,
        })
    }

    /// Returns the number of frames indexed in the Seek Table.
    #[inline]
    pub fn frame_count(&self) -> usize {
        self.entries.len()
    }

    /// Total compressed byte size across all data frames.
    #[inline]
    pub fn total_compressed_size(&self) -> u64 {
        self.total_c_size
    }

    /// Total logical uncompressed size across all data frames.
    #[inline]
    pub fn total_decompressed_size(&self) -> u64 {
        self.total_d_size
    }

    /// Returns true if each frame entry contains a 32-bit checksum.
    #[inline]
    pub fn has_checksums(&self) -> bool {
        self.include_checksums
    }

    /// Byte offset where the Skippable Frame Seek Table begins in the archive.
    #[inline]
    pub fn seek_table_start_offset(&self) -> u64 {
        self.seek_table_start_offset
    }

    /// Retrieves frame metadata by 0-based frame index.
    pub fn get_frame(&self, index: usize) -> Option<SeekFrameInfo> {
        if index >= self.entries.len() {
            return None;
        }
        let entry = &self.entries[index];
        Some(SeekFrameInfo {
            frame_index: index,
            c_offset: self.c_offsets[index],
            c_size: entry.c_size,
            d_offset: self.d_offsets[index],
            d_size: entry.d_size,
            checksum: if self.include_checksums {
                Some(entry.checksum)
            } else {
                None
            },
        })
    }

    /// Resolves an uncompressed byte offset to its containing frame index via $\mathcal{O}(\log N)$ binary search.
    pub fn offset_to_frame_index(&self, decompressed_offset: u64) -> Option<usize> {
        if self.entries.is_empty() || decompressed_offset >= self.total_d_size {
            return None;
        }

        let idx = self
            .d_offsets
            .partition_point(|&off| off <= decompressed_offset);
        if idx == 0 {
            Some(0)
        } else {
            Some(idx - 1)
        }
    }

    /// Decompresses a single discrete frame into the provided destination buffer.
    pub fn decompress_frame<R: Read + Seek>(
        &self,
        reader: &mut R,
        frame_index: usize,
        out_buf: &mut [u8],
    ) -> Result<usize, SeekableError> {
        let info = self
            .get_frame(frame_index)
            .ok_or(SeekableError::FrameIndexOutOfBounds {
                index: frame_index,
                total: self.entries.len(),
            })?;

        if out_buf.len() < info.d_size as usize {
            return Err(SeekableError::BufferTooSmall {
                required: info.d_size as usize,
                provided: out_buf.len(),
            });
        }

        reader.seek(SeekFrom::Start(info.c_offset))?;
        let mut comp_buf = vec![0u8; info.c_size as usize];
        reader.read_exact(&mut comp_buf)?;

        let decomp_len = zstd_decompress(&comp_buf, &mut out_buf[..info.d_size as usize])
            .map_err(|e| SeekableError::DecompressionFailed {
                frame_index,
                reason: format!("{:?}", e),
            })?;

        if let Some(expected_chk) = info.checksum {
            let actual_chk = crc32fast::hash(&out_buf[..decomp_len]);
            if actual_chk != expected_chk {
                return Err(SeekableError::ChecksumMismatch {
                    frame_index,
                    expected: expected_chk,
                    calculated: actual_chk,
                });
            }
        }

        Ok(decomp_len)
    }

    /// Randomly seeks and decompresses a continuous uncompressed byte range `[offset, offset + length)`
    /// across one or multiple underlying Zstandard frames.
    pub fn decompress_range<R: Read + Seek>(
        &self,
        reader: &mut R,
        offset: u64,
        length: usize,
        out_buf: &mut [u8],
    ) -> Result<usize, SeekableError> {
        if length == 0 {
            return Ok(0);
        }

        if offset >= self.total_d_size {
            return Err(SeekableError::OffsetOutOfBounds {
                offset,
                total: self.total_d_size,
            });
        }

        let effective_len = length.min((self.total_d_size - offset) as usize);
        if out_buf.len() < effective_len {
            return Err(SeekableError::BufferTooSmall {
                required: effective_len,
                provided: out_buf.len(),
            });
        }

        let start_frame = self
            .offset_to_frame_index(offset)
            .ok_or(SeekableError::OffsetOutOfBounds {
                offset,
                total: self.total_d_size,
            })?;

        let end_offset = offset + effective_len as u64;
        let mut cur_out_pos = 0;
        let mut frame_buf = Vec::new();

        for f_idx in start_frame..self.entries.len() {
            let info = self.get_frame(f_idx).unwrap();
            let frame_start = info.d_offset;
            let frame_end = frame_start + info.d_size as u64;

            if frame_start >= end_offset {
                break;
            }

            if frame_buf.len() < info.d_size as usize {
                frame_buf.resize(info.d_size as usize, 0);
            }

            let decomp_len = self.decompress_frame(reader, f_idx, &mut frame_buf)?;

            let slice_start = if offset > frame_start {
                (offset - frame_start) as usize
            } else {
                0
            };

            let slice_end = if end_offset < frame_end {
                (end_offset - frame_start) as usize
            } else {
                decomp_len
            };

            if slice_start < slice_end {
                let chunk = &frame_buf[slice_start..slice_end];
                out_buf[cur_out_pos..cur_out_pos + chunk.len()].copy_from_slice(chunk);
                cur_out_pos += chunk.len();
            }
        }

        Ok(cur_out_pos)
    }

    /// Decompresses a range directly from an in-memory archive slice.
    pub fn decompress_range_from_slice(
        &self,
        archive: &[u8],
        offset: u64,
        length: usize,
        out_buf: &mut [u8],
    ) -> Result<usize, SeekableError> {
        let mut cursor = io::Cursor::new(archive);
        self.decompress_range(&mut cursor, offset, length, out_buf)
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

/// Streaming random-access reader wrapping an underlying seekable input stream
/// and providing transparent uncompressed reading and seeking.
pub struct ZstdSeekableReader<R: Read + Seek> {
    reader: R,
    decoder: SeekTableDecoder,
    pos: u64,
    current_frame: Option<(usize, Vec<u8>)>,
}

impl<R: Read + Seek> ZstdSeekableReader<R> {
    /// Creates a new `ZstdSeekableReader` by parsing the Seek Table from the end of `reader`.
    pub fn new(mut reader: R) -> Result<Self, SeekableError> {
        let decoder = SeekTableDecoder::parse_from_reader(&mut reader)?;
        Ok(Self {
            reader,
            decoder,
            pos: 0,
            current_frame: None,
        })
    }

    /// Returns a reference to the parsed [`SeekTableDecoder`].
    #[inline]
    pub fn decoder(&self) -> &SeekTableDecoder {
        &self.decoder
    }

    /// Total logical uncompressed size in bytes.
    #[inline]
    pub fn total_size(&self) -> u64 {
        self.decoder.total_decompressed_size()
    }

    /// Current uncompressed read cursor position.
    #[inline]
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Unwraps and returns the underlying stream reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R: Read + Seek> Read for ZstdSeekableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.pos >= self.decoder.total_decompressed_size() {
            return Ok(0);
        }

        let frame_idx = match self.decoder.offset_to_frame_index(self.pos) {
            Some(idx) => idx,
            None => return Ok(0),
        };

        if self.current_frame.as_ref().map(|(idx, _)| *idx) != Some(frame_idx) {
            let info = self.decoder.get_frame(frame_idx).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid frame index")
            })?;
            let mut decomp = vec![0u8; info.d_size as usize];
            self.decoder
                .decompress_frame(&mut self.reader, frame_idx, &mut decomp)
                .map_err(|e| io::Error::other(e.to_string()))?;
            self.current_frame = Some((frame_idx, decomp));
        }

        let info = self.decoder.get_frame(frame_idx).unwrap();
        let (_cached_idx, ref cached_buf) = self.current_frame.as_ref().unwrap();

        let offset_in_frame = (self.pos - info.d_offset) as usize;
        let available = cached_buf.len().saturating_sub(offset_in_frame);
        let to_read = available.min(buf.len());

        buf[..to_read].copy_from_slice(&cached_buf[offset_in_frame..offset_in_frame + to_read]);
        self.pos += to_read as u64;
        Ok(to_read)
    }
}

impl<R: Read + Seek> Seek for ZstdSeekableReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let total = self.decoder.total_decompressed_size() as i64;
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => (self.pos as i64) + off,
            SeekFrom::End(off) => total + off,
        };

        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot seek to negative position",
            ));
        }

        self.pos = new_pos as u64;
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seekable_roundtrip_with_multiple_frames() {
        let mut raw_data = Vec::new();
        for i in 0..10_000 {
            raw_data.extend_from_slice(format!("Line {} of uncompressed text.\n", i).as_bytes());
        }

        let chunk_size = 8192;
        let mut archive_buf = Vec::new();
        let mut writer = ZstdSeekableWriter::new(&mut archive_buf, chunk_size, 3, true);
        writer.write_all(&raw_data).expect("write data");
        let (_, table_size) = writer.finish().expect("finish writer");

        assert!(table_size > SEEKABLE_FOOTER_SIZE);
        assert!(!archive_buf.is_empty());

        let decoder = SeekTableDecoder::parse_from_slice(&archive_buf).expect("parse seek table");
        assert!(decoder.frame_count() > 1);
        assert_eq!(decoder.total_decompressed_size(), raw_data.len() as u64);
        assert!(decoder.has_checksums());

        // Test random-access seek in the middle
        let target_offset = 25_000u64;
        let target_len = 500usize;
        let mut out = vec![0u8; target_len];
        let read_len = decoder
            .decompress_range_from_slice(&archive_buf, target_offset, target_len, &mut out)
            .expect("decompress range");

        assert_eq!(read_len, target_len);
        assert_eq!(
            &out[..],
            &raw_data[target_offset as usize..target_offset as usize + target_len]
        );
    }
}
