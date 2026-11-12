// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: check / test.

mod stream_verify;
#[cfg(test)]
mod tests;
mod tree_tracker;

use crate::cli::args::CheckResultDto;
use crate::cli::format::{
    ContainerFormat, parse_archive_entries_with_password, read_archive_data_auto,
};
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use stream_verify::deep_verify_stream_extract_with_quota;
use tree_tracker::HardlinkTracker;
use ttzip_engine::archive::tar::{TarArchive, TarEntry};
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::zip::ZipArchive;

const MAX_DEEP_VERIFY_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;

fn check_quota(bytes: u64, max_bytes: u64) -> Result<(), String> {
    if bytes > max_bytes {
        return Err(format!(
            "Deep verification byte quota exceeded: {} > {}",
            bytes, max_bytes
        ));
    }
    Ok(())
}

struct QuotaVerifySink {
    total_bytes: u64,
    max_bytes: u64,
}

impl QuotaVerifySink {
    fn new(max_bytes: u64) -> Self {
        Self {
            total_bytes: 0,
            max_bytes,
        }
    }

    fn record(&mut self, n: u64) -> Result<(), String> {
        self.total_bytes = self.total_bytes.saturating_add(n);
        check_quota(self.total_bytes, self.max_bytes)
    }
}

impl Write for QuotaVerifySink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.record(buf.len() as u64)
            .map_err(std::io::Error::other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Executes headless `check` / `test` subcommand with optional deep payload CRC32 verification.
pub fn execute_check(
    archive_path: &Path,
    password: Option<&str>,
    deep: bool,
    json: bool,
) -> Result<(), String> {
    let dto = check_archive_integrity(archive_path, password, deep)?;
    if json {
        let s = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize check JSON: {}", e))?;
        println!("{}", s);
        return if dto.is_valid {
            Ok(())
        } else {
            Err("Archive check failed with errors".to_string())
        };
    }
    let mode_str = if deep {
        " (Deep Payload CRC32 Verified)"
    } else {
        ""
    };
    if dto.is_valid {
        println!(
            "✅ [PASS] Archive {} (Format: {}, Entries: {}){} is healthy and fully compliant ({}ms).",
            archive_path.display(),
            dto.format,
            dto.total_entries,
            mode_str,
            dto.elapsed_ms
        );
        Ok(())
    } else {
        eprintln!(
            "❌ [FAIL] Archive {} verification failed ({} corrupted entries, {}ms):",
            archive_path.display(),
            dto.corrupted_entries,
            dto.elapsed_ms
        );
        for err in &dto.errors {
            eprintln!("  - {}", err);
        }
        Err("Archive verification failed".to_string())
    }
}

fn deep_verify_zip(data: &[u8], password: Option<&str>, errors: &mut Vec<String>) -> usize {
    let Ok(zip) = ZipArchive::open_slice(data) else {
        errors.push("Failed to parse ZIP central directory for deep verification".to_string());
        return 1;
    };
    let declared = zip
        .entries()
        .iter()
        .fold(0u64, |a, e| a.saturating_add(e.uncompressed_size));
    if let Err(msg) = check_quota(declared, MAX_DEEP_VERIFY_OUTPUT_BYTES) {
        errors.push(msg);
        return 1;
    }
    let mut corrupted = 0;
    let mut sink = QuotaVerifySink::new(MAX_DEEP_VERIFY_OUTPUT_BYTES);
    for (idx, entry) in zip.entries().iter().enumerate() {
        if entry.is_directory || (entry.uncompressed_size == 0 && !entry.is_encrypted) {
            continue;
        }
        if let Err(e) = zip.extract_entry_to_sink(idx, password, &mut sink) {
            corrupted += 1;
            if let Err(msg) = check_quota(sink.total_bytes, sink.max_bytes) {
                errors.push(msg);
                break;
            }
            errors.push(format!(
                "Verification failed for '{}': {:?}",
                entry.rel_path, e
            ));
        }
    }
    corrupted
}

fn deep_verify_sevenz(data: &[u8], password: Option<&str>, errors: &mut Vec<String>) -> usize {
    let Ok(sevenz) = SevenZArchive::open_slice_with_password(data, password) else {
        errors.push("Failed to parse 7z headers for deep verification".to_string());
        return 1;
    };
    let info = sevenz.info();
    let s_sum = info
        .stream_sizes
        .iter()
        .copied()
        .fold(0u64, u64::saturating_add);
    let f_sum = info.folders.iter().fold(0u64, |a, f| {
        a.saturating_add(f.unpack_sizes.last().copied().unwrap_or(0))
    });
    if let Err(msg) = check_quota(s_sum.max(f_sum), MAX_DEEP_VERIFY_OUTPUT_BYTES) {
        errors.push(msg);
        return 1;
    }
    let mut corrupted = 0;
    let mut sink = QuotaVerifySink::new(MAX_DEEP_VERIFY_OUTPUT_BYTES);
    let extractor = sevenz
        .solid_extractor()
        .with_preceding_budget(MAX_DEEP_VERIFY_OUTPUT_BYTES);
    for (idx, file) in info.files.iter().enumerate() {
        if file.is_directory || file.is_empty_stream {
            continue;
        }
        let res = extractor.extract_streaming(idx, password, |chunk| {
            sink.record(chunk.len() as u64)
                .map_err(|_| TTZipStatus::ErrSecurityViolation)
        });
        if let Err(e) = res {
            corrupted += 1;
            if sink.total_bytes > sink.max_bytes || e == TTZipStatus::ErrSolidBudgetExceeded {
                errors.push(format!(
                    "Deep verification byte quota exceeded: > {}",
                    MAX_DEEP_VERIFY_OUTPUT_BYTES
                ));
                break;
            }
            errors.push(format!(
                "Verification failed for '{}': {:?}",
                file.rel_path, e
            ));
        }
    }
    corrupted
}

fn require_tar_link_endpoint<'a>(
    entry: &'a TarEntry<'_>,
    entry_comps: Option<Vec<String>>,
) -> Result<(Vec<String>, &'a str), String> {
    let target = entry
        .link_target
        .as_deref()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| format!("TAR link missing target in '{}'", entry.path))?;
    let comps =
        entry_comps.ok_or_else(|| format!("Invalid TAR link path '{}'", entry.path))?;
    Ok((comps, target))
}

