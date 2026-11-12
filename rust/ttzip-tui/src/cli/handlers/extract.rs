// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: extract.

use crate::cli::format::{detect_archive_format, format_bytes, read_archive_data_auto, ContainerFormat};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use ttzip_archive::fs::collision::fs_ops::{RENAME_EXCL, RENAME_NOFOLLOW_ANY};
use ttzip_archive::fs::collision::{FsOps, ItemKind, LibcFsOps, StagingRoot};
use ttzip_engine::archive::tar::{TarArchive, TarEntry};
use ttzip_engine::archive::unified::extract::extract_archive_with_metrics;
use ttzip_engine::archive::unified::inspect::inspect_archive;
use ttzip_engine::archive::unified::{extract_selected_entries, extract_single_entry_memory, EntrySelection};
use ttzip_engine::codecs::brotli::brotli_decompress_to_vec;
use ttzip_engine::codecs::snappy::snappy_frame_decode_to_vec;
use ttzip_engine::fs::dir_jail::DirJail;
use ttzip_engine::fs::safe_extract::{
    create_dir_in_dir_nofollow, create_hardlink_in_dir_nofollow, create_symlink_in_dir_nofollow,
    open_extracted_file_in_dir_nofollow, sanitize_and_validate_path, validate_no_intermediate_symlinks,
};
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::types::TTZipExtractOptions;
use ttzip_engine::zip::ZipArchive;

static EXTRACT_STAGE_SEQ: AtomicU64 = AtomicU64::new(0);

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
    if !include.is_empty() && !include.iter().any(|pat| glob_match(pat, clean)) {
        return false;
    }
    if !exclude.is_empty() && exclude.iter().any(|pat| glob_match(pat, clean)) {
        return false;
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
    resolve_destination_path_with_reserved(dest_path, collision_policy, is_directory, &mut HashSet::new())
}

fn resolve_destination_path_with_reserved(
    dest_path: &Path,
    collision_policy: u8,
    is_directory: bool,
    reserved: &mut HashSet<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let taken = dest_path.exists() || dest_path.symlink_metadata().is_ok() || reserved.contains(dest_path);
    if !taken {
        reserved.insert(dest_path.to_path_buf());
        return Ok(Some(dest_path.to_path_buf()));
    }

    match collision_policy {
        ttzip_engine::types::TTZIP_COLLISION_POLICY_STOP => Err(format!(
            "Extraction stopped: destination item already exists: {}",
            dest_path.display()
        )),
        ttzip_engine::types::TTZIP_COLLISION_POLICY_SKIP => Ok(None),
        ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE => {
            reserved.insert(dest_path.to_path_buf());
            Ok(Some(dest_path.to_path_buf()))
        }
        _ => {
            let kind = if is_directory {
                ttzip_archive::fs::collision::ItemKind::Directory
            } else {
                ttzip_archive::fs::collision::ItemKind::File
            };
            let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));
            let file_name = dest_path.file_name().unwrap_or_default();
            for n in 2..=10000 {
                let cand_name = ttzip_archive::fs::collision::naming::candidate(file_name, kind, n);
                let cand_path = parent.join(cand_name);
                if !cand_path.exists() && cand_path.symlink_metadata().is_err() && !reserved.contains(&cand_path) {
                    reserved.insert(cand_path.clone());
                    return Ok(Some(cand_path));
                }
            }
            Err(format!("Failed to find unique name for existing item: {}", dest_path.display()))
        }
    }
}

/// Executes headless `extract` subcommand with default keep-both policy.
pub fn execute_extract(params: CliExtractParams<'_>) -> Result<(), String> {
    execute_extract_with_policy(params, None)
}

fn can_use_fast_extract_all(include: &[String], exclude: &[String], has_conflicts: bool, conflict_policy: u8) -> bool {
    include.is_empty()
        && exclude.is_empty()
        && (!has_conflicts || conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE)
}

