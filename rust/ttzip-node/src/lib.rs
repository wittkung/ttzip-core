// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Node.js (N-API).

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::io::Cursor;
use std::path::Path;
use ttzip_engine::codecs::brotli::{brotli_compress, brotli_compress_bound, brotli_decompress};
use ttzip_engine::codecs::deflate::{deflate_compress, deflate_compress_bound, deflate_decompress};
use ttzip_engine::codecs::fast_blocks::{
    lz4_compress, lz4_compress_bound, lz4_decompress, snappy_compress,
    snappy_max_compressed_length, snappy_decompress, snappy_uncompressed_length,
};
use ttzip_engine::codecs::zstd::{
    zstd_compress, zstd_compress_bound, zstd_decompress, zstd_decompress_stream_pipe,
    zstd_get_decompressed_size,
};
use ttzip_engine::types::{TTZipCompressionLevel, TTZipStatus};

#[napi(object)]
pub struct NapiCompressOptions {
    pub format: Option<String>,
    pub level: Option<u32>,
    pub password: Option<String>,
    pub threads: Option<u32>,
    pub strip_components: Option<u32>,
}

#[napi(object)]
pub struct NapiExtractOptions {
    pub destination: String,
    pub password: Option<String>,
    pub overwrite: Option<bool>,
    pub strip_components: Option<u32>,
}

#[napi(object)]
pub struct NapiEntryMetadata {
    pub path: String,
    pub uncompressed_size: i64,
    pub compressed_size: i64,
    pub crc32: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub modified_timestamp: i64,
    pub compression_method: String,
}

#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[napi]
pub fn is_hardware_accelerated() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        true
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        is_x86_feature_detected!("avx2") && is_x86_feature_detected!("pclmulqdq")
    }
}

#[napi]
pub fn crc32(buffer: Buffer, seed: Option<u32>) -> u32 {
    let s = seed.unwrap_or(0);
    ttzip_engine::crypto::crc32_fast(s, buffer.as_ref())
}

#[napi]
pub fn crc64(buffer: Buffer, _seed: Option<i64>) -> i64 {
    ttzip_engine::crypto::crc64_fast(buffer.as_ref()) as i64
}

#[napi]
pub fn compress_buffer(
    buffer: Buffer,
    format: Option<String>,
    level: Option<u32>,
) -> Result<Buffer> {
    let fmt_str = format.as_deref().unwrap_or("deflate");
    let lvl = level.unwrap_or(6);
    let src = buffer.as_ref();

    let compressed = match fmt_str.to_lowercase().as_str() {
        "deflate" | "zip" => {
            let cl = (lvl as i32).clamp(0, 12);
            let bound = deflate_compress_bound(src.len(), cl);
            let mut dst = vec![0u8; bound];
            let written = deflate_compress(src, &mut dst, cl)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Deflate compression failed: {:?}", e)))?;
            dst.truncate(written);
            dst
        }
        "zstd" | "zstandard" => {
            let bound = zstd_compress_bound(src.len()) + 128;
            let mut dst = vec![0u8; bound];
            let written = zstd_compress(src, &mut dst, lvl as i32)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Zstd compression failed: {:?}", e)))?;
            dst.truncate(written);
            dst
        }
        "lz4" => {
            let bound = lz4_compress_bound(src.len()) + 4;
            let mut dst = vec![0u8; bound];
            let uncompressed_len = (src.len() as u32).to_le_bytes();
            dst[0..4].copy_from_slice(&uncompressed_len);
            let written = lz4_compress(src, &mut dst[4..])
                .map_err(|e| Error::new(Status::GenericFailure, format!("LZ4 compression failed: {:?}", e)))?;
            dst.truncate(written + 4);
            dst
        }
        "snappy" => {
            let bound = snappy_max_compressed_length(src.len());
            let mut dst = vec![0u8; bound];
            let written = snappy_compress(src, &mut dst)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Snappy compression failed: {:?}", e)))?;
            dst.truncate(written);
            dst
        }
        "brotli" => {
            let bound = brotli_compress_bound(src.len());
            let mut dst = vec![0u8; bound];
            let written = brotli_compress(src, &mut dst, lvl, 22)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Brotli compression failed: {:?}", e)))?;
            dst.truncate(written);
            dst
        }
        other => {
            return Err(Error::new(Status::InvalidArg, format!("Unsupported format: {}", other)));
        }
    };

    Ok(Buffer::from(compressed))
}

