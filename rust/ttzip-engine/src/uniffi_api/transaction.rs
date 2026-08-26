// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use sha2::{Digest, Sha256};
use super::types::TTZipError;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFITransactionDiff {
    pub has_changed: bool,
    pub old_hash: String,
    pub new_hash: String,
    pub bytes_written: u64,
}

/// Inspects a staged temporary file and computes cryptographic diff against its initial hash.
#[uniffi::export]
pub fn inspect_staging_file_mutation(
    staged_path: String,
    initial_hash: String,
) -> Result<UniFFITransactionDiff, TTZipError> {
    let p = Path::new(&staged_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: staged_path });
    }

    let mut file = File::open(p)
        .map_err(|e| TTZipError::IoError { message: format!("open staged file: {}", e) })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total_bytes = 0u64;

    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| TTZipError::IoError { message: format!("read staged file chunk: {}", e) })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total_bytes += n as u64;
    }

    let new_hash = format!("{:x}", hasher.finalize());
    let has_changed = new_hash != initial_hash;

    Ok(UniFFITransactionDiff {
        has_changed,
        old_hash: initial_hash,
        new_hash,
        bytes_written: total_bytes,
    })
}

/// Applies an in-place entry delta mutation recorded into `.ttzip.wal` journal.
#[uniffi::export]
pub fn apply_in_place_entry_mutation(
    archive_path: String,
    entry_path: String,
    new_data: Vec<u8>,
) -> Result<super::types::UniFFIWalMutationSummary, TTZipError> {
    let archive_p = Path::new(&archive_path);
    if !archive_p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let file_data = std::fs::read(archive_p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut target_offset = 0u64;
    let mut target_len = 0u64;

    if let Ok(zip) = crate::zip::reader::ZipArchive::open_slice(&file_data) {
        if let Some(entry) = zip.entries().iter().find(|e| {
            e.rel_path == entry_path || e.rel_path.trim_start_matches('/') == entry_path.trim_start_matches('/')
        }) {
            target_offset = entry.lfh_offset;
            if let Ok((payload_off, _)) = crate::zip::parser::parse_local_file_header(&file_data, entry.lfh_offset as usize) {
                target_len = (payload_off as u64 - entry.lfh_offset) + entry.compressed_size;
            } else {
                target_len = entry.compressed_size + 30 + entry.rel_path.len() as u64;
            }
        }
    } else if let Ok(tar) = crate::archive::tar::reader::TarArchive::open_slice(&file_data) {
        if let Some(entry) = tar.entries().iter().find(|e| {
            e.path == entry_path || e.path.trim_start_matches('/') == entry_path.trim_start_matches('/')
        }) {
            target_offset = entry.header_offset as u64;
            target_len = (entry.data_offset - entry.header_offset) as u64 + entry.size;
        }
    }

    let summary = crate::archive::wal_mutation::append_wal_mutation(
        archive_p,
        &entry_path,
        target_offset,
        target_len,
        &new_data,
    ).map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(super::types::UniFFIWalMutationSummary {
        wal_path: summary.wal_path,
        entry_path: summary.entry_path,
        delta_bytes: summary.delta_bytes,
        total_pieces: summary.total_pieces,
        is_staged: summary.is_staged,
    })
}

/// Atomically commits staged WAL mutations to archive using APFS CoW zero-copy clone and atomic rename.
#[uniffi::export]
pub fn commit_wal_to_archive(
    archive_path: String,
) -> Result<super::types::UniFFIWalCommitResult, TTZipError> {
    let p = Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }
    let res = crate::archive::wal_mutation::commit_wal_to_archive(p)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
    Ok(super::types::UniFFIWalCommitResult {
        success: res.success,
        bytes_written: res.bytes_written,
        cow_cloned: res.cow_cloned,
        elapsed_millis: res.elapsed_millis,
    })
}

/// Discards staged WAL mutations and cleans up journal files.
#[uniffi::export]
pub fn rollback_wal_mutation(
    archive_path: String,
) -> Result<bool, TTZipError> {
    let p = Path::new(&archive_path);
    crate::archive::wal_mutation::rollback_wal_mutation(p)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })
}