fn verify_tar_link_or_dir(
    entry: &TarEntry<'_>,
    entry_comps: Option<Vec<String>>,
    hardlinks: &mut HardlinkTracker,
) -> Result<bool, String> {
    if entry.is_symlink {
        let (comps, target) = require_tar_link_endpoint(entry, entry_comps)?;
        hardlinks.on_symlink(comps, target)?;
        return Ok(true);
    }
    if entry.is_directory {
        if let Some(comps) = entry_comps {
            hardlinks.on_directory(comps)?;
        }
        return Ok(true);
    }
    if entry.is_hardlink {
        let (comps, target) = require_tar_link_endpoint(entry, entry_comps)?;
        hardlinks.record_hardlink(comps, target)?;
        return Ok(true);
    }
    Ok(false)
}

fn verify_tar_entry(
    tar: &TarArchive<'_>,
    idx: usize,
    entry: &TarEntry<'_>,
    hardlinks: &mut HardlinkTracker,
    sink: &mut QuotaVerifySink,
) -> Result<(), String> {
    let entry_comps = HardlinkTracker::parse_entry_components(&entry.path)?;
    if verify_tar_link_or_dir(entry, entry_comps.clone(), hardlinks)? {
        return Ok(());
    }
    let bytes = tar
        .extract_entry_bytes(idx)
        .map_err(|e| format!("TAR stream read error in '{}': {:?}", entry.path, e))?;
    sink.record(bytes.len() as u64)?;
    if let Some(comps) = entry_comps {
        hardlinks.on_regular_committed(comps, None)?;
    }
    Ok(())
}

