// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Universal In-Place Atomic Archive & Compression Stream Mutation Engine.
//!
//! Supports transactional append, replace, and delete operations across 18 formats:
//!
//! - 1-3. Direct archive mutation: ZIP (CD reconstruction), 7-Zip (solid append), POSIX TAR (512-byte block overwrite)
//! - 4-7. Compound TAR streams (TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZSTD) via single-pass in-memory micro-buffering
//! - 8-14. Single-file compressed streams (GZ, BZ2, XZ, ZST, Snappy, Brotli, LZFSE)
//! - 15-18. Generic containers (SquashFS, ISO 9660, CAB, WIM, DEB, RPM, CPIO, AR) with WAL transaction journals.

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
use crate::standards::sniffer::{detect_format_file, SniffResult};
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

/// Normalized mutation plan partitioned into deletions, replacements, and appends.
#[derive(Debug, Default)]
pub(crate) struct PartitionedActions {
    pub deleted: HashSet<String>,
    pub replaced: HashMap<String, PathBuf>,
    pub appended: Vec<(String, PathBuf)>,
}

impl PartitionedActions {
    pub(crate) fn from_slice(actions: &[InPlaceAction]) -> Self {
        let mut plan = Self::default();
        for action in actions {
            match action {
                InPlaceAction::Delete { entry_path } => {
                    plan.deleted.insert(entry_path.trim_start_matches('/').to_string());
                }
                InPlaceAction::Replace { entry_path, source_path } => {
                    plan.replaced
                        .insert(entry_path.trim_start_matches('/').to_string(), source_path.clone());
                }
                InPlaceAction::Append { entry_path, source_path } => {
                    plan.appended
                        .push((entry_path.trim_start_matches('/').to_string(), source_path.clone()));
                }
            }
        }
        plan
    }
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

        let tx_id = INPLACE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let shadow_path = generate_shadow_path(&archive_path, pid, tx_id);
        let wal_path = generate_wal_path(&archive_path, pid, tx_id);

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

        self.dispatch_category_mutation()?;
        fs::rename(&self.shadow_path, &self.archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if self.wal_path.exists() {
            let _ = fs::remove_file(&self.wal_path);
        }
        self.committed = true;
        Ok(())
    }

