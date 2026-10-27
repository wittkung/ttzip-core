// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: create (with optional multi-volume splitting).

use super::split::parse_size_bytes;
use crate::cli::format::{format_bytes, ContainerFormat};
use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use ttzip_engine::archive::create_archive_to_writer;
use ttzip_engine::archive::split::{SplitVolumeWriter, VolumeNamingScheme};
use ttzip_engine::sevenz::{create_7z_archive, create_7z_solid_archive_bytes};
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
};
use ttzip_engine::zip::{collect_zip_input_items, create_zip_archive};

/// Parameters for headless archive creation.
pub struct CliCreateParams<'a> {
    pub archive_path: &'a Path,
    pub sources: &'a [PathBuf],
    pub format_opt: Option<&'a str>,
    pub level: u8,
    pub password: Option<&'a str>,
    pub threads: u32,
    pub volume_size: Option<&'a str>,
    pub silent: bool,
}

/// Executes headless `create` subcommand with optional multi-volume chunk splitting (prints summary to stdout).
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
        silent: true,
    })
}

/// Executes archive creation using bundled parameter struct.
pub fn execute_create_with_params(params: CliCreateParams) -> Result<(), String> {
    let CliCreateParams {
        archive_path,
        sources,
        format_opt,
        level,
        password,
        threads,
        volume_size,
        silent,
    } = params;
    if sources.is_empty() {
        return Err("No source files specified for archive creation".to_string());
    }

    for src in sources {
        if !src.exists() {
            return Err(format!("Source file/directory not found: {}", src.display()));
        }
    }

    let start_time = Instant::now();

    // Determine target format
    let target_format = if let Some(fmt) = format_opt {
        match fmt.to_lowercase().as_str() {
            "7z" | "sevenzip" => ContainerFormat::SevenZip,
            "zip" => ContainerFormat::Zip,
            other => return Err(format!("Unsupported format: {}", other)),
        }
    } else if let Some(ext) = archive_path.extension().and_then(|s| s.to_str()) {
        if ext.eq_ignore_ascii_case("7z") {
            ContainerFormat::SevenZip
        } else {
            ContainerFormat::Zip
        }
    } else {
        ContainerFormat::Zip
    };

    let compression_level = match level {
        0 => TTZipCompressionLevel::Store,
        1..=2 => TTZipCompressionLevel::Fastest,
        3..=5 => TTZipCompressionLevel::Fast,
        6..=8 => TTZipCompressionLevel::Normal,
        9..=11 => TTZipCompressionLevel::Maximum,
        _ => TTZipCompressionLevel::Ultra,
    };

    let password_c = password.map(|p| CString::new(p).unwrap_or_default());
    let password_ptr = password_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());

    let encryption_method = if password.is_some() {
        TTZipEncryptionMethod::Aes256
    } else {
        TTZipEncryptionMethod::None
    };

    let options = TTZipCreateOptions {
        format: match target_format {
            ContainerFormat::Zip => TTZipArchiveFormat::Zip,
            ContainerFormat::SevenZip => TTZipArchiveFormat::SevenZip,
            _ => TTZipArchiveFormat::Zip,
        },
        level: compression_level,
        encryption: encryption_method,
        password: password_ptr,
        thread_budget: threads.max(1),
        solid_block_size_mb: 64,
        ..Default::default()
    };

    if let Some(v_size_str) = volume_size {
        let chunk_size = parse_size_bytes(v_size_str)?;

        let naming_scheme = match target_format {
            ContainerFormat::Zip => VolumeNamingScheme::NumberedExtension,
            _ => VolumeNamingScheme::NumberedExtension,
        };

        let split_writer = SplitVolumeWriter::new(archive_path, chunk_size, naming_scheme)
            .map_err(|e| format!("Failed to initialize multi-volume writer: {}", e))?;
        let mut guarded_writer = GuardedSplitWriter::new(split_writer);

        let (total_entries, total_uncompressed_bytes, total_compressed_bytes) = match target_format {
            ContainerFormat::Zip => {
                let compressed_bytes = create_archive_to_writer(sources, &mut guarded_writer, &options)
                    .map_err(|e| format!("Failed to create split ZIP archive: {:?}", e))?;
                let (entries, uncompressed) = calculate_source_stats(sources);
                (entries, uncompressed, compressed_bytes)
            }
            ContainerFormat::SevenZip => {
                let mut input_items = Vec::new();
                for src in sources {
                    if !src.exists() {
                        return Err(format!("Source file/directory not found: {}", src.display()));
                    }
                    let file_name = src.file_name().unwrap_or_default().to_string_lossy();
                    collect_zip_input_items(src, &file_name, &mut input_items)
                        .map_err(|e| format!("Failed to collect 7z inputs: {:?}", e))?;
                }
                let level_int = match options.level {
                    TTZipCompressionLevel::Store => 0,
                    TTZipCompressionLevel::Fastest => 1,
                    TTZipCompressionLevel::Fast => 3,
                    TTZipCompressionLevel::Normal => 6,
                    TTZipCompressionLevel::Maximum => 9,
                    TTZipCompressionLevel::Ultra => 12,
                };
                let threads = options.thread_budget.max(1);
                let binary_bytes = create_7z_solid_archive_bytes(&input_items, level_int, threads)
                    .map_err(|e| format!("Failed to create 7z solid archive: {:?}", e))?;
                guarded_writer
                    .write_all(&binary_bytes)
                    .map_err(|e| format!("Failed to write split volume segments: {}", e))?;
                let uncompressed: u64 = input_items.iter().map(|it| it.data.len() as u64).sum();
                (input_items.len(), uncompressed, binary_bytes.len() as u64)
            }
            _ => return Err("Invalid format".to_string()),
        };

        let volumes = guarded_writer
            .commit()
            .map_err(|e| format!("Failed to close split volumes: {}", e))?;

        if !silent {
            let elapsed = start_time.elapsed();
            println!(
                "Created {} multi-volume archive {} ({} volumes) with {} entries ({} -> {}) in {:.2?}",
                target_format.name(),
                archive_path.display(),
                volumes.len(),
                total_entries,
                format_bytes(total_uncompressed_bytes),
                format_bytes(total_compressed_bytes),
                elapsed
            );
            for vol in &volumes {
                println!("  - {}", vol.display());
            }
        }

        return Ok(());
    }

    let report = match target_format {
        ContainerFormat::Zip => create_zip_archive(archive_path, sources, &options)
            .map_err(|e| format!("Failed to create ZIP archive: {:?}", e))?,
        ContainerFormat::SevenZip => create_7z_archive(archive_path, sources, &options)
            .map_err(|e| format!("Failed to create 7z archive: {:?}", e))?,
        _ => return Err("Invalid format".to_string()),
    };

    if !silent {
        let elapsed = start_time.elapsed();
        println!(
            "Created {} archive {} with {} entries ({} -> {}) in {:.2?}",
            target_format.name(),
            archive_path.display(),
            report.total_entries,
            format_bytes(report.total_uncompressed_bytes),
            format_bytes(report.total_compressed_bytes),
            elapsed
        );
    }

    Ok(())
}

