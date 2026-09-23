// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Subcommand execution handler: convert.

use crate::cli::handlers::create::execute_create_silent;
use crate::cli::handlers::extract::{execute_extract, CliExtractParams};
use std::path::Path;
use std::time::Instant;

/// Executes headless `convert` subcommand (prints status to stdout).
pub fn execute_convert(
    source_archive: &Path,
    destination_archive: &Path,
    format: Option<&str>,
    level: u8,
) -> Result<(), String> {
    execute_convert_internal(source_archive, destination_archive, format, level, false)
}

/// Executes headless `convert` subcommand silently with zero stdout writes.
pub fn execute_convert_silent(
    source_archive: &Path,
    destination_archive: &Path,
    format: Option<&str>,
    level: u8,
) -> Result<(), String> {
    execute_convert_internal(source_archive, destination_archive, format, level, true)
}

fn execute_convert_internal(
    source_archive: &Path,
    destination_archive: &Path,
    format: Option<&str>,
    level: u8,
    silent: bool,
) -> Result<(), String> {
    if !source_archive.exists() {
        return Err(format!("Source archive not found: {}", source_archive.display()));
    }

    let start = Instant::now();
    let unique_name = format!("ttzip_conv_{}_{}", std::process::id(), start.elapsed().as_nanos());
    let staging_path = std::env::temp_dir().join(unique_name);
    std::fs::create_dir_all(&staging_path)
        .map_err(|e| format!("Failed to create staging directory for conversion: {}", e))?;

    if !silent {
        println!("-> Extracting source archive {}...", source_archive.display());
    }
    let extract_res = execute_extract(CliExtractParams {
        archive_path: source_archive,
        output_dir: Some(&staging_path),
        password: None,
        threads: 4,
        verbose: false,
        dry_run: false,
        include: &[],
        exclude: &[],
    });
    if let Err(e) = extract_res {
        let _ = std::fs::remove_dir_all(&staging_path);
        return Err(e);
    }

    if !silent {
        println!("-> Recompressing into destination {} (level: {})...", destination_archive.display(), level);
    }
    let mut sources = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&staging_path) {
        for entry in entries.flatten() {
            sources.push(entry.path());
        }
    }
    if sources.is_empty() {
        sources.push(staging_path.clone());
    }
    let create_res = execute_create_silent(
        destination_archive,
        &sources,
        format,
        level,
        None,
        4,
        None,
    );
    let _ = std::fs::remove_dir_all(&staging_path);
    create_res?;

    let elapsed = start.elapsed().as_millis();
    if !silent {
        println!("✅ Converted {} -> {} successfully ({}ms).", source_archive.display(), destination_archive.display(), elapsed);
    }

    Ok(())
}
