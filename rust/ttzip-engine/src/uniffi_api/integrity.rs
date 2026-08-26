// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Archive Integrity Verification, Checksum, and Split Volume Scaffolding.

use std::sync::Arc;
use super::types::{CancellationToken, ProgressHandler, TTZipError, UniFFICorruptedEntry, UniFFIIntegrityReport};

/// Verifies full archive integrity in single-pass streaming fashion without N-roundtrip FFI overhead.
#[uniffi::export]
pub fn verify_archive_integrity(
    archive_path: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<UniFFIIntegrityReport, TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let start = std::time::Instant::now();
    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    let mut corrupted = Vec::new();
    let total_entries;
    let mut verified_entries: u64 = 0;

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") || archive_path.to_lowercase().ends_with(".7z") {
        let arch = match crate::sevenz::decoder::SevenZArchive::open_slice(mapped) {
            Ok(a) => a,
            Err(_) => {
                return Ok(UniFFIIntegrityReport {
                    is_valid: false,
                    total_entries: 1,
                    verified_entries: 0,
                    corrupted_entries: vec![UniFFICorruptedEntry {
                        path: archive_path.clone(),
                        expected_crc32: 0,
                        actual_crc32: 0,
                        reason: "Corrupted 7z header".to_string(),
                    }],
                    elapsed_nanos: start.elapsed().as_nanos() as u64,
                    error_message: Some("Corrupted 7z archive header".to_string()),
                });
            }
        };

        let files = arch.files();
        total_entries = files.len() as u64;
        let budget_bytes = 100 * 1024 * 1024;

        for (idx, f) in files.iter().enumerate() {
            if let Some(ref t) = token {
                if t.is_cancelled() {
                    return Err(TTZipError::Cancelled);
                }
            }

            if f.is_directory {
                verified_entries += 1;
                continue;
            }

            if let Some(ref prg) = progress {
                if !prg.on_progress(idx as u64, total_entries, Some(f.rel_path.clone())) {
                    return Err(TTZipError::Cancelled);
                }
            }

            let res = crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
                mapped,
                arch.info(),
                arch.seek_index(),
                idx,
                password.as_deref(),
                budget_bytes,
            );

            match res {
                Ok(_) => {
                    verified_entries += 1;
                }
                Err(err) => {
                    let reason = match err {
                        crate::types::TTZipStatus::ErrInvalidPassword => "Invalid password".to_string(),
                        crate::types::TTZipStatus::ErrSolidBudgetExceeded => "Solid stream budget exceeded".to_string(),
                        _ => format!("Extraction error code: {:?}", err),
                    };
                    corrupted.push(UniFFICorruptedEntry {
                        path: f.rel_path.clone(),
                        expected_crc32: arch.seek_index().entries.get(idx).and_then(|e| e.crc).unwrap_or(0),
                        actual_crc32: 0,
                        reason,
                    });
                }
            }
        }
    } else if mapped.starts_with(b"PK\x03\x04") || mapped.starts_with(b"PK\x05\x06") || archive_path.to_lowercase().ends_with(".zip") {
        let zip_archive = match crate::zip::reader::ZipArchive::open_slice(mapped) {
            Ok(z) => z,
            Err(_) => {
                return Ok(UniFFIIntegrityReport {
                    is_valid: false,
                    total_entries: 1,
                    verified_entries: 0,
                    corrupted_entries: vec![UniFFICorruptedEntry {
                        path: archive_path.clone(),
                        expected_crc32: 0,
                        actual_crc32: 0,
                        reason: "Corrupted ZIP header or central directory".to_string(),
                    }],
                    elapsed_nanos: start.elapsed().as_nanos() as u64,
                    error_message: Some("Corrupted ZIP archive header or central directory".to_string()),
                });
            }
        };
        let entries = zip_archive.entries();
        total_entries = entries.len() as u64;

        for (idx, e) in entries.iter().enumerate() {
            if let Some(ref t) = token {
                if t.is_cancelled() {
                    return Err(TTZipError::Cancelled);
                }
            }

            if e.is_directory {
                verified_entries += 1;
                continue;
            }

            if let Some(ref prg) = progress {
                if !prg.on_progress(idx as u64, total_entries, Some(e.rel_path.clone())) {
                    return Err(TTZipError::Cancelled);
                }
            }

            match zip_archive.extract_entry_bytes(idx, password.as_deref()) {
                Ok(_) => {
                    verified_entries += 1;
                }
                Err(err) => {
                    corrupted.push(UniFFICorruptedEntry {
                        path: e.rel_path.clone(),
                        expected_crc32: e.crc32,
                        actual_crc32: 0,
                        reason: format!("CRC or payload verification failed: {:?}", err),
                    });
                }
            }
        }
    } else {
        let pwd_cstr = password.as_deref().and_then(|p| std::ffi::CString::new(p).ok());
        let options = crate::types::TTZipExtractOptions {
            struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            destination_path: std::ptr::null(),
            password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: false,
            dry_run: true,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let temp_dir = std::env::temp_dir();
        match crate::archive::unified::extract::extract_archive_with_metrics(p, &temp_dir, &options) {
            Ok(_) => {
                total_entries = 1;
                verified_entries = 1;
            }
            Err(status) => {
                return Ok(UniFFIIntegrityReport {
                    is_valid: false,
                    total_entries: 1,
                    verified_entries: 0,
                    corrupted_entries: vec![UniFFICorruptedEntry {
                        path: archive_path.clone(),
                        expected_crc32: 0,
                        actual_crc32: 0,
                        reason: format!("Extraction verification failed with status: {:?}", status),
                    }],
                    elapsed_nanos: start.elapsed().as_nanos() as u64,
                    error_message: Some(format!("Error code: {:?}", status)),
                });
            }
        }
    }

    let elapsed = start.elapsed().as_nanos() as u64;
    let is_valid = corrupted.is_empty();

    Ok(UniFFIIntegrityReport {
        is_valid,
        total_entries,
        verified_entries,
        corrupted_entries: corrupted,
        elapsed_nanos: elapsed,
        error_message: None,
    })
}