fn deep_verify_tar(data: &[u8], errors: &mut Vec<String>) -> usize {
    let Ok(tar) = TarArchive::open_slice(data) else {
        errors.push("Failed to parse TAR header table".to_string());
        return 1;
    };
    let declared = tar
        .entries()
        .iter()
        .fold(0u64, |a, e| a.saturating_add(e.size));
    if let Err(msg) = check_quota(declared, MAX_DEEP_VERIFY_OUTPUT_BYTES) {
        errors.push(msg);
        return 1;
    }
    let mut corrupted = 0;
    let mut sink = QuotaVerifySink::new(MAX_DEEP_VERIFY_OUTPUT_BYTES);
    let mut hardlinks = HardlinkTracker::default();
    for (idx, entry) in tar.entries().iter().enumerate() {
        if let Err(msg) = verify_tar_entry(&tar, idx, entry, &mut hardlinks, &mut sink) {
            corrupted += 1;
            let quota_exceeded = sink.total_bytes > sink.max_bytes;
            errors.push(msg);
            if quota_exceeded {
                break;
            }
        }
    }
    if let Err(msg) = hardlinks.finalize() {
        corrupted += 1;
        errors.push(msg);
    }
    corrupted
}

fn deep_verify_archive(
    archive_path: &Path,
    fmt: ContainerFormat,
    data: &[u8],
    is_multi_volume: bool,
    password: Option<&str>,
    errors: &mut Vec<String>,
) -> usize {
    if fmt == ContainerFormat::Unknown {
        errors.push("Cannot deep verify unknown archive format".to_string());
        return 1;
    }
    if !is_multi_volume {
        match fmt {
            ContainerFormat::Zip => return deep_verify_zip(data, password, errors),
            ContainerFormat::SevenZip => return deep_verify_sevenz(data, password, errors),
            ContainerFormat::Tar => return deep_verify_tar(data, errors),
            _ => {}
        }
    }
    deep_verify_stream_extract_with_quota(
        archive_path,
        fmt,
        password,
        MAX_DEEP_VERIFY_OUTPUT_BYTES,
        errors,
    )
}

/// Inspects archive and performs full integrity verification (with deep payload CRC32 checks).
pub fn check_archive_integrity(
    archive_path: &Path,
    password: Option<&str>,
    deep: bool,
) -> Result<CheckResultDto, String> {
    if !archive_path.exists() {
        return Err(format!(
            "Archive file not found: {}",
            archive_path.display()
        ));
    }
    let start = Instant::now();
    let mut errors = Vec::new();
    let mut corrupted_count = 0usize;
    let (volumes, data) = match read_archive_data_auto(archive_path) {
        Ok(res) => res,
        Err(e) => {
            errors.push(format!("Failed to read archive data: {}", e));
            (vec![], crate::cli::format::ArchiveBuffer::Heap(vec![]))
        }
    };
    let (format, total_entries) = if !data.is_empty() {
        match parse_archive_entries_with_password(archive_path, &data, password) {
            Ok((fmt, entries)) => {
                if deep {
                    let declared = entries
                        .iter()
                        .fold(0u64, |a, e| a.saturating_add(e.uncompressed_size));
                    if let Err(msg) = check_quota(declared, MAX_DEEP_VERIFY_OUTPUT_BYTES) {
                        corrupted_count += 1;
                        errors.push(msg);
                    } else {
                        corrupted_count += deep_verify_archive(
                            archive_path,
                            fmt,
                            &data,
                            volumes.len() > 1,
                            password,
                            &mut errors,
                        );
                    }
                }
                (fmt.name().to_string(), entries.len())
            }
            Err(e) => {
                errors.push(format!("Parsing error: {}", e));
                corrupted_count += 1;
                ("UNKNOWN".to_string(), 0)
            }
        }
    } else {
        corrupted_count += 1;
        ("UNKNOWN".to_string(), 0)
    };
    Ok(CheckResultDto {
        archive: archive_path.to_string_lossy().to_string(),
        format,
        is_valid: corrupted_count == 0 && errors.is_empty(),
        total_entries,
        corrupted_entries: corrupted_count,
        errors,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}
