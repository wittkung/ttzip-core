// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Stream Index variable-length block metadata serialization, parsing,
//! arithmetic overflow circuit breakers, and seekable binary-search jump table.
//!
//! Complies strictly with Section 4 (Index) of the .xz File Format Specification.

use std::io::{Read, Seek, SeekFrom};

use crate::crypto::crc32::crc32_fast;
use crate::xz::block::{aligned_block_size, pad_to_4};
use crate::xz::header::XzStreamFooter;
use crate::xz::types::{
    XzError, XZ_BACKWARD_SIZE_UNIT, XZ_MAX_BACKWARD_SIZE, XZ_STREAM_FOOTER_SIZE,
    XZ_STREAM_HEADER_SIZE,
};
use crate::xz::vli::{
    decode_vli, encode_vli, vli_size, XzVliError, VLI_MAX_BYTES, XZ_VLI_MAX,
};

/// Record metadata representing a single compressed Block within an XZ Stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzRecord {
    /// Unpadded size in bytes: Block Header + Compressed Data + Integrity Check.
    pub unpadded_size: u64,
    /// Uncompressed size in bytes of the block payload.
    pub uncompressed_size: u64,
}

/// Backward-compatible type alias for [`XzRecord`].
pub type XzBlockRecord = XzRecord;


impl XzRecord {
    /// Creates a new `XzRecord` describing a block.
    #[inline]
    pub const fn new(unpadded_size: u64, uncompressed_size: u64) -> Self {
        Self {
            unpadded_size,
            uncompressed_size,
        }
    }

    /// Returns the 4-byte aligned total block size in the physical stream.
    #[inline]
    pub const fn total_block_size(&self) -> u64 {
        aligned_block_size(self.unpadded_size)
    }

    /// Computes the trailing block padding size in bytes (0..=3).
    #[inline]
    pub const fn block_padding_size(&self) -> usize {
        pad_to_4(self.unpadded_size)
    }
}

/// In-memory representation of the XZ Stream Index and seekable O(log N) jump table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XzStreamIndex {
    /// Ordered list of records describing each block in the stream.
    pub records: Vec<XzRecord>,
    /// Prefix sums of uncompressed byte offsets at the start of each block.
    pub uncompressed_prefix_sums: Vec<u64>,
    /// Prefix sums of physical compressed byte offsets at the start of each block.
    pub compressed_prefix_sums: Vec<u64>,
    /// Total cumulative uncompressed size across all blocks.
    pub total_uncompressed_size: u64,
    /// Total cumulative unpadded size across all blocks.
    pub total_unpadded_size: u64,
}