fn write_resolved_entry<B, F>(
    dest_dir: &Path,
    rel_path: &str,
    is_directory: bool,
    conflict_policy: u8,
    dry_run: bool,
    fetch_bytes: F,
) -> Result<bool, String>
where
    B: AsRef<[u8]>,
    F: FnOnce() -> Result<B, String>,
{
    let out_path = sanitize_and_validate_path(dest_dir, rel_path)
        .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", rel_path, e))?;
    let Some(target_path) = resolve_destination_path(&out_path, conflict_policy, is_directory)? else {
        return Ok(false);
    };
    if is_directory {
        if !dry_run {
            create_dir_in_dir_nofollow(dest_dir, &target_path)
                .map_err(|e| format!("Failed to create directory {}: {:?}", target_path.display(), e))?;
        }
    } else if !dry_run {
        let bytes = fetch_bytes()?;
        let mut file = open_extracted_file_in_dir_nofollow(dest_dir, &target_path)
            .map_err(|e| format!("Failed to open {}: {:?}", target_path.display(), e))?;
        file.write_all(bytes.as_ref())
            .map_err(|e| format!("Failed to write {}: {}", target_path.display(), e))?;
    }
    Ok(true)
}

fn extract_zip_container(
    data: &[u8],
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    params: &CliExtractParams<'_>,
    conflict_policy: u8,
) -> Result<(usize, u64), String> {
    let archive = ZipArchive::open_slice(data).map_err(|e| format!("Failed to open ZIP archive: {:?}", e))?;
    let has_conflicts = archive
        .entries()
        .iter()
        .any(|entry| sanitize_and_validate_path(dest_dir, &entry.rel_path).is_ok_and(|p| p.exists()));
    if can_use_fast_extract_all(params.include, params.exclude, has_conflicts, conflict_policy) {
        let rep = archive.extract_all(dest_dir, options).map_err(|e| format!("ZIP extraction failed: {:?}", e))?;
        return Ok((rep.processed_entries_count, rep.total_uncompressed_bytes));
    }

    let (mut count, mut total_bytes) = (0, 0);
    for (idx, entry) in archive.entries().iter().enumerate() {
        if !pattern_matches(&entry.rel_path, params.include, params.exclude) {
            continue;
        }
        let extracted = write_resolved_entry(dest_dir, &entry.rel_path, entry.is_directory, conflict_policy, params.dry_run, || {
            archive.extract_entry_bytes(idx, params.password).map_err(|e| format!("Extraction failed for {}: {:?}", entry.rel_path, e))
        })?;
        if extracted {
            count += 1;
            if !entry.is_directory {
                total_bytes += entry.uncompressed_size;
            }
        }
    }
    Ok((count, total_bytes))
}

fn extract_sevenz_container(
    data: &[u8],
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    params: &CliExtractParams<'_>,
    conflict_policy: u8,
) -> Result<(usize, u64), String> {
    let archive = SevenZArchive::open_slice_with_password(data, params.password)
        .map_err(|e| format!("Failed to open 7z archive: {:?}", e))?;
    let has_conflicts = archive
        .info()
        .files
        .iter()
        .any(|file| sanitize_and_validate_path(dest_dir, &file.rel_path).is_ok_and(|p| p.exists()));
    if can_use_fast_extract_all(params.include, params.exclude, has_conflicts, conflict_policy) {
        let rep = archive.extract_all(dest_dir, options).map_err(|e| format!("7z extraction failed: {:?}", e))?;
        return Ok((rep.processed_entries_count, rep.total_uncompressed_bytes));
    }

    let (mut count, mut total_bytes) = (0, 0);
    for (idx, file) in archive.info().files.iter().enumerate() {
        if !pattern_matches(&file.rel_path, params.include, params.exclude) {
            continue;
        }
        let extracted = write_resolved_entry(dest_dir, &file.rel_path, file.is_directory, conflict_policy, params.dry_run, || {
            archive.extract_entry_bytes_stream(idx, params.password).map_err(|e| format!("Extraction failed for {}: {:?}", file.rel_path, e))
        })?;
        if extracted {
            count += 1;
            if !file.is_directory {
                total_bytes += archive.info().stream_sizes.get(idx).copied().unwrap_or(0);
            }
        }
    }
    Ok((count, total_bytes))
}

