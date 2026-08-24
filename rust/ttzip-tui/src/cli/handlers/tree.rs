// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Subcommand execution handler: tree.

use crate::cli::args::{VfsNodeContractDto, VfsTreeContractDto};
use crate::cli::format::{format_bytes, parse_archive_entries, read_archive_data_auto};
use crate::cli::handlers::extract::pattern_matches;
use std::path::Path;

#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    is_directory: bool,
    size: u64,
    children: Vec<TreeNode>,
}

impl TreeNode {
    fn new_dir(name: String) -> Self {
        Self {
            name,
            is_directory: true,
            size: 0,
            children: Vec::new(),
        }
    }

    fn new_file(name: String, size: u64) -> Self {
        Self {
            name,
            is_directory: false,
            size,
            children: Vec::new(),
        }
    }

    fn insert(&mut self, parts: &[&str], is_dir: bool, size: u64) {
        if parts.is_empty() {
            return;
        }

        let first = parts[0];
        let is_leaf = parts.len() == 1;

        if is_leaf {
            if let Some(existing) = self.children.iter_mut().find(|c| c.name == first) {
                if !is_dir {
                    existing.is_directory = false;
                    existing.size = size;
                }
            } else if is_dir {
                self.children.push(TreeNode::new_dir(first.to_string()));
            } else {
                self.children.push(TreeNode::new_file(first.to_string(), size));
            }
        } else {
            let child_idx = if let Some(pos) = self.children.iter().position(|c| c.name == first) {
                pos
            } else {
                self.children.push(TreeNode::new_dir(first.to_string()));
                self.children.len() - 1
            };
            self.children[child_idx].insert(&parts[1..], is_dir, size);
        }
    }

    fn sort_recursive(&mut self) {
        self.children.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        for child in &mut self.children {
            child.sort_recursive();
        }
    }
}

fn render_node(
    node: &TreeNode,
    depth: usize,
    max_depth: usize,
    ancestor_is_last: &mut Vec<bool>,
    is_last: bool,
) {
    if depth > max_depth {
        return;
    }

    if depth > 0 {
        let mut prefix = String::new();
        for &last in ancestor_is_last.iter() {
            if last {
                prefix.push_str("    ");
            } else {
                prefix.push_str("│   ");
            }
        }
        let branch = if is_last { "└── " } else { "├── " };
        if node.is_directory {
            println!("{}{}{}/", prefix, branch, node.name);
        } else {
            println!("{}{}{} ({})", prefix, branch, node.name, format_bytes(node.size));
        }
    }

    if depth < max_depth {
        let child_count = node.children.len();
        if depth > 0 {
            ancestor_is_last.push(is_last);
        }
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == child_count - 1;
            render_node(child, depth + 1, max_depth, ancestor_is_last, child_is_last);
        }
        if depth > 0 {
            ancestor_is_last.pop();
        }
    }
}

/// Executes headless `tree` subcommand.
pub fn execute_tree(
    archive_path: &Path,
    max_depth: Option<usize>,
    json: bool,
    include: &[String],
    exclude: &[String],
) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let (_volumes, data) = read_archive_data_auto(archive_path)?;
    let (format, all_entries) = parse_archive_entries(archive_path, &data)?;

    let entries: Vec<_> = all_entries
        .into_iter()
        .filter(|e| pattern_matches(&e.relative_path, include, exclude))
        .collect();

    let total_uncompressed: u64 = entries.iter().map(|e| e.uncompressed_size).sum();

    if json {
        let nodes: Vec<VfsNodeContractDto> = entries
            .iter()
            .map(|e| VfsNodeContractDto {
                name: e.name.clone(),
                relative_path: e.relative_path.clone(),
                is_directory: e.is_directory,
                uncompressed_size: e.uncompressed_size,
                compressed_size: e.compressed_size,
                crc32: e.crc32,
                is_encrypted: e.is_encrypted,
                match_indices: None,
            })
            .collect();

        let contract = VfsTreeContractDto {
            root_path: archive_path.to_string_lossy().to_string(),
            total_entries_count: entries.len(),
            total_uncompressed_bytes: total_uncompressed,
            nodes,
        };

        let json_str = serde_json::to_string_pretty(&contract)
            .map_err(|e| format!("Failed to serialize contract JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("{}/ ({})", archive_path.display(), format.name());
    let depth_limit = max_depth.unwrap_or(usize::MAX);

    let mut root = TreeNode::new_dir(archive_path.display().to_string());
    for entry in &entries {
        let parts: Vec<&str> = entry
            .relative_path
            .trim_matches('/')
            .split('/')
            .filter(|p| !p.is_empty())
            .collect();
        root.insert(&parts, entry.is_directory, entry.uncompressed_size);
    }
    root.sort_recursive();

    let mut ancestor_is_last = Vec::new();
    let child_count = root.children.len();
    for (i, child) in root.children.iter().enumerate() {
        let is_last = i == child_count - 1;
        render_node(child, 1, depth_limit, &mut ancestor_is_last, is_last);
    }

    let dir_count = entries.iter().filter(|e| e.is_directory).count();
    let file_count = entries.len() - dir_count;
    println!(
        "\n{} directories, {} files ({})",
        dir_count,
        file_count,
        format_bytes(total_uncompressed)
    );

    Ok(())
}
