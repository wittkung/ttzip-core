// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Background asynchronous extraction task execution.

use super::state::AppState;
use super::types::AppMode;
use crate::event::AppEvent;
use crate::ui::progress::ProgressSnapshot;
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use ttzip_engine::fs::safe_extract::sanitize_and_validate_path;
use ttzip_engine::runtime::cancellation::CancellationToken;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::zip::ZipArchive;

enum ExtractionContainer<'a> {
    Zip {
        archive: ZipArchive<'a>,
        index_map: HashMap<String, usize>,
    },
    SevenZ {
        archive: Box<SevenZArchive<'a>>,
        index_map: HashMap<String, usize>,
    },
}

impl<'a> ExtractionContainer<'a> {
    fn open(is_zip: bool, raw_data: &'a [u8]) -> Result<Self, String> {
        if is_zip {
            let archive = ZipArchive::open_slice(raw_data)
                .map_err(|e| format!("Failed to open ZIP archive: {:?}", e))?;
            let index_map = archive
                .entries()
                .iter()
                .enumerate()
                .map(|(idx, e)| (e.rel_path.clone(), idx))
                .collect();
            Ok(Self::Zip { archive, index_map })
        } else {
            let archive = SevenZArchive::open_slice(raw_data)
                .map_err(|e| format!("Failed to open 7z archive: {:?}", e))?;
            let index_map = archive
                .files()
                .iter()
                .enumerate()
                .map(|(idx, f)| (f.rel_path.clone(), idx))
                .collect();
            Ok(Self::SevenZ {
                archive: Box::new(archive),
                index_map,
            })
        }
    }

    fn total_uncompressed_bytes(&self, selected_paths: &[String]) -> u64 {
        match self {
            Self::Zip { archive, index_map } => selected_paths
                .iter()
                .filter_map(|p| index_map.get(p))
                .map(|&idx| archive.entries()[idx].uncompressed_size)
                .sum(),
            Self::SevenZ { archive, .. } => {
                let mut total = 0u64;
                for path in selected_paths {
                    total += sevenz_entry_stream_size(archive, path);
                }
                total
            }
        }
    }

    fn extract_entry(&self, rel_path: &str, password: Option<&str>) -> Result<(Vec<u8>, u64), String> {
        match self {
            Self::Zip { archive, index_map } => {
                let &idx = index_map
                    .get(rel_path)
                    .ok_or_else(|| format!("Entry '{}' not found in archive index", rel_path))?;
                let expected_size = archive.entries()[idx].uncompressed_size;
                let bytes = archive
                    .extract_entry_bytes(idx, password)
                    .map_err(|e| format!("Extraction failed for entry '{}': {:?}", rel_path, e))?;
                Ok((bytes, expected_size))
            }
            Self::SevenZ { archive, index_map } => {
                let &idx = index_map
                    .get(rel_path)
                    .ok_or_else(|| format!("Entry '{}' not found in archive index", rel_path))?;
                let bytes = archive
                    .extract_entry_bytes(idx, password)
                    .map_err(|e| format!("Extraction failed for entry '{}': {:?}", rel_path, e))?;
                let sz = bytes.len() as u64;
                Ok((bytes, sz))
            }
        }
    }
}

fn sevenz_entry_stream_size(archive: &SevenZArchive<'_>, target_path: &str) -> u64 {
    let info = archive.info();
    let mut stream_idx = 0usize;
    for f in &info.files {
        if !f.is_directory && !f.is_empty_stream {
            if f.rel_path == target_path {
                return info.stream_sizes.get(stream_idx).copied().unwrap_or(0);
            }
            stream_idx += 1;
        }
    }
    0
}

fn write_extracted_entry(dest_dir: &Path, rel_path: &str, file_data: &[u8]) -> Result<(), String> {
    let target_path = sanitize_and_validate_path(dest_dir, rel_path)
        .map_err(|_| format!("Security violation on path: {}", rel_path))?;
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create target directory {}: {}", parent.display(), e))?;
    }
    fs::write(&target_path, file_data)
        .map_err(|e| format!("Failed to write extracted file {}: {}", target_path.display(), e))
}

