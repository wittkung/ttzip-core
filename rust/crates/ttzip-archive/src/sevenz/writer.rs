// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! 7-Zip Solid Archive Creation and Header Serialization Engine.
//!
//! Features multi-threaded Fast-LZMA2 solid compression, Store mode, UTF-16LE metadata encoding,
//! and accurate 7z SignatureHeader CRC calculations.

use crate::codecs::lzma2::{Fl2CStream, Fl2InBuffer, Fl2OutBuffer};
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::format::*;
use crate::types::{TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};
use crate::zip::writer::{collect_zip_input_items, ZipCreateReport, ZipInputItem};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Configuration parameters for 7z AES-256-CBC encryption in the metadata header.
#[derive(Debug, Clone)]
pub struct SevenZAesWriterConfig {
    pub salt: [u8; 16],
    pub iv: [u8; 16],
    pub num_cycles_power: u32,
    pub raw_comp_len: u64,
}

/// Constructs 7z Metadata Header bytes for a solid block of items, supporting optional AES-256 coder.
pub fn build_7z_metadata_header_full(
    items: &[ZipInputItem],
    stream_sizes: &[u64],
    stream_crcs: &[u32],
    compressed_len: u64,
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
    aes_config: Option<&SevenZAesWriterConfig>,
) -> Vec<u8> {
    let mut h = Vec::new();

    // kHeader
    h.push(K_HEADER);

    // 1. kMainStreamsInfo
    h.push(K_MAIN_STREAMS_INFO);

    // 1.1 kPackInfo
    h.push(K_PACK_INFO);
    write_varint(0, &mut h); // packPos = 0
    write_varint(1, &mut h); // numPackStreams = 1
    h.push(K_SIZE);
    write_varint(compressed_len, &mut h);
    h.push(K_END);

    // 1.2 kUnpackInfo
    h.push(K_UNPACK_INFO);
    h.push(K_FOLDER);
    write_varint(1, &mut h); // numFolders = 1
    h.push(0); // external = 0

    if let Some(aes) = aes_config {
        write_varint(2, &mut h); // numCoders = 2

        // Coder 0: AES-256-CBC (METHOD_AES = 0x06F10701)
        let mut aes_props = Vec::with_capacity(34);
        let b0 = 0x80 | 0x40 | ((aes.num_cycles_power as u8) & 0x3F);
        let b1 = 0xFF; // (15 << 4) | 15 (salt 16 bytes, iv 16 bytes)
        aes_props.push(b0);
        aes_props.push(b1);
        aes_props.extend_from_slice(&aes.salt);
        aes_props.extend_from_slice(&aes.iv);

        let coder_flags = 4 | 0x20; // 4-byte method id + properties flag
        h.push(coder_flags);
        h.extend_from_slice(&[0x06, 0xF1, 0x07, 0x01]);
        write_varint(aes_props.len() as u64, &mut h);
        h.extend_from_slice(&aes_props);

        // Coder 1: Compression coder (LZMA2 or COPY)
        let mut method_bytes = Vec::new();
        let mut temp_mid = method_id;
        while temp_mid > 0 {
            method_bytes.push((temp_mid & 0xFF) as u8);
            temp_mid >>= 8;
        }
        if method_bytes.is_empty() {
            method_bytes.push(0);
        }
        method_bytes.reverse();

        let mut comp_flags = (method_bytes.len() as u8) & 0x0F;
        if !coder_props.is_empty() {
            comp_flags |= 0x20;
        }
        h.push(comp_flags);
        h.extend_from_slice(&method_bytes);

        if !coder_props.is_empty() {
            write_varint(coder_props.len() as u64, &mut h);
            h.extend_from_slice(coder_props);
        }

        // BindPairs: in_stream 1 (Coder 1 LZMA2 input) bound to out_stream 0 (Coder 0 AES output)
        write_varint(1, &mut h); // in_index = 1
        write_varint(0, &mut h); // out_index = 0

        // kCodersUnpackSize
        h.push(K_CODERS_UNPACK_SIZE);
        write_varint(aes.raw_comp_len, &mut h); // Coder 0 unpack size
        write_varint(uncompressed_len, &mut h); // Coder 1 unpack size
        h.push(K_END);
    } else {
        // Coder in Folder
        write_varint(1, &mut h); // numCoders = 1

        let mut method_bytes = Vec::new();
        let mut temp_mid = method_id;
        while temp_mid > 0 {
            method_bytes.push((temp_mid & 0xFF) as u8);
            temp_mid >>= 8;
        }
        if method_bytes.is_empty() {
            method_bytes.push(0);
        }
        method_bytes.reverse();

        let mut coder_flags = (method_bytes.len() as u8) & 0x0F;
        if !coder_props.is_empty() {
            coder_flags |= 0x20; // has properties
        }
        h.push(coder_flags);
        h.extend_from_slice(&method_bytes);

        if !coder_props.is_empty() {
            write_varint(coder_props.len() as u64, &mut h);
            h.extend_from_slice(coder_props);
        }

        // kCodersUnpackSize
        h.push(K_CODERS_UNPACK_SIZE);
        write_varint(uncompressed_len, &mut h);
        h.push(K_END);
    }

    // 1.3 kSubStreamsInfo
    if stream_sizes.len() > 1 {
        h.push(K_SUB_STREAMS_INFO);
        h.push(K_NUM_UNPACK_STREAM);
        write_varint(stream_sizes.len() as u64, &mut h);

        h.push(K_SIZE);
        for &sz in &stream_sizes[..stream_sizes.len() - 1] {
            write_varint(sz, &mut h);
        }

        h.push(K_CRC);
        h.push(1); // allDefined = 1
        for &c in stream_crcs {
            h.extend_from_slice(&c.to_le_bytes());
        }

        h.push(K_END);
    } else if stream_sizes.len() == 1 && !stream_crcs.is_empty() {
        h.push(K_SUB_STREAMS_INFO);
        h.push(K_CRC);
        h.push(1);
        h.extend_from_slice(&stream_crcs[0].to_le_bytes());
        h.push(K_END);
    }

    h.push(K_END); // end kMainStreamsInfo

    // 2. kFilesInfo
    h.push(K_FILES_INFO);
    write_varint(items.len() as u64, &mut h);

    // 2.1 kEmptyStream
    let has_empty = items.iter().any(|it| it.is_directory || it.data.is_empty());
    if has_empty {
        h.push(K_EMPTY_STREAM);
        let num_bytes = items.len().div_ceil(8);
        write_varint(num_bytes as u64, &mut h);

        for chunk in items.chunks(8) {
            let mut byte = 0u8;
            for (bit, item) in chunk.iter().enumerate() {
                if item.is_directory || item.data.is_empty() {
                    byte |= 1 << (7 - bit);
                }
            }
            h.push(byte);
        }
    }

    // 2.2 kName
    h.push(K_NAME);
    let mut names_u16_bytes = Vec::new();
    for item in items {
        for u in item.rel_path.encode_utf16() {
            names_u16_bytes.extend_from_slice(&u.to_le_bytes());
        }
        names_u16_bytes.extend_from_slice(&0u16.to_le_bytes()); // Null terminator
    }
    write_varint((1 + names_u16_bytes.len()) as u64, &mut h);
    h.push(0); // external = 0
    h.extend_from_slice(&names_u16_bytes);

    // 2.3 kWinAttributes
    h.push(K_WIN_ATTRIBUTES);
    let num_attr_bytes = 2 + (items.len() * 4);
    write_varint(num_attr_bytes as u64, &mut h);
    h.push(1); // allDefined = 1
    h.push(0); // external = 0
    for item in items {
        let attr: u32 = if item.is_directory { 0x10 } else { 0x20 };
        h.extend_from_slice(&attr.to_le_bytes());
    }

    h.push(K_END); // end kFilesInfo
    h.push(K_END); // end kHeader

    h
}

