// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Smart Archive Extraction Path Resolution and Directory Derivation.
//!
//! Analyzes archive entry hierarchy, eliminates redundant folder-in-folder wrapping,
//! filters OS metadata/AppleDouble junk, and determines collision-safe destination paths.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Smart extraction decision record exposed via UniFFI.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFISmartExtractDecision {
    /// Decision mode: "directExtract", "wrapInFolder", or "emptyArchive".
    pub mode: String,
    /// Number of distinct top-level root entities (excluding system metadata).
    pub effective_root_count: u32,
    /// Name of the single root file or folder when `effective_root_count == 1`.
    pub single_root_name: Option<String>,
    /// Resolved absolute or relative destination folder path for extraction.
    pub destination_folder: String,
}

/// Resolves the optimal extraction destination directory and wrapping strategy in pure Rust.
///
/// - `entry_paths`: Array of relative archive paths.
/// - `destination_parent`: Directory where the archive extraction is being triggered.
/// - `archive_stem`: Base name of the archive file (without extension).
/// - `collision_policy`: Policy when destination directory already exists ("autoRenameNumbered", etc.).
#[uniffi::export]
pub fn resolve_smart_extract_decision(
    entry_paths: Vec<String>,
    destination_parent: String,
    archive_stem: String,
    collision_policy: String,
) -> UniFFISmartExtractDecision {
    let parent_path = Path::new(&destination_parent);
    let mut effective_roots = BTreeSet::new();

    for raw_path in entry_paths {
        let normalized = normalize_entry_path(&raw_path);
        if normalized.is_empty() {
            continue;
        }

        if is_system_or_mac_junk(&normalized) {
            continue;
        }

        let root_segment = match normalized.find('/') {
            Some(idx) => &normalized[..idx],
            None => &normalized,
        };

        if !root_segment.is_empty() {
            effective_roots.insert(root_segment.to_string());
        }
    }

    let root_count = effective_roots.len() as u32;

    if root_count == 0 {
        UniFFISmartExtractDecision {
            mode: "emptyArchive".to_string(),
            effective_root_count: 0,
            single_root_name: None,
            destination_folder: destination_parent,
        }
    } else if root_count == 1 {
        let single_name = effective_roots.into_iter().next();
        UniFFISmartExtractDecision {
            mode: "directExtract".to_string(),
            effective_root_count: 1,
            single_root_name: single_name,
            destination_folder: destination_parent,
        }
    } else {
        let folder_name = if archive_stem.trim().is_empty() {
            "Archive_Extracted".to_string()
        } else {
            archive_stem
        };
        let target_path = parent_path.join(&folder_name);
        let final_path = if collision_policy == "autoRenameNumbered" {
            resolve_numbered_collision(&target_path)
        } else {
            target_path
        };

        UniFFISmartExtractDecision {
            mode: "wrapInFolder".to_string(),
            effective_root_count: root_count,
            single_root_name: None,
            destination_folder: final_path.to_string_lossy().into_owned(),
        }
    }
}

// MARK: - Internal Path Normalization and Metadata Filtering

#[inline]
fn normalize_entry_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while normalized.starts_with("./") {
        normalized.drain(..2);
    }
    let trimmed = normalized.trim_matches('/');
    trimmed.to_string()
}

#[inline]
fn is_system_or_mac_junk(normalized: &str) -> bool {
    if normalized == "__MACOSX"
        || normalized.starts_with("__MACOSX/")
        || normalized.ends_with("/__MACOSX")
        || normalized.contains("/__MACOSX/")
    {
        return true;
    }

    let file_name = match normalized.rfind('/') {
        Some(idx) => &normalized[idx + 1..],
        None => normalized,
    };

    if file_name.starts_with("._")
        || file_name == ".DS_Store"
        || file_name == ".localized"
        || file_name == ".VolumeIcon.icns"
    {
        return true;
    }

    if file_name.starts_with(".Spotlight-V100")
        || file_name.starts_with(".Trashes")
        || file_name.starts_with(".fseventsd")
        || file_name.starts_with(".TemporaryItems")
        || file_name.starts_with("PaxHeader")
    {
        return true;
    }

    if file_name.eq_ignore_ascii_case("Thumbs.db")
        || file_name.eq_ignore_ascii_case("desktop.ini")
        || file_name.eq_ignore_ascii_case("ehthumbs.db")
        || file_name.eq_ignore_ascii_case("$RECYCLE.BIN")
    {
        return true;
    }

    false
}

