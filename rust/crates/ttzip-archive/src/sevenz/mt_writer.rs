// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multi-Threaded Offline Solid Block Preparation and Sequential Append Pipeline.
//!
//! Provides `prepare_block` for parallel, CPU-bound solid compression across worker threads
//! and `SevenZArchiveWriter` for $O(1)$ lock-free sequential append and 7z metadata header finalization.

use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom, Write};

use crate::codecs::lzma2::{calculate_lzma2_dictionary_mb, Fl2CParameter, Fl2CStream};
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::dag::SevenZError;
use crate::sevenz::format::*;
use crate::types::{TTZipCompressionLevel, TTZipStatus};
use crate::zip::writer::ZipInputItem;

pub(crate) const MAX_REUSABLE_FL2_DICT_BYTES: usize = 2 * 1024 * 1024;

thread_local! {
    static REUSABLE_FL2_CSTREAM: RefCell<Option<Fl2CStream>> = const { RefCell::new(None) };
}

fn resolve_fl2_level_and_dict(
    level: i32,
    expected_size: Option<u64>,
    dict_override: Option<u32>,
    solid_block_size_mb: u32,
) -> (usize, usize, usize, usize) {
    let (high_comp, eff_level, tt_level) = if level <= 1 {
        (0usize, 1usize, TTZipCompressionLevel::Fastest)
    } else if level <= 4 {
        (0usize, level as usize, TTZipCompressionLevel::Fast)
    } else if level <= 7 {
        (0usize, level as usize, TTZipCompressionLevel::Normal)
    } else if level <= 9 {
        (0usize, level as usize, TTZipCompressionLevel::Maximum)
    } else {
        (1usize, 10usize, TTZipCompressionLevel::Ultra)
    };
    let default_dict = (calculate_lzma2_dictionary_mb(tt_level, 8.0) as usize) * 1024 * 1024;
    let block_bytes = (solid_block_size_mb as usize).saturating_mul(1024 * 1024);
    let exp_cap = expected_size
        .map(|e| e.max(1 << 20).next_power_of_two() as usize)
        .unwrap_or(usize::MAX);
    let auto_dict = default_dict
        .min(if block_bytes > 0 { block_bytes.max(1 << 20) } else { usize::MAX })
        .min(exp_cap)
        .max(1 << 20);
    let dict_bytes = dict_override.map(|d| (d as usize).max(1 << 20)).unwrap_or(auto_dict);
    let interval = if block_bytes > 0 {
        (block_bytes / dict_bytes.max(1)).clamp(1, 16)
    } else {
        0
    };
    (high_comp, eff_level, dict_bytes, interval)
}

pub(crate) fn apply_fl2_cstream_params(
    cstream: &mut Fl2CStream,
    level: i32,
    expected_size: Option<u64>,
    dict_override: Option<u32>,
    solid_block_size_mb: u32,
) -> Result<usize, TTZipStatus> {
    let (high_comp, eff_level, dict_bytes, interval) =
        resolve_fl2_level_and_dict(level, expected_size, dict_override, solid_block_size_mb);
    cstream.set_parameter(Fl2CParameter::HighCompression, high_comp)?;
    cstream.set_parameter(Fl2CParameter::CompressionLevel, eff_level)?;
    cstream.set_parameter(Fl2CParameter::DictionarySize, dict_bytes)?;
    cstream.set_parameter(Fl2CParameter::ResetInterval, interval)?;
    cstream.set_parameter(Fl2CParameter::OmitProperties, 1)?;
    cstream.init(0)?;
    Ok(dict_bytes)
}

