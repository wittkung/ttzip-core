// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Scratch directory guard, single-stream codec adapters, and damaged-archive fixtures for scenario evaluators.

use std::path::{Path, PathBuf};

use crate::archive::repair::{repair_damaged_tar, repair_damaged_zip};
use crate::benchmark::container_driver::{
    ContainerBenchmarkDriver, SevenZContainerDriver, TarContainerDriver, TarGzContainerDriver,
    TarZstContainerDriver, ZipContainerDriver,
};
use crate::codecs::brotli::{brotli_compress_to_vec, brotli_decompress_to_vec};
use crate::codecs::bzip2::{bzip2_compress_to_vec, bzip2_decompress_to_vec};
use crate::codecs::deflate::{gzip_compress, gzip_compress_bound, gzip_decompress};
use crate::codecs::lzfse::{lzfse_compress_to_vec, lzfse_decompress_to_vec};
use crate::codecs::snappy::{snappy_frame_decode_to_vec, snappy_frame_encode_to_vec};
use crate::codecs::zstd::{zstd_compress, zstd_compress_bound, zstd_decompress};
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use crate::zip::writer::{assemble_zip_archive, ZipCompressedItem, ZipInputItem};

/// RAII guard ensuring temporary benchmarking scratchpad directories are always cleaned up.
pub(super) struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    pub(super) fn new(path: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&path);
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub(super) fn compress_single_stream(
    format_tag: &str,
    raw_data: &[u8],
    level: i32,
) -> Result<Vec<u8>, TTZipStatus> {
    match format_tag {
        "GZIP" => {
            let mut gz_buf = vec![0u8; gzip_compress_bound(raw_data.len(), level)];
            let gz_len = gzip_compress(raw_data, &mut gz_buf, level)?;
            gz_buf.truncate(gz_len);
            Ok(gz_buf)
        }
        "BROTLI" => brotli_compress_to_vec(raw_data, level.clamp(0, 11) as u32, 22),
        "SNAPPY" => snappy_frame_encode_to_vec(raw_data),
        "LZFSE" => lzfse_compress_to_vec(raw_data),
        "BZIP2" => bzip2_compress_to_vec(raw_data, level),
        _ => {
            let mut zst_buf = vec![0u8; zstd_compress_bound(raw_data.len())];
            let zst_len = zstd_compress(raw_data, &mut zst_buf, level)?;
            zst_buf.truncate(zst_len);
            Ok(zst_buf)
        }
    }
}

pub(super) fn decompress_and_verify_single_stream(
    format_tag: &str,
    compressed: &[u8],
    raw_data: &[u8],
) -> Result<(usize, bool), TTZipStatus> {
    let bound = raw_data.len() * 2 + 1024;
    let decomp = match format_tag {
        "GZIP" => {
            let mut buf = vec![0u8; raw_data.len() + 1024];
            let actual = gzip_decompress(compressed, &mut buf)?;
            buf.truncate(actual);
            buf
        }
        "BROTLI" => brotli_decompress_to_vec(compressed, bound)?,
        "SNAPPY" => snappy_frame_decode_to_vec(compressed, bound)?,
        "LZFSE" => lzfse_decompress_to_vec(compressed, raw_data.len())?,
        "BZIP2" => bzip2_decompress_to_vec(compressed, bound)?,
        _ => {
            let mut buf = vec![0u8; raw_data.len() + 1024];
            let actual = zstd_decompress(compressed, &mut buf)?;
            buf.truncate(actual);
            buf
        }
    };
    Ok((decomp.len(), decomp == raw_data))
}

pub(super) fn create_initial_inplace_archive(
    format: TTZipArchiveFormat,
    initial_items: &[ZipInputItem],
) -> Result<(&'static str, Vec<u8>), TTZipStatus> {
    match format {
        TTZipArchiveFormat::Zip => Ok((
            "zip",
            ZipContainerDriver.create_archive(initial_items, 1, Some("Deflate"), None)?,
        )),
        TTZipArchiveFormat::SevenZip => Ok((
            "7z",
            SevenZContainerDriver.create_archive(initial_items, 1, None, None)?,
        )),
        TTZipArchiveFormat::TarGz => Ok((
            "tar.gz",
            TarGzContainerDriver.create_archive(initial_items, 1, None, None)?,
        )),
        TTZipArchiveFormat::TarZstd => Ok((
            "tar.zst",
            TarZstContainerDriver.create_archive(initial_items, 1, None, None)?,
        )),
        _ => Ok((
            "tar",
            TarContainerDriver.create_archive(initial_items, 0, None, None)?,
        )),
    }
}

pub(super) fn run_damaged_archive_repair_fixture(
    format_tag: &str,
    damaged_file: &Path,
    repaired_file: &Path,
    item_data: &[u8],
) -> Result<usize, TTZipStatus> {
    let orig_bytes = item_data.len();
    if format_tag == "ZIP" {
        let item = ZipCompressedItem {
            rel_path: "document.txt".to_string(),
            uncompressed_size: orig_bytes as u64,
            compressed_size: orig_bytes as u64,
            crc32: 0x12345678,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: item_data.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        };
        let full_zip = assemble_zip_archive(&[item])?;
        let truncated = full_zip[..30 + 12 + orig_bytes].to_vec();
        std::fs::write(damaged_file, &truncated).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        repair_damaged_zip(damaged_file, repaired_file)
    } else {
        let items = [ZipInputItem {
            rel_path: "salvage.txt".to_string(),
            data: item_data.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let tar_bytes = TarContainerDriver.create_archive(&items, 0, None, None)?;
        let truncated = tar_bytes[..512 + 512].to_vec();
        std::fs::write(damaged_file, &truncated).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        repair_damaged_tar(damaged_file, repaired_file)
    }
}
