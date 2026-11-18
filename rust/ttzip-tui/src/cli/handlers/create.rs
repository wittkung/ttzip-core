// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: create (multi-format, filter-aware, dry-run, and split volumes).

use super::extract::glob_match;
use super::split::parse_size_bytes;
use crate::cli::args::{VfsNodeContractDto, VfsTreeContractDto};
use crate::cli::format::format_bytes;
use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use ttzip_engine::archive::unified::create::create_archive_explicit_with_files_detailed_full_opts_outcome_with_cancel;
use ttzip_engine::archive::unified::inspect::inspect_archive;
use ttzip_engine::types::{
    with_extended_create_options, TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions,
    TTZipEncryptionMethod, TTZipExtendedCreateOptions,
};
use ttzip_engine::uniffi_api::extraction_session::{
    CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession, OutcomeStatus,
    ResolutionSet,
};

struct FormatSpec {
    flags: &'static [&'static str],
    extensions: &'static [&'static str],
    format: TTZipArchiveFormat,
    label: &'static str,
}

const FORMAT_SPECS: &[FormatSpec] = &[
    FormatSpec {
        flags: &["tar.gz", "tgz"],
        extensions: &[".tar.gz", ".tgz"],
        format: TTZipArchiveFormat::TarGz,
        label: "TAR.GZ",
    },
    FormatSpec {
        flags: &["tar.zst", "tar.zstd", "tzst"],
        extensions: &[".tar.zst", ".tar.zstd", ".tzst"],
        format: TTZipArchiveFormat::TarZstd,
        label: "TAR.ZST",
    },
    FormatSpec {
        flags: &["tar.xz", "txz"],
        extensions: &[".tar.xz", ".txz"],
        format: TTZipArchiveFormat::TarXz,
        label: "TAR.XZ",
    },
    FormatSpec {
        flags: &["tar.bz2", "tbz2", "tbz"],
        extensions: &[".tar.bz2", ".tbz2", ".tbz"],
        format: TTZipArchiveFormat::TarBz2,
        label: "TAR.BZ2",
    },
    FormatSpec {
        flags: &["tar.lz4", "tlz4"],
        extensions: &[".tar.lz4", ".tlz4"],
        format: TTZipArchiveFormat::TarLz4,
        label: "TAR.LZ4",
    },
    FormatSpec {
        flags: &["tar.br", "tar.brotli", "tbr"],
        extensions: &[".tar.br", ".tbr"],
        format: TTZipArchiveFormat::TarBrotli,
        label: "TAR.BR",
    },
    FormatSpec {
        flags: &["tar"],
        extensions: &[".tar"],
        format: TTZipArchiveFormat::Tar,
        label: "TAR",
    },
    FormatSpec {
        flags: &["7z", "sevenzip", "cb7"],
        extensions: &[".7z", ".cb7"],
        format: TTZipArchiveFormat::SevenZip,
        label: "7Z",
    },
    FormatSpec {
        flags: &["zip", "cbz", "jar", "apk"],
        extensions: &[".zip", ".cbz", ".jar", ".apk"],
        format: TTZipArchiveFormat::Zip,
        label: "ZIP",
    },
    FormatSpec {
        flags: &["gz", "gzip"],
        extensions: &[".gz"],
        format: TTZipArchiveFormat::Gzip,
        label: "GZIP",
    },
    FormatSpec {
        flags: &["zst", "zstd"],
        extensions: &[".zst", ".zstd"],
        format: TTZipArchiveFormat::Zstd,
        label: "ZSTD",
    },
    FormatSpec {
        flags: &["xz"],
        extensions: &[".xz"],
        format: TTZipArchiveFormat::Xz,
        label: "XZ",
    },
    FormatSpec {
        flags: &["bz2", "bzip2"],
        extensions: &[".bz2"],
        format: TTZipArchiveFormat::Bzip2,
        label: "BZIP2",
    },
    FormatSpec {
        flags: &["lz4"],
        extensions: &[".lz4"],
        format: TTZipArchiveFormat::Lz4,
        label: "LZ4",
    },
    FormatSpec {
        flags: &["br", "brotli"],
        extensions: &[".br"],
        format: TTZipArchiveFormat::Brotli,
        label: "BROTLI",
    },
];

