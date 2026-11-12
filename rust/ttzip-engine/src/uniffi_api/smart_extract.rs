// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
    let mut effective_roots = BTreeSet::new();
    for raw_path in &entry_paths {
        record_effective_root(&mut effective_roots, raw_path);
    }
    finalize_smart_extract_decision(
        effective_roots,
        destination_parent,
        archive_stem,
        &collision_policy,
    )
}

/// Resolves the optimal extraction destination directory directly from an in-memory `VfsCompactArena`
/// by borrowing interned node paths without per-entry `String` allocations.
pub fn analyze_vfs_arena_structure(
    arena: &crate::fs::vfs::VfsCompactArena,
    destination_parent: String,
    archive_stem: String,
    collision_policy: String,
) -> UniFFISmartExtractDecision {
    let mut effective_roots = BTreeSet::new();
    for idx in 1..arena.total_nodes {
        let path = arena.node_full_path_str(idx as u32);
        record_effective_root(&mut effective_roots, path);
    }
    finalize_smart_extract_decision(
        effective_roots,
        destination_parent,
        archive_stem,
        &collision_policy,
    )
}

fn record_effective_root(effective_roots: &mut BTreeSet<String>, raw_path: &str) {
    if let Some(root_segment) = extract_effective_root_segment(raw_path)
        && !effective_roots.contains(root_segment)
    {
        effective_roots.insert(root_segment.to_string());
    }
}

fn finalize_smart_extract_decision(
    effective_roots: BTreeSet<String>,
    destination_parent: String,
    archive_stem: String,
    collision_policy: &str,
) -> UniFFISmartExtractDecision {
    let parent_path = Path::new(&destination_parent);
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
fn trim_relative_prefixes(mut path: &str) -> &str {
    loop {
        let stripped = path.trim_start_matches(['/', '\\']);
        if let Some(rest) = stripped.strip_prefix("./").or_else(|| stripped.strip_prefix(".\\")) {
            path = rest;
        } else {
            return stripped.trim_end_matches(['/', '\\']);
        }
    }
}

#[inline]
fn extract_effective_root_segment(raw_path: &str) -> Option<&str> {
    let trimmed = trim_relative_prefixes(raw_path);
    if trimmed.is_empty() || is_system_or_mac_junk_slice(trimmed) {
        return None;
    }
    let root_segment = trimmed.split(['/', '\\']).next().unwrap_or("");
    (!root_segment.is_empty() && root_segment != ".").then_some(root_segment)
}

#[cfg(test)]
#[inline]
fn normalize_entry_path(path: &str) -> String {
    let trimmed = trim_relative_prefixes(path);
    if trimmed.contains('\\') {
        trimmed.replace('\\', "/")
    } else {
        trimmed.to_string()
    }
}

#[inline]
fn is_macosx_metadata_slice(path: &str) -> bool {
    path.split(['/', '\\']).any(|seg| seg == "__MACOSX")
}

#[inline]
fn is_os_junk_filename(file_name: &str) -> bool {
    const EXACT_MAC_NAMES: &[&str] = &[".DS_Store", ".localized", ".VolumeIcon.icns"];
    const PREFIX_NAMES: &[&str] = &[
        "._",
        ".Spotlight-V100",
        ".Trashes",
        ".fseventsd",
        ".TemporaryItems",
        "PaxHeader",
    ];
    const WINDOWS_JUNK_NAMES: &[&str] = &["Thumbs.db", "desktop.ini", "ehthumbs.db", "$RECYCLE.BIN"];

    EXACT_MAC_NAMES.contains(&file_name)
        || PREFIX_NAMES.iter().any(|p| file_name.starts_with(p))
        || WINDOWS_JUNK_NAMES
            .iter()
            .any(|w| file_name.eq_ignore_ascii_case(w))
}

#[inline]
fn is_system_or_mac_junk_slice(path: &str) -> bool {
    if is_macosx_metadata_slice(path) {
        return true;
    }
    let file_name = path
        .rfind(['/', '\\'])
        .map_or(path, |idx| &path[idx + 1..]);
    is_os_junk_filename(file_name)
}

#[cfg(test)]
#[inline]
fn is_system_or_mac_junk(normalized: &str) -> bool {
    is_system_or_mac_junk_slice(normalized)
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
    use crate::fs::vfs::{VfsCompactArena, VfsEntry};
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
    fn test_path_normalization_variations_and_arena_analysis() {
        assert_eq!(normalize_entry_path(".\\a\\b\\c"), "a/b/c");
        assert_eq!(normalize_entry_path("././a/b/"), "a/b");
        assert_eq!(normalize_entry_path("///folder/file///"), "folder/file");
        assert!(is_system_or_mac_junk("__MACOSX/._test.png"));
        assert!(is_system_or_mac_junk("folder/sub/Thumbs.db"));
        assert!(is_system_or_mac_junk(".DS_Store"));
        assert!(!is_system_or_mac_junk("folder/DS_Store.txt"));

        let e1 = VfsEntry {
            path: "BundleRoot/src/lib.rs".to_string(),
            ..Default::default()
        };
        let e2 = VfsEntry {
            path: "__MACOSX/BundleRoot/._lib.rs".to_string(),
            ..Default::default()
        };
        let arena = VfsCompactArena::build_from_entries(&[e1, e2], "Bundle.zip");
        let decision = analyze_vfs_arena_structure(
            &arena,
            "/tmp/out".to_string(),
            "Bundle".to_string(),
            "autoRenameNumbered".to_string(),
        );
        assert_eq!(decision.mode, "directExtract");
        assert_eq!(decision.effective_root_count, 1);
        assert_eq!(decision.single_root_name.as_deref(), Some("BundleRoot"));
    }
}