fn resolve_numbered_collision(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }

    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let base_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Archive");

    let mut counter = 2;
    while counter < 1000 {
        let candidate_name = format!("{} {}", base_name, counter);
        let candidate = parent.join(&candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }

    parent.join(format!("{}_{}", base_name, counter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_smart_extract_empty_archive() {
        let entries = vec![
            "".to_string(),
            "__MACOSX/._pic.png".to_string(),
            ".DS_Store".to_string(),
            "dir/.DS_Store".to_string(),
            "Thumbs.db".to_string(),
        ];
        let decision = resolve_smart_extract_decision(
            entries,
            "/tmp/dest".to_string(),
            "MyArchive".to_string(),
            "autoRenameNumbered".to_string(),
        );

        assert_eq!(decision.mode, "emptyArchive");
        assert_eq!(decision.effective_root_count, 0);
        assert_eq!(decision.single_root_name, None);
        assert_eq!(decision.destination_folder, "/tmp/dest");
    }

    #[test]
    fn test_smart_extract_single_root_directory() {
        let entries = vec![
            "MyFolder/file1.txt".to_string(),
            "MyFolder/file2.txt".to_string(),
            "MyFolder/sub/file3.txt".to_string(),
            "__MACOSX/MyFolder/._file1.txt".to_string(),
        ];
        let decision = resolve_smart_extract_decision(
            entries,
            "/dest/parent".to_string(),
            "ArchiveStem".to_string(),
            "autoRenameNumbered".to_string(),
        );

        assert_eq!(decision.mode, "directExtract");
        assert_eq!(decision.effective_root_count, 1);
        assert_eq!(decision.single_root_name.as_deref(), Some("MyFolder"));
        assert_eq!(decision.destination_folder, "/dest/parent");
    }

    #[test]
    fn test_smart_extract_single_root_file() {
        let entries = vec![
            "./document.pdf".to_string(),
            ".DS_Store".to_string(),
        ];
        let decision = resolve_smart_extract_decision(
            entries,
            "/dest/parent".to_string(),
            "ArchiveStem".to_string(),
            "autoRenameNumbered".to_string(),
        );

        assert_eq!(decision.mode, "directExtract");
        assert_eq!(decision.effective_root_count, 1);
        assert_eq!(decision.single_root_name.as_deref(), Some("document.pdf"));
        assert_eq!(decision.destination_folder, "/dest/parent");
    }

    #[test]
    fn test_smart_extract_multiple_loose_roots_wrapping() {
        let entries = vec![
            "file1.txt".to_string(),
            "file2.txt".to_string(),
            "subfolder/file3.txt".to_string(),
        ];
        let decision = resolve_smart_extract_decision(
            entries,
            "/dest/parent".to_string(),
            "ProjectBundle".to_string(),
            "autoRenameNumbered".to_string(),
        );

        assert_eq!(decision.mode, "wrapInFolder");
        assert_eq!(decision.effective_root_count, 3);
        assert_eq!(decision.single_root_name, None);
        assert_eq!(decision.destination_folder, "/dest/parent/ProjectBundle");
    }

    #[test]
    fn test_smart_extract_numbered_collision() {
        let temp_dir = tempdir().unwrap();
        let parent_path = temp_dir.path();

        let initial_folder = parent_path.join("TargetFolder");
        std::fs::create_dir(&initial_folder).unwrap();

        let entries = vec![
            "a.txt".to_string(),
            "b.txt".to_string(),
        ];

        let decision = resolve_smart_extract_decision(
            entries.clone(),
            parent_path.to_str().unwrap().to_string(),
            "TargetFolder".to_string(),
            "autoRenameNumbered".to_string(),
        );

        assert_eq!(decision.mode, "wrapInFolder");
        assert_eq!(decision.effective_root_count, 2);
        assert_eq!(
            decision.destination_folder,
            parent_path.join("TargetFolder 2").to_str().unwrap()
        );

        // Create "TargetFolder 2" and test increment to 3
        std::fs::create_dir(parent_path.join("TargetFolder 2")).unwrap();
        let decision3 = resolve_smart_extract_decision(
            entries,
            parent_path.to_str().unwrap().to_string(),
            "TargetFolder".to_string(),
            "autoRenameNumbered".to_string(),
        );
        assert_eq!(
            decision3.destination_folder,
            parent_path.join("TargetFolder 3").to_str().unwrap()
        );
    }

    #[test]
    fn test_path_normalization_variations() {
        assert_eq!(normalize_entry_path(".\\a\\b\\c"), "a/b/c");
        assert_eq!(normalize_entry_path("././a/b/"), "a/b");
        assert_eq!(normalize_entry_path("///folder/file///"), "folder/file");
        assert!(is_system_or_mac_junk("__MACOSX/._test.png"));
        assert!(is_system_or_mac_junk("folder/sub/Thumbs.db"));
        assert!(is_system_or_mac_junk(".DS_Store"));
        assert!(!is_system_or_mac_junk("folder/DS_Store.txt"));
    }
}
