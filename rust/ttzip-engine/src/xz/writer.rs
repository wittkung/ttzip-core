// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Container Stream Writer, Adaptive Block Encoder, and BCJ/LZMA2 Filter Pipeline.
//!
//! Implements streaming single-threaded (`XzStreamWriter`) and multi-threaded
//! parallel chunked (`XzParallelStreamWriter`) XZ compression compliant with the
//! .xz File Format Specification (§2 Stream Header/Footer, §3 Block, §4 Index).

use crate::codecs::lzma2::{
    fl2_compress_bound, Fl2CCtx, Fl2CParameter,
};
use crate::xz::block::{
    pad_to_4, XzBlockHeader, XzFilterConfig,
};
use crate::xz::checksum::{XzChecksumEngine, XzChecksumType};
use crate::xz::header::{XzStreamFlags, XzStreamFooter, XzStreamHeader};
use crate::xz::index::{XzBlockRecord, XzStreamIndex};
use crate::xz::types::{XzCheckType, XzError};
use rayon::prelude::*;
use std::io::{self, Write};

use crate::xz::bcj::{BcjArm, BcjArm64, BcjRiscv, BcjX86, BranchFilter};

/// Supported Branch Conversion (BCJ) architecture filters for XZ containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XzBcjType {
    /// x86 / x86_64 32-bit relative CALL and JMP branch target normalization.
    X86,
    /// ARM (32-bit Little-Endian) BL branch target normalization.
    Arm,
    /// ARM64 (AArch64) 26-bit BL and 21-bit ADRP branch target normalization.
    Arm64,
    /// RISC-V (RV32 / RV64) JAL and AUIPC+inst2 branch target normalization.
    Riscv,
}

impl XzBcjType {
    /// Applies the BCJ branch conversion filter in-place to executable byte buffer.
    pub fn encode(&self, data: &mut [u8], ip: u32) -> usize {
        match self {
            Self::X86 => BcjX86::new().encode(data, ip),
            Self::Arm => BcjArm::new().encode(data, ip),
            Self::Arm64 => BcjArm64::new().encode(data, ip),
            Self::Riscv => BcjRiscv::new().encode(data, ip),
        }
    }

    /// Restores original relative branch offsets in-place from normalized absolute addresses.
    pub fn decode(&self, data: &mut [u8], ip: u32) -> usize {
        match self {
            Self::X86 => BcjX86::new().decode(data, ip),
            Self::Arm => BcjArm::new().decode(data, ip),
            Self::Arm64 => BcjArm64::new().decode(data, ip),
            Self::Riscv => BcjRiscv::new().decode(data, ip),
        }
    }
}

/// Compression and container formatting options for XZ stream encoders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XzEncoderOptions {
    /// Integrity check algorithm for all blocks within this stream (default: CRC64).
    pub check_type: XzCheckType,
    /// LZMA2 dictionary size in bytes (default: 8 MB).
    pub dict_size: u32,
    /// Compression preset level (0..=9, default: 6).
    pub preset_level: u32,
    /// Optional BCJ branch conversion filter to chain before LZMA2.
    pub bcj_filter: Option<XzBcjType>,
    /// Target uncompressed block size in bytes (default: 16 MB).
    pub block_size: usize,
}

impl Default for XzEncoderOptions {
    fn default() -> Self {
        Self {
            check_type: XzCheckType::Crc64,
            dict_size: 8 * 1024 * 1024, // 8 MB
            preset_level: 6,
            bcj_filter: None,
            block_size: 16 * 1024 * 1024, // 16 MB
        }
    }
}

impl XzEncoderOptions {
    /// Creates a new `XzEncoderOptions` instance with default parameters.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the integrity check type.
    #[inline]
    pub fn with_check_type(mut self, check_type: XzCheckType) -> Self {
        self.check_type = check_type;
        self
    }

    /// Sets the dictionary size in bytes.
    #[inline]
    pub fn with_dict_size(mut self, dict_size: u32) -> Self {
        self.dict_size = dict_size.max(4096);
        self
    }

    /// Sets the compression preset level (clamped to 0..=9).
    #[inline]
    pub fn with_preset_level(mut self, level: u32) -> Self {
        self.preset_level = level.min(9);
        self
    }

    /// Configures an optional BCJ branch conversion filter.
    #[inline]
    pub fn with_bcj_filter(mut self, bcj: Option<XzBcjType>) -> Self {
        self.bcj_filter = bcj;
        self
    }

    /// Configures a BCJ branch conversion filter.
    #[inline]
    pub fn with_bcj(mut self, bcj: XzBcjType) -> Self {
        self.bcj_filter = Some(bcj);
        self
    }

    /// Sets target uncompressed block size in bytes.
    #[inline]
    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size.max(4096);
        self
    }
}

/// Standalone single XZ Block encoder and filter pipeline executor.
#[derive(Debug, Clone, Copy, Default)]
pub struct XzBlockEncoder;