/// Parameters for headless archive creation.
pub struct CliCreateParams<'a> {
    pub archive_path: &'a Path,
    pub sources: &'a [PathBuf],
    pub format_opt: Option<&'a str>,
    pub level: u8,
    pub password: Option<&'a str>,
    pub threads: u32,
    pub volume_size: Option<&'a str>,
    pub dry_run: bool,
    pub include: &'a [String],
    pub exclude: &'a [String],
    pub silent: bool,
}

/// Executes headless `create` subcommand with optional multi-volume chunk splitting.
pub fn execute_create(
    archive_path: &Path,
    sources: &[PathBuf],
    format_opt: Option<&str>,
    level: u8,
    password: Option<&str>,
    threads: u32,
    volume_size: Option<&str>,
) -> Result<(), String> {
    execute_create_with_params(CliCreateParams {
        archive_path,
        sources,
        format_opt,
        level,
        password,
        threads,
        volume_size,
        dry_run: false,
        include: &[],
        exclude: &[],
        silent: false,
    })
}

/// Executes archive creation silently with zero stdout writes (used by MCP and programmatic bridges).
pub fn execute_create_silent(
    archive_path: &Path,
    sources: &[PathBuf],
    format_opt: Option<&str>,
    level: u8,
    password: Option<&str>,
    threads: u32,
    volume_size: Option<&str>,
) -> Result<(), String> {
    execute_create_with_params(CliCreateParams {
        archive_path,
        sources,
        format_opt,
        level,
        password,
        threads,
        volume_size,
        dry_run: false,
        include: &[],
        exclude: &[],
        silent: true,
    })
}

fn resolve_target_format(
    archive_path: &Path,
    format_opt: Option<&str>,
) -> Result<(TTZipArchiveFormat, &'static str), String> {
    if let Some(fmt_raw) = format_opt {
        let clean = fmt_raw.trim().trim_start_matches('.').to_ascii_lowercase();
        let found = FORMAT_SPECS
            .iter()
            .find(|spec| spec.flags.contains(&clean.as_str()));
        return found
            .map(|spec| (spec.format, spec.label))
            .ok_or_else(|| format!("Unsupported format: {}", fmt_raw));
    }
    let file_name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let stripped = file_name.strip_suffix(".001").unwrap_or(&file_name);
    if let Some(spec) = FORMAT_SPECS
        .iter()
        .find(|spec| spec.extensions.iter().any(|ext| stripped.ends_with(ext)))
    {
        return Ok((spec.format, spec.label));
    }
    Ok((TTZipArchiveFormat::Zip, "ZIP"))
}

fn map_cli_compression_level(level: u8) -> TTZipCompressionLevel {
    match level {
        0 => TTZipCompressionLevel::Store,
        1 => TTZipCompressionLevel::Fastest,
        2..=4 => TTZipCompressionLevel::Fast,
        5..=6 => TTZipCompressionLevel::Normal,
        7..=9 => TTZipCompressionLevel::Maximum,
        _ => TTZipCompressionLevel::Ultra,
    }
}

fn entry_matches_filters(rel_path: &str, include: &[String], exclude: &[String]) -> bool {
    let clean = rel_path.trim_start_matches('/');
    let base_name = Path::new(clean)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(clean);
    if !include.is_empty()
        && !include
            .iter()
            .any(|pat| glob_match(pat, clean) || glob_match(pat, base_name))
    {
        return false;
    }
    if !exclude.is_empty()
        && exclude
            .iter()
            .any(|pat| glob_match(pat, clean) || glob_match(pat, base_name))
    {
        return false;
    }
    true
}

