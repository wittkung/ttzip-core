// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: extract.

use crate::cli::format::{detect_archive_format, format_bytes, read_archive_data_auto, ContainerFormat};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use ttzip_engine::archive::tar::TarArchive;
use ttzip_engine::codecs::brotli::brotli_decompress_to_vec;
use ttzip_engine::codecs::snappy::snappy_frame_decode_to_vec;
use ttzip_engine::fs::safe_extract::sanitize_and_validate_path;
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

/// Parses the CLI --on-conflict string into engine collision policy constant (0..=3).
pub fn parse_conflict_policy(s: &str) -> Result<u8, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "keep-both" | "keep_both" | "keepboth" => {
            Ok(ttzip_engine::types::TTZIP_COLLISION_POLICY_KEEP_BOTH)
        }
        "replace" => Ok(ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE),
        "skip" => Ok(ttzip_engine::types::TTZIP_COLLISION_POLICY_SKIP),
        "stop" => Ok(ttzip_engine::types::TTZIP_COLLISION_POLICY_STOP),
        other => Err(format!(
            "Invalid --on-conflict policy '{}'. Supported policies: keep-both, replace, skip, stop",
            other
        )),
    }
}

/// Resolves effective destination path based on conflict policy and existing filesystem state.
pub fn resolve_destination_path(
    dest_path: &Path,
    collision_policy: u8,
    is_directory: bool,
) -> Result<Option<PathBuf>, String> {
    if !dest_path.exists() {
        return Ok(Some(dest_path.to_path_buf()));
    }

    match collision_policy {
        ttzip_engine::types::TTZIP_COLLISION_POLICY_STOP => {
            Err(format!(
                "Extraction stopped: destination item already exists: {}",
                dest_path.display()
            ))
        }
        ttzip_engine::types::TTZIP_COLLISION_POLICY_SKIP => Ok(None),
        ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE => Ok(Some(dest_path.to_path_buf())),
        _ => {
            let kind = if is_directory {
                ttzip_archive::fs::collision::ItemKind::Directory
            } else {
                ttzip_archive::fs::collision::ItemKind::File
            };
            let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));
            let file_name = dest_path.file_name().unwrap_or_default();
            let mut n = 2;
            while n <= 10000 {
                let cand_name = ttzip_archive::fs::collision::naming::candidate(file_name, kind, n);
                let cand_path = parent.join(cand_name);
                if !cand_path.exists() {
                    return Ok(Some(cand_path));
                }
                n += 1;
            }
            Err(format!(
                "Failed to find unique name for existing item: {}",
                dest_path.display()
            ))
        }
    }
}

/// Executes headless `extract` subcommand with default keep-both policy.
pub fn execute_extract(params: CliExtractParams<'_>) -> Result<(), String> {
    execute_extract_with_policy(params, None)
}

