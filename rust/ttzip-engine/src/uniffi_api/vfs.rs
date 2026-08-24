// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI In-Memory VFS Tree Object Scaffolding.

use std::sync::Arc;
use super::types::{UniFFIEntryMetadata, UniFFIVfsMatch, UniFFIVfsNodeSummary, UniFFIVfsStats};

/// Thread-safe in-memory VFS Tree object exposed to Swift and multi-language SDKs.
#[derive(uniffi::Object)]
pub struct UniFFIVfsTree {
    tree: parking_lot::RwLock<crate::fs::vfs::tree::VfsTree>,
}

#[uniffi::export]
impl UniFFIVfsTree {
    #[uniffi::constructor]
    pub fn build(entries: Vec<UniFFIEntryMetadata>, root_name: String) -> Arc<Self> {
        let vfs_entries: Vec<crate::fs::vfs::node::VfsEntry> = entries
            .into_iter()
            .map(|e| crate::fs::vfs::node::VfsEntry {
                path: e.path,
                uncompressed_size: e.uncompressed_size,
                compressed_size: e.compressed_size,
                crc32: e.crc32,
                mtime_epoch_secs: e.mtime_epoch_secs,
                mode: e.mode,
                is_directory: e.is_directory,
                is_encrypted: e.is_encrypted,
            })
            .collect();

        let tree = crate::fs::vfs::tree::VfsTree::build_from_entries(&vfs_entries, &root_name);
        Arc::new(Self {
            tree: parking_lot::RwLock::new(tree),
        })
    }

    pub fn search(&self, query: String, max_results: u32) -> Vec<UniFFIVfsMatch> {
        let guard = self.tree.read();
        let matches = guard.fuzzy_search(&query);
        matches
            .into_iter()
            .take(max_results as usize)
            .map(|m| UniFFIVfsMatch {
                path: m.path,
                name: m.name,
                is_directory: m.is_directory,
                size: m.uncompressed_size,
            })
            .collect()
    }

    pub fn get_children(&self, subpath: Option<String>, offset: u32, limit: u32) -> Vec<UniFFIVfsNodeSummary> {
        let guard = self.tree.read();
        let target_node = if let Some(ref sp) = subpath {
            let clean = sp.trim_matches('/');
            if clean.is_empty() {
                &guard.root
            } else {
                let segments: Vec<&str> = clean.split('/').collect();
                let mut curr = &guard.root;
                for seg in segments {
                    if let Some(child) = curr.children.iter().find(|c| c.name == seg && c.is_directory) {
                        curr = child;
                    } else {
                        return Vec::new();
                    }
                }
                curr
            }
        } else {
            &guard.root
        };

        target_node
            .children
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|c| UniFFIVfsNodeSummary {
                name: c.name.clone(),
                path: c.path.clone(),
                uncompressed_size: c.uncompressed_size,
                compressed_size: c.compressed_size,
                crc32: c.crc32,
                mtime_epoch_secs: c.mtime_epoch_secs,
                mode: c.mode,
                is_directory: c.is_directory,
                is_encrypted: c.is_encrypted,
                has_children: !c.children.is_empty(),
            })
            .collect()
    }

    pub fn render_tree(&self) -> String {
        let guard = self.tree.read();
        guard.render_tree()
    }

    pub fn get_stats(&self) -> UniFFIVfsStats {
        let guard = self.tree.read();
        UniFFIVfsStats {
            total_files: guard.root.total_files() as u64,
            total_dirs: guard.root.total_directories() as u64,
            total_uncompressed_bytes: guard.root.uncompressed_size,
        }
    }

    pub fn total_entries(&self) -> u64 {
        let guard = self.tree.read();
        guard.total_entries as u64
    }
}
