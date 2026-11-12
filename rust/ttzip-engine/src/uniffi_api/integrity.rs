// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Archive Integrity Verification, Checksum, and Split Volume Scaffolding.

use std::sync::Arc;
use super::types::{CancellationToken, ProgressHandler, TTZipError, UniFFICorruptedEntry, UniFFIIntegrityReport};

#[derive(Default)]
struct Crc32StreamSink {
    crc: u32,
}

impl std::io::Write for Crc32StreamSink {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.crc = crate::crypto::crc32::crc32_fast(self.crc, buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn verify_zip_entry_streaming(
    zip_archive: &crate::zip::reader::ZipArchive<'_>,
    idx: usize,
    password: Option<&str>,
) -> Result<u32, (crate::types::TTZipStatus, u32)> {
    let mut sink = Crc32StreamSink::default();
    match zip_archive.extract_entry_to_sink(idx, password, &mut sink) {
        Ok(_) => Ok(sink.crc),
        Err(status) => Err((status, sink.crc)),
    }
}

fn corrupted_header_report(
    archive_path: String,
    reason: &str,
    error_message: String,
    start: std::time::Instant,
) -> UniFFIIntegrityReport {
    UniFFIIntegrityReport {
        is_valid: false,
        total_entries: 1,
        verified_entries: 0,
        corrupted_entries: vec![UniFFICorruptedEntry {
            path: archive_path,
            expected_crc32: 0,
            actual_crc32: 0,
            reason: reason.to_string(),
        }],
        elapsed_nanos: start.elapsed().as_nanos() as u64,
        error_message: Some(error_message),
    }
}

fn verify_7z_archive_integrity(
    mapped: &[u8],
    archive_path: &str,
    password: Option<&str>,
    progress: Option<&dyn ProgressHandler>,
    token: Option<&Arc<CancellationToken>>,
    start: std::time::Instant,
) -> Result<UniFFIIntegrityReport, TTZipError> {
    let Ok(arch) = crate::sevenz::decoder::SevenZArchive::open_slice(mapped) else {
        return Ok(corrupted_header_report(
            archive_path.to_string(),
            "Corrupted 7z header",
            "Corrupted 7z archive header".to_string(),
            start,
        ));
    };

    let files = arch.files();
    let total_entries = files.len() as u64;
    let mut verified_entries: u64 = 0;
    let mut corrupted = Vec::new();
    let budget_bytes = 64 * 1024 * 1024;

    for (idx, f) in files.iter().enumerate() {
        if token.is_some_and(|t| t.is_cancelled()) {
            return Err(TTZipError::Cancelled);
        }
        if f.is_directory {
            verified_entries += 1;
            continue;
        }
        if let Some(prg) = progress {
            if !prg.on_progress(idx as u64, total_entries, Some(f.rel_path.clone())) {
                return Err(TTZipError::Cancelled);
            }
        }

        match crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            idx,
            password,
            budget_bytes,
        ) {
            Ok(_) => verified_entries += 1,
            Err(err) => {
                let reason = match err {
                    crate::types::TTZipStatus::ErrInvalidPassword => "Invalid password".to_string(),
                    crate::types::TTZipStatus::ErrSolidBudgetExceeded => {
                        "Solid stream budget exceeded".to_string()
                    }
                    _ => format!("Extraction error code: {err:?}"),
                };
                corrupted.push(UniFFICorruptedEntry {
                    path: f.rel_path.clone(),
                    expected_crc32: arch
                        .seek_index()
                        .entries
                        .get(idx)
                        .and_then(|e| e.crc)
                        .unwrap_or(0),
                    actual_crc32: 0,
                    reason,
                });
            }
        }
    }

    Ok(UniFFIIntegrityReport {
        is_valid: corrupted.is_empty(),
        total_entries,
        verified_entries,
        corrupted_entries: corrupted,
        elapsed_nanos: start.elapsed().as_nanos() as u64,
        error_message: None,
    })
}

fn verify_zip_archive_integrity(
    mapped: &[u8],
    archive_path: &str,
    password: Option<&str>,
    progress: Option<&dyn ProgressHandler>,
    token: Option<&Arc<CancellationToken>>,
    start: std::time::Instant,
) -> Result<UniFFIIntegrityReport, TTZipError> {
    let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) else {
        return Ok(corrupted_header_report(
            archive_path.to_string(),
            "Corrupted ZIP header or central directory",
            "Corrupted ZIP archive header or central directory".to_string(),
            start,
        ));
    };

