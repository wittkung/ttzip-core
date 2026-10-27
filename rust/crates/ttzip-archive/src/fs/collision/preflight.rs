// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Preflight and post-stage collision detection.

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use crate::fs::collision::fs_ops::FsOps;
use crate::fs::collision::probe::{probe_destination, ProbeResult};
use crate::fs::collision::table::{ConflictTable, NewConflict};
use crate::fs::collision::{CollisionError, ConflictKind, ItemKind, ItemMeta};

pub fn preflight_detect<F: FsOps>(
    fs: &F,
    dest_dirfd: RawFd,
    dest_root: &Path,
    sorted_items: &[(PathBuf, ItemMeta)],
) -> Result<ConflictTable, CollisionError> {
    if !dest_root.exists() {
        return ConflictTable::builder().build();
    }

    let mut builder = ConflictTable::builder();
    let mut absent_dirs: HashSet<PathBuf> = HashSet::new();
    let mut present_dirs: HashSet<PathBuf> = HashSet::new();

    for (rel_path, incoming_meta) in sorted_items {
        let rel_str = rel_path.to_str().unwrap_or_default();
        if rel_str.is_empty() {
            continue;
        }

        // Hierarchical prefix pruning: evaluate ancestors from top-level down
        let mut ancestors = Vec::new();
        let mut p = rel_path.parent();
        while let Some(parent) = p {
            if !parent.as_os_str().is_empty() {
                ancestors.push(parent);
            }
            p = parent.parent();
        }
        ancestors.reverse();

        let mut is_pruned = false;
        for anc in ancestors {
            if absent_dirs.contains(anc) {
                is_pruned = true;
                break;
            }
            if !present_dirs.contains(anc) {
                match probe_destination(fs, dest_dirfd, anc) {
                    Ok(ProbeResult::Absent) => {
                        absent_dirs.insert(anc.to_path_buf());
                        is_pruned = true;
                        break;
                    }
                    Ok(ProbeResult::Item(..)) => {
                        present_dirs.insert(anc.to_path_buf());
                    }
                    Err(_) => {}
                }
            }
        }

        if is_pruned {
            continue;
        }

        match probe_destination(fs, dest_dirfd, rel_path) {
            Ok(ProbeResult::Absent) => {
                if incoming_meta.kind == ItemKind::Directory {
                    absent_dirs.insert(rel_path.clone());
                }
            }
            Ok(ProbeResult::Item(existing_meta, existing_name)) => {
                if incoming_meta.kind == ItemKind::Directory && existing_meta.kind == ItemKind::Directory {
                    present_dirs.insert(rel_path.clone());
                } else if ConflictKind::classify(existing_meta.kind, incoming_meta.kind).is_some() {
                    builder.push(NewConflict {
                        relative_path: rel_str,
                        existing_name: &existing_name,
                        existing: existing_meta,
                        incoming: *incoming_meta,
                    })?;
                }
            }
            Err(_) => {}
        }
    }

    builder.build()
}

pub fn post_stage_detect<F: FsOps>(
    fs: &F,
    dest_dirfd: RawFd,
    staged_root: &Path,
) -> Result<ConflictTable, CollisionError> {
    let mut builder = ConflictTable::builder();
    let mut dirs_to_visit = vec![staged_root.to_path_buf()];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let meta = match std::fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let file_type = meta.file_type();
                let kind = if file_type.is_dir() {
                    dirs_to_visit.push(path.clone());
                    ItemKind::Directory
                } else if file_type.is_symlink() {
                    ItemKind::Symlink
                } else {
                    ItemKind::File
                };

                let size = if kind == ItemKind::File { meta.len() } else { 0 };
                let mtime_unix_ns = meta.mtime() * 1_000_000_000 + meta.mtime_nsec();

                let incoming_meta = ItemMeta {
                    kind,
                    size,
                    mtime_unix_ns,
                };

                let rel_path = match path.strip_prefix(staged_root) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let rel_str = rel_path.to_str().unwrap_or_default();

                if let Ok(ProbeResult::Item(existing_meta, existing_name)) = probe_destination(fs, dest_dirfd, rel_path) {
                    if ConflictKind::classify(existing_meta.kind, incoming_meta.kind).is_some() {
                        builder.push(NewConflict {
                            relative_path: rel_str,
                            existing_name: &existing_name,
                            existing: existing_meta,
                            incoming: incoming_meta,
                        })?;
                    }
                }
            }
        }
    }

    builder.build()
}
