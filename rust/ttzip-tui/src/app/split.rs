// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Asynchronous multi-volume archive split pipeline runner.

use super::state::AppState;
use super::types::AppMode;
use crate::cli::format::format_bytes;
use crate::event::AppEvent;
use crate::ui::progress::ProgressSnapshot;
use crossbeam_channel::Sender;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;
use ttzip_engine::archive::split::{SplitVolumeWriter, VolumeNamingScheme};
use ttzip_engine::runtime::cancellation::CancellationToken;

fn build_split_progress(
    chunk_size: u64,
    processed_bytes: u64,
    total_bytes: u64,
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
        task_title: format!("Splitting Archive into {} chunks", format_bytes(chunk_size)),
        current_entry_name: format!("Writing segment offset {}", format_bytes(processed_bytes)),
        processed_bytes,
        total_bytes,
        processed_entries: 1,
        total_entries: 1,
        instant_throughput_mb_per_sec: speed_mb,
        elapsed_seconds: elapsed,
        eta_seconds: eta,
    }
}

fn run_split_worker(
    source_path: &Path,
    base_target_path: &Path,
    chunk_size: u64,
    naming_scheme: VolumeNamingScheme,
    total_bytes: u64,
    token: &CancellationToken,
    event_sender: &Sender<AppEvent>,
) -> Result<(Vec<String>, String), String> {
    let start = Instant::now();
    let mut source_file = File::open(source_path)
        .map_err(|e| format!("Failed to open source archive: {}", e))?;
    let mut writer = SplitVolumeWriter::new(base_target_path, chunk_size, naming_scheme)
        .map_err(|e| format!("Failed to init SplitVolumeWriter: {}", e))?;

    let mut buffer = vec![0u8; 256 * 1024];
    let mut processed_bytes = 0u64;
    let mut last_progress_time = Instant::now();

    loop {
        if token.is_cancelled() {
            return Err("Split cancelled by user".to_string());
        }

        let read_bytes = source_file
            .read(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;
        if read_bytes == 0 {
            break;
        }

        std::io::Write::write_all(&mut writer, &buffer[..read_bytes])
            .map_err(|e| format!("Write error: {}", e))?;

        processed_bytes += read_bytes as u64;
        let is_complete = total_bytes > 0 && processed_bytes >= total_bytes;
        if last_progress_time.elapsed().as_millis() >= 100 || is_complete {
            last_progress_time = Instant::now();
            let snap = build_split_progress(chunk_size, processed_bytes, total_bytes, start);
            let _ = event_sender.send(AppEvent::Progress(snap));
        }
    }

    let vols = writer
        .close()
        .map_err(|e| format!("Failed to finalize split volumes: {}", e))?;
    let vol_names: Vec<String> = vols.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let summary = format!(
        "Successfully split archive into {} volumes ({}) in {:.2?}",
        vols.len(),
        format_bytes(processed_bytes),
        start.elapsed()
    );
    Ok((vol_names, summary))
}

impl AppState {
    /// Triggers asynchronous multi-volume splitting of the active archive.
    pub fn trigger_split(&mut self, event_sender: Sender<AppEvent>) {
        let Some(split_state) = self.split_modal_state.clone() else {
            return;
        };

        let chunk_size = match split_state.current_chunk_size_bytes() {
            Ok(sz) => sz,
            Err(err) => {
                self.set_status(format!("Split Error: {}", err));
                return;
            }
        };

        let source_path = self.archive_path.clone();
        if !source_path.exists() {
            self.set_status("Source archive file not found".to_string());
            return;
        }

        let naming_scheme = split_state.naming_scheme();
        let out_dir = if split_state.output_dir.is_empty() {
            source_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
        } else {
            PathBuf::from(&split_state.output_dir)
        };

        let filename = source_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("archive"));
        let base_target_path = out_dir.join(filename);

        self.current_mode = AppMode::Progress;
        self.cancellation_token = CancellationToken::new();

        let token = self.cancellation_token.clone();
        let total_bytes = self.total_size_bytes;

        thread::spawn(move || {
            match run_split_worker(
                &source_path,
                &base_target_path,
                chunk_size,
                naming_scheme,
                total_bytes,
                &token,
                &event_sender,
            ) {
                Ok((vol_names, summary)) => {
                    let _ = event_sender.send(AppEvent::SplitCompleted(Ok(vol_names)));
                    let _ = event_sender.send(AppEvent::TaskCompleted(Ok(summary)));
                }
                Err(err) => {
                    let _ = event_sender.send(AppEvent::TaskCompleted(Err(err)));
                }
            }
        });
    }
}
