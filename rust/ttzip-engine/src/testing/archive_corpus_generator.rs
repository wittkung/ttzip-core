// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deterministic Reverse-Syntax Compliant Archive Generator (`ArchiveCorpusGenerator`).
//!
//! Synthesizes deterministic, fully format-compliant yet structurally extreme ZIP and 7z
//! container byte slices directly from an arbitrary PRNG seed (`SplitMix64`), bypassing
//! standard forward compression pipelines.

use crate::crypto::crc32::crc32_fast;
use crate::sevenz::format::*;
use crate::testing::fuzz::SplitMix64;

// MARK: - 1. Constants & Magic Signatures

pub const ZIP_LFH_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
pub const ZIP_CDH_MAGIC: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
pub const ZIP64_EOCD_MAGIC: [u8; 4] = [0x50, 0x4B, 0x06, 0x06];
pub const ZIP64_LOCATOR_MAGIC: [u8; 4] = [0x50, 0x4B, 0x06, 0x07];
pub const ZIP_EOCD_MAGIC: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];

pub const ZIP64_TAG: u16 = 0x0001;

// MARK: - 2. Data Models & Configuration

/// Container format family for synthesized archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveContainerKind {
    Zip,
    SevenZ,
}

/// Extreme syntactic conditions for ZIP generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipExtremeVariant {
    ZeroLengthFileName,
    OversizedExtraField,
    NonStandardMethod(u16),
    Zip64BoundaryCrossing,
    DegenerateHuffmanStream,
    StreamingDataDescriptor,
    CorruptedCrc,
}

/// Configuration parameters for deterministic ZIP container synthesis.
#[derive(Debug, Clone)]
pub struct ZipSynthesisConfig {
    pub num_entries: usize,
    pub variants: Vec<ZipExtremeVariant>,
    pub extra_field_bytes: usize,
    pub virtual_uncompressed_size: u64,
    pub compression_method: u16,
    pub payload_len: usize,
}

impl Default for ZipSynthesisConfig {
    fn default() -> Self {
        Self {
            num_entries: 1,
            variants: Vec::new(),
            extra_field_bytes: 0,
            virtual_uncompressed_size: 1024,
            compression_method: 0, // Store
            payload_len: 128,
        }
    }
}

/// Extreme syntactic conditions for 7z container generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevenZExtremeVariant {
    ComplexCodersDag,
    Bcj2FourStreamBinding,
    EmptyStreamMarkers,
    ExtremeRepeatOffsets,
    MultiPackStreams,
}

/// Configuration parameters for deterministic 7z container synthesis.
#[derive(Debug, Clone)]
pub struct SevenZSynthesisConfig {
    pub num_files: usize,
    pub variants: Vec<SevenZExtremeVariant>,
    pub primary_method: u64,
    pub payload_size: usize,
    pub num_unpack_streams: usize,
}

impl Default for SevenZSynthesisConfig {
    fn default() -> Self {
        Self {
            num_files: 1,
            variants: Vec::new(),
            primary_method: METHOD_COPY,
            payload_size: 256,
            num_unpack_streams: 1,
        }
    }
}

/// Synthesized archive payload artifact with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArchive {
    pub kind: ArchiveContainerKind,
    pub data: Vec<u8>,
    pub virtual_uncompressed_bytes: u64,
    pub seed: u64,
    pub description: String,
}

// MARK: - 3. Reverse Syntax Archive Corpus Generator

/// High-throughput deterministic generator for reverse-synthesizing extreme ZIP and 7z archives.
#[derive(Debug, Clone)]
pub struct ArchiveCorpusGenerator {
    prng: SplitMix64,
    initial_seed: u64,
}

