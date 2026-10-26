// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! VFS View presentation models for terminal table rendering and visible tree traversal.

use super::arena::{VfsCompactArena, VFS_FLAG_IS_DIR, VFS_FLAG_IS_ENCRYPTED};
use crate::types::TTZipVfsNodeSummary;
use std::ffi::c_char;
use std::path::Path;

/// Information for a single visible node in the UI hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsVisibleNode {
    pub node_id: u32,
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub is_encrypted: bool,
    pub is_expanded: bool,
    pub is_selected: bool,
    pub match_indices: Vec<usize>,
    pub entry_idx: Option<usize>,
}

impl VfsVisibleNode {
    /// Determines appropriate icon for terminal rendering.
    pub fn icon(&self) -> &'static str {
        if self.is_dir {
            if self.is_expanded {
                "📂 "
            } else {
                "📁 "
            }
        } else if self.is_encrypted {
            "🔒 "
        } else {
            let path = Path::new(&self.name);
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            match ext.to_lowercase().as_str() {
                "rs" => "🦀 ",
                "swift" => "🕊️ ",
                "c" | "h" | "cpp" | "hpp" => "🇨 ",
                "json" | "toml" | "yaml" | "yml" | "xml" | "plist" => "⚙️ ",
                "md" | "txt" | "log" => "📝 ",
                "zip" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" => "📦 ",
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "icns" => "🖼️ ",
                "sh" | "bash" | "zsh" => "🐚 ",
                _ => "📄 ",
            }
        }
    }
}

/// An entry in a flattened visible view of the tree (for terminal rendering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsVisibleItem {
    pub node: VfsVisibleNode,
    pub depth: usize,
    pub is_last_child: bool,
    pub indent_prefix: String,
}

impl VfsCompactArena {
    /// Flattens the currently visible (expanded) nodes into a linear list.
    /// Traversal is non-recursive using an explicit heap stack.
    pub fn flatten_visible(&self) -> Vec<VfsVisibleItem> {
        if self.total_nodes <= 1 {
            return Vec::new();
        }

        let mut items = Vec::new();

        struct Frame {
            node_id: u32,
            child_idx: usize,
            depth: usize,
            prefix: String,
        }

        let mut stack = vec![Frame {
            node_id: 0,
            child_idx: 0,
            depth: 0,
            prefix: String::new(),
        }];

        while let Some(top_idx) = stack.len().checked_sub(1) {
            let (cid, depth, prefix, is_last) = {
                let top = &mut stack[top_idx];
                let children = self.get_children(top.node_id);
                if top.child_idx < children.len() {
                    let child_id = children[top.child_idx];
                    let is_last = top.child_idx + 1 == children.len();
                    top.child_idx += 1;
                    (child_id, top.depth, top.prefix.clone(), is_last)
                } else {
                    stack.pop();
                    continue;
                }
            };

            let nid = cid as usize;
            if nid >= self.total_nodes {
                continue;
            }

            let is_dir = (self.node_attributes[nid] & VFS_FLAG_IS_DIR) != 0;
            let is_enc = (self.node_attributes[nid] & VFS_FLAG_IS_ENCRYPTED) != 0;
            let is_expanded = self.expanded.get(cid as usize).copied().unwrap_or(false);
            let is_selected = self.selected.get(cid as usize).copied().unwrap_or(false);
            let entry_idx = self.entry_index(cid);

            let branch = if is_last { "└── " } else { "├── " };
            let indent_prefix = if depth == 0 {
                String::new()
            } else {
                format!("{}{}", prefix, branch)
            };

            let visible_node = VfsVisibleNode {
                node_id: cid,
                name: self.node_name(cid).to_string(),
                relative_path: self.node_full_path(cid),
                is_dir,
                uncompressed_size: self.uncompressed_sizes[nid],
                compressed_size: self.compressed_sizes[nid],
                crc32: self.crc32s[nid],
                is_encrypted: is_enc,
                is_expanded,
                is_selected,
                match_indices: Vec::new(),
                entry_idx,
            };

            items.push(VfsVisibleItem {
                node: visible_node,
                depth,
                is_last_child: is_last,
                indent_prefix,
            });

            if is_dir && is_expanded {
                let next_prefix = if depth == 0 {
                    ""
                } else if is_last {
                    "    "
                } else {
                    "│   "
                };
                let combined_prefix = format!("{}{}", prefix, next_prefix);
                stack.push(Frame {
                    node_id: cid,
                    child_idx: 0,
                    depth: depth + 1,
                    prefix: combined_prefix,
                });
            }
        }

        items
    }

