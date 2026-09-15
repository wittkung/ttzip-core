// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI In-Place Transactional Archive Mutation Export Layer.
//!
//! Provides memory-safe, mutex-protected atomic in-place append, replace,
//! delete, commit, and rollback capabilities for foreign language bindings.

use crate::archive::in_place_edit::InPlaceArchiveSession;
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use crate::uniffi_api::types::{ArchiveFormat, TTZipError};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;

/// Convenience alias matching UniFFI namespace naming conventions.
pub type UniFFIArchiveFormat = ArchiveFormat;

fn map_uniffi_format_to_internal(format: ArchiveFormat) -> TTZipArchiveFormat {
    match format {
        ArchiveFormat::Auto => TTZipArchiveFormat::Auto,
        ArchiveFormat::Zip => TTZipArchiveFormat::Zip,
        ArchiveFormat::SevenZip => TTZipArchiveFormat::SevenZip,
        ArchiveFormat::Tar => TTZipArchiveFormat::Tar,
        ArchiveFormat::TarGz => TTZipArchiveFormat::TarGz,
        ArchiveFormat::TarBz2 => TTZipArchiveFormat::TarBz2,
        ArchiveFormat::TarXz => TTZipArchiveFormat::TarXz,
        ArchiveFormat::TarZstd => TTZipArchiveFormat::TarZstd,
        ArchiveFormat::TarLz4 => TTZipArchiveFormat::TarLz4,
        ArchiveFormat::TarBrotli => TTZipArchiveFormat::TarBrotli,
        ArchiveFormat::TarLzip => TTZipArchiveFormat::TarLzip,
        ArchiveFormat::TarLrzip => TTZipArchiveFormat::TarLrzip,
        ArchiveFormat::Dmg => TTZipArchiveFormat::Dmg,
        ArchiveFormat::Lzfse => TTZipArchiveFormat::Lzfse,
        ArchiveFormat::Snappy => TTZipArchiveFormat::Snappy,
        ArchiveFormat::Gzip => TTZipArchiveFormat::Gzip,
        ArchiveFormat::Bzip2 => TTZipArchiveFormat::Bzip2,
        ArchiveFormat::Xz => TTZipArchiveFormat::Xz,
        ArchiveFormat::Zstd => TTZipArchiveFormat::Zstd,
        ArchiveFormat::Lz4 => TTZipArchiveFormat::Lz4,
        ArchiveFormat::Brotli => TTZipArchiveFormat::Brotli,
        ArchiveFormat::Iso => TTZipArchiveFormat::Iso,
        ArchiveFormat::Cab => TTZipArchiveFormat::Cab,
        ArchiveFormat::Wim => TTZipArchiveFormat::Wim,
        ArchiveFormat::Rar => TTZipArchiveFormat::Rar,
        ArchiveFormat::Aar => TTZipArchiveFormat::Aar,
        ArchiveFormat::Lzip => TTZipArchiveFormat::Lzip,
        ArchiveFormat::Lrzip => TTZipArchiveFormat::Lrzip,
        ArchiveFormat::Cpio => TTZipArchiveFormat::Cpio,
        ArchiveFormat::Ar => TTZipArchiveFormat::Ar,
        ArchiveFormat::Deb => TTZipArchiveFormat::Deb,
        ArchiveFormat::Rpm => TTZipArchiveFormat::Rpm,
        ArchiveFormat::Xar => TTZipArchiveFormat::Xar,
        ArchiveFormat::Squashfs => TTZipArchiveFormat::Squashfs,
        ArchiveFormat::Lzh => TTZipArchiveFormat::Lzh,
    }
}

fn map_status_to_ttzip_error(status: TTZipStatus, path_hint: Option<&str>) -> TTZipError {
    match status {
        TTZipStatus::ErrFileNotFound => TTZipError::FileNotFound {
            path: path_hint.unwrap_or("unknown").to_string(),
        },
        TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
            details: "Corrupted archive header encountered during in-place operation".to_string(),
            offset: 0,
        },
        TTZipStatus::ErrSecurityViolation => TTZipError::SecurityViolation {
            reason: "Security policy violation detected during in-place operation".to_string(),
        },
        TTZipStatus::Cancelled => TTZipError::Cancelled,
        _ => TTZipError::EngineError {
            code: status as i32,
        },
    }
}