#[napi]
pub fn decompress_buffer(
    buffer: Buffer,
    format: Option<String>,
) -> Result<Buffer> {
    let fmt_str = format.as_deref().unwrap_or("deflate");
    let src = buffer.as_ref();

    let decompressed = match fmt_str.to_lowercase().as_str() {
        "deflate" | "zip" => {
            let mut dst = vec![0u8; (src.len() * 4 + 4096).max(65536)];
            loop {
                match deflate_decompress(src, &mut dst) {
                    Ok(written) => {
                        dst.truncate(written);
                        break dst;
                    }
                    Err(TTZipStatus::ErrExtractionFailed) if dst.len() < 2 * 1024 * 1024 * 1024 => {
                        dst.resize(dst.len() * 2, 0u8);
                    }
                    Err(st) => return Err(Error::new(Status::GenericFailure, format!("Deflate decompression failed: {:?}", st))),
                }
            }
        }
        "zstd" | "zstandard" => {
            let content_size = zstd_get_decompressed_size(src).unwrap_or(0);
            if content_size > 0 && content_size <= 2 * 1024 * 1024 * 1024 {
                let mut dst = vec![0u8; content_size as usize];
                if let Ok(written) = zstd_decompress(src, &mut dst) {
                    dst.truncate(written);
                    dst
                } else {
                    let mut cursor = Cursor::new(src);
                    let mut out_buf = Vec::with_capacity(src.len() * 4 + 4096);
                    zstd_decompress_stream_pipe(&mut cursor, &mut out_buf, None)
                        .map_err(|st| Error::new(Status::GenericFailure, format!("Zstd stream decompression failed: {:?}", st)))?;
                    out_buf
                }
            } else {
                let mut cursor = Cursor::new(src);
                let mut out_buf = Vec::with_capacity(src.len() * 4 + 4096);
                zstd_decompress_stream_pipe(&mut cursor, &mut out_buf, None)
                    .map_err(|st| Error::new(Status::GenericFailure, format!("Zstd decompression failed: {:?}", st)))?;
                out_buf
            }
        }
        "lz4" => {
            if src.len() >= 4 {
                let uncompressed_len = u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize;
                if uncompressed_len > 0 && uncompressed_len <= 1024 * 1024 * 1024 {
                    let mut dst = vec![0u8; uncompressed_len];
                    let written = lz4_decompress(&src[4..], &mut dst)
                        .map_err(|st| Error::new(Status::GenericFailure, format!("LZ4 decompression failed: {:?}", st)))?;
                    dst.truncate(written);
                    dst
                } else {
                    let mut dst = vec![0u8; src.len() * 8 + 4096];
                    let written = lz4_decompress(src, &mut dst)
                        .map_err(|st| Error::new(Status::GenericFailure, format!("LZ4 decompression failed: {:?}", st)))?;
                    dst.truncate(written);
                    dst
                }
            } else {
                let mut dst = vec![0u8; src.len() * 8 + 4096];
                let written = lz4_decompress(src, &mut dst)
                    .map_err(|st| Error::new(Status::GenericFailure, format!("LZ4 decompression failed: {:?}", st)))?;
                dst.truncate(written);
                dst
            }
        }
        "snappy" => {
            let uncomp_len = snappy_uncompressed_length(src)
                .map_err(|st| Error::new(Status::GenericFailure, format!("Snappy length parse error: {:?}", st)))?;
            let mut dst = vec![0u8; uncomp_len];
            let written = snappy_decompress(src, &mut dst)
                .map_err(|st| Error::new(Status::GenericFailure, format!("Snappy decompression failed: {:?}", st)))?;
            dst.truncate(written);
            dst
        }
        "brotli" => {
            let mut dst = vec![0u8; src.len() * 8 + 65536];
            let written = brotli_decompress(src, &mut dst)
                .map_err(|st| Error::new(Status::GenericFailure, format!("Brotli decompression failed: {:?}", st)))?;
            dst.truncate(written);
            dst
        }
        other => {
            return Err(Error::new(Status::InvalidArg, format!("Unsupported format: {}", other)));
        }
    };

    Ok(Buffer::from(decompressed))
}