/// RAII guard wrapping SplitVolumeWriter to purge partial volume slices upon error or panic.
pub(crate) struct GuardedSplitWriter {
    writer: SplitVolumeWriter,
    committed: bool,
}

impl GuardedSplitWriter {
    pub(crate) fn new(writer: SplitVolumeWriter) -> Self {
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

impl Write for GuardedSplitWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl Drop for GuardedSplitWriter {
    fn drop(&mut self) {
        if !self.committed {
            self.writer.cancel_and_cleanup();
        }
    }
}

/// Recursively calculates total entry count and uncompressed bytes from input sources.
fn calculate_source_stats(sources: &[PathBuf]) -> (usize, u64) {
    let mut total_entries = 0usize;
    let mut total_uncompressed_bytes = 0u64;

    for src in sources {
        if src.is_dir() {
            let mut stack = vec![src.clone()];
            while let Some(dir) = stack.pop() {
                if let Ok(rd) = fs::read_dir(&dir) {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        total_entries += 1;
                        if path.is_dir() {
                            stack.push(path);
                        } else if let Ok(meta) = entry.metadata() {
                            total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(meta.len());
                        }
                    }
                }
            }
        } else {
            total_entries += 1;
            if let Ok(meta) = src.metadata() {
                total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(meta.len());
            }
        }
    }

    (total_entries, total_uncompressed_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_guarded_split_writer_cleanup_on_error() {
        let temp_dir = tempdir().expect("tempdir failed");
        let base_path = temp_dir.path().join("test_cleanup.zip");

        {
            let split_writer = SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension)
                .expect("SplitVolumeWriter new failed");
            let mut guarded = GuardedSplitWriter::new(split_writer);
            guarded.write_all(&[0xAAu8; 250]).expect("write_all failed");
            // Deliberately drop guarded without calling commit() to simulate error/abort
        }

        let vol1 = temp_dir.path().join("test_cleanup.zip.001");
        let vol2 = temp_dir.path().join("test_cleanup.zip.002");
        let vol3 = temp_dir.path().join("test_cleanup.zip.003");

        assert!(!vol1.exists(), "vol1 should have been cleaned up");
        assert!(!vol2.exists(), "vol2 should have been cleaned up");
        assert!(!vol3.exists(), "vol3 should have been cleaned up");
    }

    #[test]
    fn test_guarded_split_writer_commit_preserves_volumes() {
        let temp_dir = tempdir().expect("tempdir failed");
        let base_path = temp_dir.path().join("test_preserve.zip");

        let volumes = {
            let split_writer = SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension)
                .expect("SplitVolumeWriter new failed");
            let mut guarded = GuardedSplitWriter::new(split_writer);
            guarded.write_all(&[0xBBu8; 250]).expect("write_all failed");
            guarded.commit().expect("commit failed")
        };

        assert_eq!(volumes.len(), 3);
        for vol in &volumes {
            assert!(vol.exists(), "vol {} should exist after commit", vol.display());
        }
    }
}
