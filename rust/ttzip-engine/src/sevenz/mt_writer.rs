// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-Threaded Offline Solid Block Preparation and Sequential Append Pipeline.
//!
//! Provides `prepare_block` for parallel, CPU-bound solid compression across worker threads
//! and `SevenZArchiveWriter` for $O(1)$ lock-free sequential append and 7z metadata header finalization.

use std::io::{Read, Seek, SeekFrom, Write};

use crate::codecs::lzma2::{Fl2CParameter, Fl2CStream, Fl2InBuffer, Fl2OutBuffer};
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::dag::SevenZError;
use crate::sevenz::format::*;

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

    let mut substream_sizes = Vec::with_capacity(entries_data.len());
    let mut substream_crcs = Vec::with_capacity(entries_data.len());
    let mut empty_flags = Vec::with_capacity(entries_data.len());
    let mut total_unpack_size: u64 = 0;

    // Buffer uncompressed input payload and compute per-stream CRC32
    let mut uncompressed_payload = Vec::new();
    const READ_CHUNK_SIZE: usize = 64 * 1024;
    let mut read_buf = vec![0u8; READ_CHUNK_SIZE];

    for (name, size_hint, mut reader) in entries_data {
        if name.ends_with('/') {
            // Directory entry (empty stream)
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
            let bytes_read = reader
                .read(&mut read_buf)
                .map_err(|e| SevenZError::Io(e.to_string()))?;
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

    let (compressed_data, coder_props) = if method == SevenZEncoderMethod::Copy
        || total_unpack_size == 0
        || options.compression_level == 0
    {
        (uncompressed_payload, Vec::new())
    } else {
        let mut cstream = if options.thread_budget > 1 {
            Fl2CStream::new_mt(options.thread_budget)?
        } else {
            Fl2CStream::new()?
        };

        cstream.set_parameter(
            Fl2CParameter::CompressionLevel,
            options.compression_level as usize,
        )?;
        cstream.set_parameter(Fl2CParameter::OmitProperties, 1)?;
        if let Some(dict_sz) = options.dict_size {
            cstream.set_parameter(Fl2CParameter::DictionarySize, dict_sz as usize)?;
        }
        cstream.init(0)?;

        // 2MB bounded micro-buffer for chunked streaming compression pipeline
        const OUT_CHUNK_SIZE: usize = 2 * 1024 * 1024;
        let mut out_chunk = vec![0u8; OUT_CHUNK_SIZE];
        let mut comp_data = Vec::new();

        let mut in_offset = 0usize;
        while in_offset < uncompressed_payload.len() {
            let slice = &uncompressed_payload[in_offset..];
            let mut in_buf = Fl2InBuffer {
                src: slice.as_ptr() as *const libc::c_void,
                size: slice.len(),
                pos: 0,
            };

            while in_buf.pos < in_buf.size {
                let mut out_buf = Fl2OutBuffer {
                    dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                    size: out_chunk.len(),
                    pos: 0,
                };

                let res = cstream.compress_chunk(&mut in_buf, &mut out_buf)?;
                if out_buf.pos > 0 {
                    comp_data.extend_from_slice(&out_chunk[..out_buf.pos]);
                }
                if res == 0 && in_buf.pos == 0 {
                    break;
                }
            }
            in_offset += in_buf.pos;
        }

        // Finalize stream and drain all remaining compressed blocks
        loop {
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: out_chunk.len(),
                pos: 0,
            };
            let remaining = cstream.end_stream(&mut out_buf)?;
            if out_buf.pos > 0 {
                comp_data.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
            if remaining == 0 {
                break;
            }
        }

        let dict_prop = cstream.dict_property();
        (comp_data, vec![dict_prop])
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
        writer
            .write_all(&placeholder)
            .map_err(|e| SevenZError::Io(e.to_string()))?;

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
            self.writer
                .write_all(&block.compressed_data)
                .map_err(|e| SevenZError::Io(e.to_string()))?;
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

        self.writer
            .write_all(&header_bytes)
            .map_err(|e| SevenZError::Io(e.to_string()))?;

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

        self.writer
            .seek(SeekFrom::Start(0))
            .map_err(|e| SevenZError::Io(e.to_string()))?;

        self.writer
            .write_all(&sig_bytes)
            .map_err(|e| SevenZError::Io(e.to_string()))?;

        self.writer
            .flush()
            .map_err(|e| SevenZError::Io(e.to_string()))?;

        self.is_finalized = true;
        let total_size = 32 + self.current_payload_pos + (header_bytes.len() as u64);
        Ok(total_size)
    }
}

/// Constructs 7z binary Metadata Header bytes for multi-folder solid archives.
fn build_multi_folder_7z_header(folders: &[FolderRecord], files: &[FileRecord]) -> Vec<u8> {
    let mut h = Vec::new();
    h.push(K_HEADER);

    if !folders.is_empty() {
        // 1. kMainStreamsInfo
        h.push(K_MAIN_STREAMS_INFO);

        // 1.1 kPackInfo
        h.push(K_PACK_INFO);
        write_varint(0, &mut h); // packPos = 0
        write_varint(folders.len() as u64, &mut h); // numPackStreams
        h.push(K_SIZE);
        for f in folders {
            write_varint(f.pack_size, &mut h);
        }
        h.push(K_CRC);
        h.push(1); // allDefined = 1
        for f in folders {
            h.extend_from_slice(&f.block_crc.to_le_bytes());
        }
        h.push(K_END); // end kPackInfo

        // 1.2 kUnpackInfo
        h.push(K_UNPACK_INFO);
        h.push(K_FOLDER);
        write_varint(folders.len() as u64, &mut h); // numFolders
        h.push(0); // external = 0

        for f in folders {
            write_varint(1, &mut h); // numCoders = 1
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
                write_varint(f.coder_props.len() as u64, &mut h);
                h.extend_from_slice(&f.coder_props);
            }
        }

        // CodersUnpackSize
        h.push(K_CODERS_UNPACK_SIZE);
        for f in folders {
            write_varint(f.unpack_size, &mut h);
        }
        h.push(K_END);

        // 1.3 kSubStreamsInfo
        let total_substreams: usize = folders.iter().map(|f| f.substream_sizes.len()).sum();
        let has_multi_stream_folders = folders.iter().any(|f| f.substream_sizes.len() > 1);

        if total_substreams > 0 {
            h.push(K_SUB_STREAMS_INFO);
            h.push(K_NUM_UNPACK_STREAM);
            for f in folders {
                write_varint(f.substream_sizes.len() as u64, &mut h);
            }

            if has_multi_stream_folders {
                h.push(K_SIZE);
                for f in folders {
                    if f.substream_sizes.len() > 1 {
                        for &sz in &f.substream_sizes[..f.substream_sizes.len() - 1] {
                            write_varint(sz, &mut h);
                        }
                    }
                }
            }

            h.push(K_CRC);
            h.push(1); // allDefined = 1
            for f in folders {
                for &c in &f.substream_crcs {
                    h.extend_from_slice(&c.to_le_bytes());
                }
            }

            h.push(K_END); // end kSubStreamsInfo
        }

        h.push(K_END); // end kMainStreamsInfo
    }

    // 2. kFilesInfo
    h.push(K_FILES_INFO);
    write_varint(files.len() as u64, &mut h);

    if !files.is_empty() {
        // 2.1 kEmptyStream
        let has_empty = files.iter().any(|f| f.is_empty_stream);
        if has_empty {
            h.push(K_EMPTY_STREAM);
            let num_bytes = files.len().div_ceil(8);
            write_varint(num_bytes as u64, &mut h);

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

        // 2.2 kName
        h.push(K_NAME);
        let mut names_u16_bytes = Vec::new();
        for f in files {
            for u in f.rel_path.encode_utf16() {
                names_u16_bytes.extend_from_slice(&u.to_le_bytes());
            }
            names_u16_bytes.extend_from_slice(&0u16.to_le_bytes()); // Null terminator
        }
        write_varint((1 + names_u16_bytes.len()) as u64, &mut h);
        h.push(0); // external = 0
        h.extend_from_slice(&names_u16_bytes);

        // 2.3 kWinAttributes
        h.push(K_WIN_ATTRIBUTES);
        let num_attr_bytes = 2 + (files.len() * 4);
        write_varint(num_attr_bytes as u64, &mut h);
        h.push(1); // allDefined = 1
        h.push(0); // external = 0
        for f in files {
            let attr: u32 = if f.is_directory { 0x10 } else { 0x20 };
            h.extend_from_slice(&attr.to_le_bytes());
        }
    }

    h.push(K_END); // end kFilesInfo
    h.push(K_END); // end kHeader

    h
}