pub(crate) fn encode_fl2_items_with_sink<F>(
    items: &[ZipInputItem],
    level: i32,
    threads: u32,
    uncompressed_len: u64,
    mut on_out: F,
) -> Result<u8, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let resolved_threads = super::writer::resolve_7z_threads(threads);
    let chunk_len = (uncompressed_len as usize).clamp(64 * 1024, 1024 * 1024);
    let mut run_encode = |cstream: &mut Fl2CStream| -> Result<u8, TTZipStatus> {
        super::decoder::codec_pump::with_decode_scratch_buf(chunk_len, |out_chunk| {
            for item in items.iter().filter(|it| !it.is_directory && !it.data.is_empty()) {
                super::writer::feed_fl2_slice(cstream, &item.data, out_chunk, &mut on_out)?;
            }
            super::writer::finish_fl2_stream(cstream, out_chunk, &mut on_out)
        })
    };
    if resolved_threads == 1 {
        return REUSABLE_FL2_CSTREAM.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut() {
                let cs = match slot.as_mut() {
                    Some(cs) => cs,
                    None => slot.insert(Fl2CStream::new()?),
                };
                let dict_bytes = apply_fl2_cstream_params(
                    cs,
                    level,
                    Some(uncompressed_len),
                    None,
                    0,
                )?;
                let res = run_encode(cs);
                if res.is_err() || dict_bytes > MAX_REUSABLE_FL2_DICT_BYTES {
                    *slot = None;
                }
                return res;
            }
            let mut cs = super::writer::configure_fl2_stream(
                level,
                resolved_threads,
                Some(uncompressed_len),
                None,
                0,
            )?;
            run_encode(&mut cs)
        });
    }
    let mut cs = super::writer::configure_fl2_stream(
        level,
        resolved_threads,
        Some(uncompressed_len),
        None,
        0,
    )?;
    run_encode(&mut cs)
}

/// Supported compression methods for 7z solid block encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevenZEncoderMethod {
    /// Uncompressed Store / Copy mode.
    Copy,
    /// Fast-LZMA2 multi-threaded compression mode.
    Lzma2,
}

impl SevenZEncoderMethod {
    /// Maps the encoder method to the standard 7z binary method ID.
    #[inline]
    #[must_use]
    pub const fn to_method_id(self) -> u64 {
        match self {
            Self::Copy => METHOD_COPY,
            Self::Lzma2 => METHOD_LZMA2,
        }
    }
}

/// Configuration options for 7z solid block encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SevenZEncoderOptions {
    /// Compression level (0 = Store, 1..9 = Fast to Ultra).
    pub compression_level: u32,
    /// Internal thread budget allocated to LZMA2 compressor.
    pub thread_budget: u32,
    /// Optional explicit dictionary size in bytes.
    pub dict_size: Option<u32>,
}

impl Default for SevenZEncoderOptions {
    fn default() -> Self {
        Self {
            compression_level: 6,
            thread_budget: 1,
            dict_size: None,
        }
    }
}

/// Self-contained, pre-compressed solid block ready for sequential append.
#[derive(Debug, Clone)]
pub struct PreparedBlock {
    /// Compressed payload bytes for this solid block (Folder).
    pub compressed_data: Vec<u8>,
    /// Uncompressed sizes of each substream in this block.
    pub substream_sizes: Vec<u64>,
    /// CRC32 checksums of each uncompressed substream in this block.
    pub substream_crcs: Vec<u32>,
    /// CRC32 checksum of the compressed data payload.
    pub block_crc: u32,
    /// Total uncompressed size of all substreams in this block.
    pub unpack_size: u64,
    /// Compression method used for this block.
    pub method: SevenZEncoderMethod,
    /// Coder properties (e.g. dict property byte for LZMA2).
    pub coder_props: Vec<u8>,
    /// Flags indicating whether each entry in the input batch was an empty stream.
    pub empty_flags: Vec<bool>,
}

type UncompressedBlockEntries = (Vec<u8>, Vec<u64>, Vec<u32>, Vec<bool>, u64);
type PooledLzma2BlockResult = Option<Result<(Vec<u8>, Vec<u8>), TTZipStatus>>;

