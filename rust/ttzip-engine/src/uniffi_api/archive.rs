// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Archive Creation, Inspection, and Format Detection Scaffolding.

use std::sync::Arc;
use rayon::prelude::*;
use super::types::{ArchiveFormat, CancellationToken, CompressionReport, PasswordRecoveryOutcome, ProgressHandler, SniffMetadata, TTZipError, UniFFIEntryMetadata};

/// Detects archive format from file using the full 16-format magic and SFX sniffer.
#[uniffi::export]
pub fn detect_archive_format(path: String) -> Result<ArchiveFormat, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let sniff = crate::standards::detect_format_file(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let fmt = match sniff.format {
        crate::standards::signatures::DetectedFormat::Zip => ArchiveFormat::Zip,
        crate::standards::signatures::DetectedFormat::SevenZip => ArchiveFormat::SevenZip,
        crate::standards::signatures::DetectedFormat::Tar => {
            match sniff.compound_format {
                Some(crate::standards::signatures::CompoundFormat::TarGz) => ArchiveFormat::TarGz,
                Some(crate::standards::signatures::CompoundFormat::TarBz2) => ArchiveFormat::TarBz2,
                Some(crate::standards::signatures::CompoundFormat::TarXz) => ArchiveFormat::TarXz,
                Some(crate::standards::signatures::CompoundFormat::TarZstd) => ArchiveFormat::TarZstd,
                Some(crate::standards::signatures::CompoundFormat::TarLz4) => ArchiveFormat::TarLz4,
                Some(crate::standards::signatures::CompoundFormat::TarBrotli) => ArchiveFormat::TarBrotli,
                Some(crate::standards::signatures::CompoundFormat::TarLzip) => ArchiveFormat::TarLzip,
                Some(crate::standards::signatures::CompoundFormat::TarLrzip) => ArchiveFormat::TarLrzip,
                _ => ArchiveFormat::Tar,
            }
        }
        crate::standards::signatures::DetectedFormat::Gzip => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarGz) {
                ArchiveFormat::TarGz
            } else {
                ArchiveFormat::Gzip
            }
        }
        crate::standards::signatures::DetectedFormat::Bzip2 => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarBz2) {
                ArchiveFormat::TarBz2
            } else {
                ArchiveFormat::Bzip2
            }
        }
        crate::standards::signatures::DetectedFormat::Xz => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarXz) {
                ArchiveFormat::TarXz
            } else {
                ArchiveFormat::Xz
            }
        }
        crate::standards::signatures::DetectedFormat::Zstd => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarZstd) {
                ArchiveFormat::TarZstd
            } else {
                ArchiveFormat::Zstd
            }
        }
        crate::standards::signatures::DetectedFormat::Lz4 => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarLz4) {
                ArchiveFormat::TarLz4
            } else {
                ArchiveFormat::Lz4
            }
        }
        crate::standards::signatures::DetectedFormat::Brotli => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarBrotli) {
                ArchiveFormat::TarBrotli
            } else {
                ArchiveFormat::Brotli
            }
        }
        crate::standards::signatures::DetectedFormat::Lzip => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarLzip) {
                ArchiveFormat::TarLzip
            } else {
                ArchiveFormat::Lzip
            }
        }
        crate::standards::signatures::DetectedFormat::Lrzip => {
            if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarLrzip) {
                ArchiveFormat::TarLrzip
            } else {
                ArchiveFormat::Lrzip
            }
        }
        crate::standards::signatures::DetectedFormat::Dmg => ArchiveFormat::Dmg,
        crate::standards::signatures::DetectedFormat::Lzfse => ArchiveFormat::Lzfse,
        crate::standards::signatures::DetectedFormat::Snappy => ArchiveFormat::Snappy,
        crate::standards::signatures::DetectedFormat::Iso => ArchiveFormat::Iso,
        crate::standards::signatures::DetectedFormat::Cab => ArchiveFormat::Cab,
        crate::standards::signatures::DetectedFormat::Wim => ArchiveFormat::Wim,
        crate::standards::signatures::DetectedFormat::Rar => ArchiveFormat::Rar,
        crate::standards::signatures::DetectedFormat::Aar => ArchiveFormat::Aar,
        crate::standards::signatures::DetectedFormat::Cpio => ArchiveFormat::Cpio,
        crate::standards::signatures::DetectedFormat::Ar => ArchiveFormat::Ar,
        crate::standards::signatures::DetectedFormat::Deb => ArchiveFormat::Deb,
        crate::standards::signatures::DetectedFormat::Rpm => ArchiveFormat::Rpm,
        crate::standards::signatures::DetectedFormat::Xar => ArchiveFormat::Xar,
        crate::standards::signatures::DetectedFormat::Squashfs => ArchiveFormat::Squashfs,
        crate::standards::signatures::DetectedFormat::Lzh => ArchiveFormat::Lzh,
        _ => ArchiveFormat::Auto,
    };
    Ok(fmt)
}