    let entries = zip_archive.entries();
    let total_entries = entries.len() as u64;
    let mut verified_entries: u64 = 0;
    let mut corrupted = Vec::new();

    for (idx, e) in entries.iter().enumerate() {
        if token.is_some_and(|t| t.is_cancelled()) {
            return Err(TTZipError::Cancelled);
        }
        if e.is_directory {
            verified_entries += 1;
            continue;
        }
        if let Some(prg) = progress {
            if !prg.on_progress(idx as u64, total_entries, Some(e.rel_path.clone())) {
                return Err(TTZipError::Cancelled);
            }
        }

        match verify_zip_entry_streaming(&zip_archive, idx, password) {
            Ok(_) => verified_entries += 1,
            Err((err, actual_crc32)) => {
                corrupted.push(UniFFICorruptedEntry {
                    path: e.rel_path.clone(),
                    expected_crc32: e.crc32,
                    actual_crc32,
                    reason: format!("CRC or payload verification failed: {err:?}"),
                });
            }
        }
    }

    Ok(UniFFIIntegrityReport {
        is_valid: corrupted.is_empty(),
        total_entries,
        verified_entries,
        corrupted_entries: corrupted,
        elapsed_nanos: start.elapsed().as_nanos() as u64,
        error_message: None,
    })
}

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

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") || archive_path.to_lowercase().ends_with(".7z") {
        return verify_7z_archive_integrity(
            mapped,
            &archive_path,
            password.as_deref(),
            progress.as_deref(),
            token.as_ref(),
            start,
        );
    }
    if mapped.starts_with(b"PK\x03\x04")
        || mapped.starts_with(b"PK\x05\x06")
        || archive_path.to_lowercase().ends_with(".zip")
    {
        return verify_zip_archive_integrity(
            mapped,
            &archive_path,
            password.as_deref(),
            progress.as_deref(),
            token.as_ref(),
            start,
        );
    }

    let pwd_cstr = password.as_deref().and_then(|p| std::ffi::CString::new(p).ok());
    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_3,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        collision_policy: crate::types::TTZIP_COLLISION_POLICY_REPLACE,
        preserve_permissions: false,
        dry_run: true,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let staging_root = ttzip_archive::fs::collision::StagingRoot(std::env::temp_dir());
    match crate::archive::unified::extract::extract_archive_with_metrics(p, &staging_root, &options)
    {
        Ok(_) => Ok(UniFFIIntegrityReport {
            is_valid: true,
            total_entries: 1,
            verified_entries: 1,
            corrupted_entries: Vec::new(),
            elapsed_nanos: start.elapsed().as_nanos() as u64,
            error_message: None,
        }),
        Err(status) => Ok(corrupted_header_report(
            archive_path,
            &format!("Extraction verification failed with status: {status:?}"),
            format!("Error code: {status:?}"),
            start,
        )),
    }
}

fn stream_file_chunks<F: FnMut(&[u8])>(
    file: &mut std::fs::File,
    mut on_chunk: F,
) -> Result<(), TTZipError> {
    use std::io::Read;
    let mut buf = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
        if n == 0 {
            break;
        }
        on_chunk(&buf[..n]);
    }
    Ok(())
}

fn format_lowercase_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn stream_xxh3_64_hex(file: &mut std::fs::File) -> Result<String, TTZipError> {
    let mut hasher = crate::crypto::xxh3::Xxh3_64::new();
    stream_file_chunks(file, |c| hasher.update(c))?;
    Ok(format!("{:016x}", hasher.finalize()))
}

