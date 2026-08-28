// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-Format Container Benchmark Drivers.
//!
//! Provides unified container abstraction and native high-throughput drivers for:
//! - ZIP (Deflate, Store, WinZip AES-256, ZipCrypto)
//! - POSIX TAR (USTAR / PAX 512-byte aligned streaming)
//! - TAR.GZ (libdeflate streaming gzip compression)
//! - TAR.ZST (Facebook Zstandard streaming with LDM capability)
//! - 7-Zip (LZMA2 solid/non-solid blocks with AES-256 data & header encryption)
//! - Apple Archive AAR (LZFSE-compressed streaming TAR)
//! - Brotli Streaming Tarball (TAR.BR)
//! - Snappy Framed Tarball (TAR.SZ)

use crate::archive::tar::reader::TarArchive;
use crate::archive::tar::writer::TarWriter;
use crate::codecs::brotli::{brotli_compress_to_vec, brotli_decompress_to_vec};
use crate::codecs::deflate::{gzip_compress, gzip_compress_bound, gzip_decompress};
use crate::codecs::fast_blocks::{lzfse_compress, lzfse_decompress};
use crate::codecs::snappy::{snappy_frame_decode_to_vec, snappy_frame_encode_to_vec};
use crate::codecs::zstd::{
    zstd_compress, zstd_compress_advanced, zstd_compress_bound, zstd_decompress,
    zstd_get_decompressed_size, ZstdConfig,
};
use crate::sevenz::decoder::SevenZArchive;
use crate::sevenz::writer::create_7z_solid_archive_bytes;
use crate::types::{TTZipEncryptionMethod, TTZipStatus};
use crate::zip::parser::parse_all_entries;
use crate::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

/// Unified interface for benchmarking container formats.
pub trait ContainerBenchmarkDriver: Send + Sync {
    /// Identifier of the container format (e.g., "ZIP", "TAR", "TAR.GZ", "TAR.ZST", "7Z").
    fn container_id(&self) -> &'static str;

    /// List of compression/container algorithms supported by this driver.
    fn supported_algorithms(&self) -> &[&'static str];

    /// Compresses a collection of input items into an archive byte stream.
    fn create_archive(
        &self,
        items: &[ZipInputItem],
        level: i32,
        algorithm: Option<&str>,
        password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus>;

    /// Parses and extracts entries from the archive byte stream, returning the number of valid extracted entries.
    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        password: Option<&str>,
    ) -> Result<usize, TTZipStatus>;
}

// MARK: - ZIP Container Driver

/// Benchmark driver for ZIP archives.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZipContainerDriver;

impl ContainerBenchmarkDriver for ZipContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "ZIP"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["Deflate", "Store", "WinZip-AES256", "ZipCrypto"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        level: i32,
        algorithm: Option<&str>,
        password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let enc_method = if password.is_some() {
            if algorithm == Some("ZipCrypto") {
                TTZipEncryptionMethod::ZipCrypto
            } else {
                TTZipEncryptionMethod::Aes256
            }
        } else {
            TTZipEncryptionMethod::None
        };

        let effective_level = if algorithm == Some("Store") {
            0
        } else {
            level
        };

        let compressed = compress_items_parallel(
            items.to_vec(),
            effective_level,
            enc_method,
            password,
            4,
        )?;
        assemble_zip_archive(&compressed)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        let entries = parse_all_entries(archive_bytes)?;
        Ok(entries.len())
    }
}

// MARK: - POSIX TAR Container Driver

/// Benchmark driver for POSIX USTAR / PAX archives.
#[derive(Debug, Default, Clone, Copy)]
pub struct TarContainerDriver;

impl ContainerBenchmarkDriver for TarContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "TAR"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["POSIX.1-2001 PAX", "USTAR"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        _level: i32,
        _algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let mut writer = TarWriter::new(Vec::with_capacity(items.len() * 1024));
        for item in items {
            writer.append_file(
                &item.rel_path,
                &item.data,
                item.mode,
                item.mtime_epoch_secs as i64,
            )?;
        }
        writer.finish()?;
        Ok(writer.into_inner())
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        let tar = TarArchive::open_slice(archive_bytes)?;
        for i in 0..tar.len() {
            let _ = tar.extract_entry_bytes(i)?;
        }
        Ok(tar.len())
    }
}

// MARK: - TAR.GZ Container Driver

/// Benchmark driver for TAR.GZ archives with libdeflate streaming.
#[derive(Debug, Default, Clone, Copy)]
pub struct TarGzContainerDriver;