#[napi]
pub fn decompress_into(
    compressed: Buffer,
    mut target: Buffer,
    format: Option<String>,
) -> Result<u32> {
    let fmt_str = format.as_deref().unwrap_or("deflate");
    let decomp = decompress_buffer(compressed, Some(fmt_str.to_string()))?;
    let slice = decomp.as_ref();
    let target_mut = target.as_mut();

    if target_mut.len() < slice.len() {
        return Err(Error::new(
            Status::InvalidArg,
            format!("Target buffer too small: {} < {}", target_mut.len(), slice.len()),
        ));
    }

    target_mut[..slice.len()].copy_from_slice(slice);
    Ok(slice.len() as u32)
}

#[napi]
pub fn compress(
    inputs: Vec<String>,
    destination: String,
    options: Option<NapiCompressOptions>,
) -> Result<()> {
    let dest_path = Path::new(&destination);
    let mut builder = ttzip_engine::archive::ArchiveBuilder::new().destination(dest_path);

    if let Some(opts) = options {
        if let Some(lvl) = opts.level {
            let comp_lvl = match lvl {
                0 => TTZipCompressionLevel::Store,
                1..=2 => TTZipCompressionLevel::Fastest,
                3..=5 => TTZipCompressionLevel::Fast,
                6..=8 => TTZipCompressionLevel::Normal,
                9..=11 => TTZipCompressionLevel::Maximum,
                _ => TTZipCompressionLevel::Ultra,
            };
            builder = builder.level(comp_lvl);
        }
        if let Some(pwd) = opts.password {
            builder = builder.password(pwd);
        }
        if let Some(th) = opts.threads {
            builder = builder.thread_budget(th);
        }
    }

    for input in &inputs {
        builder = builder.add_source(input);
    }

    builder
        .build()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Archive creation failed: {:?}", e)))?;

    Ok(())
}

#[napi]
pub fn extract(
    archive_path: String,
    destination: String,
    options: Option<NapiExtractOptions>,
) -> Result<()> {
    let mut extractor = ttzip_engine::archive::ExtractBuilder::new()
        .source(&archive_path)
        .destination(&destination);

    if let Some(opts) = options {
        if let Some(pwd) = opts.password {
            extractor = extractor.password(pwd);
        }
        if let Some(ovw) = opts.overwrite {
            extractor = extractor.overwrite(ovw);
        }
    }

    extractor
        .extract()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Archive extraction failed: {:?}", e)))?;

    Ok(())
}

#[napi]
pub fn inspect(
    archive_path: String,
    password: Option<String>,
) -> Result<Vec<NapiEntryMetadata>> {
    let mut reader = ttzip_engine::archive::ArchiveReader::open(&archive_path)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open archive: {:?}", e)))?;

    if let Some(pwd) = password {
        reader = reader.with_password(pwd);
    }

    let entries = reader
        .entries()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to read entries: {:?}", e)))?;

    let mut result = Vec::with_capacity(entries.len());

    for entry in entries {
        result.push(NapiEntryMetadata {
            path: entry.path,
            uncompressed_size: entry.uncompressed_size as i64,
            compressed_size: entry.compressed_size as i64,
            crc32: entry.crc32,
            is_directory: entry.is_directory,
            is_encrypted: entry.is_encrypted,
            modified_timestamp: entry.mtime_epoch_secs,
            compression_method: format!("{}", entry.compression_method),
        });
    }

    Ok(result)
}
