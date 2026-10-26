// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Non-blocking algebraic session status and pre-flight volume fault tolerance.
//!
//! Exposes `VfsSessionStatus`, `EncryptedHeaderChallenge`, and `VfsMissingVolumeDescriptor`
//! via UniFFI, completely eliminating thread-blocking password callbacks and unhandled
//! split archive file gaps.

use std::path::Path;
use std::sync::Arc;

use super::vfs::{build_arena_from_archive_path, VfsSession, VfsSessionError};
pub use ttzip_archive::archive::split::{
    MissingVolumeDescriptor, MissingVolumeSeverity, PreFlightVolumeProber, VolumePartInfo,
};

/// UniFFI representation of missing volume severity.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsMissingVolumeSeverity {
    CatalogBlocked,
    DataDegraded,
}

impl From<MissingVolumeSeverity> for VfsMissingVolumeSeverity {
    fn from(s: MissingVolumeSeverity) -> Self {
        match s {
            MissingVolumeSeverity::CatalogBlocked => Self::CatalogBlocked,
            MissingVolumeSeverity::DataDegraded => Self::DataDegraded,
        }
    }
}

/// UniFFI representation of volume segment information.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct VfsVolumePartInfo {
    pub index: u32,
    pub path: String,
    pub file_size: u64,
}

impl From<VolumePartInfo> for VfsVolumePartInfo {
    fn from(p: VolumePartInfo) -> Self {
        Self {
            index: p.index,
            path: p.path,
            file_size: p.file_size,
        }
    }
}

/// UniFFI representation of missing volume descriptor.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct VfsMissingVolumeDescriptor {
    pub format: String,
    pub base_stem: String,
    pub seed_path: String,
    pub expected_total_volumes: Option<u32>,
    pub present_volumes: Vec<VfsVolumePartInfo>,
    pub missing_indices: Vec<u32>,
    pub missing_filenames: Vec<String>,
    pub severity: VfsMissingVolumeSeverity,
    pub recovery_hint: String,
}

impl From<MissingVolumeDescriptor> for VfsMissingVolumeDescriptor {
    fn from(d: MissingVolumeDescriptor) -> Self {
        Self {
            format: d.format,
            base_stem: d.base_stem,
            seed_path: d.seed_path,
            expected_total_volumes: d.expected_total_volumes,
            present_volumes: d.present_volumes.into_iter().map(Into::into).collect(),
            missing_indices: d.missing_indices,
            missing_filenames: d.missing_filenames,
            severity: d.severity.into(),
            recovery_hint: d.recovery_hint,
        }
    }
}

/// Challenge handle returned when an archive requires password before its header can be parsed.
#[derive(Debug, uniffi::Object)]
pub struct EncryptedHeaderChallenge {
    path: String,
}

impl EncryptedHeaderChallenge {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

#[uniffi::export]
impl EncryptedHeaderChallenge {
    pub fn get_path(&self) -> String {
        self.path.clone()
    }

    pub fn unlock(&self, password: String) -> Result<Arc<VfsSession>, VfsSessionError> {
        let p = Path::new(&self.path);
        let arena = build_arena_from_archive_path(p, Some(&password))?;
        Ok(Arc::new(VfsSession::from_arena(arena)))
    }
}

/// Non-blocking algebraic session status returned by `open_archive_session`.
#[derive(Debug, uniffi::Enum)]
pub enum VfsSessionStatus {
    Ready {
        session: Arc<VfsSession>,
        has_encrypted_entries: bool,
    },
    PasswordRequired {
        challenge: Arc<EncryptedHeaderChallenge>,
    },
    MissingVolume {
        descriptor: VfsMissingVolumeDescriptor,
    },
}

/// Ingests and opens an archive into a non-blocking algebraic VfsSessionStatus.
#[uniffi::export]
pub fn open_archive_session(
    archive_path: String,
    password_hint: Option<String>,
) -> Result<VfsSessionStatus, VfsSessionError> {
    let p = Path::new(&archive_path);
    if !p.exists() {
        return Err(VfsSessionError::InspectionFailed {
            reason: format!("Archive file not found: {}", archive_path),
        });
    }

    // Step 1: Pre-flight volume topology check for missing segments
    if let Some(desc) = PreFlightVolumeProber::probe(p) {
        return Ok(VfsSessionStatus::MissingVolume {
            descriptor: desc.into(),
        });
    }

    // Step 2: Ingest archive metadata into in-memory CSR arena
    match build_arena_from_archive_path(p, password_hint.as_deref()) {
        Ok(arena) => {
            let has_encrypted_entries = arena
                .node_attributes
                .iter()
                .any(|&a| (a & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0);
            Ok(VfsSessionStatus::Ready {
                session: Arc::new(VfsSession::from_arena(arena)),
                has_encrypted_entries,
            })
        }
        Err(VfsSessionError::BadPassword) => Ok(VfsSessionStatus::PasswordRequired {
            challenge: Arc::new(EncryptedHeaderChallenge::new(archive_path)),
        }),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_open_archive_session_missing_volume_intercept() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("split_test.7z.001");
        let p3 = dir.path().join("split_test.7z.003");
        File::create(&p1).unwrap().write_all(b"7z_part1").unwrap();
        File::create(&p3).unwrap().write_all(b"7z_part3").unwrap();

        let status = open_archive_session(p1.to_string_lossy().to_string(), None).unwrap();
        match status {
            VfsSessionStatus::MissingVolume { descriptor } => {
                assert_eq!(descriptor.format, "NumberedExtension");
                assert_eq!(descriptor.missing_indices, vec![2]);
                assert_eq!(descriptor.missing_filenames, vec!["split_test.7z.002"]);
            }
            _ => panic!("Expected MissingVolume status"),
        }
    }

    #[test]
    fn test_open_archive_session_not_found() {
        let err = open_archive_session("/nonexistent/file.zip".to_string(), None).unwrap_err();
        match err {
            VfsSessionError::InspectionFailed { reason } => {
                assert!(reason.contains("not found"));
            }
            _ => panic!("Expected InspectionFailed error"),
        }
    }
}