    fn dispatch_category_mutation(&self) -> Result<(), TTZipStatus> {
        match self.category {
            InPlaceCategory::Zip => in_place_edit_zip(&self.archive_path, &self.shadow_path, &self.actions),
            InPlaceCategory::SevenZip => in_place_edit_sevenz(&self.archive_path, &self.shadow_path, &self.actions),
            InPlaceCategory::Tar => in_place_edit_tar(&self.archive_path, &self.shadow_path, &self.actions),
            InPlaceCategory::CompoundTar(comp_fmt) => {
                in_place_edit_compound_stream(&self.archive_path, &self.shadow_path, comp_fmt, &self.actions)
            }
            InPlaceCategory::SingleStream(det_fmt) => {
                in_place_edit_single_stream(&self.archive_path, &self.shadow_path, det_fmt, &self.actions)
            }
            InPlaceCategory::GenericContainer(det_fmt) => in_place_edit_generic_container_wal(
                &self.archive_path,
                &self.shadow_path,
                &self.wal_path,
                det_fmt,
                &self.actions,
            ),
        }
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

fn generate_shadow_path(archive_path: &Path, pid: u32, tx_id: u64) -> PathBuf {
    let parent = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = archive_path.file_name().and_then(|s| s.to_str()).unwrap_or("archive");
    parent.join(format!("{}.ttzip_inplace_{}_{}.tmp", stem, pid, tx_id))
}

fn generate_wal_path(archive_path: &Path, pid: u32, tx_id: u64) -> PathBuf {
    let parent = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = archive_path.file_name().and_then(|s| s.to_str()).unwrap_or("archive");
    parent.join(format!("{}.ttzip_wal_{}_{}.log", stem, pid, tx_id))
}

const EXTENSION_CATEGORY_TABLE: &[(&str, InPlaceCategory)] = &[
    (".zip", InPlaceCategory::Zip),
    (".7z", InPlaceCategory::SevenZip),
    (".cb7", InPlaceCategory::SevenZip),
    (".tar.gz", InPlaceCategory::CompoundTar(CompoundFormat::TarGz)),
    (".tgz", InPlaceCategory::CompoundTar(CompoundFormat::TarGz)),
    (".tar.bz2", InPlaceCategory::CompoundTar(CompoundFormat::TarBz2)),
    (".tbz2", InPlaceCategory::CompoundTar(CompoundFormat::TarBz2)),
    (".tar.xz", InPlaceCategory::CompoundTar(CompoundFormat::TarXz)),
    (".txz", InPlaceCategory::CompoundTar(CompoundFormat::TarXz)),
    (".tar.zst", InPlaceCategory::CompoundTar(CompoundFormat::TarZstd)),
    (".tzst", InPlaceCategory::CompoundTar(CompoundFormat::TarZstd)),
    (".tar", InPlaceCategory::Tar),
    (".gz", InPlaceCategory::SingleStream(DetectedFormat::Gzip)),
    (".bz2", InPlaceCategory::SingleStream(DetectedFormat::Bzip2)),
    (".xz", InPlaceCategory::SingleStream(DetectedFormat::Xz)),
    (".zst", InPlaceCategory::SingleStream(DetectedFormat::Zstd)),
    (".sz", InPlaceCategory::SingleStream(DetectedFormat::Snappy)),
    (".snappy", InPlaceCategory::SingleStream(DetectedFormat::Snappy)),
    (".br", InPlaceCategory::SingleStream(DetectedFormat::Brotli)),
    (".lzfse", InPlaceCategory::SingleStream(DetectedFormat::Lzfse)),
    (".ar", InPlaceCategory::GenericContainer(DetectedFormat::Ar)),
    (".deb", InPlaceCategory::GenericContainer(DetectedFormat::Ar)),
    (".iso", InPlaceCategory::GenericContainer(DetectedFormat::Iso)),
    (".cab", InPlaceCategory::GenericContainer(DetectedFormat::Cab)),
    (".wim", InPlaceCategory::GenericContainer(DetectedFormat::Wim)),
];

/// Classifies archive path and format hint into execution dispatch category.
pub fn detect_in_place_category(path: &Path, explicit_fmt: Option<TTZipArchiveFormat>) -> InPlaceCategory {
    if let Some(cat) = explicit_fmt.and_then(category_from_explicit_format) {
        return cat;
    }

    let name = path.to_string_lossy().to_lowercase();
    if let Ok(sniff) = detect_format_file(path) {
        if let Some(cat) = category_from_sniff(&sniff, &name) {
            return cat;
        }
    }

    category_from_extension(&name)
}

fn category_from_explicit_format(fmt: TTZipArchiveFormat) -> Option<InPlaceCategory> {
    match fmt {
        TTZipArchiveFormat::Zip => Some(InPlaceCategory::Zip),
        TTZipArchiveFormat::SevenZip => Some(InPlaceCategory::SevenZip),
        TTZipArchiveFormat::Tar => Some(InPlaceCategory::Tar),
        TTZipArchiveFormat::TarGz => Some(InPlaceCategory::CompoundTar(CompoundFormat::TarGz)),
        TTZipArchiveFormat::TarBz2 => Some(InPlaceCategory::CompoundTar(CompoundFormat::TarBz2)),
        TTZipArchiveFormat::TarXz => Some(InPlaceCategory::CompoundTar(CompoundFormat::TarXz)),
        TTZipArchiveFormat::TarZstd => Some(InPlaceCategory::CompoundTar(CompoundFormat::TarZstd)),
        TTZipArchiveFormat::Lzfse => Some(InPlaceCategory::SingleStream(DetectedFormat::Lzfse)),
        TTZipArchiveFormat::Snappy => Some(InPlaceCategory::SingleStream(DetectedFormat::Snappy)),
        _ => None,
    }
}

fn category_from_sniff(sniff: &SniffResult, name: &str) -> Option<InPlaceCategory> {
    if let Some(compound) = sniff.compound_format {
        return Some(InPlaceCategory::CompoundTar(compound));
    }
    match sniff.format {
        DetectedFormat::Zip => Some(InPlaceCategory::Zip),
        DetectedFormat::SevenZip => Some(InPlaceCategory::SevenZip),
        DetectedFormat::Tar => Some(InPlaceCategory::Tar),
        DetectedFormat::Gzip | DetectedFormat::Bzip2 | DetectedFormat::Xz | DetectedFormat::Zstd => {
            Some(classify_compressed_stream(sniff.format, name))
        }
        DetectedFormat::Snappy | DetectedFormat::Brotli | DetectedFormat::Lzfse => {
            Some(InPlaceCategory::SingleStream(sniff.format))
        }
        DetectedFormat::Ar
        | DetectedFormat::Cab
        | DetectedFormat::Iso
        | DetectedFormat::Wim
        | DetectedFormat::Xar
        | DetectedFormat::Lzh => Some(InPlaceCategory::GenericContainer(sniff.format)),
        _ => None,
    }
}

fn classify_compressed_stream(format: DetectedFormat, name: &str) -> InPlaceCategory {
    const COMPOUND_STREAM_SUFFIXES: &[(DetectedFormat, &[&str], CompoundFormat)] = &[
        (DetectedFormat::Gzip, &[".tar.gz", ".tgz"], CompoundFormat::TarGz),
        (DetectedFormat::Bzip2, &[".tar.bz2", ".tbz2"], CompoundFormat::TarBz2),
        (DetectedFormat::Xz, &[".tar.xz", ".txz"], CompoundFormat::TarXz),
        (DetectedFormat::Zstd, &[".tar.zst", ".tzst"], CompoundFormat::TarZstd),
    ];
    for &(fmt, suffixes, compound) in COMPOUND_STREAM_SUFFIXES {
        if fmt == format && suffixes.iter().any(|s| name.ends_with(s)) {
            return InPlaceCategory::CompoundTar(compound);
        }
    }
    InPlaceCategory::SingleStream(format)
}

fn category_from_extension(name: &str) -> InPlaceCategory {
    EXTENSION_CATEGORY_TABLE
        .iter()
        .find_map(|&(ext, cat)| name.ends_with(ext).then_some(cat))
        .unwrap_or(InPlaceCategory::Zip)
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
