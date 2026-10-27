// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Background asynchronous extraction task execution.

use super::state::AppState;
use super::types::AppMode;
use crate::event::AppEvent;
use crate::ui::progress::ProgressSnapshot;
use crossbeam_channel::Sender;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;
use ttzip_engine::fs::safe_extract::sanitize_and_validate_path;
use ttzip_engine::runtime::cancellation::CancellationToken;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::zip::ZipArchive;

impl AppState {
    /// Triggers extraction of either marked entries or the selected entry to `./` or target dir.
    pub fn trigger_extraction(&mut self, event_sender: Sender<AppEvent>) {
        let mut selected_paths = self.vfs.get_selected_paths();
        if selected_paths.is_empty() {
            let visible = self.vfs.flatten_visible();
            if let Some(item) = visible.get(self.selected_index) {
                if !item.node.is_dir {
                    selected_paths.push(item.node.relative_path.clone());
                }
            }
        }

        if selected_paths.is_empty() {
            self.set_status("No files selected to extract.".to_string());
            return;
        }

        self.current_mode = AppMode::Progress;
        self.cancellation_token = CancellationToken::new();

        let token = self.cancellation_token.clone();
        let raw_data = self.archive_raw_data.clone();
        let is_zip = self.archive_format == "ZIP";
        let dest_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let password = self.recovered_password.clone();

        thread::spawn(move || {
            let start = Instant::now();
            let total_entries = selected_paths.len();
            let mut processed_bytes = 0u64;
            let mut total_bytes = 0u64;

            // Open archive once outside the loop and build path -> index lookup map
            let zip_archive = if is_zip {
                match ZipArchive::open_slice(&raw_data) {
                    Ok(a) => Some(a),
                    Err(e) => {
                        let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                            "Failed to open ZIP archive: {:?}",
                            e
                        ))));
                        return;
                    }
                }
            } else {
                None
            };

            let sevenz_archive = if !is_zip {
                match SevenZArchive::open_slice(&raw_data) {
                    Ok(a) => Some(a),
                    Err(e) => {
                        let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                            "Failed to open 7z archive: {:?}",
                            e
                        ))));
                        return;
                    }
                }
            } else {
                None
            };

            let zip_index_map: std::collections::HashMap<String, usize> = if let Some(ref archive) = zip_archive {
                archive
                    .entries()
                    .iter()
                    .enumerate()
                    .map(|(idx, e)| (e.rel_path.clone(), idx))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };

            let sevenz_index_map: std::collections::HashMap<String, usize> = if let Some(ref archive) = sevenz_archive {
                archive
                    .files()
                    .iter()
                    .enumerate()
                    .map(|(idx, f)| (f.rel_path.clone(), idx))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };

            // Calculate total size using O(1) index map
            if let Some(ref archive) = zip_archive {
                for path in &selected_paths {
                    if let Some(&idx) = zip_index_map.get(path) {
                        total_bytes += archive.entries()[idx].uncompressed_size;
                    }
                }
            } else if let Some(ref archive) = sevenz_archive {
                let info = archive.info();
                for path in &selected_paths {
                    let mut stream_idx = 0usize;
                    for f in &info.files {
                        if !f.is_directory && !f.is_empty_stream {
                            if f.rel_path == *path {
                                let sz = info.stream_sizes.get(stream_idx).copied().unwrap_or(0);
                                total_bytes += sz;
                                break;
                            }
                            stream_idx += 1;
                        }
                    }
                }
            }

            for (proc_count, rel_path) in selected_paths.iter().enumerate() {
                if token.is_cancelled() {
                    let _ = event_sender.send(AppEvent::TaskCompleted(Err("Extraction cancelled by user".to_string())));
                    return;
                }

                let target_path = match sanitize_and_validate_path(&dest_dir, rel_path) {
                    Ok(p) => p,
                    Err(_) => {
                        let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!("Security violation on path: {}", rel_path))));
                        return;
                    }
                };

                if let Some(parent) = target_path.parent() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                            "Failed to create target directory {}: {}",
                            parent.display(),
                            e
                        ))));
                        return;
                    }
                }

                let (file_data, uncomp_size) = if is_zip {
                    let archive = zip_archive.as_ref().unwrap();
                    let idx = match zip_index_map.get(rel_path) {
                        Some(&i) => i,
                        None => {
                            let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                                "Entry '{}' not found in archive index",
                                rel_path
                            ))));
                            return;
                        }
                    };
                    let expected_size = archive.entries()[idx].uncompressed_size;
                    let bytes = match archive.extract_entry_bytes(idx, password.as_deref()) {
                        Ok(b) => b,
                        Err(e) => {
                            let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                                "Extraction failed for entry '{}': {:?}",
                                rel_path, e
                            ))));
                            return;
                        }
                    };
                    (bytes, expected_size)
                } else if let Some(ref archive) = sevenz_archive {
                    let idx = match sevenz_index_map.get(rel_path) {
                        Some(&i) => i,
                        None => {
                            let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                                "Entry '{}' not found in archive index",
                                rel_path
                            ))));
                            return;
                        }
                    };
                    let bytes = match archive.extract_entry_bytes(idx, password.as_deref()) {
                        Ok(b) => b,
                        Err(e) => {
                            let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                                "Extraction failed for entry '{}': {:?}",
                                rel_path, e
                            ))));
                            return;
                        }
                    };
                    let sz = bytes.len() as u64;
                    (bytes, sz)
                } else {
                    let _ = event_sender.send(AppEvent::TaskCompleted(Err(
                        "Unsupported archive container format".to_string(),
                    )));
                    return;
                };

                if let Err(e) = fs::write(&target_path, &file_data) {
                    let _ = event_sender.send(AppEvent::TaskCompleted(Err(format!(
                        "Failed to write extracted file {}: {}",
                        target_path.display(),
                        e
                    ))));
                    return;
                }
                processed_bytes += uncomp_size;

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

                let snap = ProgressSnapshot {
                    task_title: format!("Extracting {} entries", total_entries),
                    current_entry_name: rel_path.to_string(),
                    processed_bytes,
                    total_bytes,
                    processed_entries: proc_count + 1,
                    total_entries,
                    instant_throughput_mb_per_sec: speed_mb,
                    elapsed_seconds: elapsed,
                    eta_seconds: eta,
                };

                let _ = event_sender.send(AppEvent::Progress(snap));
            }

            let _ = event_sender.send(AppEvent::TaskCompleted(Ok(format!(
                "Successfully extracted {} files ({}) in {:.2?}",
                total_entries,
                crate::ui::explorer::format_bytes(processed_bytes),
                start.elapsed()
            ))));
        });
    }
}