impl ContainerBenchmarkDriver for TarGzContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "TAR.GZ"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["Deflate", "Gzip"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        level: i32,
        _algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let tar_driver = TarContainerDriver;
        let tar_bytes = tar_driver.create_archive(items, 0, None, None)?;

        let max_bound = gzip_compress_bound(tar_bytes.len(), level);
        let mut gz_buf = vec![0u8; max_bound];
        let gz_len = gzip_compress(&tar_bytes, &mut gz_buf, level)?;
        gz_buf.truncate(gz_len);
        Ok(gz_buf)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        if archive_bytes.len() < 10 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let isize_guess = if archive_bytes.len() >= 4 {
            u32::from_le_bytes(
                archive_bytes[archive_bytes.len() - 4..]
                    .try_into()
                    .unwrap_or([0, 0, 0, 0]),
            ) as usize
        } else {
            0
        };
        let mut capacity = isize_guess.max(archive_bytes.len() * 3).max(8192);
        let mut tar_buf = vec![0u8; capacity];
        let actual_len = match gzip_decompress(archive_bytes, &mut tar_buf) {
            Ok(len) => len,
            Err(_) => {
                capacity = capacity.max(1024 * 1024 * 8);
                tar_buf.resize(capacity, 0);
                gzip_decompress(archive_bytes, &mut tar_buf)?
            }
        };
        tar_buf.truncate(actual_len);

        let tar = TarArchive::open_slice(&tar_buf)?;
        for i in 0..tar.len() {
            let _ = tar.extract_entry_bytes(i)?;
        }
        Ok(tar.len())
    }
}

// MARK: - TAR.ZST Container Driver

/// Benchmark driver for TAR.ZST archives with hardware accelerated Zstandard.
#[derive(Debug, Default, Clone, Copy)]
pub struct TarZstContainerDriver;

impl ContainerBenchmarkDriver for TarZstContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "TAR.ZST"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["Zstandard", "Zstd-LDM"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        level: i32,
        algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let tar_driver = TarContainerDriver;
        let tar_bytes = tar_driver.create_archive(items, 0, None, None)?;

        let is_ldm = algorithm == Some("Zstd-LDM");
        let mut zst_buf = vec![0u8; zstd_compress_bound(tar_bytes.len())];

        let zst_len = if is_ldm {
            let config = ZstdConfig {
                level,
                nb_workers: 2,
                job_size_mb: 1,
                overlap_log: 2,
                window_log: 20,
                enable_ldm: true,
                enable_checksum: true,
            };
            zstd_compress_advanced(&tar_bytes, &mut zst_buf, &config)?
        } else {
            zstd_compress(&tar_bytes, &mut zst_buf, level)?
        };

        zst_buf.truncate(zst_len);
        Ok(zst_buf)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        let detected_size = zstd_get_decompressed_size(archive_bytes)
            .unwrap_or(archive_bytes.len() as u64 * 4)
            .max(8192) as usize;
        let mut tar_buf = vec![0u8; detected_size];
        let actual_len = zstd_decompress(archive_bytes, &mut tar_buf)?;
        tar_buf.truncate(actual_len);

        let tar = TarArchive::open_slice(&tar_buf)?;
        for i in 0..tar.len() {
            let _ = tar.extract_entry_bytes(i)?;
        }
        Ok(tar.len())
    }
}

// MARK: - 7-Zip Container Driver

/// Benchmark driver for 7-Zip archives.
#[derive(Debug, Default, Clone, Copy)]
pub struct SevenZContainerDriver;

impl ContainerBenchmarkDriver for SevenZContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "7Z"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["LZMA2", "Solid-LZMA2", "AES-256-CBC"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        level: i32,
        _algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        create_7z_solid_archive_bytes(items, level, 4)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        let arch = SevenZArchive::open_slice(archive_bytes)?;
        for i in 0..arch.len() {
            let _ = arch.extract_entry_bytes_stream(i, None)?;
        }
        Ok(arch.len())
    }
}

// MARK: - Apple Archive AAR (Tar + LZFSE) Driver

/// Benchmark driver for Apple Archive format (TAR + LZFSE compression).
#[derive(Debug, Default, Clone, Copy)]
pub struct AarContainerDriver;

impl ContainerBenchmarkDriver for AarContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "AAR"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["Apple-LZFSE", "PAX"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        _level: i32,
        _algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let tar_driver = TarContainerDriver;
        let tar_bytes = tar_driver.create_archive(items, 0, None, None)?;

        let mut comp_buf = vec![0u8; tar_bytes.len() + 4096];
        let comp_len = lzfse_compress(&tar_bytes, &mut comp_buf)?;
        comp_buf.truncate(comp_len);

        let mut out = Vec::with_capacity(8 + comp_len);
        out.extend_from_slice(&(tar_bytes.len() as u64).to_le_bytes());
        out.extend_from_slice(&comp_buf);
        Ok(out)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        if archive_bytes.len() < 8 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let uncompressed_len = u64::from_le_bytes(archive_bytes[0..8].try_into().unwrap_or([0; 8])) as usize;
        let payload = &archive_bytes[8..];
        let mut tar_buf = vec![0u8; uncompressed_len.max(4096)];
        let decomp_len = lzfse_decompress(payload, &mut tar_buf)?;
        tar_buf.truncate(decomp_len);

        let tar = TarArchive::open_slice(&tar_buf)?;
        for i in 0..tar.len() {
            let _ = tar.extract_entry_bytes(i)?;
        }
        Ok(tar.len())
    }

}