impl XzStreamIndex {
    /// Creates a new empty `XzStreamIndex`.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty `XzStreamIndex` with pre-allocated vector capacities.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Vec::with_capacity(capacity),
            uncompressed_prefix_sums: Vec::with_capacity(capacity),
            compressed_prefix_sums: Vec::with_capacity(capacity),
            total_uncompressed_size: 0,
            total_unpadded_size: 0,
        }
    }

    /// Returns `true` if the index contains no records.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the number of records contained in this index.
    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns the total physical compressed size occupied by all blocks in the stream.
    #[inline]
    pub fn total_compressed_size(&self) -> u64 {
        if self.records.is_empty() {
            0
        } else {
            let last_idx = self.records.len() - 1;
            self.compressed_prefix_sums[last_idx] + self.records[last_idx].total_block_size()
        }
    }

    /// Appends a new block record to the index with strict overflow circuit breaking.
    #[inline]
    pub fn add_record(&mut self, record: XzBlockRecord) -> Result<(), XzError> {
        self.append(record.unpadded_size, record.uncompressed_size)
    }

    /// Appends a new block record to the index with strict overflow circuit breaking.
    ///
    /// # Defense Gates
    /// 1. `unpadded_size` must be non-zero and `<= XZ_VLI_MAX`.
    /// 2. `uncompressed_size` must be `<= XZ_VLI_MAX`.
    /// 3. Cumulative totals and prefix sums must not overflow 64-bit bounds or exceed `XZ_VLI_MAX`.
    pub fn append(&mut self, unpadded_size: u64, uncompressed_size: u64) -> Result<(), XzError> {
        if unpadded_size == 0 || unpadded_size > XZ_VLI_MAX {
            return Err(XzError::InvalidUnpaddedSize(unpadded_size));
        }
        if uncompressed_size > XZ_VLI_MAX {
            return Err(XzError::InvalidVli(XzVliError::ValueTooLarge {
                val: uncompressed_size,
            }));
        }

        let new_total_unpadded = self
            .total_unpadded_size
            .checked_add(unpadded_size)
            .ok_or(XzError::SizeOverflow("Total unpadded size overflow"))?;
        if new_total_unpadded > XZ_VLI_MAX {
            return Err(XzError::SizeOverflow("Total unpadded size exceeds VLI_MAX"));
        }

        let new_total_uncompressed = self
            .total_uncompressed_size
            .checked_add(uncompressed_size)
            .ok_or(XzError::SizeOverflow("Total uncompressed size overflow"))?;
        if new_total_uncompressed > XZ_VLI_MAX {
            return Err(XzError::SizeOverflow("Total uncompressed size exceeds VLI_MAX"));
        }

        let record = XzRecord::new(unpadded_size, uncompressed_size);

        if self.records.is_empty() {
            self.uncompressed_prefix_sums.push(0);
            self.compressed_prefix_sums.push(0);
        } else {
            let last_idx = self.records.len() - 1;
            let next_u_sum = self.uncompressed_prefix_sums[last_idx]
                .checked_add(self.records[last_idx].uncompressed_size)
                .ok_or(XzError::SizeOverflow("Uncompressed prefix sum overflow"))?;
            let next_c_sum = self.compressed_prefix_sums[last_idx]
                .checked_add(self.records[last_idx].total_block_size())
                .ok_or(XzError::SizeOverflow("Compressed prefix sum overflow"))?;

            self.uncompressed_prefix_sums.push(next_u_sum);
            self.compressed_prefix_sums.push(next_c_sum);
        }

        self.total_unpadded_size = new_total_unpadded;
        self.total_uncompressed_size = new_total_uncompressed;
        self.records.push(record);

        Ok(())
    }

    /// Computes the exact encoded byte size of the Index field.
    ///
    /// Includes Index Indicator (1 byte), Number of Records (VLI), all Record fields (2 VLIs each),
    /// Index Padding (0..=3 zero bytes), and Index CRC32 (4 bytes).
    /// The return value is guaranteed to be a multiple of 4.
    pub fn index_size(&self) -> Result<usize, XzError> {
        let num_records = self.records.len() as u64;
        let mut raw_len = 1usize + vli_size(num_records)?;

        for rec in &self.records {
            let u_len = vli_size(rec.unpadded_size)?;
            let c_len = vli_size(rec.uncompressed_size)?;
            raw_len = raw_len
                .checked_add(u_len + c_len)
                .ok_or(XzError::SizeOverflow("Index payload length overflow"))?;
        }

        let pad = pad_to_4(raw_len as u64);
        let total_size = raw_len
            .checked_add(pad + 4)
            .ok_or(XzError::SizeOverflow("Total encoded index size overflow"))?;

        if (total_size as u64) > XZ_MAX_BACKWARD_SIZE {
            return Err(XzError::InvalidBackwardSize(total_size as u64));
        }

        Ok(total_size)
    }

    /// Serializes this `XzStreamIndex` into a compliant XZ Index byte buffer.
    pub fn encode(&self) -> Result<Vec<u8>, XzError> {
        let target_size = self.index_size()?;
        let mut buf = Vec::with_capacity(target_size);

        // 1. Index Indicator (0x00)
        buf.push(0x00);

        // 2. Number of Records (VLI)
        let mut vli_scratch = [0u8; VLI_MAX_BYTES];
        let mut pos = 0;
        let len = encode_vli(self.records.len() as u64, &mut vli_scratch, &mut pos)?;
        buf.extend_from_slice(&vli_scratch[..len]);

        // 3. List of Records
        for rec in &self.records {
            pos = 0;
            let u_len = encode_vli(rec.unpadded_size, &mut vli_scratch, &mut pos)?;
            buf.extend_from_slice(&vli_scratch[..u_len]);

            pos = 0;
            let c_len = encode_vli(rec.uncompressed_size, &mut vli_scratch, &mut pos)?;
            buf.extend_from_slice(&vli_scratch[..c_len]);
        }

        // 4. Index Padding (0..=3 zero bytes to align payload to 4 bytes)
        let pad = pad_to_4(buf.len() as u64);
        buf.resize(buf.len() + pad, 0x00);

        // 5. Index CRC32 (4 bytes, little-endian)
        let crc = crc32_fast(0, &buf);
        buf.extend_from_slice(&crc.to_le_bytes());

        debug_assert_eq!(buf.len(), target_size);
        Ok(buf)
    }

    /// Parses and validates an `XzStreamIndex` from a raw byte buffer.
    ///
    /// Enforces Index Indicator `0x00`, CRC32 checksum, non-zero padding checks,
    /// and canonical record count bounds.
    pub fn parse(input: &[u8]) -> Result<Self, XzError> {
        if input.len() < 8 {
            return Err(XzError::TruncatedData {
                expected: 8,
                actual: input.len(),
            });
        }

        if !input.len().is_multiple_of(4) {
            return Err(XzError::InvalidBackwardSize(input.len() as u64));
        }

        let payload_len = input.len() - 4;
        let expected_crc = u32::from_le_bytes(
            input[payload_len..]
                .try_into()
                .map_err(|_| XzError::TruncatedData {
                    expected: 4,
                    actual: 0,
                })?,
        );
        let actual_crc = crc32_fast(0, &input[..payload_len]);

        if expected_crc != actual_crc {
            return Err(XzError::IndexCrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        if input[0] != 0x00 {
            return Err(XzError::InvalidIndexIndicator(input[0]));
        }

        let mut pos = 1;
        let num_records_u64 = decode_vli(&input[..payload_len], &mut pos)?;
        if num_records_u64 > (usize::MAX as u64) {
            return Err(XzError::SizeOverflow("Record count exceeds system address limit"));
        }
        let num_records = num_records_u64 as usize;

        let mut index = Self::with_capacity(num_records);

        for _ in 0..num_records {
            let unpadded_size = decode_vli(&input[..payload_len], &mut pos)?;
            let uncompressed_size = decode_vli(&input[..payload_len], &mut pos)?;
            index.append(unpadded_size, uncompressed_size)?;
        }

        let padding_slice = &input[pos..payload_len];
        if padding_slice.len() > 3 {
            return Err(XzError::IndexRecordCountMismatch {
                expected: num_records,
                actual: index.len(),
            });
        }

        if padding_slice.iter().any(|&b| b != 0x00) {
            return Err(XzError::NonZeroIndexPadding);
        }

        Ok(index)
    }

    /// Parses and validates an `XzStreamIndex` directly from a streaming reader.
    pub fn parse_stream<R: Read>(reader: &mut R) -> Result<(Self, usize), XzError> {
        use crate::xz::vli::decode_vli_stream;

        let mut indicator_buf = [0u8; 1];
        reader.read_exact(&mut indicator_buf)?;
        if indicator_buf[0] != 0x00 {
            return Err(XzError::InvalidIndexIndicator(indicator_buf[0]));
        }

        let mut payload = vec![0x00];
        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(&indicator_buf);

        let num_records_u64 = decode_vli_stream(reader)?;
        let mut vli_buf = [0u8; 9];
        let mut vpos = 0;
        let len = encode_vli(num_records_u64, &mut vli_buf, &mut vpos)?;
        crc_hasher.update(&vli_buf[..len]);
        payload.extend_from_slice(&vli_buf[..len]);

        let num_records = num_records_u64 as usize;
        let mut index = Self::with_capacity(num_records);

        for _ in 0..num_records {
            let unpadded = decode_vli_stream(reader)?;
            vpos = 0;
            let len1 = encode_vli(unpadded, &mut vli_buf, &mut vpos)?;
            crc_hasher.update(&vli_buf[..len1]);
            payload.extend_from_slice(&vli_buf[..len1]);

            let uncompressed = decode_vli_stream(reader)?;
            vpos = 0;
            let len2 = encode_vli(uncompressed, &mut vli_buf, &mut vpos)?;
            crc_hasher.update(&vli_buf[..len2]);
            payload.extend_from_slice(&vli_buf[..len2]);

            index.append(unpadded, uncompressed)?;
        }

        let pad_len = pad_to_4(payload.len() as u64);
        if pad_len > 0 {
            let mut pad_buf = vec![0u8; pad_len];
            reader.read_exact(&mut pad_buf)?;
            if pad_buf.iter().any(|&b| b != 0x00) {
                return Err(XzError::NonZeroIndexPadding);
            }
            crc_hasher.update(&pad_buf);
            payload.extend_from_slice(&pad_buf);
        }

        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;
        let expected_crc = u32::from_le_bytes(crc_buf);
        let actual_crc = crc_hasher.finalize();

        if expected_crc != actual_crc {
            return Err(XzError::IndexCrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        let total_consumed = payload.len() + 4;
        Ok((index, total_consumed))
    }

    /// Performs branchless binary-search jump table lookup to locate the target block
    /// containing `target_uncompressed_offset`.
    ///
    /// # Return Value
    /// Returns `Some((block_index, block_compressed_offset, block_uncompressed_offset))` if found,
    /// or `None` if the index is empty or `target_uncompressed_offset >= total_uncompressed_size`.
    pub fn locate_block(&self, target_uncompressed_offset: u64) -> Option<(usize, u64, u64)> {
        if self.records.is_empty() || target_uncompressed_offset >= self.total_uncompressed_size {
            return None;
        }

        // Binary search using partition_point on uncompressed prefix sums
        let block_idx = self
            .uncompressed_prefix_sums
            .partition_point(|&sum| sum <= target_uncompressed_offset)
            .saturating_sub(1);

        if block_idx >= self.records.len() {
            return None;
        }

        let comp_offset = self.compressed_prefix_sums[block_idx];
        let uncomp_offset = self.uncompressed_prefix_sums[block_idx];

        Some((block_idx, comp_offset, uncomp_offset))
    }

    /// Reads and parses an `XzStreamIndex` directly from the Stream Footer by reverse seeking.
    ///
    /// # Arguments
    /// * `reader` - Streaming reader with seek capabilities.
    /// * `footer_end_offset` - Physical byte offset at the end of the Stream Footer
    ///   (usually the end of the file or stream chunk).
    ///
    /// # Returns
    /// A tuple of `(XzStreamIndex, index_start_offset)` where `index_start_offset` is the
    /// physical stream offset at which the Index begins.
    pub fn parse_from_footer<R: Read + Seek>(
        reader: &mut R,
        footer_end_offset: u64,
    ) -> Result<(Self, u64), XzError> {
        const MIN_REQUIRED_STREAM_SIZE: u64 =
            (XZ_STREAM_HEADER_SIZE + 8 + XZ_STREAM_FOOTER_SIZE) as u64;

        if footer_end_offset < MIN_REQUIRED_STREAM_SIZE {
            return Err(XzError::TruncatedData {
                expected: MIN_REQUIRED_STREAM_SIZE as usize,
                actual: footer_end_offset as usize,
            });
        }

        let footer_start_offset = footer_end_offset - (XZ_STREAM_FOOTER_SIZE as u64);
        reader.seek(SeekFrom::Start(footer_start_offset))?;

        let mut footer_bytes = [0u8; XZ_STREAM_FOOTER_SIZE];
        reader.read_exact(&mut footer_bytes)?;

        let footer = XzStreamFooter::parse(&footer_bytes)?;
        let backward_size = footer.backward_size;

        if backward_size > XZ_MAX_BACKWARD_SIZE || (backward_size % XZ_BACKWARD_SIZE_UNIT) != 0 {
            return Err(XzError::InvalidBackwardSize(backward_size));
        }

        let total_min_size =
            (XZ_STREAM_HEADER_SIZE as u64) + backward_size + (XZ_STREAM_FOOTER_SIZE as u64);
        if footer_end_offset < total_min_size {
            return Err(XzError::TruncatedData {
                expected: total_min_size as usize,
                actual: footer_end_offset as usize,
            });
        }

        let index_start_offset = footer_start_offset - backward_size;
        reader.seek(SeekFrom::Start(index_start_offset))?;

        let mut index_buf = vec![0u8; backward_size as usize];
        reader.read_exact(&mut index_buf)?;

        let index = Self::parse(&index_buf)?;
        let computed_size = index.index_size()? as u64;

        if computed_size != backward_size {
            return Err(XzError::BackwardSizeMismatch {
                expected: backward_size,
                actual: computed_size,
            });
        }

        Ok((index, index_start_offset))
    }
}

