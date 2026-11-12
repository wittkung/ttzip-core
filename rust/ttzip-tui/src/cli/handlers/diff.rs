// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: diff.

use crate::cli::args::DiffResultDto;
use crate::cli::format::{parse_archive_entries, read_archive_data_auto};
use std::collections::HashMap;
use std::path::Path;

fn compute_archive_diff(archive_a: &Path, archive_b: &Path) -> Result<DiffResultDto, String> {
    let (_, data_a) = read_archive_data_auto(archive_a)?;
    let (_, entries_a) = parse_archive_entries(archive_a, &data_a)?;
    let (_, data_b) = read_archive_data_auto(archive_b)?;
    let (_, entries_b) = parse_archive_entries(archive_b, &data_b)?;

    let map_a: HashMap<_, _> = entries_a.iter().map(|e| (e.relative_path.clone(), e)).collect();
    let map_b: HashMap<_, _> = entries_b.iter().map(|e| (e.relative_path.clone(), e)).collect();

    let mut a_only = Vec::new();
    let mut b_only = Vec::new();
    let mut modified = Vec::new();
    let mut identical_count = 0;

    for (path, ea) in &map_a {
        if let Some(eb) = map_b.get(path) {
            if ea.uncompressed_size != eb.uncompressed_size || ea.crc32 != eb.crc32 {
                modified.push(path.clone());
            } else {
                identical_count += 1;
            }
        } else {
            a_only.push(path.clone());
        }
    }

    for path in map_b.keys() {
        if !map_a.contains_key(path) {
            b_only.push(path.clone());
        }
    }

    a_only.sort();
    b_only.sort();
    modified.sort();
    let is_identical = a_only.is_empty() && b_only.is_empty() && modified.is_empty();

    Ok(DiffResultDto {
        archive_a: archive_a.to_string_lossy().to_string(),
        archive_b: archive_b.to_string_lossy().to_string(),
        entries_a_only: a_only,
        entries_b_only: b_only,
        modified_entries: modified,
        identical_count,
        is_identical,
    })
}

fn print_diff_section(header: &str, prefix: &str, items: &[String]) {
    if !items.is_empty() {
        println!("  {} ({} items):", header, items.len());
        for p in items {
            println!("    {} {}", prefix, p);
        }
    }
}

/// Executes headless `diff` subcommand.
pub fn execute_diff(archive_a: &Path, archive_b: &Path, json: bool) -> Result<(), String> {
    if !archive_a.exists() {
        return Err(format!("Archive A not found: {}", archive_a.display()));
    }
    if !archive_b.exists() {
        return Err(format!("Archive B not found: {}", archive_b.display()));
    }

    let dto = compute_archive_diff(archive_a, archive_b)?;

    if json {
        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize diff JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("{:=<60}", "");
    println!("  TTZip Archive Comparison");
    println!("  Archive A: {}", archive_a.display());
    println!("  Archive B: {}", archive_b.display());
    println!("{:=<60}", "");

    if dto.is_identical {
        println!("  Status: IDENTICAL ({} identical entries)", dto.identical_count);
    } else {
        print_diff_section("Only in A", "-", &dto.entries_a_only);
        print_diff_section("Only in B", "+", &dto.entries_b_only);
        print_diff_section("Modified", "*", &dto.modified_entries);
        println!("  Identical entries: {}", dto.identical_count);
    }
    println!("{:=<60}", "");

    Ok(())
}