impl XzBlockEncoder {
    /// Encodes a single uncompressed buffer into a fully formed XZ Block byte buffer.
    ///
    /// The returned tuple contains `(block_bytes, block_record)`.
    pub fn encode_block(
        uncompressed: &[u8],
        options: &XzEncoderOptions,
    ) -> Result<(Vec<u8>, XzBlockRecord), XzError> {
        // 1. Compute integrity checksum over uncompressed payload
        let check_type_enum = match options.check_type {
            XzCheckType::None => XzChecksumType::None,
            XzCheckType::Crc32 => XzChecksumType::Crc32,
            XzCheckType::Crc64 => XzChecksumType::Crc64,
            XzCheckType::Sha256 => XzChecksumType::Sha256,
        };

        let mut check_engine = XzChecksumEngine::new(check_type_enum);
        check_engine.update(uncompressed);
        let check_bytes = check_engine.digest();

        // 2. Apply BCJ filter if configured
        let mut filtered = uncompressed.to_vec();
        if let Some(bcj) = options.bcj_filter {
            bcj.encode(&mut filtered, 0);
        }

        // 3. Compress filtered data with LZMA2
        let compressed_data = if filtered.is_empty() {
            vec![0x00] // LZMA2 End of stream chunk marker
        } else {
            Self::compress_lzma2(&filtered, options)?
        };

        // 4. Build Block Filter chain
        let mut filters = Vec::new();
        if let Some(bcj) = options.bcj_filter {
            match bcj {
                XzBcjType::X86 => filters.push(XzFilterConfig::bcj_x86(None)),
                XzBcjType::Arm => filters.push(XzFilterConfig::bcj_arm(None)),
                XzBcjType::Arm64 => filters.push(XzFilterConfig::bcj_arm64(None)),
                XzBcjType::Riscv => filters.push(XzFilterConfig::bcj_riscv(None)),
            }
        }
        filters.push(XzFilterConfig::lzma2(options.dict_size));

        // 5. Construct and encode Block Header
        let block_header = XzBlockHeader::new(filters, options.check_type)?
            .with_sizes(
                Some(compressed_data.len() as u64),
                Some(uncompressed.len() as u64),
            )?;
        let header_bytes = block_header.encode()?;

        // 6. Compute block padding to 4-byte boundary
        let block_pad = pad_to_4(compressed_data.len() as u64);

        // 7. Assemble final block buffer
        let total_block_len =
            header_bytes.len() + compressed_data.len() + block_pad + check_bytes.len();
        let mut block_buf = Vec::with_capacity(total_block_len);
        block_buf.extend_from_slice(&header_bytes);
        block_buf.extend_from_slice(&compressed_data);
        block_buf.resize(block_buf.len() + block_pad, 0x00);
        block_buf.extend_from_slice(&check_bytes);

        let unpadded_size =
            (header_bytes.len() + compressed_data.len() + check_bytes.len()) as u64;
        let record = XzBlockRecord::new(unpadded_size, uncompressed.len() as u64);

        Ok((block_buf, record))
    }

    /// Compresses a slice into raw LZMA2 chunks using fast-lzma2 context.
    fn compress_lzma2(data: &[u8], options: &XzEncoderOptions) -> Result<Vec<u8>, XzError> {
        let mut cctx = Fl2CCtx::new().map_err(|e| {
            XzError::DecompressError(format!("Failed to create FL2 compression context: {:?}", e))
        })?;

        cctx.set_parameter(
            Fl2CParameter::CompressionLevel,
            options.preset_level as usize,
        )
        .map_err(|e| {
            XzError::DecompressError(format!("Failed to set compression level: {:?}", e))
        })?;

        cctx.set_parameter(
            Fl2CParameter::DictionarySize,
            options.dict_size as usize,
        )
        .map_err(|e| {
            XzError::DecompressError(format!("Failed to set dictionary size: {:?}", e))
        })?;

        cctx.set_parameter(Fl2CParameter::OmitProperties, 1)
            .map_err(|e| {
                XzError::DecompressError(format!("Failed to set omit properties: {:?}", e))
            })?;

        cctx.set_parameter(Fl2CParameter::DoXXHash, 0)
            .map_err(|e| {
                XzError::DecompressError(format!("Failed to set do xxhash: {:?}", e))
            })?;

        let max_bound = fl2_compress_bound(data.len()) + 1024;
        let mut comp_buf = vec![0u8; max_bound];

        let comp_len = cctx
            .compress(data, &mut comp_buf, 0)
            .map_err(|e| {
                XzError::DecompressError(format!("FL2 compression failed: {:?}", e))
            })?;

        comp_buf.truncate(comp_len);
        Ok(comp_buf)
    }
}

/// Streaming single-threaded XZ container writer implementing `std::io::Write`.
pub struct XzStreamWriter<W: Write> {
    writer: W,
    options: XzEncoderOptions,
    index: XzStreamIndex,
    buffer: Vec<u8>,
    header_written: bool,
}