struct TarSliceExtractContext<'a> {
    dest_dir: &'a Path,
    options: &'a TTZipExtractOptions,
    params: &'a CliExtractParams<'a>,
    conflict_policy: u8,
    label: &'a str,
}

fn write_resolved_tar_link(
    ctx: &TarSliceExtractContext<'_>,
    entry: &TarEntry<'_>,
) -> Result<bool, String> {
    let out_path = sanitize_and_validate_path(ctx.dest_dir, &entry.path)
        .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", entry.path, e))?;
    let Some(target_path) = resolve_destination_path(&out_path, ctx.conflict_policy, false)? else {
        return Ok(false);
    };
    if ctx.params.dry_run {
        return Ok(true);
    }
    validate_no_intermediate_symlinks(ctx.dest_dir, &target_path)
        .map_err(|e| format!("Security validation blocked intermediate symlink for {}: {:?}", entry.path, e))?;
    if ctx.conflict_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE
        && target_path.symlink_metadata().is_ok_and(|m| !m.is_dir())
    {
        let _ = fs::remove_file(&target_path);
    }
    let link_target = entry
        .link_target
        .as_deref()
        .ok_or_else(|| format!("Missing link target for {}", entry.path))?;
    if entry.is_symlink {
        create_symlink_in_dir_nofollow(ctx.dest_dir, &target_path, link_target).map_err(|e| {
            format!("Failed to create symlink {}: {:?}", target_path.display(), e)
        })?;
    } else if entry.is_hardlink {
        let src_path = sanitize_and_validate_path(ctx.dest_dir, link_target).map_err(|e| {
            format!("Invalid hardlink target {} for {}: {:?}", link_target, entry.path, e)
        })?;
        create_hardlink_in_dir_nofollow(ctx.dest_dir, &target_path, &src_path).map_err(|e| {
            format!("Failed to create hardlink {}: {:?}", target_path.display(), e)
        })?;
    }
    Ok(true)
}

fn extract_tar_slice(
    tar: &TarArchive,
    ctx: &TarSliceExtractContext<'_>,
) -> Result<(usize, u64), String> {
    let has_conflicts = tar.entries().iter().any(|entry| {
        sanitize_and_validate_path(ctx.dest_dir, &entry.path)
            .map(|p| p.exists() || p.symlink_metadata().is_ok())
            .unwrap_or(false)
    });
    if can_use_fast_extract_all(ctx.params.include, ctx.params.exclude, has_conflicts, ctx.conflict_policy) {
        let rep = tar
            .extract_all(ctx.dest_dir, ctx.options)
            .map_err(|e| format!("{} extraction failed: {:?}", ctx.label, e))?;
        return Ok((rep.processed_entries_count, rep.total_uncompressed_bytes));
    }

    let mut count = 0;
    let mut total_bytes = 0;
    for (idx, entry) in tar.entries().iter().enumerate() {
        if !pattern_matches(&entry.path, ctx.params.include, ctx.params.exclude) {
            continue;
        }
        let extracted = if entry.is_symlink || entry.is_hardlink {
            write_resolved_tar_link(ctx, entry)?
        } else {
            write_resolved_entry(
                ctx.dest_dir,
                &entry.path,
                entry.is_directory,
                ctx.conflict_policy,
                ctx.params.dry_run,
                || {
                    tar.extract_entry_bytes(idx)
                        .map_err(|e| format!("Extraction failed for {}: {:?}", entry.path, e))
                },
            )?
        };
        if extracted {
            count += 1;
            if !entry.is_directory {
                total_bytes += entry.size;
            }
        }
    }
    Ok((count, total_bytes))
}