/// Computes hash or checksum of a file in zero-copy chunked streaming fashion.
///
/// Supported algorithms: "crc32", "sha256", "sha1", "md5".
#[uniffi::export]
pub fn compute_file_hash(path: String, algorithm: String) -> Result<String, TTZipError> {
    use std::io::Read;
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let mut file = std::fs::File::open(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut buf = [0u8; 64 * 1024];

    let algo_norm = algorithm.trim().to_ascii_lowercase().replace('-', "").replace('_', "");
    match algo_norm.as_str() {
        "crc32" => {
            let mut crc = 0u32;
            loop {
                let n = file.read(&mut buf).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
                if n == 0 { break; }
                crc = crate::crypto::crc32::crc32_fast(crc, &buf[..n]);
            }
            Ok(format!("{:08X}", crc))
        }
        "sha256" => {
            let mut hasher = crate::crypto::sha256::FastSha256::new();
            loop {
                let n = file.read(&mut buf).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            let hash = hasher.finalize();
            Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
        }
        "sha1" => {
            let mut hasher = crate::crypto::sha1::FastSha1::new();
            loop {
                let n = file.read(&mut buf).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            let hash = hasher.finalize();
            Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
        }
        "md5" => {
            let mut hasher = crate::crypto::md5::FastMd5::new();
            loop {
                let n = file.read(&mut buf).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            let hash = hasher.finalize();
            Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
        }
        _ => Err(TTZipError::EngineError { code: -1 }),
    }
}

/// Computes SHA-256 hex digest of a file in zero-copy streaming fashion.
#[uniffi::export]
pub fn compute_file_sha256(file_path: String) -> Result<String, TTZipError> {
    use std::io::Read;
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let mut file = std::fs::File::open(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut buf = [0u8; 64 * 1024];
    let mut hasher = crate::crypto::sha256::FastSha256::new();
    loop {
        let n = file.read(&mut buf).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();
    Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Computes CRC32 checksum of a file in zero-copy streaming fashion.
#[uniffi::export]
pub fn compute_file_crc32(file_path: String) -> Result<u32, TTZipError> {
    use std::io::Read;
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let mut file = std::fs::File::open(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut buf = [0u8; 64 * 1024];
    let mut crc = 0u32;
    loop {
        let n = file.read(&mut buf).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
        if n == 0 { break; }
        crc = crate::crypto::crc32::crc32_fast(crc, &buf[..n]);
    }
    Ok(crc)
}

/// Computes in-memory CRC32 checksum via hardware SIMD.
#[uniffi::export]
pub fn compute_bytes_crc32(data: Vec<u8>) -> u32 {
    crate::crypto::crc32::crc32(&data)
}

/// Computes in-memory Adler-32 checksum via hardware SIMD.
#[uniffi::export]
pub fn compute_bytes_adler32(data: Vec<u8>) -> u32 {
    crate::crypto::adler32::adler32(&data)
}

/// Computes in-memory SHA-256 hex digest via hardware SIMD.
#[uniffi::export]
pub fn compute_bytes_sha256(data: Vec<u8>) -> String {
    let hash = crate::crypto::sha256::FastSha256::digest(&data);
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Combines two CRC-32 checksums into the CRC-32 of their concatenation.
#[uniffi::export]
pub fn combine_crc32(crc1: u32, crc2: u32, len2: u64) -> u32 {
    crate::crypto::crc32::crc32_combine(crc1, crc2, len2)
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

/// Slices an archive file into multi-volume segments according to the specified naming pattern.
#[uniffi::export]
pub fn slice_archive_file(
    source_path: String,
    split_size_bytes: u64,
    naming_pattern: String,
) -> Result<Vec<String>, TTZipError> {
    if split_size_bytes == 0 {
        return Err(TTZipError::EngineError { code: -1 });
    }
    let src = std::path::Path::new(&source_path);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: source_path });
    }

    let pattern_clean = naming_pattern.trim().to_ascii_lowercase();
    let scheme = if pattern_clean == "pkzipspanned" || pattern_clean == "pkzip_spanned" {
        crate::archive::split::VolumeNamingScheme::PkzipSpanned
    } else if pattern_clean == "rawsplit" || pattern_clean == "raw_split" {
        crate::archive::split::VolumeNamingScheme::RawSplit
    } else {
        crate::archive::split::VolumeNamingScheme::NumberedExtension
    };

    let mut in_file = std::fs::File::open(src)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let mut writer = crate::archive::split::SplitVolumeWriter::new(src, split_size_bytes, scheme)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let mut buffer = [0u8; 64 * 1024]; // 64 KB micro-buffer stream
    use std::io::{Read, Write};
    loop {
        match in_file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if let Err(e) = writer.write_all(&buffer[..n]) {
                    writer.cancel_and_cleanup();
                    return Err(TTZipError::IoError { message: e.to_string() });
                }
            }
            Err(e) => {
                writer.cancel_and_cleanup();
                return Err(TTZipError::IoError { message: e.to_string() });
            }
        }
    }

    let generated = writer.close().map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    Ok(generated.into_iter().filter_map(|p| p.to_str().map(|s| s.to_string())).collect())
}