fn collect_uncompressed_block_entries<R: Read>(
    entries_data: Vec<(String, u64, R)>,
) -> Result<UncompressedBlockEntries, SevenZError> {
    let mut substream_sizes = Vec::with_capacity(entries_data.len());
    let mut substream_crcs = Vec::with_capacity(entries_data.len());
    let mut empty_flags = Vec::with_capacity(entries_data.len());
    let mut total_unpack_size: u64 = 0;

    let mut uncompressed_payload = Vec::new();
    const READ_CHUNK_SIZE: usize = 64 * 1024;
    let mut read_buf = vec![0u8; READ_CHUNK_SIZE];

    for (name, size_hint, mut reader) in entries_data {
        if name.ends_with('/') {
            empty_flags.push(true);
            continue;
        }

        let mut entry_len: u64 = 0;
        let mut entry_crc: u32 = 0;
        let start_offset = uncompressed_payload.len();

        if size_hint > 0 {
            uncompressed_payload.reserve(size_hint as usize);
        }

        loop {
            let bytes_read = reader.read(&mut read_buf)?;
            if bytes_read == 0 {
                break;
            }
            let chunk = &read_buf[..bytes_read];
            entry_crc = crc32_fast(entry_crc, chunk);
            entry_len += bytes_read as u64;
            uncompressed_payload.extend_from_slice(chunk);
        }

        if entry_len == 0 {
            empty_flags.push(true);
        } else {
            empty_flags.push(false);
            substream_sizes.push(entry_len);
            substream_crcs.push(entry_crc);
            total_unpack_size += entry_len;
        }

        debug_assert_eq!(uncompressed_payload.len() - start_offset, entry_len as usize);
    }

    Ok((
        uncompressed_payload,
        substream_sizes,
        substream_crcs,
        empty_flags,
        total_unpack_size,
    ))
}

fn compress_solid_block_lzma2(
    uncompressed_payload: &[u8],
    total_unpack_size: u64,
    options: SevenZEncoderOptions,
) -> Result<(Vec<u8>, Vec<u8>), SevenZError> {
    let resolved_threads = super::writer::resolve_7z_threads(options.thread_budget);
    let chunk_len = (total_unpack_size as usize).clamp(64 * 1024, 2 * 1024 * 1024);
    let run_block = |cstream: &mut Fl2CStream| -> Result<(Vec<u8>, Vec<u8>), TTZipStatus> {
        let mut comp_data = Vec::new();
        let dict_prop = super::decoder::codec_pump::with_decode_scratch_buf(chunk_len, |out_chunk| {
            super::writer::feed_fl2_slice(cstream, uncompressed_payload, out_chunk, |chunk| {
                comp_data.extend_from_slice(chunk);
                Ok(())
            })?;
            super::writer::finish_fl2_stream(cstream, out_chunk, |chunk| {
                comp_data.extend_from_slice(chunk);
                Ok(())
            })
        })?;
        Ok((comp_data, vec![dict_prop]))
    };

    if resolved_threads == 1 {
        let pooled = REUSABLE_FL2_CSTREAM.with(|cell| -> PooledLzma2BlockResult {
            let mut slot = cell.try_borrow_mut().ok()?;
            let cs = match slot.as_mut() {
                Some(cs) => cs,
                None => match Fl2CStream::new() {
                    Ok(s) => slot.insert(s),
                    Err(e) => return Some(Err(e)),
                },
            };
            let dict_bytes = match apply_fl2_cstream_params(
                cs,
                options.compression_level as i32,
                Some(total_unpack_size),
                options.dict_size,
                0,
            ) {
                Ok(d) => d,
                Err(e) => {
                    *slot = None;
                    return Some(Err(e));
                }
            };
            let res = run_block(cs);
            if res.is_err() || dict_bytes > MAX_REUSABLE_FL2_DICT_BYTES {
                *slot = None;
            }
            Some(res)
        });
        if let Some(res) = pooled {
            return res.map_err(SevenZError::from);
        }
    }

    let mut cstream = super::writer::configure_fl2_stream(
        options.compression_level as i32,
        resolved_threads,
        Some(total_unpack_size),
        options.dict_size,
        0,
    )?;
    run_block(&mut cstream).map_err(SevenZError::from)
}

