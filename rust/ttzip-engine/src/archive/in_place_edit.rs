// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Universal In-Place Atomic Archive & Compression Stream Mutation Engine.
//!
//! Supports transactional append, replace, and delete operations across 18 formats:
//! 1. ZIP (zero-recompression Central Directory reconstruction)
//! 2. 7-Zip (non-solid / solid append and header index reconstruction)
//! 3. POSIX TAR (512-byte aligned in-place block overwrite, append, and double zero-block termination)
//! 4-7. Compound TAR streams (TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZSTD) via single-pass in-memory micro-buffering
//! 8-14. Single-file compressed streams (GZ, BZ2, XZ, ZST, Snappy, Brotli, LZFSE)
//! 15-18. Generic containers (SquashFS, ISO 9660, CAB, WIM, DEB, RPM, CPIO, AR) with WAL transaction journals.

pub mod compound;
pub mod container;
pub mod sevenz;
pub mod single_stream;
pub mod tar;
pub mod wal;
pub mod zip;

#[cfg(test)]
mod tests;

pub use compound::in_place_edit_compound_stream;
pub use container::in_place_edit_generic_container_wal;
pub use sevenz::in_place_edit_sevenz;
pub use single_stream::in_place_edit_single_stream;
pub use tar::in_place_edit_tar;
pub use wal::{WalJournalRecord, WalState, WalTransactionJournal, INPLACE_COUNTER};
pub use zip::in_place_edit_zip;

use crate::standards::signatures::{CompoundFormat, DetectedFormat};
use crate::standards::sniffer::detect_format_file;
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

/// Action performed on an archive entry during an in-place editing transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InPlaceAction {
    Append { entry_path: String, source_path: PathBuf },
    Replace { entry_path: String, source_path: PathBuf },
    Delete { entry_path: String },
}

/// Category classification for format-specific mutation pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InPlaceCategory {
    Zip,
    SevenZip,
    Tar,
    CompoundTar(CompoundFormat),
    SingleStream(DetectedFormat),
    GenericContainer(DetectedFormat),
}

/// Transactional session managing atomic in-place archive mutations across 18 formats.
#[derive(Debug)]
pub struct InPlaceArchiveSession {
    pub archive_path: PathBuf,
    pub shadow_path: PathBuf,
    pub wal_path: PathBuf,
    pub format: TTZipArchiveFormat,
    pub category: InPlaceCategory,
    pub actions: Vec<InPlaceAction>,
    pub committed: bool,
}

impl InPlaceArchiveSession {
    /// Begins a new in-place archive mutation transaction.
    pub fn begin(archive_path: impl AsRef<Path>, format: Option<TTZipArchiveFormat>) -> Result<Self, TTZipStatus> {
        let archive_path = archive_path.as_ref().to_path_buf();
        if !archive_path.exists() {
            return Err(TTZipStatus::ErrFileNotFound);
        }

        let category = detect_in_place_category(&archive_path, format);
        let fmt = match format {
            Some(f) if f != TTZipArchiveFormat::Auto => f,
            _ => category_to_legacy_format(category),
        };

        let shadow_path = generate_shadow_path(&archive_path);
        let wal_path = generate_wal_path(&archive_path);

        Ok(Self {
            archive_path,
            shadow_path,
            wal_path,
            format: fmt,
            category,
            actions: Vec::new(),
            committed: false,
        })
    }

    /// Queues an entry append operation.
    pub fn append(&mut self, entry_path: &str, source_path: impl AsRef<Path>) -> Result<(), TTZipStatus> {
        let src = source_path.as_ref().to_path_buf();
        if !src.exists() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        self.actions.push(InPlaceAction::Append {
            entry_path: entry_path.to_string(),
            source_path: src,
        });
        Ok(())
    }

    /// Queues an entry replace operation.
    pub fn replace(&mut self, entry_path: &str, source_path: impl AsRef<Path>) -> Result<(), TTZipStatus> {
        let src = source_path.as_ref().to_path_buf();
        if !src.exists() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        self.actions.push(InPlaceAction::Replace {
            entry_path: entry_path.to_string(),
            source_path: src,
        });
        Ok(())
    }

    /// Queues an entry delete operation.
    pub fn delete(&mut self, entry_path: &str) -> Result<(), TTZipStatus> {
        self.actions.push(InPlaceAction::Delete {
            entry_path: entry_path.to_string(),
        });
        Ok(())
    }

