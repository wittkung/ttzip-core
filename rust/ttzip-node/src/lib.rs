// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

mod relocate;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::io::Cursor;
use std::path::Path;
pub(crate) use relocate::{relocate_stripped_tree, resolve_extract_collision_policy};
use ttzip_engine::codecs::brotli::{brotli_compress, brotli_compress_bound, brotli_decompress};
use ttzip_engine::codecs::deflate::{deflate_compress, deflate_compress_bound, deflate_decompress};
use ttzip_engine::codecs::fast_blocks::{
    lz4_compress, lz4_compress_bound, lz4_decompress, snappy_compress, snappy_decompress,
    snappy_decompress_to_vec, snappy_max_compressed_length,
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

fn decompress_deflate_slice(src: &[u8]) -> Result<Vec<u8>> {
    let mut dst = vec![0u8; (src.len() * 4 + 4096).max(65536)];
    loop {
        match deflate_decompress(src, &mut dst) {
            Ok(written) => {
                dst.truncate(written);
                return Ok(dst);
            }
            Err(TTZipStatus::ErrExtractionFailed) if dst.len() < 2 * 1024 * 1024 * 1024 => {
                dst.resize(dst.len() * 2, 0u8);
            }
            Err(st) => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("Deflate decompression failed: {:?}", st),
                ));
            }
        }
    }
}

fn decompress_zstd_slice(src: &[u8]) -> Result<Vec<u8>> {
    let content_size = zstd_get_decompressed_size(src).unwrap_or(0);
    if content_size > 0 && content_size <= 2 * 1024 * 1024 * 1024 {
        let mut dst = vec![0u8; content_size as usize];
        if let Ok(written) = zstd_decompress(src, &mut dst) {
            dst.truncate(written);
            return Ok(dst);
        }
    }
    let mut cursor = Cursor::new(src);
    let mut out_buf = Vec::with_capacity(src.len() * 4 + 4096);
    zstd_decompress_stream_pipe(&mut cursor, &mut out_buf, None).map_err(|st| {
        Error::new(
            Status::GenericFailure,
            format!("Zstd decompression failed: {:?}", st),
        )
    })?;
    Ok(out_buf)
}

fn decompress_lz4_slice(src: &[u8]) -> Result<Vec<u8>> {
    if src.len() >= 4 {
        let uncompressed_len = u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize;
        if uncompressed_len > 0 && uncompressed_len <= 1024 * 1024 * 1024 {
            let mut dst = vec![0u8; uncompressed_len];
            let written = lz4_decompress(&src[4..], &mut dst).map_err(|st| {
                Error::new(
                    Status::GenericFailure,
                    format!("LZ4 decompression failed: {:?}", st),
                )
            })?;
            dst.truncate(written);
            return Ok(dst);
        }
    }
    let mut dst = vec![0u8; src.len() * 8 + 4096];
    let written = lz4_decompress(src, &mut dst).map_err(|st| {
        Error::new(
            Status::GenericFailure,
            format!("LZ4 decompression failed: {:?}", st),
        )
    })?;
    dst.truncate(written);
    Ok(dst)
}

