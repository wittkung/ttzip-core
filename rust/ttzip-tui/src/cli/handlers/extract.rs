// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Subcommand execution handler: extract.

use crate::cli::format::{detect_archive_format, format_bytes, read_archive_data_auto, ContainerFormat};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use ttzip_engine::archive::tar::TarArchive;
use ttzip_engine::codecs::brotli::brotli_decompress_to_vec;
use ttzip_engine::codecs::snappy::snappy_frame_decode_to_vec;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::types::TTZipExtractOptions;
use ttzip_engine::zip::ZipArchive;

/// Simple glob match helper for include/exclude patterns.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat = pattern.trim_start_matches("./").trim_start_matches('/');
    let txt = text.trim_start_matches("./").trim_start_matches('/');
    if pat == "*" || pat == "**" {
        return true;
    }
    if pat.starts_with('*') && pat.ends_with('*') && pat.len() > 2 {
        let sub = &pat[1..pat.len() - 1];
        return txt.contains(sub);
    }
    if let Some(suffix) = pat.strip_prefix('*') {
        return txt.ends_with(suffix);
    }
    if let Some(prefix) = pat.strip_suffix('*') {
        return txt.starts_with(prefix);
    }
    txt == pat
}

/// Checks if an entry path satisfies include/exclude pattern filters.
pub fn pattern_matches(rel_path: &str, include: &[String], exclude: &[String]) -> bool {
    let clean = rel_path.trim_start_matches('/');
    if !include.is_empty() {
        let matched = include.iter().any(|pat| glob_match(pat, clean));
        if !matched {
            return false;
        }
    }
    if !exclude.is_empty() {
        let excluded = exclude.iter().any(|pat| glob_match(pat, clean));
        if excluded {
            return false;
        }
    }
    true
}

/// Execution parameters for the headless `extract` CLI subcommand.
#[derive(Debug, Clone)]
pub struct CliExtractParams<'a> {
    pub archive_path: &'a Path,
    pub output_dir: Option<&'a Path>,
    pub password: Option<&'a str>,
    pub threads: u32,
    pub verbose: bool,
    pub dry_run: bool,
    pub include: &'a [String],
    pub exclude: &'a [String],
}