/// Inspects current WAL journal staging status for given archive.
#[uniffi::export]
pub fn inspect_wal_mutation_status(
    archive_path: String,
) -> Result<Option<super::types::UniFFIWalMutationSummary>, TTZipError> {
    let p = Path::new(&archive_path);
    let opt = crate::archive::wal_mutation::inspect_wal_status(p)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
    Ok(opt.map(|s| super::types::UniFFIWalMutationSummary {
        wal_path: s.wal_path,
        entry_path: s.entry_path,
        delta_bytes: s.delta_bytes,
        total_pieces: s.total_pieces,
        is_staged: s.is_staged,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_inspect_staging_file_mutation_lifecycle() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ttzip_test_tx_{}", nanos));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_file = temp_dir.join("payload.txt");

        // Write initial content
        {
            let mut f = File::create(&test_file).unwrap();
            f.write_all(b"Hello TTZip In-Place Transaction!").unwrap();
        }

        // Test with empty initial hash -> should report has_changed: true and return correct new_hash
        let diff1 = inspect_staging_file_mutation(test_file.to_str().unwrap().to_string(), String::new()).unwrap();
        assert!(diff1.has_changed);
        assert_eq!(diff1.bytes_written, 33);
        assert!(!diff1.new_hash.is_empty());
        let computed_hash = diff1.new_hash.clone();

        // Test with matched initial hash -> has_changed: false
        let diff2 = inspect_staging_file_mutation(test_file.to_str().unwrap().to_string(), computed_hash.clone()).unwrap();
        assert!(!diff2.has_changed);
        assert_eq!(diff2.new_hash, computed_hash);
        assert_eq!(diff2.bytes_written, 33);

        // Modify file content
        {
            let mut f = File::create(&test_file).unwrap();
            f.write_all(b"Hello TTZip In-Place Transaction Modified!").unwrap();
        }

        // Test modified file -> has_changed: true
        let diff3 = inspect_staging_file_mutation(test_file.to_str().unwrap().to_string(), computed_hash).unwrap();
        assert!(diff3.has_changed);
        assert_eq!(diff3.bytes_written, 42);

        // Test non-existent file -> FileNotFound error
        let missing = temp_dir.join("missing.txt");
        let res = inspect_staging_file_mutation(missing.to_str().unwrap().to_string(), String::new());
        assert!(matches!(res, Err(TTZipError::FileNotFound { .. })));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_uniffi_wal_mutation_lifecycle_and_rollback() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ttzip_test_uniffi_wal_{}", nanos));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let archive_path = temp_dir.join("test.zip");

        // Create initial ZIP archive
        let items = vec![
            crate::zip::writer::ZipInputItem {
                rel_path: "doc1.txt".to_string(),
                data: b"Original Doc 1 Payload".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            crate::zip::writer::ZipInputItem {
                rel_path: "doc2.txt".to_string(),
                data: b"Original Doc 2 Payload".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
        ];
        let compressed = crate::zip::writer::compress_items_parallel(items, 6, crate::types::TTZipEncryptionMethod::None, None, 1).unwrap();
        let zip_bytes = crate::zip::writer::assemble_zip_archive(&compressed).unwrap();
        std::fs::write(&archive_path, &zip_bytes).unwrap();

        let arch_str = archive_path.to_str().unwrap().to_string();

        // 1. Stage mutation
        let summary = apply_in_place_entry_mutation(
            arch_str.clone(),
            "doc1.txt".to_string(),
            b"Modified Doc 1 Delta Content".to_vec(),
        ).expect("apply wal mutation");
        assert!(summary.is_staged);
        assert_eq!(summary.delta_bytes, 28);
        assert!(summary.total_pieces >= 1);

        // 2. Inspect WAL status
        let status = inspect_wal_mutation_status(arch_str.clone()).expect("inspect wal status");
        assert!(status.is_some());
        assert!(status.unwrap().is_staged);

        // 3. Commit WAL to archive
        let commit_res = commit_wal_to_archive(arch_str.clone()).expect("commit wal");
        assert!(commit_res.success);
        assert!(commit_res.bytes_written > 0);

        // Verify WAL is cleaned up after commit
        let status_after = inspect_wal_mutation_status(arch_str.clone()).expect("inspect post commit");
        assert!(status_after.is_none());

        // 4. Test rollback
        let _ = apply_in_place_entry_mutation(
            arch_str.clone(),
            "doc2.txt".to_string(),
            b"Discarded Delta".to_vec(),
        ).unwrap();
        let rolled_back = rollback_wal_mutation(arch_str.clone()).expect("rollback");
        assert!(rolled_back);

        let status_rb = inspect_wal_mutation_status(arch_str.clone()).expect("inspect post rollback");
        assert!(status_rb.is_none());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