/// Constructs 7z Metadata Header bytes for a solid block of items without encryption.
pub fn build_7z_metadata_header(
    items: &[ZipInputItem],
    stream_sizes: &[u64],
    stream_crcs: &[u32],
    compressed_len: u64,
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
) -> Vec<u8> {
    build_7z_metadata_header_full(
        items,
        stream_sizes,
        stream_crcs,
        compressed_len,
        uncompressed_len,
        method_id,
        coder_props,
        None,
    )
}

/// Creates a complete 7z Solid archive from input items with optional AES-256 encryption.
pub fn create_7z_solid_archive_bytes_with_password(
    items: &[ZipInputItem],
    level: i32,
    threads: u32,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    if items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut stream_sizes = Vec::with_capacity(items.len());
    let mut stream_crcs = Vec::with_capacity(items.len());
    let mut total_uncompressed_len: u64 = 0;

    for item in items {
        if !item.is_directory && !item.data.is_empty() {
            let crc = crc32_fast(0, &item.data);
            stream_sizes.push(item.data.len() as u64);
            stream_crcs.push(crc);
            total_uncompressed_len += item.data.len() as u64;
        }
    }

    let uncompressed_len = total_uncompressed_len;
    let (method_id, compressed_payload, coder_props) = if level == 0 || uncompressed_len == 0 {
        let mut store_payload = Vec::with_capacity(total_uncompressed_len as usize);
        for item in items {
            if !item.is_directory && !item.data.is_empty() {
                store_payload.extend_from_slice(&item.data);
            }
        }
        (METHOD_COPY, store_payload, Vec::new())
    } else {
        let mut cstream = if threads > 1 {
            Fl2CStream::new_mt(threads)?
        } else {
            Fl2CStream::new()?
        };
        cstream.set_parameter(crate::codecs::lzma2::Fl2CParameter::CompressionLevel, level as usize)?;
        cstream.set_parameter(crate::codecs::lzma2::Fl2CParameter::OmitProperties, 1)?;
        cstream.init(0)?;

        // 2MB bounded micro-buffer for chunked streaming compression pipeline
        const CHUNK_SIZE: usize = 2 * 1024 * 1024;
        let mut out_chunk = vec![0u8; CHUNK_SIZE];
        let mut comp_payload = Vec::new();

        for item in items {
            if item.is_directory || item.data.is_empty() {
                continue;
            }

            let mut in_offset = 0;
            while in_offset < item.data.len() {
                let slice = &item.data[in_offset..];
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
                        comp_payload.extend_from_slice(&out_chunk[..out_buf.pos]);
                    }
                    if res == 0 && in_buf.pos == 0 {
                        break;
                    }
                }
                in_offset += in_buf.pos;
            }
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
                comp_payload.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
            if remaining == 0 {
                break;
            }
        }

        let dict_prop = cstream.dict_property();
        (METHOD_LZMA2, comp_payload, vec![dict_prop])
    };

    let (final_payload, aes_config) = if let Some(pass) = password.filter(|p| !p.is_empty()) {
        let raw_comp_len = compressed_payload.len() as u64;
        let mut salt = [0u8; 16];
        let mut iv = [0u8; 16];
        unsafe {
            libc::arc4random_buf(salt.as_mut_ptr() as *mut libc::c_void, 16);
            libc::arc4random_buf(iv.as_mut_ptr() as *mut libc::c_void, 16);
        }
        let num_cycles_power = 19u32;
        let key = crate::crypto::sha256::sha256_7z_kdf(pass, &salt, num_cycles_power);

        let padded_len = compressed_payload.len().div_ceil(16).max(1) * 16;
        let mut padded_buf = compressed_payload;
        padded_buf.resize(padded_len, 0);

        let mut encrypted = vec![0u8; padded_len];
        crate::crypto::aes256::aes256_cbc_encrypt(&key, &iv, &padded_buf, &mut encrypted)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        let cfg = SevenZAesWriterConfig {
            salt,
            iv,
            num_cycles_power,
            raw_comp_len,
        };
        (encrypted, Some(cfg))
    } else {
        (compressed_payload, None)
    };

    let compressed_len = final_payload.len() as u64;

    let header_bytes = build_7z_metadata_header_full(
        items,
        &stream_sizes,
        &stream_crcs,
        compressed_len,
        uncompressed_len,
        method_id,
        &coder_props,
        aes_config.as_ref(),
    );

    let next_header_offset = compressed_len;
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

    let mut out = Vec::with_capacity(32 + (compressed_len as usize) + (next_header_size as usize));
    out.extend_from_slice(&sig_bytes);
    out.extend_from_slice(&final_payload);
    out.extend_from_slice(&header_bytes);

    Ok(out)
}