/// Sniffs format and metadata from in-memory byte buffer.
#[uniffi::export]
pub fn sniff_format_buffer(data: Vec<u8>, filename_hint: Option<String>) -> SniffMetadata {
    let sniff = crate::standards::detect_format_buffer(&data, filename_hint.as_deref());
    let is_archive = sniff.format != crate::standards::signatures::DetectedFormat::Unknown;
    SniffMetadata {
        format_name: sniff.description.to_string(),
        mime_type: sniff.mime_type.to_string(),
        is_archive,
        is_sfx: sniff.is_sfx,
        sfx_offset: sniff.sfx_offset as u64,
        confidence: sniff.confidence as u32,
    }
}

/// Sniffs format and metadata from file on disk.
#[uniffi::export]
pub fn sniff_format_file(path: String) -> Result<SniffMetadata, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }
    let sniff = crate::standards::detect_format_file(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let is_archive = sniff.format != crate::standards::signatures::DetectedFormat::Unknown;
    Ok(SniffMetadata {
        format_name: sniff.description.to_string(),
        mime_type: sniff.mime_type.to_string(),
        is_archive,
        is_sfx: sniff.is_sfx,
        sfx_offset: sniff.sfx_offset as u64,
        confidence: sniff.confidence as u32,
    })
}

/// Measures Shannon entropy of byte data using SIMD.
#[uniffi::export]
pub fn estimate_shannon_entropy(data: Vec<u8>) -> f64 {
    crate::analytics::compute_shannon_entropy(&data)
}

/// Recommends codec algorithm name based on entropy and scenario.
#[uniffi::export]
pub fn recommend_codec(data: Vec<u8>, scenario: i32) -> String {
    let sc = match scenario {
        0 => crate::analytics::Scenario::InstantTransfer,
        2 => crate::analytics::Scenario::ColdStorage,
        _ => crate::analytics::Scenario::BalancedDaily,
    };
    crate::analytics::CascadedCodecSelector::recommend(&data, sc).recommended_algorithm.to_string()
}

/// Inspects all archive entry metadata items.
#[uniffi::export]
pub fn inspect_archive_entries(
    archive_path: String,
    password: Option<String>,
) -> Result<Vec<UniFFIEntryMetadata>, TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    struct EntryCollector {
        entries: Vec<UniFFIEntryMetadata>,
    }

    let mut collector = EntryCollector {
        entries: Vec::new(),
    };

    unsafe extern "C" fn inspect_cb(
        meta: *const crate::types::TTZipEntryMetadata,
        user_data: *mut libc::c_void,
    ) -> bool {
        if meta.is_null() || user_data.is_null() {
            return true;
        }
        let collector = &mut *(user_data as *mut EntryCollector);
        let m = &*meta;
        let path = if !m.path.is_null() {
            std::ffi::CStr::from_ptr(m.path).to_string_lossy().into_owned()
        } else {
            String::new()
        };
        let method = match m.compression_method {
            0 => "store",
            8 => "deflate",
            12 => "bzip2",
            14 => "lzma",
            95 => "zstd",
            _ => "unknown",
        }.to_string();
        let encoding = if !m.detected_encoding.is_null() {
            Some(std::ffi::CStr::from_ptr(m.detected_encoding).to_string_lossy().into_owned())
        } else {
            None
        };

        collector.entries.push(UniFFIEntryMetadata {
            path,
            uncompressed_size: m.uncompressed_size,
            compressed_size: m.compressed_size,
            crc32: m.crc32,
            mtime_epoch_secs: m.mtime_epoch_secs,
            mode: m.mode,
            is_directory: m.is_directory,
            is_encrypted: m.is_encrypted,
            compression_method: method,
            detected_encoding: encoding,
        });
        true
    }

    let res = crate::archive::unified::inspect::inspect_archive(
        p,
        password.as_deref(),
        true,
        Some(inspect_cb),
        &mut collector as *mut EntryCollector as *mut libc::c_void,
    );

    match res {
        Ok(_) => Ok(collector.entries),
        Err(status) => {
            if status == crate::types::TTZipStatus::ErrInvalidPassword {
                Err(TTZipError::InvalidPassword)
            } else {
                Err(TTZipError::EngineError { code: status as i32 })
            }
        }
    }
}

/// Extracts a single entry stream preview from a solid or non-solid archive.
#[uniffi::export]
pub fn extract_single_entry_stream(
    archive_path: String,
    entry_index: u64,
    password: Option<String>,
) -> Result<Vec<u8>, TTZipError> {
    extract_single_entry_stream_guarded(archive_path, entry_index, password, 100)
}

