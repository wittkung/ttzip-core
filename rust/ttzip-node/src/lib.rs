// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Node.js (N-API).

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::Path;

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
    ttzip_engine::crypto::checksum::crc32_fast(s, buffer.as_ref())
}

#[napi]
pub fn crc64(buffer: Buffer, seed: Option<i64>) -> i64 {
    let s = seed.unwrap_or(0) as u64;
    ttzip_engine::crypto::checksum::crc64_fast(s, buffer.as_ref()) as i64
}

#[napi]
pub fn compress_buffer(
    buffer: Buffer,
    format: Option<String>,
    level: Option<u32>,
) -> Result<Buffer> {
    let fmt_str = format.as_deref().unwrap_or("deflate");
    let lvl = level.unwrap_or(6);

    let compressed = match fmt_str.to_lowercase().as_str() {
        "deflate" | "zip" => {
            ttzip_engine::codecs::deflate::deflate_compress(buffer.as_ref(), lvl)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Deflate compression failed: {:?}", e)))?
        }
        "zstd" | "zstandard" => {
            ttzip_engine::codecs::zstd::zstd_compress(buffer.as_ref(), lvl as i32)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Zstd compression failed: {:?}", e)))?
        }
        "lz4" => {
            ttzip_engine::codecs::fast_blocks::lz4_compress_block(buffer.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, format!("LZ4 compression failed: {:?}", e)))?
        }
        "snappy" => {
            ttzip_engine::codecs::snappy::snappy_compress(buffer.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, format!("Snappy compression failed: {:?}", e)))?
        }
        "brotli" => {
            ttzip_engine::codecs::brotli::brotli_compress(buffer.as_ref(), lvl)
                .map_err(|e| Error::new(Status::GenericFailure, format!("Brotli compression failed: {:?}", e)))?
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

    let decompressed = match fmt_str.to_lowercase().as_str() {
        "deflate" | "zip" => {
            ttzip_engine::codecs::deflate::deflate_decompress(buffer.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, format!("Deflate decompression failed: {:?}", e)))?
        }
        "zstd" | "zstandard" => {
            ttzip_engine::codecs::zstd::zstd_decompress(buffer.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, format!("Zstd decompression failed: {:?}", e)))?
        }
        "lz4" => {
            ttzip_engine::codecs::fast_blocks::lz4_decompress_block(buffer.as_ref(), 128 * 1024 * 1024)
                .map_err(|e| Error::new(Status::GenericFailure, format!("LZ4 decompression failed: {:?}", e)))?
        }
        "snappy" => {
            ttzip_engine::codecs::snappy::snappy_decompress(buffer.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, format!("Snappy decompression failed: {:?}", e)))?
        }
        "brotli" => {
            ttzip_engine::codecs::brotli::brotli_decompress(buffer.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, format!("Brotli decompression failed: {:?}", e)))?
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
    let mut builder = ttzip_engine::archive::ArchiveBuilder::new(dest_path);

    if let Some(opts) = options {
        if let Some(lvl) = opts.level {
            builder = builder.level(lvl);
        }
        if let Some(pwd) = opts.password {
            builder = builder.password(pwd);
        }
    }

    for input in &inputs {
        let p = Path::new(input);
        if p.is_dir() {
            builder.add_directory(p, "")
                .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to add dir {}: {:?}", input, e)))?;
        } else if p.is_file() {
            builder.add_file(p, "")
                .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to add file {}: {:?}", input, e)))?;
        }
    }

    builder.build()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Archive creation failed: {:?}", e)))?;

    Ok(())
}

#[napi]
pub fn extract(
    archive_path: String,
    destination: String,
    options: Option<NapiExtractOptions>,
) -> Result<()> {
    let src = Path::new(&archive_path);
    let dest = Path::new(&destination);

    let password = options.as_ref().and_then(|o| o.password.as_deref());

    let mut extractor = ttzip_engine::archive::ExtractBuilder::new(src, dest);
    if let Some(pwd) = password {
        extractor = extractor.password(pwd);
    }

    extractor.extract()
        .map_err(|e| Error::new(Status::GenericFailure, format!("Archive extraction failed: {:?}", e)))?;

    Ok(())
}

#[napi]
pub fn inspect(
    archive_path: String,
    password: Option<String>,
) -> Result<Vec<NapiEntryMetadata>> {
    let src = Path::new(&archive_path);
    let reader = ttzip_engine::archive::ArchiveReader::open_with_password(src, password.as_deref())
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to open archive: {:?}", e)))?;

    let entries = reader.entries();
    let mut result = Vec::with_capacity(entries.len());

    for entry in entries {
        result.push(NapiEntryMetadata {
            path: entry.path.clone(),
            uncompressed_size: entry.uncompressed_size as i64,
            compressed_size: entry.compressed_size as i64,
            crc32: entry.crc32,
            is_directory: entry.is_directory,
            is_encrypted: entry.is_encrypted,
            modified_timestamp: entry.modified_time as i64,
            compression_method: format!("{:?}", entry.method),
        });
    }

    Ok(result)
}
