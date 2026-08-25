// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Archive Creation, Extraction, and Inspection Scaffolding.

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
                _ => ArchiveFormat::Tar,
            }
        }
        crate::standards::signatures::DetectedFormat::Gzip => ArchiveFormat::TarGz,
        crate::standards::signatures::DetectedFormat::Bzip2 => ArchiveFormat::TarBz2,
        crate::standards::signatures::DetectedFormat::Xz => ArchiveFormat::TarXz,
        crate::standards::signatures::DetectedFormat::Zstd => ArchiveFormat::TarZstd,
        crate::standards::signatures::DetectedFormat::Dmg => ArchiveFormat::Dmg,
        crate::standards::signatures::DetectedFormat::Lzfse => ArchiveFormat::Lzfse,
        crate::standards::signatures::DetectedFormat::Snappy => ArchiveFormat::Snappy,
        crate::standards::signatures::DetectedFormat::Wim => ArchiveFormat::Wim,
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
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let file_bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    if file_bytes.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(&file_bytes)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        arch.extract_entry_bytes_stream(entry_index as usize, password.as_deref())
            .map_err(|_| TTZipError::InvalidPassword)
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(&file_bytes) {
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

/// Extracts full archive with progress reporting.
#[uniffi::export]
pub fn extract_archive_stream(
    archive_path: String,
    destination_dir: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let src = std::path::Path::new(&archive_path);
    let dst = std::path::Path::new(&destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

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

    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: &mut pbox as *mut ProgressBox as *mut libc::c_void,
    };

    let bytes = crate::archive::unified::extract::extract_archive_with_metrics(src, dst, &options)
        .map_err(|s| {
            if s == crate::types::TTZipStatus::Cancelled {
                TTZipError::Cancelled
            } else if s == crate::types::TTZipStatus::ErrInvalidPassword {
                TTZipError::InvalidPassword
            } else {
                TTZipError::EngineError { code: s as i32 }
            }
        })?;

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let throughput_mbs = (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;

    Ok(CompressionReport {
        uncompressed_bytes: bytes,
        compressed_bytes: std::fs::metadata(src).map(|m| m.len()).unwrap_or(bytes),
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct: 0.0,
        engine_provenance: "Mozilla UniFFI Native Core Pipeline".to_string(),
    })
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
        ArchiveFormat::Dmg => crate::types::TTZipArchiveFormat::Dmg,
        ArchiveFormat::Lzfse => crate::types::TTZipArchiveFormat::Lzfse,
        ArchiveFormat::Snappy => crate::types::TTZipArchiveFormat::Snappy,
        ArchiveFormat::Auto | ArchiveFormat::Wim => crate::types::TTZipArchiveFormat::Auto,
    };

    let comp_level = match level {
        0 => crate::types::TTZipCompressionLevel::Store,
        1 => crate::types::TTZipCompressionLevel::Fastest,
        2 => crate::types::TTZipCompressionLevel::Fast,
        9 => crate::types::TTZipCompressionLevel::Ultra,
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

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let comp_size = std::fs::metadata(out_p).map(|m| m.len()).unwrap_or(0);
    let throughput_mbs = (comp_size as f64 / (1024.0 * 1024.0)) / elapsed_secs;

    Ok(CompressionReport {
        uncompressed_bytes: comp_size,
        compressed_bytes: comp_size,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct: 0.0,
        engine_provenance: "Mozilla UniFFI Native Create Pipeline".to_string(),
    })
}

/// Extracts selected subset of entries from an archive into destination directory.
#[uniffi::export]
pub fn extract_selected_entries(
    archive_path: String,
    target_entries: Vec<String>,
    destination_dir: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<u64, TTZipError> {
    let src = std::path::Path::new(&archive_path);
    let dst = std::path::Path::new(&destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

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

    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: &mut pbox as *mut ProgressBox as *mut libc::c_void,
    };

    crate::archive::unified::extract_single::extract_selected_entries(
        src,
        &target_entries,
        dst,
        &options,
    )
    .map(|count| count as u64)
    .map_err(|s| {
        if s == crate::types::TTZipStatus::Cancelled {
            TTZipError::Cancelled
        } else if s == crate::types::TTZipStatus::ErrInvalidPassword {
            TTZipError::InvalidPassword
        } else {
            TTZipError::EngineError { code: s as i32 }
        }
    })
}

/// Computes SHA-256 hex digest of a file.
#[uniffi::export]
pub fn compute_file_sha256(file_path: String) -> Result<String, TTZipError> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut hasher = crate::crypto::sha256::FastSha256::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Computes CRC32 checksum of a file.
#[uniffi::export]
pub fn compute_file_crc32(file_path: String) -> Result<u32, TTZipError> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    Ok(crate::crypto::crc32::crc32(&bytes))
}

/// Detects split volume chain members starting from seed file.
#[uniffi::export]
pub fn detect_split_volume_chain(seed_path: String) -> Result<Vec<String>, TTZipError> {
    let p = std::path::Path::new(&seed_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: seed_path });
    }
    let chain = crate::archive::split::detect_volume_chain(p)
        .map_err(|_| TTZipError::EngineError { code: -1 })?;
    Ok(chain.into_iter().filter_map(|p| p.to_str().map(|s| s.to_string())).collect())
}

/// Joins multi-volume split archive files into a continuous output file.
#[uniffi::export]
pub fn join_split_volume_chain(
    first_volume_path: String,
    output_path: String,
) -> Result<(), TTZipError> {
    let first = std::path::Path::new(&first_volume_path);
    let out = std::path::Path::new(&output_path);
    if !first.exists() {
        return Err(TTZipError::FileNotFound { path: first_volume_path });
    }
    let chain = crate::archive::split::detect_volume_chain(first)
        .map_err(|_| TTZipError::EngineError { code: -1 })?;
    let mut virtual_reader = crate::archive::split::VirtualMultiVolumeReader::from_volumes(chain)
        .map_err(|_| TTZipError::EngineError { code: -2 })?;
    let mut out_file = std::fs::File::create(out)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    std::io::copy(&mut virtual_reader, &mut out_file)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    Ok(())
}

/// Repairs damaged archive file and writes to output destination.
#[uniffi::export]
pub fn repair_archive_file(damaged_path: String, output_path: String) -> Result<u64, TTZipError> {
    let damaged = std::path::Path::new(&damaged_path);
    let output = std::path::Path::new(&output_path);
    if !damaged.exists() {
        return Err(TTZipError::FileNotFound { path: damaged_path });
    }
    crate::archive::unified::repair::repair_archive(damaged, output)
        .map(|count| count as u64)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })
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

/// Atomically mutates archive in-place (append, replace, delete) without full recompression.
#[uniffi::export]
pub fn in_place_mutate_archive(
    archive_path: String,
    actions: Vec<super::types::InPlaceMutationAction>,
) -> Result<(), TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let mut session = crate::archive::in_place_edit::InPlaceArchiveSession::begin(p, None)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    for act in actions {
        if act.is_delete {
            session.delete(&act.entry_path)
                .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
        } else if let Some(ref src) = act.source_path {
            let src_path = std::path::Path::new(src);
            session.replace(&act.entry_path, src_path)
                .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
        }
    }

    session.commit()
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(())
}