/// Creates a complete 7z Solid archive from input items using a bounded 2MB chunked streaming pipeline.
pub fn create_7z_solid_archive_bytes(
    items: &[ZipInputItem],
    level: i32,
    threads: u32,
) -> Result<Vec<u8>, TTZipStatus> {
    create_7z_solid_archive_bytes_with_password(items, level, threads, None)
}

/// Creates a 7z archive file directly from input source paths.
pub fn create_7z_archive(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();

    let mut input_items = Vec::new();
    for src in source_paths {
        if !src.exists() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let file_name = src.file_name().unwrap_or_default().to_string_lossy();
        collect_zip_input_items(src, &file_name, &mut input_items)?;
    }

    let level_int = match options.level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 12,
    };

    let threads = options.thread_budget.max(1);
    let password = if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password).to_str().ok() }
    } else {
        None
    };

    let binary_bytes = create_7z_solid_archive_bytes_with_password(&input_items, level_int, threads, password)?;

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    }

    let mut file = File::create(dest_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&binary_bytes).map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    let total_uncomp_bytes: u64 = input_items.iter().map(|it| it.data.len() as u64).sum();

    Ok(ZipCreateReport {
        total_entries: input_items.len(),
        total_uncompressed_bytes: total_uncomp_bytes,
        total_compressed_bytes: binary_bytes.len() as u64,
        duration_ms: start_time.elapsed().as_millis() as u64,
    })
}