fn collect_filtered_source_entries(
    sources: &[PathBuf],
    include: &[String],
    exclude: &[String],
) -> Result<(Vec<(PathBuf, String)>, u64), String> {
    if sources.is_empty() {
        return Err("No source files specified for archive creation".to_string());
    }
    let mut planned = Vec::new();
    let mut total_uncompressed = 0u64;
    for src in sources {
        if fs::symlink_metadata(src).is_err() {
            return Err(format!("Source file/directory not found: {}", src.display()));
        }
        let base_parent = src.parent().unwrap_or(src);
        let mut stack = vec![src.clone()];
        while let Some(curr) = stack.pop() {
            let meta = fs::symlink_metadata(&curr)
                .map_err(|e| format!("Failed to read metadata for {}: {}", curr.display(), e))?;
            let rel = curr
                .strip_prefix(base_parent)
                .unwrap_or(&curr)
                .to_string_lossy()
                .replace('\\', "/");
            if meta.is_dir() {
                let mut children: Vec<PathBuf> = fs::read_dir(&curr)
                    .map_err(|e| format!("Failed to read directory {}: {}", curr.display(), e))?
                    .flatten()
                    .map(|e| e.path())
                    .collect();
                if children.is_empty() {
                    if !rel.is_empty() && entry_matches_filters(&rel, include, exclude) {
                        planned.push((curr, rel));
                    }
                } else {
                    children.sort_by(|a, b| b.cmp(a));
                    stack.extend(children);
                }
            } else if entry_matches_filters(&rel, include, exclude) {
                let sz = if meta.file_type().is_symlink() { 0 } else { meta.len() };
                total_uncompressed = total_uncompressed.saturating_add(sz);
                planned.push((curr, rel));
            }
        }
    }
    if planned.is_empty() {
        return Err("No source entries matched the specified include/exclude filters".to_string());
    }
    Ok((planned, total_uncompressed))
}

fn resolve_split_destination(archive_path: &Path, split_size: u64) -> PathBuf {
    if split_size > 0 && archive_path.extension().and_then(|s| s.to_str()) != Some("001") {
        let fname = archive_path
            .file_name()
            .map(|n| format!("{}.001", n.to_string_lossy()))
            .unwrap_or_else(|| "archive.001".to_string());
        archive_path.with_file_name(fname)
    } else {
        archive_path.to_path_buf()
    }
}

/// Executes archive creation using bundled parameter struct.
pub fn execute_create_with_params(params: CliCreateParams) -> Result<(), String> {
    let (target_format, format_label) =
        resolve_target_format(params.archive_path, params.format_opt)?;
    let (planned_entries, total_uncompressed_bytes) =
        collect_filtered_source_entries(params.sources, params.include, params.exclude)?;
    if params.dry_run {
        if !params.silent {
            println!(
                "[DRY-RUN] Planned {} archive {} with {} entries ({}):",
                format_label,
                params.archive_path.display(),
                planned_entries.len(),
                format_bytes(total_uncompressed_bytes)
            );
            for (_, rel) in &planned_entries {
                println!("  + {}", rel);
            }
        }
        return Ok(());
    }

    let split_size = params.volume_size.map(parse_size_bytes).transpose()?.unwrap_or(0);
    let start_time = Instant::now();
    let password_c = params
        .password
        .filter(|p| !p.is_empty())
        .map(|p| CString::new(p).unwrap_or_default());
    let options = TTZipCreateOptions {
        format: target_format,
        level: map_cli_compression_level(params.level),
        encryption: if password_c.is_some() {
            TTZipEncryptionMethod::Aes256
        } else {
            TTZipEncryptionMethod::None
        },
        password: password_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: params.threads.max(1),
        solid_block_size_mb: 64,
        ..Default::default()
    };
    let effective_dest = resolve_split_destination(params.archive_path, split_size);
    let encrypt_names =
        (target_format == TTZipArchiveFormat::SevenZip && password_c.is_some()).then_some(true);
    let ext_opts = TTZipExtendedCreateOptions {
        dictionary_size_mb: 0,
        enable_ldm: false,
        raw_level: Some(params.level),
    };
    let commit_out = with_extended_create_options(ext_opts, || {
        create_archive_explicit_with_files_detailed_full_opts_outcome_with_cancel(
            &planned_entries,
            &effective_dest,
            &options,
            split_size,
            encrypt_names,
            0,
            || false,
        )
    })
    .map_err(|e| format!("Failed to create {} archive: {:?}", format_label, e))?;

    if !params.silent {
        print_create_summary(
            format_label,
            params.archive_path,
            &commit_out.committed_files,
            planned_entries.len(),
            total_uncompressed_bytes,
            split_size > 0,
            start_time.elapsed(),
        );
    }
    Ok(())
}