/// Prepares and compresses a batch of input entries into a standalone, self-contained `PreparedBlock`.
///
/// This function is thread-safe and pure, designed to be executed across multiple worker threads
/// concurrently. It calculates per-entry uncompressed CRC32s and compresses the concatenated solid
/// stream using the specified method (LZMA2 or COPY).
pub fn prepare_block<R: Read>(
    entries_data: Vec<(String, u64, R)>,
    method: SevenZEncoderMethod,
    options: SevenZEncoderOptions,
) -> Result<PreparedBlock, SevenZError> {
    if entries_data.is_empty() {
        return Ok(PreparedBlock {
            compressed_data: Vec::new(),
            substream_sizes: Vec::new(),
            substream_crcs: Vec::new(),
            block_crc: 0,
            unpack_size: 0,
            method,
            coder_props: Vec::new(),
            empty_flags: Vec::new(),
        });
    }

    let (
        uncompressed_payload,
        substream_sizes,
        substream_crcs,
        empty_flags,
        total_unpack_size,
    ) = collect_uncompressed_block_entries(entries_data)?;

    let (compressed_data, coder_props) = if method == SevenZEncoderMethod::Copy
        || total_unpack_size == 0
        || options.compression_level == 0
    {
        (uncompressed_payload, Vec::new())
    } else {
        compress_solid_block_lzma2(&uncompressed_payload, total_unpack_size, options)?
    };

    let block_crc = crc32_fast(0, &compressed_data);

    Ok(PreparedBlock {
        compressed_data,
        substream_sizes,
        substream_crcs,
        block_crc,
        unpack_size: total_unpack_size,
        method,
        coder_props,
        empty_flags,
    })
}

/// Internal metadata tracking record for an individual 7z Folder (Solid Block).
#[derive(Debug, Clone)]
struct FolderRecord {
    pack_size: u64,
    unpack_size: u64,
    method_id: u64,
    coder_props: Vec<u8>,
    substream_sizes: Vec<u64>,
    substream_crcs: Vec<u32>,
    block_crc: u32,
}

/// Internal metadata tracking record for a file entry within the 7z archive.
#[derive(Debug, Clone)]
struct FileRecord {
    rel_path: String,
    is_directory: bool,
    is_empty_stream: bool,
}

/// Sequential 7z Archive Writer supporting lock-free append of pre-compressed solid blocks.
pub struct SevenZArchiveWriter<W: Write + Seek> {
    writer: W,
    current_payload_pos: u64,
    folders: Vec<FolderRecord>,
    files: Vec<FileRecord>,
    is_finalized: bool,
}

impl<W: Write + Seek> SevenZArchiveWriter<W> {
    /// Creates a new `SevenZArchiveWriter` and reserves the 32-byte `SevenZSignatureHeader` at offset 0.
    pub fn new(mut writer: W) -> Result<Self, SevenZError> {
        let placeholder = [0u8; 32];
        writer.write_all(&placeholder)?;

        Ok(Self {
            writer,
            current_payload_pos: 0,
            folders: Vec::new(),
            files: Vec::new(),
            is_finalized: false,
        })
    }

    /// Appends a pre-compressed solid block to the archive with $O(1)$ memory allocation and sequential disk I/O.
    pub fn push_prepared_block(
        &mut self,
        block: PreparedBlock,
        names: Vec<String>,
    ) -> Result<(), SevenZError> {
        if self.is_finalized {
            return Err(SevenZError::InvalidParameter(
                "archive writer already finalized".to_string(),
            ));
        }

        if !block.compressed_data.is_empty() {
            self.writer.write_all(&block.compressed_data)?;
        }

        let folder = FolderRecord {
            pack_size: block.compressed_data.len() as u64,
            unpack_size: block.unpack_size,
            method_id: block.method.to_method_id(),
            coder_props: block.coder_props,
            substream_sizes: block.substream_sizes,
            substream_crcs: block.substream_crcs,
            block_crc: block.block_crc,
        };
        self.folders.push(folder);

        for (i, name) in names.into_iter().enumerate() {
            let is_directory = name.ends_with('/');
            let is_empty = is_directory || block.empty_flags.get(i).copied().unwrap_or(false);
            self.files.push(FileRecord {
                rel_path: name,
                is_directory,
                is_empty_stream: is_empty,
            });
        }

        self.current_payload_pos += block.compressed_data.len() as u64;
        Ok(())
    }