    /// Exports flattened list of all nodes conforming to `TUIVfsTreeContract`.
    pub fn to_contract_nodes_flat(&self) -> Vec<serde_json::Value> {
        let mut out = Vec::with_capacity(self.total_nodes.saturating_sub(1));
        for i in 1..self.total_nodes {
            let is_dir = (self.node_attributes[i] & VFS_FLAG_IS_DIR) != 0;
            let is_enc = (self.node_attributes[i] & VFS_FLAG_IS_ENCRYPTED) != 0;
            let obj = serde_json::json!({
                "name": self.node_name(i as u32),
                "relativePath": self.node_full_path(i as u32),
                "isDirectory": is_dir,
                "uncompressedSize": self.uncompressed_sizes[i],
                "compressedSize": self.compressed_sizes[i],
                "crc32": self.crc32s[i],
                "isEncrypted": is_enc,
            });
            out.push(obj);
        }
        out
    }

    /// Serializes entire tree state into JSON Value matching `tui_vfs_tree_contract.json`.
    pub fn to_contract_json(&self) -> serde_json::Value {
        serde_json::json!({
            "rootPath": self.root_path,
            "totalEntriesCount": self.total_entries_count,
            "totalUncompressedBytes": if self.total_nodes > 0 { self.uncompressed_sizes[0] } else { 0 },
            "nodes": self.to_contract_nodes_flat()
        })
    }

    /// Checks if a node is currently expanded.
    pub fn is_expanded(&self, node_id: u32) -> bool {
        self.expanded.get(node_id as usize).copied().unwrap_or(false)
    }

    /// Sets the expanded state of a specific node.
    pub fn set_expanded(&mut self, node_id: u32, expanded: bool) {
        if (node_id as usize) < self.total_nodes {
            self.expanded[node_id as usize] = expanded;
        }
    }

    /// Sets the expanded state of all nodes.
    pub fn set_all_expanded(&mut self, expanded: bool) {
        self.expanded.fill(expanded);
    }

    /// Toggles the expanded state of a directory node identified by relative path.
    pub fn toggle_expanded(&mut self, relative_path: &str) -> Option<bool> {
        let node_id = self.resolve_path(relative_path)?;
        let nid = node_id as usize;
        if (self.node_attributes[nid] & VFS_FLAG_IS_DIR) != 0 {
            self.expanded[nid] = !self.expanded[nid];
            Some(self.expanded[nid])
        } else {
            None
        }
    }

    /// Checks if a node is currently selected.
    pub fn is_selected(&self, node_id: u32) -> bool {
        self.selected.get(node_id as usize).copied().unwrap_or(false)
    }

    /// Recursively sets the selected state of a node and all its descendants.
    pub fn set_selected_recursive(&mut self, node_id: u32, selected: bool) {
        if (node_id as usize) >= self.total_nodes {
            return;
        }
        let mut stack = vec![node_id];
        while let Some(curr) = stack.pop() {
            let cid = curr as usize;
            if cid < self.total_nodes {
                self.selected[cid] = selected;
                for &child in self.get_children(curr) {
                    stack.push(child);
                }
            }
        }
    }

    /// Toggles selection of a node and all its descendants.
    pub fn toggle_selected(&mut self, relative_path: &str) -> Option<bool> {
        let node_id = self.resolve_path(relative_path)?;
        let new_state = !self.is_selected(node_id);
        self.set_selected_recursive(node_id, new_state);
        Some(new_state)
    }

    /// Sets selection state of all nodes.
    pub fn select_all(&mut self, selected: bool) {
        self.selected.fill(selected);
    }