fn print_create_summary(
    format_label: &str,
    archive_path: &Path,
    committed_files: &[PathBuf],
    total_entries: usize,
    total_uncompressed_bytes: u64,
    is_split: bool,
    elapsed: std::time::Duration,
) {
    let total_compressed_bytes: u64 = committed_files
        .iter()
        .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    if is_split {
        println!(
            "Created {} multi-volume archive {} ({} volumes) with {} entries ({} -> {}) in {:.2?}",
            format_label,
            archive_path.display(),
            committed_files.len(),
            total_entries,
            format_bytes(total_uncompressed_bytes),
            format_bytes(total_compressed_bytes),
            elapsed
        );
        for vol in committed_files {
            println!("  - {}", vol.display());
        }
    } else {
        println!(
            "Created {} archive {} with {} entries ({} -> {}) in {:.2?}",
            format_label,
            archive_path.display(),
            total_entries,
            format_bytes(total_uncompressed_bytes),
            format_bytes(total_compressed_bytes),
            elapsed
        );
    }
}

/// Returns `true` when `path` is a compressed TAR, stream, or multi-volume format handled by the unified engine.
pub fn prefers_unified_archive_handler(path: &Path) -> bool {
    if ttzip_engine::archive::split::detect_volume_chain(path)
        .is_ok_and(|volumes| volumes.len() > 1)
    {
        return true;
    }
    matches!(
        resolve_target_format(path, None),
        Ok((
            TTZipArchiveFormat::TarGz
                | TTZipArchiveFormat::TarZstd
                | TTZipArchiveFormat::TarXz
                | TTZipArchiveFormat::TarBz2
                | TTZipArchiveFormat::TarLz4
                | TTZipArchiveFormat::Gzip
                | TTZipArchiveFormat::Zstd
                | TTZipArchiveFormat::Xz
                | TTZipArchiveFormat::Bzip2
                | TTZipArchiveFormat::Lz4,
            _
        ))
    )
}

/// Fallback unified inspector for compressed TAR and stream container formats in `ttzip_cli list`.
pub fn execute_list_unified(
    archive_path: &Path,
    password: Option<&str>,
    json: bool,
    include: &[String],
    exclude: &[String],
) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }
    super::check::ensure_utf8_c_locale();
    let mut nodes = Vec::new();
    inspect_archive(archive_path, password, false, |entry| {
        let rel = entry
            .path
            .trim_start_matches("./")
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string();
        if !rel.is_empty() && entry_matches_filters(&rel, include, exclude) {
            let name = Path::new(&rel)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| rel.clone());
            nodes.push(VfsNodeContractDto {
                name,
                relative_path: rel,
                is_directory: entry.is_directory,
                uncompressed_size: entry.uncompressed_size,
                compressed_size: entry.compressed_size,
                crc32: entry.crc32,
                is_encrypted: entry.is_encrypted,
                match_indices: None,
            });
        }
        true
    })
    .map_err(|e| format!("Failed to inspect archive {}: {:?}", archive_path.display(), e))?;

    let total_uncompressed: u64 = nodes
        .iter()
        .filter(|n| !n.is_directory)
        .map(|n| n.uncompressed_size)
        .sum();
    if json {
        let contract = VfsTreeContractDto {
            root_path: archive_path.to_string_lossy().to_string(),
            total_entries_count: nodes.len(),
            total_uncompressed_bytes: total_uncompressed,
            nodes,
        };
        let json_str = serde_json::to_string_pretty(&contract)
            .map_err(|e| format!("Failed to serialize contract JSON: {}", e))?;
        println!("{}", json_str);
    } else {
        println!(
            "Archive: {} (Entries: {}, Uncompressed: {})",
            archive_path.display(),
            nodes.len(),
            format_bytes(total_uncompressed)
        );
        for n in &nodes {
            println!("  {} ({})", n.relative_path, format_bytes(n.uncompressed_size));
        }
    }
    Ok(())
}