#[napi]
pub fn decompress_buffer(
    buffer: Buffer,
    format: Option<String>,
) -> Result<Buffer> {
    let fmt_str = format.as_deref().unwrap_or("deflate");
    let src = buffer.as_ref();

    let decompressed = match fmt_str.to_lowercase().as_str() {
        "deflate" | "zip" => decompress_deflate_slice(src)?,
        "zstd" | "zstandard" => decompress_zstd_slice(src)?,
        "lz4" => decompress_lz4_slice(src)?,
        "snappy" => snappy_decompress_to_vec(src)
            .map_err(|st| Error::new(Status::GenericFailure, format!("Snappy decompression failed: {:?}", st)))?,
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

fn map_direct_decode_status(label: &str, st: TTZipStatus) -> (Status, String) {
    match st {
        TTZipStatus::ErrExtractionFailed | TTZipStatus::ErrOutOfMemory => (
            Status::InvalidArg,
            format!("{} target buffer too small or decompression failed: {:?}", label, st),
        ),
        other => (
            Status::GenericFailure,
            format!("{} decompression failed: {:?}", label, other),
        ),
    }
}

pub(crate) fn decompress_slice_into(
    src: &[u8],
    target_mut: &mut [u8],
    fmt_str: &str,
) -> std::result::Result<usize, (Status, String)> {
    match fmt_str.to_lowercase().as_str() {
        "deflate" | "zip" => deflate_decompress(src, target_mut)
            .map_err(|st| map_direct_decode_status("Deflate", st)),
        "zstd" | "zstandard" => {
            if let Some(content_size) = zstd_get_decompressed_size(src) {
                if content_size > 0 && target_mut.len() < content_size as usize {
                    return Err((
                        Status::InvalidArg,
                        format!(
                            "Target buffer too small: {} < {}",
                            target_mut.len(),
                            content_size
                        ),
                    ));
                }
            }
            zstd_decompress(src, target_mut).map_err(|st| map_direct_decode_status("Zstd", st))
        }
        "lz4" => {
            if src.len() >= 4 {
                let uncompressed_len =
                    u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize;
                if uncompressed_len > 0 && uncompressed_len <= 1024 * 1024 * 1024 {
                    if target_mut.len() < uncompressed_len {
                        return Err((
                            Status::InvalidArg,
                            format!(
                                "Target buffer too small: {} < {}",
                                target_mut.len(),
                                uncompressed_len
                            ),
                        ));
                    }
                    return lz4_decompress(&src[4..], &mut target_mut[..uncompressed_len])
                        .map_err(|st| map_direct_decode_status("LZ4", st));
                }
            }
            lz4_decompress(src, target_mut).map_err(|st| map_direct_decode_status("LZ4", st))
        }
        "snappy" => snappy_decompress(src, target_mut)
            .map_err(|st| map_direct_decode_status("Snappy", st)),
        "brotli" => brotli_decompress(src, target_mut)
            .map_err(|st| map_direct_decode_status("Brotli", st)),
        other => Err((Status::InvalidArg, format!("Unsupported format: {}", other))),
    }
}

pub(crate) fn ranges_overlap(
    src_ptr: *const u8,
    src_len: usize,
    dst_ptr: *const u8,
    dst_len: usize,
) -> bool {
    if src_len == 0 || dst_len == 0 {
        return false;
    }
    let (src_start, dst_start) = (src_ptr as usize, dst_ptr as usize);
    src_start < dst_start.saturating_add(dst_len) && dst_start < src_start.saturating_add(src_len)
}

pub(crate) unsafe fn decompress_raw_into(
    src_ptr: *const u8,
    src_len: usize,
    dst_ptr: *mut u8,
    dst_len: usize,
    fmt_str: &str,
) -> std::result::Result<usize, (Status, String)> {
    if ranges_overlap(src_ptr, src_len, dst_ptr, dst_len) {
        let src_copy = unsafe { std::slice::from_raw_parts(src_ptr, src_len) }.to_vec();
        let target_mut = unsafe { std::slice::from_raw_parts_mut(dst_ptr, dst_len) };
        decompress_slice_into(&src_copy, target_mut, fmt_str)
    } else {
        let src = if src_len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(src_ptr, src_len) }
        };
        let target_mut = if dst_len == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(dst_ptr, dst_len) }
        };
        decompress_slice_into(src, target_mut, fmt_str)
    }
}

