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
}