fn resolve_unified_conflict_policy(on_conflict: Option<&str>) -> Result<CollisionPolicy, String> {
    let code = on_conflict
        .map(super::extract::parse_conflict_policy)
        .transpose()?
        .unwrap_or(ttzip_engine::types::TTZIP_COLLISION_POLICY_KEEP_BOTH);
    Ok(match code {
        ttzip_engine::types::TTZIP_COLLISION_POLICY_REPLACE => CollisionPolicy::Replace,
        ttzip_engine::types::TTZIP_COLLISION_POLICY_SKIP => CollisionPolicy::Skip,
        ttzip_engine::types::TTZIP_COLLISION_POLICY_STOP => CollisionPolicy::Stop,
        _ => CollisionPolicy::KeepBoth,
    })
}

fn collect_unified_filter_targets(
    archive_path: &Path,
    password: Option<&str>,
    include: &[String],
    exclude: &[String],
) -> Result<Option<Vec<String>>, String> {
    if include.is_empty() && exclude.is_empty() {
        return Ok(None);
    }
    let mut matched = Vec::new();
    let mut excluded = Vec::new();
    inspect_archive(archive_path, password, false, |entry| {
        let rel = entry
            .path
            .trim_start_matches("./")
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string();
        if !rel.is_empty() {
            if entry_matches_filters(&rel, include, exclude) {
                matched.push((rel, entry.is_directory));
            } else {
                excluded.push(rel);
            }
        }
        true
    })
    .map_err(|e| format!("Failed to inspect archive {}: {:?}", archive_path.display(), e))?;

    let mut targets = Vec::new();
    for (rel, is_dir) in &matched {
        let prefix = format!("{}/", rel);
        let has_excluded_descendant = *is_dir && excluded.iter().any(|ex| ex.starts_with(&prefix));
        if !has_excluded_descendant {
            targets.push(rel.clone());
        }
    }
    Ok(Some(targets))
}

fn run_unified_extraction_session(
    archive_path: &Path,
    request: ExtractionRequest,
) -> Result<(), String> {
    let policy = request.policy;
    let session = ExtractionSession::new(request)
        .map_err(|e| format!("Unified extraction failed for {}: {}", archive_path.display(), e))?;
    if session
        .preflight()
        .map_err(|e| format!("Unified extraction failed for {}: {}", archive_path.display(), e))?
        .is_some()
        && policy == CollisionPolicy::Stop
    {
        let _ = session.abort();
        return Err(format!(
            "Extraction stopped: destination item already exists when extracting {}",
            archive_path.display()
        ));
    }
    let stage_rep = session
        .stage(None, None)
        .map_err(|e| format!("Unified extraction failed for {}: {}", archive_path.display(), e))?;
    if stage_rep.post_stage_conflicts.is_some() && policy == CollisionPolicy::Stop {
        let _ = session.abort();
        return Err(format!(
            "Extraction stopped: destination item already exists when extracting {}",
            archive_path.display()
        ));
    }
    let resolutions = ResolutionSet {
        default_action: policy.default_action(),
        overrides: Vec::new(),
    };
    match session
        .commit(resolutions)
        .map_err(|e| format!("Unified extraction failed for {}: {}", archive_path.display(), e))?
    {
        CommitResult::Finished { outcome } if outcome.status != OutcomeStatus::Stopped => Ok(()),
        _ => {
            let _ = session.abort();
            Err(format!(
                "Extraction stopped: destination item already exists when extracting {}",
                archive_path.display()
            ))
        }
    }
}