/// Executes headless `extract` subcommand with explicit collision policy.
pub fn execute_extract_with_policy(
    params: CliExtractParams<'_>,
    on_conflict: Option<&str>,
) -> Result<(), String> {
    if !params.archive_path.exists() {
        return Err(format!("Archive file not found: {}", params.archive_path.display()));
    }

    let conflict_policy = on_conflict
        .map(parse_conflict_policy)
        .transpose()?
        .unwrap_or(ttzip_engine::types::TTZIP_COLLISION_POLICY_KEEP_BOTH);

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
        collision_policy: conflict_policy,
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
            let has_conflicts = archive.entries().iter().any(|entry| {
                sanitize_and_validate_path(dest_dir, &entry.rel_path)
                    .map(|p| p.exists())
                    .unwrap_or(false)
            });
            if include.is_empty() && exclude.is_empty() && (!has_conflicts || conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE) {
                let rep = archive
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("ZIP extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let mut count = 0;
                let mut total_bytes = 0;
                for (idx, entry) in archive.entries().iter().enumerate() {
                    if pattern_matches(&entry.rel_path, include, exclude) {
                        let out_path = sanitize_and_validate_path(dest_dir, &entry.rel_path)
                            .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", entry.rel_path, e))?;
                        let target_path = match resolve_destination_path(&out_path, conflict_policy, entry.is_directory)? {
                            Some(p) => p,
                            None => continue,
                        };
                        if entry.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&target_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = target_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = archive.extract_entry_bytes(idx, password)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", entry.rel_path, e))?;
                                fs::write(&target_path, &bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", target_path.display(), e))?;
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
            let has_conflicts = archive.info().files.iter().any(|file| {
                sanitize_and_validate_path(dest_dir, &file.rel_path)
                    .map(|p| p.exists())
                    .unwrap_or(false)
            });
            if include.is_empty() && exclude.is_empty() && (!has_conflicts || conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE) {
                let rep = archive
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("7z extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let mut count = 0;
                let mut total_bytes = 0;
                for (idx, file) in archive.info().files.iter().enumerate() {
                    if pattern_matches(&file.rel_path, include, exclude) {
                        let out_path = sanitize_and_validate_path(dest_dir, &file.rel_path)
                            .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", file.rel_path, e))?;
                        let target_path = match resolve_destination_path(&out_path, conflict_policy, file.is_directory)? {
                            Some(p) => p,
                            None => continue,
                        };
                        if file.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&target_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = target_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = archive.extract_entry_bytes_stream(idx, password)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", file.rel_path, e))?;
                                fs::write(&target_path, &bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", target_path.display(), e))?;
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
            let has_conflicts = archive.entries().iter().any(|entry| {
                sanitize_and_validate_path(dest_dir, &entry.path)
                    .map(|p| p.exists())
                    .unwrap_or(false)
            });
            if include.is_empty() && exclude.is_empty() && (!has_conflicts || conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE) {
                let rep = archive
                    .extract_all(dest_dir, &options)
                    .map_err(|e| format!("TAR extraction failed: {:?}", e))?;
                (rep.processed_entries_count, rep.total_uncompressed_bytes)
            } else {
                let mut count = 0;
                let mut total_bytes = 0;
                for (idx, entry) in archive.entries().iter().enumerate() {
                    if pattern_matches(&entry.path, include, exclude) {
                        let out_path = sanitize_and_validate_path(dest_dir, &entry.path)
                            .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", entry.path, e))?;
                        let target_path = match resolve_destination_path(&out_path, conflict_policy, entry.is_directory)? {
                            Some(p) => p,
                            None => continue,
                        };
                        if entry.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&target_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = target_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = archive.extract_entry_bytes(idx)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", entry.path, e))?;
                                fs::write(&target_path, bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", target_path.display(), e))?;
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
                let has_conflicts = tar.entries().iter().any(|entry| {
                    sanitize_and_validate_path(dest_dir, &entry.path)
                        .map(|p| p.exists())
                        .unwrap_or(false)
                });
                if !has_conflicts || conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE {
                    let rep = tar
                        .extract_all(dest_dir, &options)
                        .map_err(|e| format!("Snappy TAR extraction failed: {:?}", e))?;
                    (rep.processed_entries_count, rep.total_uncompressed_bytes)
                } else {
                    let mut count = 0;
                    let mut total_bytes = 0;
                    for (idx, entry) in tar.entries().iter().enumerate() {
                        let out_path = sanitize_and_validate_path(dest_dir, &entry.path)
                            .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", entry.path, e))?;
                        let target_path = match resolve_destination_path(&out_path, conflict_policy, entry.is_directory)? {
                            Some(p) => p,
                            None => continue,
                        };
                        if entry.is_directory {
                            if !dry_run {
                                fs::create_dir_all(&target_path).ok();
                            }
                            count += 1;
                        } else {
                            if !dry_run {
                                if let Some(parent) = target_path.parent() {
                                    fs::create_dir_all(parent).ok();
                                }
                                let bytes = tar.extract_entry_bytes(idx)
                                    .map_err(|e| format!("Extraction failed for {}: {:?}", entry.path, e))?;
                                fs::write(&target_path, bytes)
                                    .map_err(|e| format!("Failed to write {}: {}", target_path.display(), e))?;
                            }
                            count += 1;
                            total_bytes += entry.size;
                        }
                    }
                    (count, total_bytes)
                }
            } else {
                let filename = archive_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("decompressed.bin");
                let inner_name = filename
                    .strip_suffix(".sz")
                    .or_else(|| filename.strip_suffix(".snappy"))
                    .unwrap_or(filename);
                let out_file_path = sanitize_and_validate_path(dest_dir, inner_name)
                    .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", inner_name, e))?;
                let target_path = match resolve_destination_path(&out_file_path, conflict_policy, false)? {
                    Some(p) => p,
                    None => return Ok(()),
                };
                let mut out_file = File::create(&target_path)
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
                    let has_conflicts = tar.entries().iter().any(|entry| {
                        sanitize_and_validate_path(dest_dir, &entry.path)
                            .map(|p| p.exists())
                            .unwrap_or(false)
                    });
                    if !has_conflicts || conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE {
                        let rep = tar
                            .extract_all(dest_dir, &options)
                            .map_err(|e| format!("Brotli TAR extraction failed: {:?}", e))?;
                        (rep.processed_entries_count, rep.total_uncompressed_bytes)
                    } else {
                        let mut count = 0;
                        let mut total_bytes = 0;
                        for (idx, entry) in tar.entries().iter().enumerate() {
                            let out_path = sanitize_and_validate_path(dest_dir, &entry.path)
                                .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", entry.path, e))?;
                            let target_path = match resolve_destination_path(&out_path, conflict_policy, entry.is_directory)? {
                                Some(p) => p,
                                None => continue,
                            };
                            if entry.is_directory {
                                if !dry_run {
                                    fs::create_dir_all(&target_path).ok();
                                }
                                count += 1;
                            } else {
                                if !dry_run {
                                    if let Some(parent) = target_path.parent() {
                                        fs::create_dir_all(parent).ok();
                                    }
                                    let bytes = tar.extract_entry_bytes(idx)
                                        .map_err(|e| format!("Extraction failed for {}: {:?}", entry.path, e))?;
                                    fs::write(&target_path, bytes)
                                        .map_err(|e| format!("Failed to write {}: {}", target_path.display(), e))?;
                                }
                                count += 1;
                                total_bytes += entry.size;
                            }
                        }
                        (count, total_bytes)
                    }
                } else {
                    return Err("Failed to parse inner TAR archive".to_string());
                }
            } else {
                let filename = archive_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("decompressed.bin");
                let inner_name = filename.strip_suffix(".br").unwrap_or(filename);
                let out_file_path = sanitize_and_validate_path(dest_dir, inner_name)
                    .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", inner_name, e))?;
                let target_path = match resolve_destination_path(&out_file_path, conflict_policy, false)? {
                    Some(p) => p,
                    None => return Ok(()),
                };
                let mut out_file = File::create(&target_path)
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
    if params.verbose {
        println!(
            "Extracted {} entries ({}) to {} in {:.2?}",
            entries_count,
            format_bytes(total_uncompressed_bytes),
            dest_dir.display(),
            elapsed
        );
    }

    Ok(())
}