#[napi]
pub fn decompress_into(
    compressed: Buffer,
    mut target: Buffer,
    format: Option<String>,
) -> Result<u32> {
    let fmt_str = format.as_deref().unwrap_or("deflate");
    let (src_ptr, src_len) = (compressed.as_ref().as_ptr(), compressed.len());
    let (dst_ptr, dst_len) = (target.as_mut().as_mut_ptr(), target.len());
    let written = unsafe { decompress_raw_into(src_ptr, src_len, dst_ptr, dst_len, fmt_str) }
        .map_err(|(status, msg)| Error::new(status, msg))?;
    Ok(written as u32)
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

    let mut strip_components = 0u32;
    let mut collision_policy = resolve_extract_collision_policy(None);

    if let Some(opts) = options {
        if let Some(pwd) = opts.password {
            extractor = extractor.password(pwd);
        }
        collision_policy = resolve_extract_collision_policy(opts.overwrite);
        if let Some(ovw) = opts.overwrite {
            extractor = extractor.overwrite(ovw);
        }
        if let Some(strip) = opts.strip_components {
            strip_components = strip;
        }
    }

    if strip_components == 0 {
        extractor
            .extract()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Archive extraction failed: {:?}", e)))?;
        return Ok(());
    }

    let dest_path = Path::new(&destination);
    let jail = ttzip_engine::fs::DirJail::open(dest_path).map_err(|st| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to open destination directory jail: {:?}", st),
        )
    })?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stage_dir = dest_path.join(format!(".ttzip_strip_stage_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&stage_dir).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to create staging directory: {}", e),
        )
    })?;

    let extract_res = extractor.extract_to(&stage_dir);
    if let Err(e) = extract_res {
        let _ = std::fs::remove_dir_all(&stage_dir);
        return Err(Error::new(
            Status::GenericFailure,
            format!("Archive extraction failed: {:?}", e),
        ));
    }

    let relocate_res =
        relocate_stripped_tree(&stage_dir, &stage_dir, &jail, strip_components, collision_policy);
    let _ = std::fs::remove_dir_all(&stage_dir);
    relocate_res.map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to apply strip_components={}: {}", strip_components, e),
        )
    })?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompress_slice_into_direct_buffer() {
        let payload = b"Zero-copy direct slice decompression test payload ".repeat(64);
        for algo in ["deflate", "zstd", "lz4", "snappy", "brotli"] {
            let compressed = match algo {
                "deflate" => {
                    let mut dst = vec![0u8; deflate_compress_bound(payload.len(), 6)];
                    let n = deflate_compress(&payload, &mut dst, 6).expect("deflate");
                    dst.truncate(n);
                    dst
                }
                "zstd" => {
                    let mut dst = vec![0u8; zstd_compress_bound(payload.len()) + 128];
                    let n = zstd_compress(&payload, &mut dst, 3).expect("zstd");
                    dst.truncate(n);
                    dst
                }
                "lz4" => {
                    let mut dst = vec![0u8; lz4_compress_bound(payload.len()) + 4];
                    dst[0..4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
                    let n = lz4_compress(&payload, &mut dst[4..]).expect("lz4");
                    dst.truncate(n + 4);
                    dst
                }
                "snappy" => {
                    let mut dst = vec![0u8; snappy_max_compressed_length(payload.len())];
                    let n = snappy_compress(&payload, &mut dst).expect("snappy");
                    dst.truncate(n);
                    dst
                }
                "brotli" => {
                    let mut dst = vec![0u8; brotli_compress_bound(payload.len())];
                    let n = brotli_compress(&payload, &mut dst, 4, 22).expect("brotli");
                    dst.truncate(n);
                    dst
                }
                _ => unreachable!(),
            };
            let mut target = vec![0u8; payload.len() + 32];
            let written =
                decompress_slice_into(&compressed, &mut target, algo).expect("decompress_into");
            assert_eq!(written, payload.len());
            assert_eq!(&target[..written], payload.as_slice());

            let mut small_target = vec![0u8; 8];
            assert!(decompress_slice_into(&compressed, &mut small_target, algo).is_err());

            let mut shared = vec![0u8; payload.len() + 64];
            shared[..compressed.len()].copy_from_slice(&compressed);
            assert!(ranges_overlap(
                shared.as_ptr(),
                compressed.len(),
                shared.as_ptr(),
                payload.len()
            ));
            let written_overlap = unsafe {
                decompress_raw_into(
                    shared.as_ptr(),
                    compressed.len(),
                    shared.as_mut_ptr(),
                    payload.len(),
                    algo,
                )
            }
            .expect("overlapping decompress_raw_into");
            assert_eq!(written_overlap, payload.len());
            assert_eq!(&shared[..written_overlap], payload.as_slice());
        }
    }
}