fn write_decompressed_stream_file(
    archive_path: &Path,
    dest_dir: &Path,
    decompressed: &[u8],
    suffixes: &[&str],
    conflict_policy: u8,
    codec_label: &str,
) -> Result<Option<(usize, u64)>, String> {
    let filename = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("decompressed.bin");
    let mut inner_name = filename;
    for suf in suffixes {
        if let Some(stripped) = filename.strip_suffix(suf) {
            inner_name = stripped;
            break;
        }
    }
    let out_file_path = sanitize_and_validate_path(dest_dir, inner_name)
        .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", inner_name, e))?;
    let Some(target_path) = resolve_destination_path(&out_file_path, conflict_policy, false)? else {
        return Ok(None);
    };
    let mut out_file = open_extracted_file_in_dir_nofollow(dest_dir, &target_path)
        .map_err(|e| format!("Failed to write decompressed {} file: {:?}", codec_label, e))?;
    out_file
        .write_all(decompressed)
        .map_err(|e| format!("Failed to write decompressed {} data: {}", codec_label, e))?;
    Ok(Some((1, decompressed.len() as u64)))
}

fn extract_stream_or_tar(
    format: ContainerFormat,
    data: &[u8],
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    params: &CliExtractParams<'_>,
    conflict_policy: u8,
) -> Result<Option<(usize, u64)>, String> {
    match format {
        ContainerFormat::Snappy => {
            let decompressed = snappy_frame_decode_to_vec(data, 1024 * 1024 * 512)
                .map_err(|e| format!("Failed to decompress Snappy stream: {:?}", e))?;
            if let Ok(tar) = TarArchive::open_slice(&decompressed) {
                let ctx = TarSliceExtractContext {
                    dest_dir,
                    options,
                    params,
                    conflict_policy,
                    label: "Snappy TAR",
                };
                extract_tar_slice(&tar, &ctx).map(Some)
            } else {
                write_decompressed_stream_file(
                    params.archive_path,
                    dest_dir,
                    &decompressed,
                    &[".sz", ".snappy"],
                    conflict_policy,
                    "Snappy",
                )
            }
        }
        ContainerFormat::Brotli | ContainerFormat::TarBrotli => {
            let decompressed = brotli_decompress_to_vec(data, 1024 * 1024 * 512)
                .map_err(|e| format!("Failed to decompress Brotli stream: {:?}", e))?;
            if format == ContainerFormat::TarBrotli || TarArchive::open_slice(&decompressed).is_ok() {
                let tar = TarArchive::open_slice(&decompressed)
                    .map_err(|_| "Failed to parse inner TAR archive".to_string())?;
                let ctx = TarSliceExtractContext {
                    dest_dir,
                    options,
                    params,
                    conflict_policy,
                    label: "Brotli TAR",
                };
                extract_tar_slice(&tar, &ctx).map(Some)
            } else {
                write_decompressed_stream_file(
                    params.archive_path,
                    dest_dir,
                    &decompressed,
                    &[".br"],
                    conflict_policy,
                    "Brotli",
                )
            }
        }
        ContainerFormat::Unknown => Err(format!(
            "Cannot extract unrecognized archive: {}",
            params.archive_path.display()
        )),
        _ => extract_engine_container(dest_dir, options, params, conflict_policy).map(Some),
    }
}

struct PlannedEngineEntry {
    entry_idx: usize,
    rel_path: String,
    target_path: PathBuf,
    is_dir: bool,
    uncompressed_size: u64,
    is_duplicate_path: bool,
}

fn create_exclusive_stage_dir(dest_dir: &Path) -> Result<PathBuf, String> {
    for _ in 0..1024 {
        let seq = EXTRACT_STAGE_SEQ.fetch_add(1, Ordering::Relaxed);
        let stage_dir = dest_dir.join(format!(".ttzip_extract_stage_{}_{}", std::process::id(), seq));
        match fs::create_dir(&stage_dir) {
            Ok(()) => return Ok(stage_dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!("Failed to create staging directory {}: {}", stage_dir.display(), e));
            }
        }
    }
    Err(format!("Failed to allocate exclusive staging directory in {}", dest_dir.display()))
}