/// Extracts a single entry stream preview with configurable preceding solid budget in MB.
#[uniffi::export]
pub fn extract_single_entry_stream_guarded(
    archive_path: String,
    entry_index: u64,
    password: Option<String>,
    max_preceding_budget_mb: u32,
) -> Result<Vec<u8>, TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(mapped)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        let budget_bytes = (max_preceding_budget_mb as u64) * 1024 * 1024;
        crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            entry_index as usize,
            password.as_deref(),
            budget_bytes,
        ).map_err(|status| match status {
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrSolidBudgetExceeded => TTZipError::EngineError { code: -24 },
            _ => TTZipError::EngineError { code: status as i32 },
        })
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        zip_archive.extract_entry_bytes(entry_index as usize, password.as_deref())
            .map_err(|status| match status {
                crate::types::TTZipStatus::ErrCorruptHeader => {
                    TTZipError::CorruptHeader { details: "Corrupted entry CRC or payload".to_string(), offset: 0 }
                }
                crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
                _ => TTZipError::EngineError { code: status as i32 },
            })
    } else {
        Err(TTZipError::EngineError { code: -1 })
    }
}

/// Compresses source paths into destination archive.
#[uniffi::export]
pub fn create_archive_stream(
    source_paths: Vec<String>,
    output_path: String,
    format: ArchiveFormat,
    level: i32,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    if source_paths.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }
    let out_p = std::path::Path::new(&output_path);
    let paths: Vec<std::path::PathBuf> = source_paths.iter().map(std::path::PathBuf::from).collect();

    let start = std::time::Instant::now();
    let pwd_cstr = password.as_deref().and_then(|p| std::ffi::CString::new(p).ok());

    struct ProgressBox {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
    }
    let mut pbox = ProgressBox { handler: progress, token };

    unsafe extern "C" fn progress_cb(
        processed: u64,
        total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let p = &*(user_data as *const ProgressBox);
        if let Some(ref t) = p.token {
            if t.is_cancelled() {
                return false;
            }
        }
        if let Some(ref h) = p.handler {
            let name = if !entry_name.is_null() {
                Some(std::ffi::CStr::from_ptr(entry_name).to_string_lossy().into_owned())
            } else {
                None
            };
            return h.on_progress(processed, total, name);
        }
        true
    }

    let raw_fmt = match format {
        ArchiveFormat::Zip => crate::types::TTZipArchiveFormat::Zip,
        ArchiveFormat::SevenZip => crate::types::TTZipArchiveFormat::SevenZip,
        ArchiveFormat::Tar => crate::types::TTZipArchiveFormat::Tar,
        ArchiveFormat::TarGz => crate::types::TTZipArchiveFormat::TarGz,
        ArchiveFormat::TarBz2 => crate::types::TTZipArchiveFormat::TarBz2,
        ArchiveFormat::TarXz => crate::types::TTZipArchiveFormat::TarXz,
        ArchiveFormat::TarZstd => crate::types::TTZipArchiveFormat::TarZstd,
        ArchiveFormat::TarLz4 => crate::types::TTZipArchiveFormat::TarLz4,
        ArchiveFormat::TarBrotli => crate::types::TTZipArchiveFormat::TarBrotli,
        ArchiveFormat::TarLzip => crate::types::TTZipArchiveFormat::TarLzip,
        ArchiveFormat::TarLrzip => crate::types::TTZipArchiveFormat::TarLrzip,
        ArchiveFormat::Dmg => crate::types::TTZipArchiveFormat::Dmg,
        ArchiveFormat::Lzfse => crate::types::TTZipArchiveFormat::Lzfse,
        ArchiveFormat::Snappy => crate::types::TTZipArchiveFormat::Snappy,
        ArchiveFormat::Gzip => crate::types::TTZipArchiveFormat::Gzip,
        ArchiveFormat::Bzip2 => crate::types::TTZipArchiveFormat::Bzip2,
        ArchiveFormat::Xz => crate::types::TTZipArchiveFormat::Xz,
        ArchiveFormat::Zstd => crate::types::TTZipArchiveFormat::Zstd,
        ArchiveFormat::Lz4 => crate::types::TTZipArchiveFormat::Lz4,
        ArchiveFormat::Brotli => crate::types::TTZipArchiveFormat::Brotli,
        ArchiveFormat::Iso => crate::types::TTZipArchiveFormat::Iso,
        ArchiveFormat::Cab => crate::types::TTZipArchiveFormat::Cab,
        ArchiveFormat::Wim => crate::types::TTZipArchiveFormat::Wim,
        ArchiveFormat::Rar => crate::types::TTZipArchiveFormat::Rar,
        ArchiveFormat::Aar => crate::types::TTZipArchiveFormat::Aar,
        ArchiveFormat::Lzip => crate::types::TTZipArchiveFormat::Lzip,
        ArchiveFormat::Lrzip => crate::types::TTZipArchiveFormat::Lrzip,
        ArchiveFormat::Cpio => crate::types::TTZipArchiveFormat::Cpio,
        ArchiveFormat::Ar => crate::types::TTZipArchiveFormat::Ar,
        ArchiveFormat::Deb => crate::types::TTZipArchiveFormat::Deb,
        ArchiveFormat::Rpm => crate::types::TTZipArchiveFormat::Rpm,
        ArchiveFormat::Xar => crate::types::TTZipArchiveFormat::Xar,
        ArchiveFormat::Squashfs => crate::types::TTZipArchiveFormat::Squashfs,
        ArchiveFormat::Lzh => crate::types::TTZipArchiveFormat::Lzh,
        ArchiveFormat::Auto => crate::types::TTZipArchiveFormat::Auto,
    };

    let comp_level = match level {
        0 => crate::types::TTZipCompressionLevel::Store,
        1 => crate::types::TTZipCompressionLevel::Fastest,
        2..=4 => crate::types::TTZipCompressionLevel::Fast,
        5..=6 => crate::types::TTZipCompressionLevel::Normal,
        7..=9 => crate::types::TTZipCompressionLevel::Maximum,
        10..=22 => crate::types::TTZipCompressionLevel::Ultra,
        _ => crate::types::TTZipCompressionLevel::Normal,
    };

    let enc_method = if pwd_cstr.is_some() {
        crate::types::TTZipEncryptionMethod::Aes256
    } else {
        crate::types::TTZipEncryptionMethod::None
    };

    let options = crate::types::TTZipCreateOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipCreateOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        format: raw_fmt,
        level: comp_level,
        encryption: enc_method,
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        solid_block_size_mb: 64,
        progress_callback: Some(progress_cb),
        user_data: &mut pbox as *mut ProgressBox as *mut libc::c_void,
    };

    crate::archive::unified::create::create_archive(&paths, out_p, &options, 0)
        .map_err(|s| {
            if s == crate::types::TTZipStatus::Cancelled {
                TTZipError::Cancelled
            } else {
                TTZipError::EngineError { code: s as i32 }
            }
        })?;

    fn calculate_paths_uncompressed_bytes(paths: &[std::path::PathBuf]) -> u64 {
        fn calculate_single(path: &std::path::Path) -> u64 {
            if path.is_file() {
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            } else if path.is_dir() {
                let mut total = 0;
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        total += calculate_single(&entry.path());
                    }
                }
                total
            } else {
                0
            }
        }
        paths.iter().map(|p| calculate_single(p)).sum()
    }

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let comp_size = std::fs::metadata(out_p).map(|m| m.len()).unwrap_or(0);
    let uncompressed_bytes = calculate_paths_uncompressed_bytes(&paths);
    let benchmark_bytes = if uncompressed_bytes > 0 {
        uncompressed_bytes
    } else {
        comp_size
    };
    let throughput_mbs = (benchmark_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
    let space_savings_pct = if uncompressed_bytes > 0 && comp_size <= uncompressed_bytes {
        ((uncompressed_bytes - comp_size) as f64 / uncompressed_bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressionReport {
        uncompressed_bytes,
        compressed_bytes: comp_size,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct,
        engine_provenance: "Mozilla UniFFI Native Create Pipeline".to_string(),
    })
}

/// Recovers password against an encrypted archive using Rayon multi-core parallel probing.
#[uniffi::export]
pub fn recover_archive_password(
    archive_path: String,
    dictionary: Vec<String>,
) -> Result<PasswordRecoveryOutcome, TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let file_bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let start = std::time::Instant::now();
    let total_attempts = dictionary.len() as u64;

    // Use Rayon parallel search
    let found_password = dictionary.into_par_iter().find_any(|candidate| {
        if file_bytes.starts_with(b"7z\xBC\xAF\x27\x1C") {
            if let Ok(arch) = crate::sevenz::decoder::SevenZArchive::open_slice(&file_bytes) {
                return arch.extract_entry_bytes_stream(0, Some(candidate.as_str())).is_ok();
            }
        } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(&file_bytes) {
            return zip_archive.extract_entry_bytes(0, Some(candidate.as_str())).is_ok();
        }
        false
    });

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let attempts_per_second = (total_attempts as f64) / elapsed_secs;

    Ok(PasswordRecoveryOutcome {
        found_password,
        total_attempts,
        elapsed_nanos,
        attempts_per_second,
    })
}
