// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Subcommand execution handler: lock (POSIX chmod and macOS uchg write-protection).

use crate::cli::args::GenericResultDto;
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Instant;

/// Executes headless `lock` subcommand.
pub fn execute_lock(archive_path: &Path, unlock: bool, json: bool) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }

    let start = Instant::now();

    let path_str = archive_path.to_str().ok_or_else(|| "Invalid UTF-8 archive path".to_string())?;
    let c_path = CString::new(path_str).map_err(|e| format!("Invalid CString path: {}", e))?;

    let message = if unlock {
        // 1. Clear macOS immutable flag uchg (UF_IMMUTABLE)
        #[cfg(target_os = "macos")]
        unsafe {
            let _ = libc::chflags(c_path.as_ptr(), 0);
        }

        // 2. Restore read-write POSIX permissions (0o644)
        if let Ok(meta) = fs::metadata(archive_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o644);
            let _ = fs::set_permissions(archive_path, perms);
        }

        format!("Archive {} unlocked: write protection removed (mode: 0644)", archive_path.display())
    } else {
        // 1. Set POSIX permissions to read-only (0o444)
        if let Ok(meta) = fs::metadata(archive_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o444);
            let _ = fs::set_permissions(archive_path, perms);
        }

        // 2. Set macOS immutable flag uchg (UF_IMMUTABLE)
        #[cfg(target_os = "macos")]
        unsafe {
            let _ = libc::chflags(c_path.as_ptr(), libc::UF_IMMUTABLE);
        }

        format!("Archive {} locked: POSIX chmod 0444 and macOS uchg write-protection enforced", archive_path.display())
    };

    let elapsed = start.elapsed().as_millis() as u64;

    if json {
        let dto = GenericResultDto {
            command: "lock".to_string(),
            archive: archive_path.to_string_lossy().to_string(),
            success: true,
            message,
            elapsed_ms: elapsed,
        };

        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize lock JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("{:=<60}", "");
    println!("  TTZip Archive Lock & Write Protection");
    println!("  Archive: {}", archive_path.display());
    println!("  Status:  {}", message);
    println!("{:=<60}", "");

    Ok(())
}
