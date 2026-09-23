// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Seekable Zstandard table decoding and random-access frame decompression.

use super::types::{
    SeekFrameInfo, SeekTableEntry, SeekableError, SEEKABLE_FOOTER_SIZE, SEEKABLE_MAGIC_NUMBER,
    SEEK_TABLE_FLAG_CHECKSUM, SKIPPABLE_HEADER_SIZE, SKIPPABLE_MAGIC_NUMBER,
};
use crate::codecs::zstd::zstd_decompress;
use std::io::{self, Read, Seek, SeekFrom};

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