/// Transactional in-place archive mutation session exposed to foreign runtimes.
///
/// Encapsulates atomic append, replace, delete, commit, and cancel operations
/// with mutex-guarded state safety and deterministic RAII rollback on drop.
#[derive(uniffi::Object)]
pub struct UniFFIInPlaceSession {
    inner: Mutex<InPlaceArchiveSession>,
}

#[uniffi::export]
impl UniFFIInPlaceSession {
    /// Begins a new transactional in-place mutation session against the specified archive file.
    #[uniffi::constructor]
    pub fn begin(
        archive_path: String,
        format: Option<ArchiveFormat>,
    ) -> Result<Arc<Self>, TTZipError> {
        let path = Path::new(&archive_path);
        if !path.exists() {
            return Err(TTZipError::FileNotFound { path: archive_path });
        }

        let internal_fmt = format.map(map_uniffi_format_to_internal);
        let session = InPlaceArchiveSession::begin(path, internal_fmt)
            .map_err(|st| map_status_to_ttzip_error(st, Some(&archive_path)))?;

        Ok(Arc::new(Self {
            inner: Mutex::new(session),
        }))
    }

    /// Queues an entry append operation from an external file on disk.
    pub fn append(&self, entry_path: String, source_file_path: String) -> Result<(), TTZipError> {
        let mut session = self.inner.lock();
        if session.committed {
            return Err(TTZipError::IoError {
                message: "In-place session has already been committed".to_string(),
            });
        }
        let src_path = Path::new(&source_file_path);
        if !src_path.exists() {
            return Err(TTZipError::FileNotFound {
                path: source_file_path,
            });
        }
        session
            .append(&entry_path, src_path)
            .map_err(|st| map_status_to_ttzip_error(st, Some(&source_file_path)))
    }

    /// Queues an entry replacement operation with content from an external source file.
    pub fn replace(&self, entry_path: String, source_file_path: String) -> Result<(), TTZipError> {
        let mut session = self.inner.lock();
        if session.committed {
            return Err(TTZipError::IoError {
                message: "In-place session has already been committed".to_string(),
            });
        }
        let src_path = Path::new(&source_file_path);
        if !src_path.exists() {
            return Err(TTZipError::FileNotFound {
                path: source_file_path,
            });
        }
        session
            .replace(&entry_path, src_path)
            .map_err(|st| map_status_to_ttzip_error(st, Some(&source_file_path)))
    }

    /// Queues an entry deletion operation.
    pub fn delete(&self, entry_path: String) -> Result<(), TTZipError> {
        let mut session = self.inner.lock();
        if session.committed {
            return Err(TTZipError::IoError {
                message: "In-place session has already been committed".to_string(),
            });
        }
        session
            .delete(&entry_path)
            .map_err(|st| map_status_to_ttzip_error(st, Some(&entry_path)))
    }

    /// Atomically commits all queued mutations into the original archive file.
    pub fn commit(&self) -> Result<(), TTZipError> {
        let mut session = self.inner.lock();
        session
            .commit()
            .map_err(|st| map_status_to_ttzip_error(st, None))
    }

    /// Cancels all pending mutations and discards any temporary shadow or WAL files.
    pub fn cancel(&self) -> Result<(), TTZipError> {
        let mut session = self.inner.lock();
        session
            .cancel()
            .map_err(|st| map_status_to_ttzip_error(st, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    #[test]
    fn test_uniffi_inplace_file_not_found() {
        let res = UniFFIInPlaceSession::begin("/path/to/nonexistent/archive.zip".to_string(), None);
        assert!(res.is_err());
        match res.err().unwrap() {
            TTZipError::FileNotFound { path } => {
                assert_eq!(path, "/path/to/nonexistent/archive.zip");
            }
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn test_uniffi_inplace_session_lifecycle() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");

        let items = vec![ZipInputItem {
            rel_path: "hello.txt".to_string(),
            data: b"Hello world".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(items, 6, crate::types::TTZipEncryptionMethod::None, None, 2).unwrap();
        let zip_bytes = assemble_zip_archive(&compressed).unwrap();
        std::fs::write(&zip_path, zip_bytes).unwrap();

        let session = UniFFIInPlaceSession::begin(
            zip_path.to_str().unwrap().to_string(),
            Some(ArchiveFormat::Zip),
        )
        .expect("Session begin should succeed");

        let new_file = dir.path().join("added.txt");
        std::fs::write(&new_file, b"New file content").unwrap();

        let append_res = session.append("added.txt".to_string(), new_file.to_str().unwrap().to_string());
        assert!(append_res.is_ok());

        let commit_res = session.commit();
        assert!(commit_res.is_ok());
    }
}