/// Creates a 7z archive file directly from explicit pre-collected entries.
pub fn create_7z_archive_explicit(
    dest_path: &Path,
    entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = std::time::Instant::now();

    let mut input_items = Vec::with_capacity(entries.len());
    for (disk_path, rel_path) in entries {
        if disk_path.is_dir() {
            input_items.push(ZipInputItem {
                rel_path: rel_path.clone(),
                data: Vec::new(),
                mtime_epoch_secs: 0,
                mode: 0o755,
                is_directory: true,
            });
        } else {
            let data = fs::read(disk_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            input_items.push(ZipInputItem {
                rel_path: rel_path.clone(),
                data,
                mtime_epoch_secs: 0,
                mode: 0o644,
                is_directory: false,
            });
        }
    }

    let level_int = match options.level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 12,
    };

    let threads = options.thread_budget.max(1);
    let password = if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password).to_str().ok() }
    } else {
        None
    };

    let binary_bytes = create_7z_solid_archive_bytes_with_password(&input_items, level_int, threads, password)?;

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    }

    let mut file = File::create(dest_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    file.write_all(&binary_bytes).map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    let total_uncomp_bytes: u64 = input_items.iter().map(|it| it.data.len() as u64).sum();

    Ok(ZipCreateReport {
        total_entries: input_items.len(),
        total_uncompressed_bytes: total_uncomp_bytes,
        total_compressed_bytes: binary_bytes.len() as u64,
        duration_ms: start_time.elapsed().as_millis() as u64,
    })
}