fn build_progress_snapshot(
    total_entries: usize,
    rel_path: &str,
    processed_bytes: u64,
    total_bytes: u64,
    proc_count: usize,
    start: Instant,
) -> ProgressSnapshot {
    let elapsed = start.elapsed().as_secs_f64();
    let speed_mb = if elapsed > 0.0 {
        (processed_bytes as f64 / (1024.0 * 1024.0)) / elapsed
    } else {
        0.0
    };
    let eta = if speed_mb > 0.0 && total_bytes > processed_bytes {
        ((total_bytes - processed_bytes) as f64 / (1024.0 * 1024.0)) / speed_mb
    } else {
        0.0
    };
    ProgressSnapshot {
        task_title: format!("Extracting {} entries", total_entries),
        current_entry_name: rel_path.to_string(),
        processed_bytes,
        total_bytes,
        processed_entries: proc_count + 1,
        total_entries,
        instant_throughput_mb_per_sec: speed_mb,
        elapsed_seconds: elapsed,
        eta_seconds: eta,
    }
}

fn run_extraction_worker(
    raw_data: &[u8],
    is_zip: bool,
    selected_paths: &[String],
    dest_dir: &Path,
    password: Option<&str>,
    token: &CancellationToken,
    event_sender: &Sender<AppEvent>,
) -> Result<String, String> {
    let start = Instant::now();
    let total_entries = selected_paths.len();
    let mut processed_bytes = 0u64;
    let container = ExtractionContainer::open(is_zip, raw_data)?;
    let total_bytes = container.total_uncompressed_bytes(selected_paths);

    for (proc_count, rel_path) in selected_paths.iter().enumerate() {
        if token.is_cancelled() {
            return Err("Extraction cancelled by user".to_string());
        }
        let (file_data, uncomp_size) = container.extract_entry(rel_path, password)?;
        write_extracted_entry(dest_dir, rel_path, &file_data)?;
        processed_bytes += uncomp_size;
        let snap = build_progress_snapshot(
            total_entries,
            rel_path,
            processed_bytes,
            total_bytes,
            proc_count,
            start,
        );
        let _ = event_sender.send(AppEvent::Progress(snap));
    }

    Ok(format!(
        "Successfully extracted {} files ({}) in {:.2?}",
        total_entries,
        crate::ui::explorer::format_bytes(processed_bytes),
        start.elapsed()
    ))
}

impl AppState {
    fn resolve_extraction_paths(&self) -> Vec<String> {
        let mut selected_paths = self.vfs.get_selected_paths();
        if selected_paths.is_empty() {
            let visible = self.vfs.flatten_visible();
            if let Some(item) = visible.get(self.selected_index) {
                if !item.node.is_dir {
                    selected_paths.push(item.node.relative_path.clone());
                }
            }
        }
        selected_paths
    }

    /// Triggers extraction of either marked entries or the selected entry to `./` or target dir.
    pub fn trigger_extraction(&mut self, event_sender: Sender<AppEvent>) {
        let selected_paths = self.resolve_extraction_paths();
        if selected_paths.is_empty() {
            self.set_status("No files selected to extract.".to_string());
            return;
        }

        self.current_mode = AppMode::Progress;
        self.cancellation_token = CancellationToken::new();

        let token = self.cancellation_token.clone();
        let raw_data = Arc::clone(&self.archive_raw_data);
        let is_zip = self.archive_format == "ZIP";
        let dest_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let password = self.recovered_password.clone();

        thread::spawn(move || {
            let result = run_extraction_worker(
                &raw_data,
                is_zip,
                &selected_paths,
                &dest_dir,
                password.as_deref(),
                &token,
                &event_sender,
            );
            let _ = event_sender.send(AppEvent::TaskCompleted(result));
        });
    }
}
