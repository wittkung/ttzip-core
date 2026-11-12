// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: cat / view.

use crate::cli::format::{detect_archive_format, read_archive_data_auto, ContainerFormat};
use std::io::Write;
use std::path::Path;
use ttzip_engine::archive::tar::TarArchive;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::zip::ZipArchive;

fn cat_zip_entry(
    data: &[u8],
    entry_path: &str,
    normalized_target: &str,
    password: Option<&str>,
) -> Result<Vec<u8>, String> {
    let archive =
        ZipArchive::open_slice(data).map_err(|e| format!("Failed to open ZIP archive: {:?}", e))?;
    let idx = archive
        .entries()
        .iter()
        .position(|e| e.rel_path.trim_start_matches('/') == normalized_target)
        .ok_or_else(|| format!("Entry {} not found in ZIP archive", entry_path))?;
    archive
        .extract_entry_bytes(idx, password)
        .map_err(|e| format!("Failed to decompress entry {}: {:?}", entry_path, e))
}

fn cat_sevenz_entry(
    data: &[u8],
    entry_path: &str,
    normalized_target: &str,
    password: Option<&str>,
) -> Result<Vec<u8>, String> {
    let archive = SevenZArchive::open_slice(data)
        .map_err(|e| format!("Failed to open 7z archive: {:?}", e))?;
    let idx = archive
        .files()
        .iter()
        .position(|f| f.rel_path.trim_start_matches('/') == normalized_target)
        .ok_or_else(|| format!("Entry {} not found in 7z archive", entry_path))?;
    archive
        .extract_entry_bytes(idx, password)
        .map_err(|e| format!("Failed to decompress entry {}: {:?}", entry_path, e))
}

fn cat_tar_entry(
    data: &[u8],
    entry_path: &str,
    normalized_target: &str,
) -> Result<Vec<u8>, String> {
    let archive =
        TarArchive::open_slice(data).map_err(|e| format!("Failed to open TAR archive: {:?}", e))?;
    let idx = archive
        .entries()
        .iter()
        .position(|e| e.path.trim_start_matches('/') == normalized_target)
        .ok_or_else(|| format!("Entry {} not found in TAR archive", entry_path))?;
    archive
        .extract_entry_bytes(idx)
        .map(|s| s.to_vec())
        .map_err(|e| format!("Failed to extract TAR entry {}: {:?}", entry_path, e))
}

/// Executes headless `cat` subcommand, writing entry payload to stdout.
pub fn execute_cat(
    archive_path: &Path,
    entry_path: &str,
    password: Option<&str>,
) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let (_volumes, data) = read_archive_data_auto(archive_path)?;
    let format = detect_archive_format(archive_path, &data);
    let normalized_target = entry_path.trim_start_matches('/');

    let payload = match format {
        ContainerFormat::Zip => cat_zip_entry(&data, entry_path, normalized_target, password)?,
        ContainerFormat::SevenZip => {
            cat_sevenz_entry(&data, entry_path, normalized_target, password)?
        }
        ContainerFormat::Tar => cat_tar_entry(&data, entry_path, normalized_target)?,
        _ => return Err(format!("Direct cat not supported for format {:?}", format.name())),
    };

    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&payload)
        .map_err(|e| format!("Failed to write payload to stdout: {}", e))?;
    stdout
        .flush()
        .map_err(|e| format!("Failed to flush stdout: {}", e))?;

    Ok(())
}
