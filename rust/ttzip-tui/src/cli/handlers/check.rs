// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Subcommand execution handler: check / test.

use crate::cli::args::CheckResultDto;
use crate::cli::format::{parse_archive_entries, read_archive_data_auto, ContainerFormat};
use std::path::Path;
use std::time::Instant;
use ttzip_engine::archive::tar::TarArchive;
use ttzip_engine::codecs::brotli::brotli_decompress_to_vec;
use ttzip_engine::codecs::snappy::snappy_frame_decode_to_vec;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::zip::ZipArchive;

/// Executes headless `check` / `test` subcommand with optional deep payload CRC32 verification.
pub fn execute_check(
    archive_path: &Path,
    password: Option<&str>,
    deep: bool,
    json: bool,
) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let start = Instant::now();
    let mut errors = Vec::new();
    let mut is_valid = true;
    let mut corrupted_count = 0usize;

    let (_volumes, data) = match read_archive_data_auto(archive_path) {
        Ok(res) => res,
        Err(e) => {
            errors.push(format!("Failed to read archive data: {}", e));
            (vec![], crate::cli::format::ArchiveBuffer::Heap(vec![]))
        }
    };

    let (format, total_entries) = if !data.is_empty() {
        match parse_archive_entries(archive_path, &data) {
            Ok((fmt, entries)) => {
                let count = entries.len();

                if deep {
                    // Deep payload decompression and CRC32 verification
                    match fmt {
                        ContainerFormat::Zip => {
                            if let Ok(zip) = ZipArchive::open_slice(&data) {
                                for (idx, entry) in zip.entries().iter().enumerate() {
                                    if entry.is_directory || entry.uncompressed_size == 0 {
                                        continue;
                                    }
                                    match zip.extract_entry_bytes(idx, password) {
                                        Ok(decomp) => {
                                            let actual_crc = crc32_fast(0, &decomp);
                                            if entry.crc32 != 0 && actual_crc != entry.crc32 {
                                                is_valid = false;
                                                corrupted_count += 1;
                                                errors.push(format!(
                                                    "CRC32 mismatch in entry '{}': expected 0x{:08X}, got 0x{:08X}",
                                                    entry.rel_path, entry.crc32, actual_crc
                                                ));
                                            }
                                        }
                                        Err(e) => {
                                            is_valid = false;
                                            corrupted_count += 1;
                                            errors.push(format!(
                                                "Decompression failed for entry '{}': {:?}",
                                                entry.rel_path, e
                                            ));
                                        }
                                    }
                                }
                            } else {
                                is_valid = false;
                                corrupted_count += 1;
                                errors.push("Failed to parse ZIP central directory for deep verification".to_string());
                            }
                        }
                        ContainerFormat::SevenZip => {
                            if let Ok(sevenz) = SevenZArchive::open_slice_with_password(&data, password) {
                                for (idx, file) in sevenz.info().files.iter().enumerate() {
                                    if file.is_directory || file.is_empty_stream {
                                        continue;
                                    }
                                    match sevenz.extract_entry_bytes_stream(idx, password) {
                                        Ok(decomp) => {
                                            let actual_crc = crc32_fast(0, &decomp);
                                            if let Some(expected_crc) = sevenz.info().stream_crcs.get(idx).copied() {
                                                if expected_crc != 0 && actual_crc != expected_crc {
                                                    is_valid = false;
                                                    corrupted_count += 1;
                                                    errors.push(format!(
                                                        "CRC32 mismatch in 7z entry '{}': expected 0x{:08X}, got 0x{:08X}",
                                                        file.rel_path, expected_crc, actual_crc
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            is_valid = false;
                                            corrupted_count += 1;
                                            errors.push(format!(
                                                "7z decompression failed for entry '{}': {:?}",
                                                file.rel_path, e
                                            ));
                                        }
                                    }
                                }
                            } else {
                                is_valid = false;
                                corrupted_count += 1;
                                errors.push("Failed to decode 7z header/solid streams".to_string());
                            }
                        }
                        ContainerFormat::Tar => {
                            if let Ok(tar) = TarArchive::open_slice(&data) {
                                for (idx, entry) in tar.entries().iter().enumerate() {
                                    if entry.is_directory {
                                        continue;
                                    }
                                    if tar.extract_entry_bytes(idx).is_err() {
                                        is_valid = false;
                                        corrupted_count += 1;
                                        errors.push(format!("TAR entry '{}' payload truncated or corrupt", entry.path));
                                    }
                                }
                            } else {
                                is_valid = false;
                                corrupted_count += 1;
                                errors.push("Failed to parse TAR blocks".to_string());
                            }
                        }
                        ContainerFormat::Snappy => {
                            if snappy_frame_decode_to_vec(&data, 1024 * 1024 * 512).is_err() {
                                is_valid = false;
                                corrupted_count += 1;
                                errors.push("Snappy framed stream checksum error".to_string());
                            }
                        }
                        ContainerFormat::Brotli | ContainerFormat::TarBrotli => {
                            if brotli_decompress_to_vec(&data, 1024 * 1024 * 512).is_err() {
                                is_valid = false;
                                corrupted_count += 1;
                                errors.push("Brotli stream decompression error".to_string());
                            }
                        }
                        ContainerFormat::Unknown => {
                            is_valid = false;
                            corrupted_count += 1;
                            errors.push("Cannot deep verify unknown archive format".to_string());
                        }
                    }
                }

                (fmt.name().to_string(), count)
            }
            Err(e) => {
                is_valid = false;
                errors.push(format!("Parsing error: {}", e));
                corrupted_count += 1;
                ("UNKNOWN".to_string(), 0)
            }
        }
    } else {
        is_valid = false;
        corrupted_count += 1;
        ("UNKNOWN".to_string(), 0)
    };

    let elapsed = start.elapsed().as_millis() as u64;

    if json {
        let dto = CheckResultDto {
            archive: archive_path.to_string_lossy().to_string(),
            format,
            is_valid,
            total_entries,
            corrupted_entries: corrupted_count,
            errors,
            elapsed_ms: elapsed,
        };

        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize check JSON: {}", e))?;
        println!("{}", json_str);
        if !is_valid {
            return Err("Archive check failed with errors".to_string());
        }
        return Ok(());
    }

    let mode_str = if deep { " (Deep Payload CRC32 Verified)" } else { "" };

    if is_valid {
        println!(
            "✅ [PASS] Archive {} (Format: {}, Entries: {}){} is healthy and fully compliant ({}ms).",
            archive_path.display(),
            format,
            total_entries,
            mode_str,
            elapsed
        );
        Ok(())
    } else {
        eprintln!(
            "❌ [FAIL] Archive {} verification failed ({} corrupted entries, {}ms):",
            archive_path.display(),
            corrupted_count,
            elapsed
        );
        for err in &errors {
            eprintln!("  - {}", err);
        }
        Err("Archive verification failed".to_string())
    }
}