impl<W: Write> XzStreamWriter<W> {
    /// Creates a new `XzStreamWriter` and immediately writes the 12-byte Stream Header.
    pub fn new(writer: W, options: XzEncoderOptions) -> io::Result<Self> {
        let mut writer_obj = Self {
            writer,
            options,
            index: XzStreamIndex::new(),
            buffer: Vec::new(),
            header_written: false,
        };
        writer_obj.write_header()?;
        Ok(writer_obj)
    }

    /// Explicitly writes the 12-byte Stream Header if not already written.
    pub fn write_header(&mut self) -> io::Result<()> {
        if !self.header_written {
            let flags = XzStreamFlags::new(self.options.check_type);
            let header = XzStreamHeader::new(flags);
            let header_bytes = header.encode();
            self.writer.write_all(&header_bytes)?;
            self.header_written = true;
        }
        Ok(())
    }

    /// Flushes buffered uncompressed bytes into a completed XZ block.
    pub fn flush_block(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let (block_bytes, record) =
            XzBlockEncoder::encode_block(&self.buffer, &self.options)
                .map_err(|e| io::Error::other(e.to_string()))?;

        self.writer.write_all(&block_bytes)?;
        self.index
            .add_record(record)
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.buffer.clear();
        Ok(())
    }


    /// Finalizes the XZ stream: flushes remaining blocks, writes Stream Index,
    /// computes Backward Size, and writes the 12-byte Stream Footer.
    pub fn finish(mut self) -> io::Result<W> {
        self.write_header()?;
        self.flush_block()?;

        // Write Stream Index
        let index_bytes = self
            .index
            .encode()
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.writer.write_all(&index_bytes)?;

        // Write Stream Footer
        let backward_size = index_bytes.len() as u64;
        let flags = XzStreamFlags::new(self.options.check_type);
        let footer = XzStreamFooter::new(flags, backward_size);
        let footer_bytes = footer
            .encode_self()
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.writer.write_all(&footer_bytes)?;

        self.writer.flush()?;
        Ok(self.writer)
    }
}

impl<W: Write> Write for XzStreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_header()?;
        self.buffer.extend_from_slice(buf);

        if self.buffer.len() >= self.options.block_size {
            self.flush_block()?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// Multi-threaded parallel XZ stream writer leveraging Rayon.
pub struct XzParallelStreamWriter<W: Write> {
    writer: W,
    options: XzEncoderOptions,
    index: XzStreamIndex,
    _header_written: bool,
}

impl<W: Write> XzParallelStreamWriter<W> {
    /// Creates a new `XzParallelStreamWriter`.
    pub fn new(mut writer: W, options: XzEncoderOptions) -> io::Result<Self> {
        let flags = XzStreamFlags::new(options.check_type);
        let header = XzStreamHeader::new(flags);
        writer.write_all(&header.encode())?;

        Ok(Self {
            writer,
            options,
            index: XzStreamIndex::new(),
            _header_written: true,
        })
    }


    /// Compresses a large uncompressed buffer using Rayon parallel block chunking,
    /// writing the completed blocks in strict sequential order.
    pub fn write_parallel(&mut self, data: &[u8]) -> io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let chunk_size = self.options.block_size;
        let chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();

        let encoded_blocks: Result<Vec<(Vec<u8>, XzBlockRecord)>, XzError> = chunks
            .into_par_iter()
            .map(|chunk| XzBlockEncoder::encode_block(chunk, &self.options))
            .collect();

        let blocks =
            encoded_blocks.map_err(|e| io::Error::other(e.to_string()))?;

        for (block_bytes, record) in blocks {
            self.writer.write_all(&block_bytes)?;
            self.index
                .add_record(record)
                .map_err(|e| io::Error::other(e.to_string()))?;
        }


        Ok(())
    }

    /// Finalizes the parallel XZ stream: writes Stream Index and Stream Footer.
    pub fn finish(mut self) -> io::Result<W> {
        // Write Stream Index
        let index_bytes = self
            .index
            .encode()
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.writer.write_all(&index_bytes)?;

        // Write Stream Footer
        let backward_size = index_bytes.len() as u64;
        let flags = XzStreamFlags::new(self.options.check_type);
        let footer = XzStreamFooter::new(flags, backward_size);
        let footer_bytes = footer
            .encode_self()
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.writer.write_all(&footer_bytes)?;

        self.writer.flush()?;
        Ok(self.writer)
    }
}

/// Convenience standalone function to compress an entire slice into an XZ container.
pub fn xz_compress(data: &[u8], options: &XzEncoderOptions) -> io::Result<Vec<u8>> {
    let mut writer = XzStreamWriter::new(Vec::new(), options.clone())?;
    writer.write_all(data)?;
    writer.finish()
}