    /// Commits all pending mutations atomically into the target archive.
    pub fn commit(&mut self) -> Result<(), TTZipStatus> {
        if self.committed {
            return Ok(());
        }

        match self.category {
            InPlaceCategory::Zip => {
                in_place_edit_zip(&self.archive_path, &self.shadow_path, &self.actions)?;
            }
            InPlaceCategory::SevenZip => {
                in_place_edit_sevenz(&self.archive_path, &self.shadow_path, &self.actions)?;
            }
            InPlaceCategory::Tar => {
                in_place_edit_tar(&self.archive_path, &self.shadow_path, &self.actions)?;
            }
            InPlaceCategory::CompoundTar(comp_fmt) => {
                in_place_edit_compound_stream(&self.archive_path, &self.shadow_path, comp_fmt, &self.actions)?;
            }
            InPlaceCategory::SingleStream(det_fmt) => {
                in_place_edit_single_stream(&self.archive_path, &self.shadow_path, det_fmt, &self.actions)?;
            }
            InPlaceCategory::GenericContainer(det_fmt) => {
                in_place_edit_generic_container_wal(&self.archive_path, &self.shadow_path, &self.wal_path, det_fmt, &self.actions)?;
            }
        }

        fs::rename(&self.shadow_path, &self.archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if self.wal_path.exists() {
            let _ = fs::remove_file(&self.wal_path);
        }
        self.committed = true;
        Ok(())
    }

    /// Cancels and rolls back the transaction, cleaning up any shadow or WAL files.
    pub fn cancel(&mut self) -> Result<(), TTZipStatus> {
        if !self.committed {
            if self.shadow_path.exists() {
                let _ = fs::remove_file(&self.shadow_path);
            }
            if self.wal_path.exists() {
                let _ = fs::remove_file(&self.wal_path);
            }
        }
        self.actions.clear();
        Ok(())
    }

    /// Transactional rollback alias.
    pub fn rollback(&mut self) -> Result<(), TTZipStatus> {
        self.cancel()
    }
}

impl Drop for InPlaceArchiveSession {
    fn drop(&mut self) {
        if !self.committed {
            if self.shadow_path.exists() {
                let _ = fs::remove_file(&self.shadow_path);
            }
            if self.wal_path.exists() {
                let _ = fs::remove_file(&self.wal_path);
            }
        }
    }
}

fn generate_shadow_path(archive_path: &Path) -> PathBuf {
    let parent = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = archive_path.file_name().and_then(|s| s.to_str()).unwrap_or("archive");
    let count = INPLACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    parent.join(format!("{}.ttzip_inplace_{}_{}.tmp", stem, pid, count))
}

fn generate_wal_path(archive_path: &Path) -> PathBuf {
    let parent = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = archive_path.file_name().and_then(|s| s.to_str()).unwrap_or("archive");
    let count = INPLACE_COUNTER.load(Ordering::Relaxed);
    let pid = std::process::id();
    parent.join(format!("{}.ttzip_wal_{}_{}.log", stem, pid, count))
}

/// Classifies archive path and format hint into execution dispatch category.
pub fn detect_in_place_category(path: &Path, explicit_fmt: Option<TTZipArchiveFormat>) -> InPlaceCategory {
    if let Some(fmt) = explicit_fmt {
        match fmt {
            TTZipArchiveFormat::Zip => return InPlaceCategory::Zip,
            TTZipArchiveFormat::SevenZip => return InPlaceCategory::SevenZip,
            TTZipArchiveFormat::Tar => return InPlaceCategory::Tar,
            TTZipArchiveFormat::TarGz => return InPlaceCategory::CompoundTar(CompoundFormat::TarGz),
            TTZipArchiveFormat::TarBz2 => return InPlaceCategory::CompoundTar(CompoundFormat::TarBz2),
            TTZipArchiveFormat::TarXz => return InPlaceCategory::CompoundTar(CompoundFormat::TarXz),
            TTZipArchiveFormat::TarZstd => return InPlaceCategory::CompoundTar(CompoundFormat::TarZstd),
            TTZipArchiveFormat::Lzfse => return InPlaceCategory::SingleStream(DetectedFormat::Lzfse),
            TTZipArchiveFormat::Snappy => return InPlaceCategory::SingleStream(DetectedFormat::Snappy),
            _ => {}
        }
    }

    let name = path.to_string_lossy().to_lowercase();
    if let Ok(sniff) = detect_format_file(path) {
        if let Some(compound) = sniff.compound_format {
            return InPlaceCategory::CompoundTar(compound);
        }
        match sniff.format {
            DetectedFormat::Zip => return InPlaceCategory::Zip,
            DetectedFormat::SevenZip => return InPlaceCategory::SevenZip,
            DetectedFormat::Tar => return InPlaceCategory::Tar,
            DetectedFormat::Gzip => {
                if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
                    return InPlaceCategory::CompoundTar(CompoundFormat::TarGz);
                }
                return InPlaceCategory::SingleStream(DetectedFormat::Gzip);
            }
            DetectedFormat::Bzip2 => {
                if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
                    return InPlaceCategory::CompoundTar(CompoundFormat::TarBz2);
                }
                return InPlaceCategory::SingleStream(DetectedFormat::Bzip2);
            }
            DetectedFormat::Xz => {
                if name.ends_with(".tar.xz") || name.ends_with(".txz") {
                    return InPlaceCategory::CompoundTar(CompoundFormat::TarXz);
                }
                return InPlaceCategory::SingleStream(DetectedFormat::Xz);
            }
            DetectedFormat::Zstd => {
                if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
                    return InPlaceCategory::CompoundTar(CompoundFormat::TarZstd);
                }
                return InPlaceCategory::SingleStream(DetectedFormat::Zstd);
            }
            DetectedFormat::Snappy => return InPlaceCategory::SingleStream(DetectedFormat::Snappy),
            DetectedFormat::Brotli => return InPlaceCategory::SingleStream(DetectedFormat::Brotli),
            DetectedFormat::Lzfse => return InPlaceCategory::SingleStream(DetectedFormat::Lzfse),
            DetectedFormat::Ar | DetectedFormat::Cab | DetectedFormat::Iso | DetectedFormat::Wim | DetectedFormat::Xar | DetectedFormat::Lzh => {
                return InPlaceCategory::GenericContainer(sniff.format);
            }
            _ => {}
        }
    }

    if name.ends_with(".zip") {
        InPlaceCategory::Zip
    } else if name.ends_with(".7z") || name.ends_with(".cb7") {
        InPlaceCategory::SevenZip
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        InPlaceCategory::CompoundTar(CompoundFormat::TarGz)
    } else if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
        InPlaceCategory::CompoundTar(CompoundFormat::TarBz2)
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        InPlaceCategory::CompoundTar(CompoundFormat::TarXz)
    } else if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        InPlaceCategory::CompoundTar(CompoundFormat::TarZstd)
    } else if name.ends_with(".tar") {
        InPlaceCategory::Tar
    } else if name.ends_with(".gz") {
        InPlaceCategory::SingleStream(DetectedFormat::Gzip)
    } else if name.ends_with(".bz2") {
        InPlaceCategory::SingleStream(DetectedFormat::Bzip2)
    } else if name.ends_with(".xz") {
        InPlaceCategory::SingleStream(DetectedFormat::Xz)
    } else if name.ends_with(".zst") {
        InPlaceCategory::SingleStream(DetectedFormat::Zstd)
    } else if name.ends_with(".sz") || name.ends_with(".snappy") {
        InPlaceCategory::SingleStream(DetectedFormat::Snappy)
    } else if name.ends_with(".br") {
        InPlaceCategory::SingleStream(DetectedFormat::Brotli)
    } else if name.ends_with(".lzfse") {
        InPlaceCategory::SingleStream(DetectedFormat::Lzfse)
    } else if name.ends_with(".ar") || name.ends_with(".deb") {
        InPlaceCategory::GenericContainer(DetectedFormat::Ar)
    } else if name.ends_with(".iso") {
        InPlaceCategory::GenericContainer(DetectedFormat::Iso)
    } else if name.ends_with(".cab") {
        InPlaceCategory::GenericContainer(DetectedFormat::Cab)
    } else if name.ends_with(".wim") {
        InPlaceCategory::GenericContainer(DetectedFormat::Wim)
    } else {
        InPlaceCategory::Zip
    }
}

fn category_to_legacy_format(category: InPlaceCategory) -> TTZipArchiveFormat {
    match category {
        InPlaceCategory::Zip => TTZipArchiveFormat::Zip,
        InPlaceCategory::SevenZip => TTZipArchiveFormat::SevenZip,
        InPlaceCategory::Tar => TTZipArchiveFormat::Tar,
        InPlaceCategory::CompoundTar(c) => c.to_ttzip_format(),
        InPlaceCategory::SingleStream(d) => match d {
            DetectedFormat::Lzfse => TTZipArchiveFormat::Lzfse,
            DetectedFormat::Snappy => TTZipArchiveFormat::Snappy,
            _ => TTZipArchiveFormat::Unknown,
        },
        InPlaceCategory::GenericContainer(_) => TTZipArchiveFormat::Unknown,
    }
}

/// Detects container format from magic headers or file extension for backward compatibility.
pub fn detect_archive_format(path: &Path) -> TTZipArchiveFormat {
    category_to_legacy_format(detect_in_place_category(path, None))
}