/// Executes headless `extract` subcommand with zero-copy mmap and optional pattern filtering.
pub fn execute_extract(params: CliExtractParams<'_>) -> Result<(), String> {
    if !params.archive_path.exists() {
        return Err(format!("Archive file not found: {}", params.archive_path.display()));
    }

    let start_time = Instant::now();
    let dest_dir = params.output_dir.unwrap_or_else(|| Path::new("."));
    if !params.dry_run {
        fs::create_dir_all(dest_dir)
            .map_err(|e| format!("Failed to create output directory {}: {}", dest_dir.display(), e))?;
    }

    let (_volumes, data) = read_archive_data_auto(params.archive_path)?;
    let format = detect_archive_format(params.archive_path, &data);

    let password_c = params.password.map(|p| CString::new(p).unwrap_or_default());
    let password_ptr = password_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());

    let options = TTZipExtractOptions {
        password: password_ptr,
        thread_budget: params.threads.max(1),
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: params.dry_run,
        ..Default::default()
    };

    let include = params.include;
    let exclude = params.exclude;
    let dry_run = params.dry_run;
    let password = params.password;
    let archive_path = params.archive_path;

    let (entries_count, total_uncompressed_bytes) = match format {
        ContainerFormat::Zip => {
            let archive = ZipArchive::open_slice(&data)
                .map_err(|e| format!("Failed to open ZIP archive: {:?}", e))?;
            if include.is_empty() && exclude.is_empty() {
                let rep = archive
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("ZIP extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let mut count = 0;
                let mut total_bytes = 0;
                for (idx, entry) in archive.entries().iter().enumerate() {
                    if pattern_matches(&entry.rel_path, include, exclude) {
                        let out_path = dest_dir.join(&entry.rel_path);
                        if entry.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&out_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = out_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = archive.extract_entry_bytes(idx, password)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", entry.rel_path, e))?;
                                fs::write(&out_path, &bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;
                            }
                            count += 1;
                            total_bytes += entry.uncompressed_size;
                        }
                    }
                }
                (count, total_bytes)
            }
        }
        ContainerFormat::SevenZip => {
            let archive = SevenZArchive::open_slice_with_password(&data, password)
                .map_err(|e| format!("Failed to open 7z archive: {:?}", e))?;
            if include.is_empty() && exclude.is_empty() {
                let rep = archive
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("7z extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let mut count = 0;
                let mut total_bytes = 0;
                for (idx, file) in archive.info().files.iter().enumerate() {
                    if pattern_matches(&file.rel_path, include, exclude) {
                        let out_path = dest_dir.join(&file.rel_path);
                        if file.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&out_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = out_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = archive.extract_entry_bytes_stream(idx, password)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", file.rel_path, e))?;
                                fs::write(&out_path, &bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;
                            }
                            count += 1;
                            let sz = archive.info().stream_sizes.get(idx).copied().unwrap_or(0);
                            total_bytes += sz;
                        }
                    }
                }
                (count, total_bytes)
            }
        }
        ContainerFormat::Tar => {
            let archive = TarArchive::open_slice(&data)
                .map_err(|e| format!("Failed to open TAR archive: {:?}", e))?;
            if include.is_empty() && exclude.is_empty() {
                let rep = archive
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("TAR extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let mut count = 0;
                let mut total_bytes = 0;
                for (idx, entry) in archive.entries().iter().enumerate() {
                    if pattern_matches(&entry.path, include, exclude) {
                        let out_path = dest_dir.join(&*entry.path);
                        if entry.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&out_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = out_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = archive.extract_entry_bytes(idx)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", entry.path, e))?;
                                fs::write(&out_path, bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;
                            }
                            count += 1;
                            total_bytes += entry.size;
                        }
                    }
                }
                (count, total_bytes)
            }
        }
        ContainerFormat::Snappy => {
            let decompressed = snappy_frame_decode_to_vec(&data, 1024 * 1024 * 512)
                .map_err(|e| format!("Failed to decompress Snappy stream: {:?}", e))?;
            if let Ok(tar) = TarArchive::open_slice(&decompressed) {
                let rep = tar
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("Snappy TAR extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let filename = archive_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("decompressed.bin");
                let inner_name = filename
                    .strip_suffix(".sz")
                    .or_else(|| filename.strip_suffix(".snappy"))
                    .unwrap_or(filename);
                let out_file_path = dest_dir.join(inner_name);
                let mut out_file = File::create(&out_file_path)
                    .map_err(|e| format!("Failed to write decompressed Snappy file: {}", e))?;
                out_file
                    .write_all(&decompressed)
                    .map_err(|e| format!("Failed to write decompressed Snappy data: {}", e))?;
                (1, decompressed.len() as u64)
            }
        }
        ContainerFormat::Brotli | ContainerFormat::TarBrotli => {
            let decompressed = brotli_decompress_to_vec(&data, 1024 * 1024 * 512)
                .map_err(|e| format!("Failed to decompress Brotli stream: {:?}", e))?;
            if format == ContainerFormat::TarBrotli || TarArchive::open_slice(&decompressed).is_ok() {
                if let Ok(tar) = TarArchive::open_slice(&decompressed) {
                    let rep = tar
                        .extract_all(dest_dir, &options)
                        .map_err(|e| format!("Brotli TAR extraction failed: {:?}", e))?;
                    (rep.processed_entries_count, rep.total_uncompressed_bytes)
                } else {
                    return Err("Failed to parse inner TAR archive".to_string());
                }
            } else {
                let filename = archive_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("decompressed.bin");
                let inner_name = filename.strip_suffix(".br").unwrap_or(filename);
                let out_file_path = dest_dir.join(inner_name);
                let mut out_file = File::create(&out_file_path)
                    .map_err(|e| format!("Failed to write decompressed Brotli file: {}", e))?;
                out_file
                    .write_all(&decompressed)
                    .map_err(|e| format!("Failed to write decompressed Brotli data: {}", e))?;
                (1, decompressed.len() as u64)
            }
        }
        ContainerFormat::Unknown => {
            return Err(format!(
                "Cannot extract unrecognized archive: {}",
                archive_path.display()
            ));
        }
    };

    let elapsed = start_time.elapsed();
    println!(
        "Extracted {} entries ({}) to {} in {:.2?}",
        entries_count,
        format_bytes(total_uncompressed_bytes),
        dest_dir.display(),
        elapsed
    );

    Ok(())
}
