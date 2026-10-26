// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: comment (ZIP EOCD comment manipulation).

use crate::cli::args::GenericResultDto;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;
use ttzip_engine::zip::parser::MAGIC_EOCD;

/// Locates the End of Central Directory (EOCD) record offset from seekable file tail.
fn locate_eocd<F: Read + Seek>(file: &mut F, file_len: u64) -> Result<Option<(u64, Vec<u8>)>, String> {
    if file_len < 22 {
        return Ok(None);
    }
    let search_back = (file_len as usize).min(65557);
    let search_start = file_len - search_back as u64;

    file.seek(SeekFrom::Start(search_start))
        .map_err(|e| format!("Failed to seek archive tail: {}", e))?;

    let mut tail_buf = vec![0u8; search_back];
    file.read_exact(&mut tail_buf)
        .map_err(|e| format!("Failed to read archive tail: {}", e))?;

    let mut rel_pos = search_back - 22;
    loop {
        let sig = u32::from_le_bytes(tail_buf[rel_pos..rel_pos + 4].try_into().unwrap());
        if sig == MAGIC_EOCD {
            let comment_len =
                u16::from_le_bytes(tail_buf[rel_pos + 20..rel_pos + 22].try_into().unwrap()) as usize;
            if rel_pos + 22 + comment_len <= search_back {
                let eocd_abs_pos = search_start + rel_pos as u64;
                let existing_comment = tail_buf[rel_pos + 22..rel_pos + 22 + comment_len].to_vec();
                return Ok(Some((eocd_abs_pos, existing_comment)));
            }
        }
        if rel_pos == 0 {
            break;
        }
        rel_pos -= 1;
    }
    Ok(None)
}

/// Executes headless `comment` subcommand without loading entire archive into RAM.
pub fn execute_comment(archive_path: &Path, comment: Option<&str>, json: bool) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let start = Instant::now();
    let mut file = OpenOptions::new()
        .read(true)
        .write(comment.is_some())
        .open(archive_path)
        .map_err(|e| format!("Failed to open archive {}: {}", archive_path.display(), e))?;

    let file_len = file
        .metadata()
        .map_err(|e| format!("Failed to read archive metadata {}: {}", archive_path.display(), e))?
        .len();

    let eocd_info = locate_eocd(&mut file, file_len)?;

    let message = match comment {
        Some(new_comment) => {
            let (eocd_pos, _) = eocd_info.ok_or_else(|| {
                format!(
                    "Failed to set comment: {} is not a valid ZIP archive",
                    archive_path.display()
                )
            })?;

            let comment_bytes = new_comment.as_bytes();
            if comment_bytes.len() > 0xFFFF {
                return Err("Comment length exceeds maximum ZIP EOCD limit of 65,535 bytes".to_string());
            }

            // Seek to comment length field in EOCD (offset 20) and update in-place
            file.seek(SeekFrom::Start(eocd_pos + 20))
                .map_err(|e| format!("Failed to seek to EOCD comment field: {}", e))?;
            file.write_all(&(comment_bytes.len() as u16).to_le_bytes())
                .map_err(|e| format!("Failed to write comment length: {}", e))?;
            file.write_all(comment_bytes)
                .map_err(|e| format!("Failed to write comment payload: {}", e))?;

            // Truncate trailing garbage if previous comment was longer
            let new_total_len = eocd_pos + 22 + comment_bytes.len() as u64;
            file.set_len(new_total_len)
                .map_err(|e| format!("Failed to truncate archive tail: {}", e))?;
            file.flush()
                .map_err(|e| format!("Failed to flush archive writes: {}", e))?;

            format!("Set archive comment to: \"{}\"", new_comment)
        }
        None => {
            if let Some((_, existing_bytes)) = eocd_info {
                if !existing_bytes.is_empty() {
                    let text = String::from_utf8_lossy(&existing_bytes);
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