fn open_jail_parent_fd<'a>(
    jail: &DirJail,
    comps: &'a [String],
    create_dirs: bool,
) -> Result<(Option<std::os::unix::io::OwnedFd>, &'a str), String> {
    use std::os::unix::io::AsRawFd;
    let (leaf, parents) = comps.split_last().ok_or_else(|| "Empty path components".to_string())?;
    if create_dirs && !parents.is_empty() {
        jail.create_dir_all_components(parents)
            .map_err(|e| format!("Failed to create parent directory: {:?}", e))?;
    }
    let ops = LibcFsOps;
    let mut held: Option<std::os::unix::io::OwnedFd> = None;
    for seg in parents {
        let cur_fd = held.as_ref().map_or(jail.root_fd(), |fd| fd.as_raw_fd());
        let next = ops
            .openat_dir_nofollow(cur_fd, Path::new(seg))
            .map_err(|e| format!("Failed to open directory segment {}: {}", seg, e))?;
        held = Some(next);
    }
    Ok((held, leaf.as_str()))
}

fn move_staged_entry(
    stage_jail: &DirJail,
    dest_jail: &DirJail,
    rel_path: &str,
    target_path: &Path,
    allow_replace: bool,
) -> Result<(), String> {
    use std::os::unix::io::AsRawFd;
    let src_comps = DirJail::extract_clean_components(rel_path)
        .map_err(|e| format!("Invalid staged path {}: {:?}", rel_path, e))?;
    let dst_comps = dest_jail
        .resolve_path_components(target_path)
        .map_err(|e| format!("Invalid target path {}: {:?}", target_path.display(), e))?;
    let (src_parent, src_leaf) = open_jail_parent_fd(stage_jail, &src_comps, false)?;
    let (dst_parent, dst_leaf) = open_jail_parent_fd(dest_jail, &dst_comps, true)?;
    let from_fd = src_parent.as_ref().map_or(stage_jail.root_fd(), |fd| fd.as_raw_fd());
    let to_fd = dst_parent.as_ref().map_or(dest_jail.root_fd(), |fd| fd.as_raw_fd());
    let (from_name, to_name) = (Path::new(src_leaf), Path::new(dst_leaf));
    let ops = LibcFsOps;
    if let Some(meta) = ops
        .fstatat_nofollow(to_fd, to_name)
        .map_err(|e| format!("Failed to stat {}: {}", target_path.display(), e))?
    {
        if !allow_replace {
            return Err(format!("Refusing to overwrite existing destination: {}", target_path.display()));
        }
        let rm = if meta.kind == ItemKind::Directory {
            ops.remove_tree(to_fd, to_name)
        } else {
            ops.unlinkat(to_fd, to_name, false)
        };
        rm.map_err(|e| format!("Failed to replace {}: {}", target_path.display(), e))?;
    }
    ops.renameatx(from_fd, from_name, to_fd, to_name, RENAME_EXCL | RENAME_NOFOLLOW_ANY)
        .or_else(|e| {
            if matches!(e.raw_os_error(), Some(libc::ENOTSUP | libc::EOPNOTSUPP | libc::ENOSYS)) {
                ops.renameatx(from_fd, from_name, to_fd, to_name, 0)
            } else {
                Err(e)
            }
        })
        .map_err(|e| format!("Failed to move staged entry {} to {}: {}", rel_path, target_path.display(), e))
}

fn extract_duplicate_entry_by_index(
    archive_path: &Path,
    dest_dir: &Path,
    password: Option<&str>,
    item: &PlannedEngineEntry,
) -> Result<(), String> {
    let bytes = extract_single_entry_memory(
        archive_path,
        Some(&item.rel_path),
        item.entry_idx as i64,
        password,
    )
    .map_err(|e| format!("Extraction failed for {}: {:?}", item.rel_path, e))?;
    let mut file = open_extracted_file_in_dir_nofollow(dest_dir, &item.target_path)
        .map_err(|e| format!("Failed to open {}: {:?}", item.target_path.display(), e))?;
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write {}: {}", item.target_path.display(), e))
}