    /// Serializes the final 7z metadata Header, seeks back to offset 0, and writes the `SevenZSignatureHeader`.
    ///
    /// Returns the total archive size in bytes.
    pub fn finalize(&mut self) -> Result<u64, SevenZError> {
        if self.is_finalized {
            return Err(SevenZError::InvalidParameter(
                "archive writer already finalized".to_string(),
            ));
        }

        let header_bytes = build_multi_folder_7z_header(&self.folders, &self.files);

        self.writer.write_all(&header_bytes)?;

        let next_header_offset = self.current_payload_pos;
        let next_header_size = header_bytes.len() as u64;
        let next_header_crc = crc32_fast(0, &header_bytes);

        let sig_header = SevenZSignatureHeader {
            major_version: 0,
            minor_version: 4,
            start_header_crc: 0,
            next_header_offset,
            next_header_size,
            next_header_crc,
        };

        let sig_bytes = sig_header.serialize();

        self.writer.seek(SeekFrom::Start(0))?;
        self.writer.write_all(&sig_bytes)?;
        self.writer.flush()?;

        self.is_finalized = true;
        let total_size = 32 + self.current_payload_pos + (header_bytes.len() as u64);
        Ok(total_size)
    }
}

fn write_multi_folder_pack_and_unpack_info(folders: &[FolderRecord], h: &mut Vec<u8>) {
    h.push(K_PACK_INFO);
    write_varint(0, h);
    write_varint(folders.len() as u64, h);
    h.push(K_SIZE);
    for f in folders {
        write_varint(f.pack_size, h);
    }
    h.push(K_CRC);
    h.push(1);
    for f in folders {
        h.extend_from_slice(&f.block_crc.to_le_bytes());
    }
    h.push(K_END);

    h.push(K_UNPACK_INFO);
    h.push(K_FOLDER);
    write_varint(folders.len() as u64, h);
    h.push(0);

    for f in folders {
        write_varint(1, h);
        let mut method_bytes = Vec::new();
        let mut temp_mid = f.method_id;
        while temp_mid > 0 {
            method_bytes.push((temp_mid & 0xFF) as u8);
            temp_mid >>= 8;
        }
        if method_bytes.is_empty() {
            method_bytes.push(0);
        }
        method_bytes.reverse();

        let mut coder_flags = (method_bytes.len() as u8) & 0x0F;
        if !f.coder_props.is_empty() {
            coder_flags |= 0x20;
        }
        h.push(coder_flags);
        h.extend_from_slice(&method_bytes);

        if !f.coder_props.is_empty() {
            write_varint(f.coder_props.len() as u64, h);
            h.extend_from_slice(&f.coder_props);
        }
    }

    h.push(K_CODERS_UNPACK_SIZE);
    for f in folders {
        write_varint(f.unpack_size, h);
    }
    h.push(K_END);
}

fn write_multi_folder_substreams_info(folders: &[FolderRecord], h: &mut Vec<u8>) {
    let total_substreams: usize = folders.iter().map(|f| f.substream_sizes.len()).sum();
    if total_substreams == 0 {
        return;
    }
    let has_multi_stream_folders = folders.iter().any(|f| f.substream_sizes.len() > 1);

    h.push(K_SUB_STREAMS_INFO);
    h.push(K_NUM_UNPACK_STREAM);
    for f in folders {
        write_varint(f.substream_sizes.len() as u64, h);
    }

    if has_multi_stream_folders {
        h.push(K_SIZE);
        for f in folders {
            if f.substream_sizes.len() > 1 {
                for &sz in &f.substream_sizes[..f.substream_sizes.len() - 1] {
                    write_varint(sz, h);
                }
            }
        }
    }

    h.push(K_CRC);
    h.push(1);
    for f in folders {
        for &c in &f.substream_crcs {
            h.extend_from_slice(&c.to_le_bytes());
        }
    }
    h.push(K_END);
}

