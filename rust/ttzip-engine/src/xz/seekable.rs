// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput random-access seekable reader for indexed XZ archives.

use std::io::{self, Read, Seek, SeekFrom};

use crate::xz::block::{pad_to_4, XzBlockHeader};
use crate::xz::header::{XzStreamFooter, XzStreamHeader};
use crate::xz::index::XzStreamIndex;
use crate::xz::payload::decompress_block_payload;
use crate::xz::types::{XzError, XZ_FOOTER_MAGIC, XZ_STREAM_HEADER_SIZE};

/// Default memory limit for XZ seekable reading (256 MiB).
pub const DEFAULT_XZ_SEEK_MEMLIMIT: u64 = 256 * 1024 * 1024;

/// High-performance random-access seekable reader for indexed XZ archives.
pub struct XzSeekableReader<R: Read + Seek> {
    inner: R,
    stream_header: XzStreamHeader,
    stream_footer: XzStreamFooter,
    index: XzStreamIndex,
    current_pos: u64,
    cached_block_index: Option<usize>,
    cached_block_data: Vec<u8>,
    memlimit: u64,
}

impl<R: Read + Seek> XzSeekableReader<R> {
    /// Creates a new `XzSeekableReader` by parsing trailing Stream Footer and Index.
    pub fn new(inner: R) -> Result<Self, XzError> {
        Self::with_memlimit(inner, DEFAULT_XZ_SEEK_MEMLIMIT)
    }

    /// Creates an `XzSeekableReader` with custom memory limit.
    pub fn with_memlimit(mut inner: R, memlimit: u64) -> Result<Self, XzError> {
        let total_file_size = inner.seek(SeekFrom::End(0))?;
        if total_file_size < 24 {
            return Err(XzError::TruncatedData {
                expected: 24,
                actual: total_file_size as usize,
            });
        }

        // 1. Read Stream Header
        inner.seek(SeekFrom::Start(0))?;
        let mut header_buf = [0u8; 12];
        inner.read_exact(&mut header_buf)?;
        let stream_header = XzStreamHeader::parse(&header_buf)?;

        // 2. Scan backwards for Stream Footer
        let mut footer_pos = total_file_size - 12;
        let mut footer_buf = [0u8; 12];
        let stream_footer = loop {
            inner.seek(SeekFrom::Start(footer_pos))?;
            inner.read_exact(&mut footer_buf)?;
            if footer_buf[10..12] == XZ_FOOTER_MAGIC {
                break XzStreamFooter::parse_and_verify_header(&footer_buf, &stream_header.flags)?;
            }
            if footer_pos < 16 {
                return Err(XzError::InvalidFooterMagic {
                    expected: XZ_FOOTER_MAGIC,
                    actual: [footer_buf[10], footer_buf[11]],
                });
            }
            footer_pos = footer_pos.saturating_sub(4);
        };

        // 3. Read Index using parse_from_footer
        let (index, _) = XzStreamIndex::parse_from_footer(&mut inner, footer_pos + 12)?;

        Ok(Self {
            inner,
            stream_header,
            stream_footer,
            index,
            current_pos: 0,
            cached_block_index: None,
            cached_block_data: Vec::new(),
            memlimit,
        })
    }

    /// Returns the parsed stream index.
    #[inline]
    pub fn index(&self) -> &XzStreamIndex {
        &self.index
    }

    /// Returns total uncompressed size across all blocks.
    #[inline]
    pub fn total_uncompressed_size(&self) -> u64 {
        self.index.total_uncompressed_size
    }

    /// Returns the Stream Header.
    #[inline]
    pub fn stream_header(&self) -> &XzStreamHeader {
        &self.stream_header
    }

    /// Returns the Stream Footer.
    #[inline]
    pub fn stream_footer(&self) -> &XzStreamFooter {
        &self.stream_footer
    }

    /// Returns current logical uncompressed seek position.
    #[inline]
    pub fn current_position(&self) -> u64 {
        self.current_pos
    }

    /// Loads and decompresses block at index `block_idx` into the internal cache.
    fn load_block(&mut self, block_idx: usize, stream_offset: u64) -> Result<(), XzError> {
        if self.cached_block_index == Some(block_idx) {
            return Ok(());
        }

        self.inner.seek(SeekFrom::Start(stream_offset))?;

        let mut byte0 = [0u8; 1];
        self.inner.read_exact(&mut byte0)?;
        let header_size = (byte0[0] as usize + 1) * 4;

        let mut header_buf = vec![0u8; header_size];
        header_buf[0] = byte0[0];
        self.inner.read_exact(&mut header_buf[1..])?;

        let check_type = self.stream_header.flags.check_type;
        let block_header = XzBlockHeader::parse(&header_buf, check_type)?;

        let record = self.index.records.get(block_idx).ok_or(
            XzError::SizeOverflow("Block index out of bounds in index record table"),
        )?;

        let check_size = check_type.check_size();
        let comp_size = record
            .unpadded_size
            .checked_sub(header_size as u64 + check_size as u64)
            .ok_or(XzError::InvalidUnpaddedSize(record.unpadded_size))?;

        let mut comp_buf = vec![0u8; comp_size as usize];
        self.inner.read_exact(&mut comp_buf)?;

        let pad_len = pad_to_4(comp_size);
        if pad_len > 0 {
            let mut pad_buf = vec![0u8; pad_len];
            self.inner.read_exact(&mut pad_buf)?;
            if pad_buf.iter().any(|&b| b != 0x00) {
                return Err(XzError::NonZeroIndexPadding);
            }
        }

        let mut check_buf = vec![0u8; check_size];
        if check_size > 0 {
            self.inner.read_exact(&mut check_buf)?;
        }

        let uncompressed = decompress_block_payload(
            &comp_buf,
            &block_header,
            check_type,
            &check_buf,
            self.memlimit,
        )?;

        self.cached_block_data = uncompressed;
        self.cached_block_index = Some(block_idx);
        Ok(())
    }
}

impl<R: Read + Seek> Read for XzSeekableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let total_size = self.total_uncompressed_size();
        if self.current_pos >= total_size {
            return Ok(0);
        }

        let (block_idx, comp_offset, uncomp_offset) = self
            .index
            .locate_block(self.current_pos)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "Seek offset out of range")
            })?;

        let stream_offset = (XZ_STREAM_HEADER_SIZE as u64) + comp_offset;
        self.load_block(block_idx, stream_offset)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        let offset_in_block = self.current_pos - uncomp_offset;
        let available = self.cached_block_data.len().saturating_sub(offset_in_block as usize);
        let to_copy = buf.len().min(available);

        if to_copy == 0 {
            return Ok(0);
        }

        let start = offset_in_block as usize;
        buf[..to_copy].copy_from_slice(&self.cached_block_data[start..start + to_copy]);
        self.current_pos += to_copy as u64;
        Ok(to_copy)
    }
}

impl<R: Read + Seek> Seek for XzSeekableReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let total_size = self.total_uncompressed_size() as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => (self.current_pos as i64) + offset,
            SeekFrom::End(offset) => total_size + offset,
        };

        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot seek to a negative position",
            ));
        }

        self.current_pos = new_pos as u64;
        Ok(self.current_pos)
    }
}