    /// Collects relative paths of all currently selected leaf file nodes.
    pub fn get_selected_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for i in 1..self.total_nodes {
            if self.selected[i] && (self.node_attributes[i] & VFS_FLAG_IS_DIR) == 0 {
                paths.push(self.node_full_path(i as u32));
            }
        }
        paths
    }

    /// Collects archive entry indices of all currently selected leaf file nodes.
    pub fn get_selected_entry_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        for i in 1..self.total_nodes {
            if self.selected[i] && (self.node_attributes[i] & VFS_FLAG_IS_DIR) == 0 {
                if let Some(idx) = self.entry_index(i as u32) {
                    indices.push(idx);
                }
            }
        }
        indices
    }

    /// Retrieves windowed children slice for a directory node ID with true O(1) paging.
    pub fn get_children_slice(
        &self,
        dir_node_id: u32,
        offset: usize,
        limit: usize,
    ) -> (Vec<TTZipVfsNodeSummary>, usize) {
        if (dir_node_id as usize) >= self.total_nodes {
            return (Vec::new(), 0);
        }

        let total_in_dir = self.child_slice_ranges[dir_node_id as usize].1 as usize;
        let paged_ids = self.get_children_paged(dir_node_id, offset, limit);
        let mut summaries = Vec::with_capacity(paged_ids.len());

        for &child_id in paged_ids {
            let cid = child_id as usize;
            let (name_off, name_len) = self.name_offset_and_len[cid];
            let is_dir = (self.node_attributes[cid] & VFS_FLAG_IS_DIR) != 0;
            let is_enc = (self.node_attributes[cid] & VFS_FLAG_IS_ENCRYPTED) != 0;
            let has_children = self.child_slice_ranges[cid].1 > 0;
            let name_ptr = self.names_pool[name_off as usize..].as_ptr() as *const c_char;

            summaries.push(TTZipVfsNodeSummary {
                struct_size: std::mem::size_of::<TTZipVfsNodeSummary>() as u32,
                abi_version: crate::types::TTZIP_ABI_VERSION_3,
                node_id: child_id,
                name_utf8: name_ptr,
                name_len: name_len as u32,
                uncompressed_size: self.uncompressed_sizes[cid],
                compressed_size: self.compressed_sizes[cid],
                crc32: self.crc32s[cid],
                mtime_epoch_secs: self.modified_timestamps[cid],
                mode: self.modes[cid],
                is_directory: is_dir,
                is_encrypted: is_enc,
                has_children,
            });
        }

        (summaries, total_in_dir)
    }

    /// Renders ASCII/Unicode tree layout.
    pub fn render_tree(&self) -> String {
        if self.total_nodes == 0 {
            return String::new();
        }
        let mut out = String::new();
        struct Frame {
            node_id: u32,
            child_idx: usize,
            prefix: String,
            is_last: bool,
        }

        let mut stack = vec![Frame {
            node_id: 0,
            child_idx: 0,
            prefix: String::new(),
            is_last: true,
        }];

        while let Some(top) = stack.last_mut() {
            let nid = top.node_id as usize;
            if top.child_idx == 0 {
                let name = self.node_name(top.node_id);
                let display_name = if name.is_empty() { "." } else { name };
                let is_dir = (self.node_attributes[nid] & VFS_FLAG_IS_DIR) != 0;
                let size_str = if is_dir {
                    "<DIR>".to_string()
                } else {
                    format_byte_size(self.uncompressed_sizes[nid])
                };

                if top.node_id == 0 {
                    out.push_str(&format!("{} ({})\n", display_name, size_str));
                } else {
                    let connector = if top.is_last { "└── " } else { "├── " };
                    out.push_str(&format!(
                        "{}{}{} ({})\n",
                        top.prefix, connector, display_name, size_str
                    ));
                }
            }

            let children = self.get_children(top.node_id);
            if top.child_idx < children.len() {
                let child_id = children[top.child_idx];
                let is_last_child = top.child_idx + 1 == children.len();
                let child_prefix = if top.node_id == 0 {
                    String::new()
                } else if top.is_last {
                    format!("{}    ", top.prefix)
                } else {
                    format!("{}│   ", top.prefix)
                };

                top.child_idx += 1;
                stack.push(Frame {
                    node_id: child_id,
                    child_idx: 0,
                    prefix: child_prefix,
                    is_last: is_last_child,
                });
            } else {
                stack.pop();
            }
        }

        out
    }
}

/// Formats byte count into human-readable representation.
#[inline]
pub fn format_byte_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}