fn stream_xxh3_128_hex(file: &mut std::fs::File) -> Result<String, TTZipError> {
    let mut hasher = crate::crypto::xxh3::Xxh3_128::new();
    stream_file_chunks(file, |c| hasher.update(c))?;
    let (low, high) = hasher.finalize();
    Ok(format!("{high:016x}{low:016x}"))
}

fn stream_crc64_hex(file: &mut std::fs::File) -> Result<String, TTZipError> {
    let mut crc = 0u64;
    stream_file_chunks(file, |c| crc = crate::crypto::crc64::crc64(c, crc))?;
    Ok(format!("{crc:016x}"))
}

fn stream_crc32_hex(file: &mut std::fs::File) -> Result<String, TTZipError> {
    let mut crc = 0u32;
    stream_file_chunks(file, |c| crc = crate::crypto::crc32::crc32_fast(crc, c))?;
    Ok(format!("{crc:08X}"))
}

fn stream_adler32_hex(file: &mut std::fs::File) -> Result<String, TTZipError> {
    let mut adler = 1u32;
    stream_file_chunks(file, |c| adler = crate::crypto::adler32::adler32_fast(adler, c))?;
    Ok(format!("{adler:08X}"))
}

fn compute_checksum_stream_hash(
    file: &mut std::fs::File,
    algo_norm: &str,
) -> Option<Result<String, TTZipError>> {
    match algo_norm {
        "xxh364" => Some(stream_xxh3_64_hex(file)),
        "xxh3128" => Some(stream_xxh3_128_hex(file)),
        "crc64" => Some(stream_crc64_hex(file)),
        "crc32" => Some(stream_crc32_hex(file)),
        "adler32" => Some(stream_adler32_hex(file)),
        _ => None,
    }
}

/// Computes hash or checksum of a file in zero-copy chunked streaming fashion across all supported algorithms.
#[uniffi::export]
pub fn compute_file_stream_hash(file_path: String, algorithm: String) -> Result<String, TTZipError> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }

    let mut file = std::fs::File::open(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let algo_norm = algorithm.trim().to_ascii_lowercase().replace(['-', '_'], "");

    if let Some(res) = compute_checksum_stream_hash(&mut file, &algo_norm) {
        return res;
    }

    match algo_norm.as_str() {
        "blake3" => {
            let mut hasher = crate::crypto::blake3::Hasher::new();
            stream_file_chunks(&mut file, |c| {
                hasher.update(c);
            })?;
            Ok(crate::uniffi_api::hasher_stream::bytes_to_hex(&hasher.finalize()))
        }
        "sha256" => {
            let mut hasher = crate::crypto::sha256::FastSha256::new();
            stream_file_chunks(&mut file, |c| hasher.update(c))?;
            Ok(format_lowercase_hex(&hasher.finalize()))
        }
        "sha1" => {
            let mut hasher = crate::crypto::sha1::FastSha1::new();
            stream_file_chunks(&mut file, |c| hasher.update(c))?;
            Ok(format_lowercase_hex(&hasher.finalize()))
        }
        "md5" => {
            let mut hasher = crate::crypto::md5::FastMd5::new();
            stream_file_chunks(&mut file, |c| hasher.update(c))?;
            Ok(format_lowercase_hex(&hasher.finalize()))
        }
        _ => Err(TTZipError::EngineError { code: -1 }),
    }
}

/// Computes hash or checksum of a file in zero-copy chunked streaming fashion.
#[uniffi::export]
pub fn compute_file_hash(path: String, algorithm: String) -> Result<String, TTZipError> {
    compute_file_stream_hash(path, algorithm)
}

/// Computes SHA-256 hex digest of a file in zero-copy streaming fashion.
#[uniffi::export]
pub fn compute_file_sha256(file_path: String) -> Result<String, TTZipError> {
    compute_file_stream_hash(file_path, "sha256".to_string())
}

