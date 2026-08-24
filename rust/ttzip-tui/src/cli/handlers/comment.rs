// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Subcommand execution handler: comment (ZIP EOCD comment manipulation).

use crate::cli::args::GenericResultDto;
use std::fs;
use std::path::Path;
use std::time::Instant;
use ttzip_engine::zip::parser::MAGIC_EOCD;

/// Finds End of Central Directory (EOCD) offset in ZIP archive data.
fn find_eocd_offset(data: &[u8]) -> Option<usize> {
    let len = data.len();
    if len < 22 {
        return None;
    }
    let search_back = len.min(65557);
    let search_start = len - search_back;

    let mut pos = len - 22;
    loop {
        if pos < search_start {
            break;
        }
        let sig = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        if sig == MAGIC_EOCD {
            let comment_len = u16::from_le_bytes(data[pos + 20..pos + 22].try_into().unwrap()) as usize;
            if pos + 22 + comment_len <= len {
                return Some(pos);
            }
        }
        if pos == 0 {
            break;
        }
        pos -= 1;
    }
    None
}

/// Executes headless `comment` subcommand.
pub fn execute_comment(archive_path: &Path, comment: Option<&str>, json: bool) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let start = Instant::now();
    let data = fs::read(archive_path)
        .map_err(|e| format!("Failed to read archive {}: {}", archive_path.display(), e))?;

    let eocd_pos = find_eocd_offset(&data);

    let message = match comment {
        Some(new_comment) => {
            let p = eocd_pos.ok_or_else(|| {
                format!("Failed to set comment: {} is not a valid ZIP archive", archive_path.display())
            })?;

            let comment_bytes = new_comment.as_bytes();
            if comment_bytes.len() > 0xFFFF {
                return Err("Comment length exceeds maximum ZIP EOCD limit of 65,535 bytes".to_string());
            }

            let mut new_data = Vec::with_capacity(p + 22 + comment_bytes.len());
            new_data.extend_from_slice(&data[..p + 20]);
            new_data.extend_from_slice(&(comment_bytes.len() as u16).to_le_bytes());
            new_data.extend_from_slice(comment_bytes);

            fs::write(archive_path, &new_data)
                .map_err(|e| format!("Failed to write updated archive {}: {}", archive_path.display(), e))?;

            format!("Set archive comment to: \"{}\"", new_comment)
        }
        None => {
            if let Some(p) = eocd_pos {
                let comment_len = u16::from_le_bytes(data[p + 20..p + 22].try_into().unwrap()) as usize;
                if comment_len > 0 && p + 22 + comment_len <= data.len() {
                    let text = String::from_utf8_lossy(&data[p + 22..p + 22 + comment_len]);
                    format!("Archive comment: \"{}\"", text)
                } else {
                    "Archive comment: none".to_string()
                }
            } else {
                "Archive comment: none (not a valid ZIP archive or comment is empty)".to_string()
            }
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;

    if json {
        let dto = GenericResultDto {
            command: "comment".to_string(),
            archive: archive_path.to_string_lossy().to_string(),
            success: true,
            message,
            elapsed_ms: elapsed,
        };

        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize comment JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("{:=<60}", "");
    println!("  TTZip Archive Comment");
    println!("  Archive: {}", archive_path.display());
    println!("  Status:  {}", message);
    println!("{:=<60}", "");

    Ok(())
}