/// Fallback unified extractor for compressed TAR and stream container formats in `ttzip_cli extract`.
pub fn execute_extract_unified(
    archive_path: &Path,
    output_dir: Option<&Path>,
    password: Option<&str>,
    dry_run: bool,
    include: &[String],
    exclude: &[String],
) -> Result<(), String> {
    execute_extract_unified_with_policy(
        archive_path,
        output_dir,
        password,
        dry_run,
        include,
        exclude,
        None,
    )
}

/// Fallback unified extractor with explicit collision policy and include/exclude filter support.
pub fn execute_extract_unified_with_policy(
    archive_path: &Path,
    output_dir: Option<&Path>,
    password: Option<&str>,
    dry_run: bool,
    include: &[String],
    exclude: &[String],
    on_conflict: Option<&str>,
) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }
    let policy = resolve_unified_conflict_policy(on_conflict)?;
    super::check::ensure_utf8_c_locale();
    if dry_run {
        return execute_list_unified(archive_path, password, false, include, exclude);
    }
    let entry_filter = collect_unified_filter_targets(archive_path, password, include, exclude)?;
    if let Some(ref targets) = entry_filter
        && targets.is_empty()
    {
        return Ok(());
    }
    let dest_dir = output_dir.unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create output directory {}: {}", dest_dir.display(), e))?;
    let request = ExtractionRequest {
        archive_path: archive_path.to_string_lossy().to_string(),
        destination_root: dest_dir.to_string_lossy().to_string(),
        policy,
        password: password.map(str::to_string),
        entry_filter,
        root_at_common_parent: false,
    };
    run_unified_extraction_session(archive_path, request)
}

#[cfg(test)]
/// RAII guard wrapping SplitVolumeWriter to purge partial volume slices upon error or panic.
pub(crate) struct GuardedSplitWriter {
    writer: ttzip_engine::archive::split::SplitVolumeWriter,
    committed: bool,
}

#[cfg(test)]
impl GuardedSplitWriter {
    pub(crate) fn new(writer: ttzip_engine::archive::split::SplitVolumeWriter) -> Self {
        Self {
            writer,
            committed: false,
        }
    }

    pub(crate) fn commit(mut self) -> std::io::Result<Vec<PathBuf>> {
        self.committed = true;
        self.writer.close()
    }
}

#[cfg(test)]
impl std::io::Write for GuardedSplitWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
impl Drop for GuardedSplitWriter {
    fn drop(&mut self) {
        if !self.committed {
            self.writer.cancel_and_cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use ttzip_engine::archive::split::{SplitVolumeWriter, VolumeNamingScheme};

    #[test]
    fn test_guarded_split_writer_cleanup_on_error() {
        let temp_dir = tempdir().expect("tempdir failed");
        let base_path = temp_dir.path().join("test_cleanup.zip");
        {
            let split_writer =
                SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension)
                    .expect("SplitVolumeWriter new failed");
            let mut guarded = GuardedSplitWriter::new(split_writer);
            guarded.write_all(&[0xAAu8; 250]).expect("write_all failed");
        }
        assert!(!temp_dir.path().join("test_cleanup.zip.001").exists());
        assert!(!temp_dir.path().join("test_cleanup.zip.002").exists());
        assert!(!temp_dir.path().join("test_cleanup.zip.003").exists());
    }

    #[test]
    fn test_guarded_split_writer_commit_preserves_volumes() {
        let temp_dir = tempdir().expect("tempdir failed");
        let base_path = temp_dir.path().join("test_preserve.zip");
        let volumes = {
            let split_writer =
                SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension)
                    .expect("SplitVolumeWriter new failed");
            let mut guarded = GuardedSplitWriter::new(split_writer);
            guarded.write_all(&[0xBBu8; 250]).expect("write_all failed");
            guarded.commit().expect("commit failed")
        };
        assert_eq!(volumes.len(), 3);
        for vol in &volumes {
            assert!(vol.exists());
        }
    }
}