/// Computes CRC32 checksum of a file in zero-copy streaming fashion.
#[uniffi::export]
pub fn compute_file_crc32(file_path: String) -> Result<u32, TTZipError> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let mut file = std::fs::File::open(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut crc = 0u32;
    stream_file_chunks(&mut file, |c| crc = crate::crypto::crc32::crc32_fast(crc, c))?;
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

    let mut buffer = [0u8; 64 * 1024];
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};
    use std::io::Write;

    #[test]
    fn test_compute_file_stream_hash_all_algorithms() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let test_file = temp_dir.path().join("hash_input.bin");
        {
            let mut f = std::fs::File::create(&test_file).expect("create test file");
            f.write_all(b"123456789").expect("write bytes");
        }
        let file_path = test_file.to_str().unwrap().to_string();

        let crc32 = compute_file_stream_hash(file_path.clone(), "crc32".to_string()).unwrap();
        assert_eq!(crc32, "CBF43926");
        assert_eq!(compute_file_crc32(file_path.clone()).unwrap(), 0xCBF43926);

        let adler32 = compute_file_stream_hash(file_path.clone(), "Adler32".to_string()).unwrap();
        assert_eq!(adler32, "091E01DE");

        let crc64 = compute_file_stream_hash(file_path.clone(), "CRC64".to_string()).unwrap();
        assert_eq!(crc64, "b86883e6fa710a9f");

        let sha256 = compute_file_stream_hash(file_path.clone(), "sha-256".to_string()).unwrap();
        assert_eq!(sha256, "15e2b0d3c33891ebb0f1ef609ec419420c20e320ce94c65fbc8c3312448eb225");
        assert_eq!(compute_file_sha256(file_path.clone()).unwrap(), sha256);

        let sha1 = compute_file_stream_hash(file_path.clone(), "sha1".to_string()).unwrap();
        assert_eq!(sha1, "f7c3bc1d808e04732adf679965ccc34ca7ae3441");

        let md5 = compute_file_stream_hash(file_path.clone(), "MD5".to_string()).unwrap();
        assert_eq!(md5, "25f9e794323b453885f5181f1b624d0b");

        let blake3 = compute_file_stream_hash(file_path.clone(), "BLAKE3".to_string()).unwrap();
        let expected_blake3 = crate::crypto::blake3::blake3(b"123456789");
        assert_eq!(blake3, crate::uniffi_api::hasher_stream::bytes_to_hex(&expected_blake3));

        let xxh3_64 = compute_file_stream_hash(file_path.clone(), "xxh3_64".to_string()).unwrap();
        let expected_xxh3_64 = format!("{:016x}", crate::crypto::xxh3::xxh3_64(b"123456789"));
        assert_eq!(xxh3_64, expected_xxh3_64);

        let xxh3_128 = compute_file_stream_hash(file_path.clone(), "xxh3-128".to_string()).unwrap();
        let (low, high) = crate::crypto::xxh3::xxh3_128(b"123456789");
        let expected_xxh3_128 = format!("{:016x}{:016x}", high, low);
        assert_eq!(xxh3_128, expected_xxh3_128);

        assert!(compute_file_stream_hash(file_path, "nonexistent".to_string()).is_err());
        assert!(compute_file_stream_hash("/path/that/does/not/exist".to_string(), "sha256".to_string()).is_err());
    }

    #[test]
    fn test_streaming_zip_integrity_verification_reports_actual_crc() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("verify.zip");
        let items = vec![
            ZipInputItem {
                rel_path: "a.txt".to_string(),
                data: b"streaming integrity payload".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            ZipInputItem {
                rel_path: "b.txt".to_string(),
                data: vec![b'x'; 128 * 1024],
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
        ];
        let comp = compress_items_parallel(
            items,
            6,
            crate::types::TTZipEncryptionMethod::None,
            None,
            1,
        )
        .expect("comp");
        let archive_bytes = assemble_zip_archive(&comp).expect("asm");
        std::fs::write(&zip_path, &archive_bytes).expect("write");

        let report = verify_archive_integrity(
            zip_path.to_string_lossy().into_owned(),
            None,
            None,
            None,
        )
        .expect("verify");
        assert!(report.is_valid);
        assert_eq!(report.total_entries, 2);
        assert_eq!(report.verified_entries, 2);
        assert!(report.corrupted_entries.is_empty());
    }
}