fn write_multi_folder_files_info(files: &[FileRecord], h: &mut Vec<u8>) {
    h.push(K_FILES_INFO);
    write_varint(files.len() as u64, h);

    if !files.is_empty() {
        if files.iter().any(|f| f.is_empty_stream) {
            h.push(K_EMPTY_STREAM);
            let num_bytes = files.len().div_ceil(8);
            write_varint(num_bytes as u64, h);

            for chunk in files.chunks(8) {
                let mut byte = 0u8;
                for (bit, f) in chunk.iter().enumerate() {
                    if f.is_empty_stream {
                        byte |= 1 << (7 - bit);
                    }
                }
                h.push(byte);
            }
        }

        h.push(K_NAME);
        let mut names_u16_bytes = Vec::new();
        for f in files {
            for u in f.rel_path.encode_utf16() {
                names_u16_bytes.extend_from_slice(&u.to_le_bytes());
            }
            names_u16_bytes.extend_from_slice(&0u16.to_le_bytes());
        }
        write_varint((1 + names_u16_bytes.len()) as u64, h);
        h.push(0);
        h.extend_from_slice(&names_u16_bytes);

        h.push(K_WIN_ATTRIBUTES);
        let num_attr_bytes = 2 + (files.len() * 4);
        write_varint(num_attr_bytes as u64, h);
        h.push(1);
        h.push(0);
        for f in files {
            let attr: u32 = if f.is_directory { 0x10 } else { 0x20 };
            h.extend_from_slice(&attr.to_le_bytes());
        }
    }

    h.push(K_END);
}

/// Constructs 7z binary Metadata Header bytes for multi-folder solid archives.
fn build_multi_folder_7z_header(folders: &[FolderRecord], files: &[FileRecord]) -> Vec<u8> {
    let mut h = Vec::new();
    h.push(K_HEADER);

    if !folders.is_empty() {
        h.push(K_MAIN_STREAMS_INFO);
        write_multi_folder_pack_and_unpack_info(folders, &mut h);
        write_multi_folder_substreams_info(folders, &mut h);
        h.push(K_END);
    }

    write_multi_folder_files_info(files, &mut h);
    h.push(K_END);
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reusable_fl2_cstream_evicts_oversized_dictionary() {
        assert_eq!(MAX_REUSABLE_FL2_DICT_BYTES, 2 * 1024 * 1024);
        let entries = vec![(
            "block.bin".to_string(),
            4096u64,
            std::io::Cursor::new(vec![0x5Au8; 4096]),
        )];
        let large_opts = SevenZEncoderOptions {
            compression_level: 7,
            thread_budget: 1,
            dict_size: Some(16 * 1024 * 1024),
        };
        let block = prepare_block(entries, SevenZEncoderMethod::Lzma2, large_opts)
            .expect("prepare 16MB dict block");
        assert!(!block.compressed_data.is_empty());

        let retained_large = REUSABLE_FL2_CSTREAM.with(|cell| cell.borrow().is_some());
        assert!(
            !retained_large,
            "16 MiB dictionary CStream exceeds 64 MiB total workspace and must be evicted"
        );

        let small_entries = vec![(
            "small.bin".to_string(),
            4096u64,
            std::io::Cursor::new(vec![0x5Au8; 4096]),
        )];
        let small_opts = SevenZEncoderOptions {
            compression_level: 5,
            thread_budget: 1,
            dict_size: Some(1024 * 1024),
        };
        let small_block = prepare_block(small_entries, SevenZEncoderMethod::Lzma2, small_opts)
            .expect("prepare 1MB dict block");
        assert!(!small_block.compressed_data.is_empty());

        let retained_small = REUSABLE_FL2_CSTREAM.with(|cell| cell.borrow().is_some());
        assert!(
            retained_small,
            "1 MiB dictionary CStream fits within 64 MiB ceiling and must be retained"
        );
    }
}
