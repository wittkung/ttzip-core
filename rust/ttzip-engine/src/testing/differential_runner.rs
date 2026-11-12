// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Differential Oracle Test Runner and RAII temporary directory isolation.

use std::fs;
use std::path::{Path, PathBuf};

use crate::types::TTZipStatus;
use super::differential::{compare_manifests, scan_directory_tree, DifferentialReport};

/// Lightweight RAII temporary directory manager for differential test isolation.
pub struct AutoCleanTempDir {
    path: PathBuf,
}

impl AutoCleanTempDir {
    pub fn new(prefix: &str) -> Result<Self, TTZipStatus> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir_name = format!("{}_{}_{}_{}", prefix, pid, nanos, count);
        let path = std::env::temp_dir().join(dir_name);
        fs::create_dir_all(&path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        Ok(Self { path })
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AutoCleanTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Automated System-Level Differential Oracle Test Runner.
pub struct DifferentialOracleTestRunner;

impl DifferentialOracleTestRunner {
    /// Executes bidirectional differential test for ZIP archives using system `/usr/bin/unzip`.
    pub fn run_zip_differential(
        items: &[crate::zip::writer::ZipInputItem],
        compression_level: i32,
    ) -> Result<DifferentialReport, TTZipStatus> {
        let temp_dir = AutoCleanTempDir::new("ttzip_diff_zip_runner")?;
        let temp_path = temp_dir.path();

        let compressed = crate::zip::writer::compress_items_parallel(
            items.to_vec(),
            compression_level,
            crate::types::TTZipEncryptionMethod::None,
            None,
            4,
        )?;
        let zip_bytes = crate::zip::writer::assemble_zip_archive(&compressed)?;
        let zip_file = temp_path.join("archive.zip");
        fs::write(&zip_file, &zip_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let oracle_dir = temp_path.join("oracle_extract");
        let ttzip_dir = temp_path.join("ttzip_extract");
        fs::create_dir_all(&oracle_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        fs::create_dir_all(&ttzip_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let output = std::process::Command::new("/usr/bin/unzip")
            .arg("-q")
            .arg("-o")
            .arg(&zip_file)
            .arg("-d")
            .arg(&oracle_dir)
            .output();

        if let Ok(out) = output {
            if !out.status.success() && out.status.code().unwrap_or(0) > 1 {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
        } else {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let archive = crate::zip::reader::ZipArchive::open_slice(&zip_bytes)?;
        let options = crate::types::TTZipExtractOptions::default();
        archive.extract_all(&ttzip_dir, &options)?;

        let oracle_manifest = scan_directory_tree(&oracle_dir)?;
        let ttzip_manifest = scan_directory_tree(&ttzip_dir)?;

        Ok(compare_manifests(&ttzip_manifest, &oracle_manifest, false, "/usr/bin/unzip", "zip"))
    }

    /// Executes bidirectional differential test for TAR archives using system `/usr/bin/tar`.
    pub fn run_tar_differential(
        items: &[crate::zip::writer::ZipInputItem],
    ) -> Result<DifferentialReport, TTZipStatus> {
        let temp_dir = AutoCleanTempDir::new("ttzip_diff_tar_runner")?;
        let temp_path = temp_dir.path();

        let tar_file = temp_path.join("archive.tar");
        let mut tar_bytes = Vec::new();
        {
            let mut writer = crate::archive::tar::writer::TarWriter::new(&mut tar_bytes);
            for item in items {
                let mtime = item.mtime_epoch_secs as i64;
                if item.is_directory {
                    writer.append_dir(&item.rel_path, item.mode, mtime)?;
                } else {
                    writer.append_file(&item.rel_path, &item.data, item.mode, mtime)?;
                }
            }
            writer.finish()?;
        }
        fs::write(&tar_file, &tar_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let oracle_dir = temp_path.join("oracle_extract");
        let ttzip_dir = temp_path.join("ttzip_extract");
        fs::create_dir_all(&oracle_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        fs::create_dir_all(&ttzip_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let output = std::process::Command::new("/usr/bin/tar")
            .arg("-xf")
            .arg(&tar_file)
            .arg("-C")
            .arg(&oracle_dir)
            .output();

        if let Ok(out) = output {
            if !out.status.success() {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
        } else {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let tar_arch = crate::archive::tar::reader::TarArchive::open_slice(&tar_bytes)?;
        let options = crate::types::TTZipExtractOptions::default();
        tar_arch.extract_all(&ttzip_dir, &options)?;

        let oracle_manifest = scan_directory_tree(&oracle_dir)?;
        let ttzip_manifest = scan_directory_tree(&ttzip_dir)?;

        Ok(compare_manifests(&ttzip_manifest, &oracle_manifest, true, "/usr/bin/tar", "tar"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::differential::{EntryType, FileTreeManifest, ManifestEntry};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn test_differential_scan_and_manifest_serialization() {
        let temp = tempdir().expect("failed to create temp dir");
        let root = temp.path();

        let f1 = root.join("hello.txt");
        fs::write(&f1, b"Hello TTZip Differential Rust Engine!").unwrap();

        let sub = root.join("nested");
        fs::create_dir(&sub).unwrap();
        let f2 = sub.join("payload.bin");
        fs::write(&f2, vec![0xAB; 1024]).unwrap();

        let manifest = scan_directory_tree(root).expect("scan directory tree failed");
        assert_eq!(manifest.total_file_count, 2);
        assert_eq!(manifest.total_directory_count, 1);
        assert_eq!(manifest.total_byte_size, 37 + 1024);

        let json = manifest.to_json();
        assert!(json.contains("hello.txt"));
        assert!(json.contains("nested/payload.bin"));

        let parsed = FileTreeManifest::from_json(&json).expect("failed to parse json manifest");
        assert_eq!(parsed.total_file_count, 2);
        assert_eq!(parsed.total_directory_count, 1);
        assert_eq!(parsed.entries.len(), 3);
    }

    #[test]
    fn test_compare_manifests_five_dimensions() {
        let mut entries_a = BTreeMap::new();
        entries_a.insert(
            "file.txt".to_string(),
            ManifestEntry::new(
                "file.txt".to_string(),
                EntryType::RegularFile,
                100,
                "abc123hash".to_string(),
                0o644,
                None,
            ),
        );

        let manifest_a = FileTreeManifest {
            root_directory: "/tmp/a".to_string(),
            entries: entries_a,
            total_byte_size: 100,
            total_file_count: 1,
            total_directory_count: 0,
            total_symlink_count: 0,
        };

        let report_pass = compare_manifests(&manifest_a, &manifest_a, true, "oracle", "tar");
        assert!(report_pass.is_passed);
        assert!(report_pass.divergence_errors.is_empty());

        let mut entries_b = BTreeMap::new();
        entries_b.insert(
            "file.txt".to_string(),
            ManifestEntry::new(
                "file.txt".to_string(),
                EntryType::RegularFile,
                100,
                "different_hash".to_string(),
                0o644,
                None,
            ),
        );

        let manifest_b = FileTreeManifest {
            root_directory: "/tmp/b".to_string(),
            entries: entries_b,
            total_byte_size: 100,
            total_file_count: 1,
            total_directory_count: 0,
            total_symlink_count: 0,
        };

        let report_diff = compare_manifests(&manifest_b, &manifest_a, true, "oracle", "tar");
        assert!(!report_diff.is_passed);
        assert_eq!(report_diff.divergence_errors.len(), 1);
        assert!(report_diff.divergence_errors[0].contains("SHA-256 checksum mismatch"));
    }

    #[test]
    fn test_differential_oracle_runner_zip_and_tar() {
        let items = vec![
            crate::zip::writer::ZipInputItem {
                rel_path: "doc1.txt".to_string(),
                data: b"TTZip Differential Oracle Zip File 1".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            crate::zip::writer::ZipInputItem {
                rel_path: "sub/nested.bin".to_string(),
                data: vec![0x42; 2048],
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
        ];

        let zip_report = DifferentialOracleTestRunner::run_zip_differential(&items, 6)
            .expect("zip differential test failed");
        assert!(zip_report.is_passed, "Zip differential errors: {:?}", zip_report.divergence_errors);

        let tar_report = DifferentialOracleTestRunner::run_tar_differential(&items)
            .expect("tar differential test failed");
        assert!(tar_report.is_passed, "Tar differential errors: {:?}", tar_report.divergence_errors);
    }
}