fn commit_staged_batch(
    archive_path: &Path,
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    planned: &[PlannedEngineEntry],
    staged_rel_paths: &[String],
) -> Result<(), String> {
    let dest_jail = DirJail::open(dest_dir)
        .map_err(|e| format!("Failed to open destination jail {}: {:?}", dest_dir.display(), e))?;
    let stage_dir = create_exclusive_stage_dir(dest_dir)?;
    let selection = EntrySelection::new(staged_rel_paths, false);
    let stage_root = StagingRoot(stage_dir.clone());
    let mut stage_opts = *options;
    stage_opts.collision_policy = ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE;
    stage_opts.dry_run = false;
    let allow_replace = options.collision_policy == ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE;

    let res = extract_selected_entries(archive_path, &selection, &stage_root, &stage_opts)
        .map_err(|e| format!("Selected extraction failed for {}: {:?}", archive_path.display(), e))
        .and_then(|_| {
            let stage_jail = DirJail::open(&stage_dir)
                .map_err(|e| format!("Failed to open stage jail {}: {:?}", stage_dir.display(), e))?;
            for item in planned.iter().filter(|item| !item.is_dir && !item.is_duplicate_path) {
                move_staged_entry(&stage_jail, &dest_jail, &item.rel_path, &item.target_path, allow_replace)?;
            }
            Ok(())
        });
    let _ = fs::remove_dir_all(&stage_dir);
    res
}

fn stage_and_commit_selected_entries(
    archive_path: &Path,
    dest_dir: &Path,
    password: Option<&str>,
    options: &TTZipExtractOptions,
    planned: &[PlannedEngineEntry],
) -> Result<(), String> {
    for item in planned.iter().filter(|item| item.is_dir) {
        create_dir_in_dir_nofollow(dest_dir, &item.target_path)
            .map_err(|e| format!("Failed to create directory {}: {:?}", item.target_path.display(), e))?;
    }
    let staged_rel_paths: Vec<String> = planned
        .iter()
        .filter(|item| !item.is_dir && !item.is_duplicate_path)
        .map(|item| item.rel_path.clone())
        .collect();
    if !staged_rel_paths.is_empty() {
        commit_staged_batch(archive_path, dest_dir, options, planned, &staged_rel_paths)?;
    }
    for item in planned.iter().filter(|item| !item.is_dir && item.is_duplicate_path) {
        extract_duplicate_entry_by_index(archive_path, dest_dir, password, item)?;
    }
    Ok(())
}

fn plan_engine_entries(
    dest_dir: &Path,
    entries: &[(String, bool, u64)],
    params: &CliExtractParams<'_>,
    conflict_policy: u8,
    path_counts: &HashMap<String, usize>,
) -> Result<Vec<PlannedEngineEntry>, String> {
    let mut planned = Vec::new();
    let mut reserved = HashSet::new();
    for (entry_idx, (rel_path, is_directory, uncompressed_size)) in entries.iter().enumerate() {
        if !pattern_matches(rel_path, params.include, params.exclude) {
            continue;
        }
        let out_path = sanitize_and_validate_path(dest_dir, rel_path)
            .map_err(|e| format!("Security validation blocked path traversal for {}: {:?}", rel_path, e))?;
        if let Some(target_path) =
            resolve_destination_path_with_reserved(&out_path, conflict_policy, *is_directory, &mut reserved)?
        {
            let norm = rel_path.trim_start_matches("./").trim_start_matches('/');
            let is_duplicate_path = !*is_directory && path_counts.get(norm).copied().unwrap_or(0) > 1;
            planned.push(PlannedEngineEntry {
                entry_idx,
                rel_path: rel_path.clone(),
                target_path,
                is_dir: *is_directory,
                uncompressed_size: *uncompressed_size,
                is_duplicate_path,
            });
        }
    }
    Ok(planned)
}