// MARK: - Brotli Streaming Tarball Driver

/// Benchmark driver for Brotli streaming tarball archives (TAR.BR).
#[derive(Debug, Default, Clone, Copy)]
pub struct TarBrotliContainerDriver;

impl ContainerBenchmarkDriver for TarBrotliContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "TAR.BR"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["Brotli", "PAX"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        level: i32,
        _algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let tar_driver = TarContainerDriver;
        let tar_bytes = tar_driver.create_archive(items, 0, None, None)?;
        let q = level.clamp(0, 11) as u32;
        brotli_compress_to_vec(&tar_bytes, q, 22)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        let tar_buf = brotli_decompress_to_vec(archive_bytes, 128 * 1024 * 1024)?;
        let tar = TarArchive::open_slice(&tar_buf)?;
        for i in 0..tar.len() {
            let _ = tar.extract_entry_bytes(i)?;
        }
        Ok(tar.len())
    }
}

// MARK: - Snappy Framed Tarball Driver

/// Benchmark driver for Snappy framed streaming tarball archives (TAR.SZ).
#[derive(Debug, Default, Clone, Copy)]
pub struct TarSnappyContainerDriver;

impl ContainerBenchmarkDriver for TarSnappyContainerDriver {
    #[inline]
    fn container_id(&self) -> &'static str {
        "TAR.SZ"
    }

    #[inline]
    fn supported_algorithms(&self) -> &[&'static str] {
        &["Snappy-Framed", "PAX"]
    }

    fn create_archive(
        &self,
        items: &[ZipInputItem],
        _level: i32,
        _algorithm: Option<&str>,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let tar_driver = TarContainerDriver;
        let tar_bytes = tar_driver.create_archive(items, 0, None, None)?;
        snappy_frame_encode_to_vec(&tar_bytes)
    }

    fn extract_archive(
        &self,
        archive_bytes: &[u8],
        _password: Option<&str>,
    ) -> Result<usize, TTZipStatus> {
        let tar_buf = snappy_frame_decode_to_vec(archive_bytes, 128 * 1024 * 1024)?;
        let tar = TarArchive::open_slice(&tar_buf)?;
        for i in 0..tar.len() {
            let _ = tar.extract_entry_bytes(i)?;
        }
        Ok(tar.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<ZipInputItem> {
        vec![
            ZipInputItem {
                rel_path: "docs/readme.txt".to_string(),
                data: b"TTZip High-performance Container Benchmark Suite.".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            ZipInputItem {
                rel_path: "src/engine.rs".to_string(),
                data: b"pub fn run_ttzip_benchmark() -> Result<(), ()> { Ok(()) }".to_vec(),
                mtime_epoch_secs: 1700000001,
                mode: 0o644,
                is_directory: false,
            },
        ]
    }

    #[test]
    fn test_zip_driver_roundtrip() {
        let driver = ZipContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 6, None, None).expect("zip create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("zip extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_tar_driver_roundtrip() {
        let driver = TarContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 0, None, None).expect("tar create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("tar extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_targz_driver_roundtrip() {
        let driver = TarGzContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 6, None, None).expect("tar.gz create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("tar.gz extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_tarzst_driver_roundtrip() {
        let driver = TarZstContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 3, None, None).expect("tar.zst create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("tar.zst extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_sevenz_driver_roundtrip() {
        let driver = SevenZContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 3, None, None).expect("7z create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("7z extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_aar_driver_roundtrip() {
        let driver = AarContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 1, None, None).expect("aar create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("aar extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_tar_brotli_driver_roundtrip() {
        let driver = TarBrotliContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 4, None, None).expect("tar.br create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("tar.br extract");
        assert_eq!(count, items.len());
    }

    #[test]
    fn test_tar_snappy_driver_roundtrip() {
        let driver = TarSnappyContainerDriver;
        let items = sample_items();
        let bytes = driver.create_archive(&items, 1, None, None).expect("tar.sz create");
        assert!(!bytes.is_empty());
        let count = driver.extract_archive(&bytes, None).expect("tar.sz extract");
        assert_eq!(count, items.len());
    }
}