impl ArchiveCorpusGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            prng: SplitMix64::new(seed),
            initial_seed: seed,
        }
    }

    #[inline]
    pub fn seed(&self) -> u64 {
        self.initial_seed
    }

    pub fn reset_seed(&mut self, seed: u64) {
        self.prng = SplitMix64::new(seed);
        self.initial_seed = seed;
    }

    // MARK: - ZIP Synthesis Implementations

    pub fn generate_extreme_zip(&mut self, config: &ZipSynthesisConfig) -> GeneratedArchive {
        let mut out = Vec::with_capacity(1024 + config.payload_len * config.num_entries);
        let mut cdh_records = Vec::new();

        let is_z64 = config.variants.contains(&ZipExtremeVariant::Zip64BoundaryCrossing)
            || config.virtual_uncompressed_size >= 0xFFFFFFFF;
        let zero_name = config.variants.contains(&ZipExtremeVariant::ZeroLengthFileName);
        let oversize_extra = config.variants.contains(&ZipExtremeVariant::OversizedExtraField);
        let degenerate_huff = config.variants.contains(&ZipExtremeVariant::DegenerateHuffmanStream);
        let streaming_desc = config.variants.contains(&ZipExtremeVariant::StreamingDataDescriptor);
        let corrupt_crc = config.variants.contains(&ZipExtremeVariant::CorruptedCrc);

        for i in 0..config.num_entries {
            let lfh_off = out.len() as u64;
            let filename = if zero_name && i == 0 { String::new() } else { format!("extreme_corpus_entry_{i:04}.bin") };
            let fn_bytes = filename.as_bytes();

            let payload = if degenerate_huff { self.synthesize_empty_dynamic_huffman_payload(8) } else { self.synthesize_pseudo_payload(config.payload_len) };
            let uncomp_size = if is_z64 { config.virtual_uncompressed_size } else { payload.len() as u64 };
            let comp_size = payload.len() as u64;
            let crc = if corrupt_crc { crc32_fast(0, &payload) ^ 0xDEADBEEF } else { crc32_fast(0, &payload) };

            let method = config.variants.iter().find_map(|v| match v {
                ZipExtremeVariant::NonStandardMethod(m) => Some(*m),
                _ => None,
            }).unwrap_or(config.compression_method);

            let mut extra_field = Vec::new();
            if is_z64 {
                extra_field.extend_from_slice(&ZIP64_TAG.to_le_bytes());
                extra_field.extend_from_slice(&16u16.to_le_bytes());
                extra_field.extend_from_slice(&uncomp_size.to_le_bytes());
                extra_field.extend_from_slice(&comp_size.to_le_bytes());
            }
            if oversize_extra {
                let pad_len = config.extra_field_bytes.max(256);
                let unknown_tag = (0xE000 | (self.prng.next_u64() & 0x0FFF)) as u16;
                extra_field.extend_from_slice(&unknown_tag.to_le_bytes());
                extra_field.extend_from_slice(&(pad_len as u16).to_le_bytes());
                extra_field.resize(extra_field.len() + pad_len, 0xAA);
            }

            // Local File Header
            out.extend_from_slice(&ZIP_LFH_MAGIC);
            out.extend_from_slice(&(if is_z64 { 45u16 } else { 20u16 }).to_le_bytes());
            let gp_flags: u16 = if streaming_desc { 0x0008 } else { 0x0000 };
            out.extend_from_slice(&gp_flags.to_le_bytes());
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&0x4A21u16.to_le_bytes()); // time
            out.extend_from_slice(&0x5C35u16.to_le_bytes()); // date
            out.extend_from_slice(&crc.to_le_bytes());

            if streaming_desc {
                out.extend_from_slice(&0u32.to_le_bytes());
                out.extend_from_slice(&0u32.to_le_bytes());
            } else if is_z64 {
                out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
                out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
            } else {
                out.extend_from_slice(&(comp_size as u32).to_le_bytes());
                out.extend_from_slice(&(uncomp_size as u32).to_le_bytes());
            }
            out.extend_from_slice(&(fn_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&(extra_field.len() as u16).to_le_bytes());
            out.extend_from_slice(fn_bytes);
            out.extend_from_slice(&extra_field);
            out.extend_from_slice(&payload);

            if streaming_desc {
                out.extend_from_slice(&[0x50, 0x4B, 0x07, 0x08]);
                out.extend_from_slice(&crc.to_le_bytes());
                if is_z64 {
                    out.extend_from_slice(&comp_size.to_le_bytes());
                    out.extend_from_slice(&uncomp_size.to_le_bytes());
                } else {
                    out.extend_from_slice(&(comp_size as u32).to_le_bytes());
                    out.extend_from_slice(&(uncomp_size as u32).to_le_bytes());
                }
            }

            // CDFH
            let mut cdh_extra = Vec::new();
            if is_z64 {
                cdh_extra.extend_from_slice(&ZIP64_TAG.to_le_bytes());
                cdh_extra.extend_from_slice(&24u16.to_le_bytes());
                cdh_extra.extend_from_slice(&uncomp_size.to_le_bytes());
                cdh_extra.extend_from_slice(&comp_size.to_le_bytes());
                cdh_extra.extend_from_slice(&lfh_off.to_le_bytes());
            }
            if oversize_extra {
                cdh_extra.extend_from_slice(&extra_field);
            }

            let mut cdh = Vec::new();
            cdh.extend_from_slice(&ZIP_CDH_MAGIC);
            cdh.extend_from_slice(&45u16.to_le_bytes());
            cdh.extend_from_slice(&(if is_z64 { 45u16 } else { 20u16 }).to_le_bytes());
            cdh.extend_from_slice(&gp_flags.to_le_bytes());
            cdh.extend_from_slice(&method.to_le_bytes());
            cdh.extend_from_slice(&0x4A21u16.to_le_bytes());
            cdh.extend_from_slice(&0x5C35u16.to_le_bytes());
            cdh.extend_from_slice(&crc.to_le_bytes());

            if is_z64 {
                cdh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
                cdh.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
            } else {
                cdh.extend_from_slice(&(comp_size as u32).to_le_bytes());
                cdh.extend_from_slice(&(uncomp_size as u32).to_le_bytes());
            }

            cdh.extend_from_slice(&(fn_bytes.len() as u16).to_le_bytes());
            cdh.extend_from_slice(&(cdh_extra.len() as u16).to_le_bytes());
            cdh.extend_from_slice(&0u16.to_le_bytes()); // comment len
            cdh.extend_from_slice(&0u16.to_le_bytes()); // disk
            cdh.extend_from_slice(&0u16.to_le_bytes()); // int attr
            cdh.extend_from_slice(&0o100644u32.rotate_left(16).to_le_bytes()); // ext attr
            cdh.extend_from_slice(&(if is_z64 { 0xFFFFFFFFu32 } else { lfh_off as u32 }).to_le_bytes());
            cdh.extend_from_slice(fn_bytes);
            cdh.extend_from_slice(&cdh_extra);
            cdh_records.push(cdh);
        }

        let cd_offset = out.len() as u64;
        let mut cd_size = 0u64;
        for cdh in &cdh_records {
            out.extend_from_slice(cdh);
            cd_size += cdh.len() as u64;
        }

        if is_z64 || cdh_records.len() >= 0xFFFF {
            let z64_eocd_off = out.len() as u64;
            out.extend_from_slice(&ZIP64_EOCD_MAGIC);
            out.extend_from_slice(&44u64.to_le_bytes());
            out.extend_from_slice(&45u16.to_le_bytes());
            out.extend_from_slice(&45u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&(cdh_records.len() as u64).to_le_bytes());
            out.extend_from_slice(&(cdh_records.len() as u64).to_le_bytes());
            out.extend_from_slice(&cd_size.to_le_bytes());
            out.extend_from_slice(&cd_offset.to_le_bytes());

            out.extend_from_slice(&ZIP64_LOCATOR_MAGIC);
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&z64_eocd_off.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes());
        }

        out.extend_from_slice(&ZIP_EOCD_MAGIC);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        let entries_field = (cdh_records.len().min(0xFFFF)) as u16;
        out.extend_from_slice(&entries_field.to_le_bytes());
        out.extend_from_slice(&entries_field.to_le_bytes());
        out.extend_from_slice(&(if is_z64 { 0xFFFFFFFFu32 } else { cd_size as u32 }).to_le_bytes());
        out.extend_from_slice(&(if is_z64 { 0xFFFFFFFFu32 } else { cd_offset as u32 }).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());

        GeneratedArchive {
            kind: ArchiveContainerKind::Zip,
            data: out,
            virtual_uncompressed_bytes: config.virtual_uncompressed_size * (config.num_entries as u64),
            seed: self.initial_seed,
            description: format!("ZIP archive with {} entries, is_zip64={is_z64}", config.num_entries),
        }
    }

    pub fn generate_zip64_boundary_archive(&mut self, virtual_size: u64) -> GeneratedArchive {
        let config = ZipSynthesisConfig {
            num_entries: 1,
            variants: vec![ZipExtremeVariant::Zip64BoundaryCrossing],
            extra_field_bytes: 0,
            virtual_uncompressed_size: virtual_size.max(0x1_0000_0001),
            compression_method: 0,
            payload_len: 64,
        };
        self.generate_extreme_zip(&config)
    }

    pub fn generate_degenerate_huffman_zip(&mut self, num_blocks: usize) -> GeneratedArchive {
        let config = ZipSynthesisConfig {
            num_entries: 1,
            variants: vec![
                ZipExtremeVariant::DegenerateHuffmanStream,
                ZipExtremeVariant::NonStandardMethod(8),
            ],
            extra_field_bytes: 0,
            virtual_uncompressed_size: (num_blocks * 64) as u64,
            compression_method: 8,
            payload_len: num_blocks * 16,
        };
        self.generate_extreme_zip(&config)
    }

    // MARK: - 7z Synthesis Implementations

    pub fn generate_extreme_7z(&mut self, config: &SevenZSynthesisConfig) -> GeneratedArchive {
        let is_bcj2 = config.variants.contains(&SevenZExtremeVariant::Bcj2FourStreamBinding);
        let has_empty = config.variants.contains(&SevenZExtremeVariant::EmptyStreamMarkers);
        let is_dag = config.variants.contains(&SevenZExtremeVariant::ComplexCodersDag);

        let mut raw = if is_bcj2 { self.synthesize_bcj2_streams_payload(config.payload_size) } else { self.synthesize_pseudo_payload(config.payload_size) };
        if config.variants.contains(&SevenZExtremeVariant::ExtremeRepeatOffsets) {
            for (idx, b) in raw.iter_mut().enumerate() {
                *b = if (idx % 64) < 32 { 0xAA } else { 0x55 };
            }
        }

        let pack_len = raw.len() as u64;
        let unpack_len = raw.len() as u64;

        let mut h = Vec::new();
        h.push(K_HEADER);
        h.push(K_MAIN_STREAMS_INFO);
        h.push(K_PACK_INFO);
        write_varint(0, &mut h);
        write_varint(if is_bcj2 { 4 } else { 1 }, &mut h);
        h.push(K_SIZE);
        if is_bcj2 {
            let chunk = pack_len / 4;
            for i in 0..4 {
                write_varint(if i == 3 { pack_len - (chunk * 3) } else { chunk }, &mut h);
            }
        } else {
            write_varint(pack_len, &mut h);
        }
        h.push(K_END);

        h.push(K_UNPACK_INFO);
        h.push(K_FOLDER);
        write_varint(1, &mut h);
        h.push(0); // external = 0

        if is_bcj2 {
            write_varint(4, &mut h);
            h.push(0x14);
            h.extend_from_slice(&[0x03, 0x03, 0x01, 0x03]); // BCJ2
            write_varint(4, &mut h);
            write_varint(1, &mut h);

            for _ in 0..3 {
                h.push(0x21);
                h.push(0x21); // LZMA2
                write_varint(1, &mut h);
                h.push(0x18);
            }

            for i in 0..3 {
                write_varint(i + 1, &mut h);
                write_varint(i + 1, &mut h);
            }
            write_varint(0, &mut h);
            write_varint(4, &mut h);
            write_varint(5, &mut h);
            write_varint(6, &mut h);
        } else if is_dag {
            write_varint(2, &mut h);
            h.push(0x03);
            h.extend_from_slice(&[0x04, 0x01, 0x08]); // Deflate
            h.push(0x01);
            h.push(0x00); // Copy
            write_varint(0, &mut h);
            write_varint(0, &mut h);
        } else {
            write_varint(1, &mut h);
            let mut mid_bytes = Vec::new();
            let mut tmp = config.primary_method;
            while tmp > 0 {
                mid_bytes.push((tmp & 0xFF) as u8);
                tmp >>= 8;
            }
            if mid_bytes.is_empty() { mid_bytes.push(0); }
            mid_bytes.reverse();
            h.push(mid_bytes.len() as u8);
            h.extend_from_slice(&mid_bytes);
        }

        h.push(K_CODERS_UNPACK_SIZE);
        if is_bcj2 {
            let chunk = unpack_len / 4;
            for i in 0..4 {
                write_varint(if i == 3 { unpack_len - (chunk * 3) } else { chunk }, &mut h);
            }
        } else if is_dag {
            write_varint(unpack_len, &mut h);
            write_varint(unpack_len, &mut h);
        } else {
            write_varint(unpack_len, &mut h);
        }
        h.push(K_END);
        h.push(K_END); // end MainStreamsInfo

        h.push(K_FILES_INFO);
        write_varint(config.num_files as u64, &mut h);

        if has_empty && config.num_files > 1 {
            h.push(K_EMPTY_STREAM);
            let num_bytes = config.num_files.div_ceil(8);
            write_varint(num_bytes as u64, &mut h);
            for f in 0..num_bytes {
                let mut b = 0u8;
                for bit in 0..8 {
                    let idx = f * 8 + bit;
                    if idx < config.num_files && idx % 2 == 1 { b |= 1 << (7 - bit); }
                }
                h.push(b);
            }
        }

        h.push(K_NAME);
        let mut u16_name_bytes = Vec::new();
        for f in 0..config.num_files {
            let name = format!("sevenz_extreme_entry_{f:04}.dat");
            for u in name.encode_utf16() {
                u16_name_bytes.extend_from_slice(&u.to_le_bytes());
            }
            u16_name_bytes.extend_from_slice(&0u16.to_le_bytes());
        }
        write_varint((1 + u16_name_bytes.len()) as u64, &mut h);
        h.push(0);
        h.extend_from_slice(&u16_name_bytes);

        h.push(K_WIN_ATTRIBUTES);
        write_varint((2 + (config.num_files * 4)) as u64, &mut h);
        h.push(1);
        h.push(0);
        for _ in 0..config.num_files {
            h.extend_from_slice(&0x20u32.to_le_bytes());
        }
        h.push(K_END);
        h.push(K_END);

        let next_header_offset = pack_len;
        let next_header_size = h.len() as u64;
        let next_header_crc = crc32_fast(0, &h);

        let sig = SevenZSignatureHeader {
            major_version: 0,
            minor_version: 4,
            start_header_crc: 0,
            next_header_offset,
            next_header_size,
            next_header_crc,
        };

        let mut output = Vec::with_capacity(32 + raw.len() + h.len());
        output.extend_from_slice(&sig.serialize());
        output.extend_from_slice(&raw);
        output.extend_from_slice(&h);

        GeneratedArchive {
            kind: ArchiveContainerKind::SevenZ,
            data: output,
            virtual_uncompressed_bytes: unpack_len,
            seed: self.initial_seed,
            description: format!("7z archive with {} files, is_bcj2={is_bcj2}", config.num_files),
        }
    }

    pub fn generate_bcj2_4stream_7z(&mut self, payload: &[u8]) -> GeneratedArchive {
        let config = SevenZSynthesisConfig {
            num_files: 1,
            variants: vec![SevenZExtremeVariant::Bcj2FourStreamBinding],
            primary_method: METHOD_LZMA2,
            payload_size: payload.len().max(64),
            num_unpack_streams: 1,
        };
        self.generate_extreme_7z(&config)
    }

    // MARK: - Helper Synthesizers

    fn synthesize_pseudo_payload(&mut self, size: usize) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        for chunk in buf.chunks_mut(8) {
            let val = self.prng.next_u64().to_le_bytes();
            let count = chunk.len();
            chunk.copy_from_slice(&val[..count]);
        }
        buf
    }

    fn synthesize_bcj2_streams_payload(&mut self, total_size: usize) -> Vec<u8> {
        let quarter = (total_size / 4).max(16);
        let mut buf = Vec::with_capacity(quarter * 4);
        for _ in 0..4 {
            buf.extend_from_slice(&self.synthesize_pseudo_payload(quarter));
        }
        buf
    }

    fn synthesize_empty_dynamic_huffman_payload(&mut self, block_count: usize) -> Vec<u8> {
        let mut bit_buf = 0u64;
        let mut num_bits = 0u32;
        let mut out = Vec::new();

        for i in 0..block_count {
            let is_final = if i == block_count - 1 { 1u32 } else { 0u32 };
            let header = (is_final) | (0b10 << 1);
            bit_buf |= (header as u64) << num_bits;
            num_bits += 3 + 14; // tree header = 0 (14 bits)

            let precode = 0b001_000_000_000u32; // 12 bits
            bit_buf |= (precode as u64) << num_bits;
            num_bits += 12 + 1; // eob = 0 (1 bit)

            while num_bits >= 8 {
                out.push((bit_buf & 0xFF) as u8);
                bit_buf >>= 8;
                num_bits -= 8;
            }
        }
        if num_bits > 0 {
            out.push((bit_buf & 0xFF) as u8);
        }
        out
    }

    pub fn generate_corpus_matrix(&mut self, count: usize) -> Vec<GeneratedArchive> {
        let mut matrix = Vec::with_capacity(count);
        for idx in 0..count {
            if (idx % 2) == 0 {
                let variant = match (idx / 2) % 6 {
                    0 => ZipExtremeVariant::ZeroLengthFileName,
                    1 => ZipExtremeVariant::OversizedExtraField,
                    2 => ZipExtremeVariant::Zip64BoundaryCrossing,
                    3 => ZipExtremeVariant::DegenerateHuffmanStream,
                    4 => ZipExtremeVariant::StreamingDataDescriptor,
                    _ => ZipExtremeVariant::NonStandardMethod(99),
                };
                let cfg = ZipSynthesisConfig {
                    num_entries: (idx % 4) + 1,
                    variants: vec![variant],
                    extra_field_bytes: 512 * (idx + 1),
                    virtual_uncompressed_size: 1024 * ((idx as u64) + 1),
                    compression_method: 0,
                    payload_len: 128 * (idx + 1),
                };
                matrix.push(self.generate_extreme_zip(&cfg));
            } else {
                let variant = match (idx / 2) % 4 {
                    0 => SevenZExtremeVariant::Bcj2FourStreamBinding,
                    1 => SevenZExtremeVariant::ComplexCodersDag,
                    2 => SevenZExtremeVariant::EmptyStreamMarkers,
                    _ => SevenZExtremeVariant::ExtremeRepeatOffsets,
                };
                let cfg = SevenZSynthesisConfig {
                    num_files: (idx % 4) + 1,
                    variants: vec![variant],
                    primary_method: METHOD_COPY,
                    payload_size: 256 * (idx + 1),
                    num_unpack_streams: 1,
                };
                matrix.push(self.generate_extreme_7z(&cfg));
            }
        }
        matrix
    }
}