fn extract_engine_container(
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    params: &CliExtractParams<'_>,
    conflict_policy: u8,
) -> Result<(usize, u64), String> {
    let mut entries = Vec::new();
    inspect_archive(params.archive_path, params.password, true, |entry| {
        entries.push((entry.path.to_string(), entry.is_directory, entry.uncompressed_size));
        true
    })
    .map_err(|e| format!("Failed to inspect archive {}: {:?}", params.archive_path.display(), e))?;

    let mut path_counts: HashMap<String, usize> = HashMap::new();
    for (rel_path, is_dir, _) in &entries {
        if !*is_dir {
            let norm = rel_path.trim_start_matches("./").trim_start_matches('/').to_string();
            *path_counts.entry(norm).or_insert(0) += 1;
        }
    }
    let has_duplicate_paths = path_counts.values().any(|&c| c > 1);
    let has_conflicts = has_duplicate_paths
        || entries.iter().any(|(rel_path, _, _)| {
            sanitize_and_validate_path(dest_dir, rel_path).is_ok_and(|p| p.symlink_metadata().is_ok())
        });

    if can_use_fast_extract_all(params.include, params.exclude, has_conflicts, conflict_policy) {
        let mut total_bytes: u64 = entries.iter().filter(|(_, is_dir, _)| !*is_dir).map(|(_, _, sz)| *sz).sum();
        if !params.dry_run {
            let staging = StagingRoot(dest_dir.to_path_buf());
            let written = extract_archive_with_metrics(params.archive_path, &staging, options)
                .map_err(|e| format!("Archive extraction failed: {:?}", e))?;
            total_bytes = written.max(total_bytes);
        }
        return Ok((entries.len(), total_bytes));
    }

    let planned = plan_engine_entries(dest_dir, &entries, params, conflict_policy, &path_counts)?;
    let count = planned.len();
    let total_bytes: u64 = planned.iter().filter(|item| !item.is_dir).map(|item| item.uncompressed_size).sum();

    if !params.dry_run {
        stage_and_commit_selected_entries(params.archive_path, dest_dir, params.password, options, &planned)?;
    }

    Ok((count, total_bytes))
}

fn dispatch_container_extraction(
    format: ContainerFormat,
    data: &[u8],
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    params: &CliExtractParams<'_>,
    conflict_policy: u8,
) -> Result<Option<(usize, u64)>, String> {
    match format {
        ContainerFormat::Zip => {
            extract_zip_container(data, dest_dir, options, params, conflict_policy).map(Some)
        }
        ContainerFormat::SevenZip => {
            extract_sevenz_container(data, dest_dir, options, params, conflict_policy).map(Some)
        }
        ContainerFormat::Tar => {
            let archive = TarArchive::open_slice(data)
                .map_err(|e| format!("Failed to open TAR archive: {:?}", e))?;
            let ctx = TarSliceExtractContext {
                dest_dir,
                options,
                params,
                conflict_policy,
                label: "TAR",
            };
            extract_tar_slice(&archive, &ctx).map(Some)
        }
        _ => extract_stream_or_tar(format, data, dest_dir, options, params, conflict_policy),
    }
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

    let (volumes, data) = read_archive_data_auto(params.archive_path)?;
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

    let extracted = if volumes.len() > 1 {
        extract_engine_container(dest_dir, &options, &params, conflict_policy).map(Some)?
    } else {
        dispatch_container_extraction(format, &data, dest_dir, &options, &params, conflict_policy)?
    };
    let Some((entries_count, total_uncompressed_bytes)) = extracted else {
        return Ok(());
    };

    if params.verbose {
        println!(
            "Extracted {} entries ({}) to {} in {:.2?}",
            entries_count,
            format_bytes(total_uncompressed_bytes),
            dest_dir.display(),
            start_time.elapsed()
        );
    }

    Ok(())
}
