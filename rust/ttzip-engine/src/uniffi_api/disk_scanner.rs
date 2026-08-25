// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Concurrent Directory Scanner & Path Autocompletion Scaffolding.

use std::path::Path;
use super::types::{DiskItemSummary, TTZipError};

/// Scans a directory and returns lightweight item summaries.
#[uniffi::export]
pub fn scan_directory(path: String, _max_depth: u32) -> Result<Vec<DiskItemSummary>, TTZipError> {
    let root = Path::new(&path);
    if !root.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let mut items = Vec::new();
    let entries = std::fs::read_dir(root).map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if name.starts_with('.') {
            continue; // Skip hidden items by default
        }

        let is_dir = p.is_dir();
        let meta = p.metadata().ok();
        let size = if is_dir { 0 } else { meta.as_ref().map(|m| m.len()).unwrap_or(0) };
        let mtime = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        items.push(DiskItemSummary {
            path: p.to_string_lossy().to_string(),
            name,
            is_directory: is_dir,
            size,
            mtime_epoch_secs: mtime,
        });
    }

    items.sort_by(|a, b| {
        match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(items)
}

/// Fast path autocompletion query based on directory scanning and prefix matching.
#[uniffi::export]
pub fn autocomplete_disk_path(
    raw_input: String,
    base_directory: String,
    max_results: u32,
) -> Vec<String> {
    let input = raw_input.trim();
    if input.is_empty() {
        return Vec::new();
    }

    let base = Path::new(&base_directory);
    let resolved_path = if input.starts_with('/') || input.starts_with('~') {
        let expanded = if let Some(stripped) = input.strip_prefix('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/Users".to_string());
            format!("{}{}", home, stripped)
        } else {
            input.to_string()
        };
        std::path::PathBuf::from(expanded)
    } else {
        base.join(input)
    };

    let (search_dir, prefix) = if input.ends_with('/') {
        (resolved_path.as_path(), "")
    } else {
        (
            resolved_path.parent().unwrap_or(base),
            resolved_path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    };

    if !search_dir.exists() || !search_dir.is_dir() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    if let Ok(entries) = std::fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if prefix.is_empty() || name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                let full = entry.path().to_string_lossy().to_string();
                matches.push(full);
                if matches.len() >= (max_results as usize) {
                    break;
                }
            }
        }
    }

    matches.sort();
    matches
}
