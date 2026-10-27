// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! MCP Tools schema registry and execution dispatchers.

use crate::cli::args::VfsNodeContractDto;
use crate::cli::format::{format_bytes, parse_archive_entries, read_archive_data_auto};
use crate::cli::handlers::extract::{pattern_matches, CliExtractParams};
use crate::cli::handlers::mcp::protocol::McpToolResult;
use crate::cli::{execute_convert_silent, execute_create_silent, execute_extract, execute_repair_silent};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use ttzip_engine::archive::unified::extract_single_entry_memory;
use ttzip_engine::types::TTZipStatus;

/// Returns all 8 MCP tool schemas conforming to Model Context Protocol specification.
pub fn get_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "ttzip_list",
            "description": "List file entries and metadata inside an archive (ZIP, 7z, TAR, Snappy, Brotli) with optional filtering and pagination",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "archive_path": { "type": "string", "description": "Absolute path to the archive file" },
                    "password": { "type": "string", "description": "Optional password for encrypted archives" },
                    "include": { "type": "array", "items": { "type": "string" }, "description": "Glob patterns to include" },
                    "exclude": { "type": "array", "items": { "type": "string" }, "description": "Glob patterns to exclude" },
                    "page": { "type": "integer", "minimum": 1, "default": 1, "description": "Pagination page index" },
                    "page_size": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 200, "description": "Max entries per page" }
                },
                "required": ["archive_path"]
            }
        }),
        json!({
            "name": "ttzip_extract",
            "description": "Safely extract archive entries to a destination directory with sandbox path protection and thread control",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "archive_path": { "type": "string", "description": "Absolute path to the archive file" },
                    "destination": { "type": "string", "description": "Target output directory (default: current directory)" },
                    "password": { "type": "string", "description": "Optional decryption password" },
                    "threads": { "type": "integer", "minimum": 1, "maximum": 64, "default": 4, "description": "Decompression threads" },
                    "dry_run": { "type": "boolean", "default": false, "description": "Simulate extraction without disk writes" },
                    "include": { "type": "array", "items": { "type": "string" }, "description": "Glob patterns to filter extraction" },
                    "exclude": { "type": "array", "items": { "type": "string" }, "description": "Glob patterns to skip" }
                },
                "required": ["archive_path"]
            }
        }),
        json!({
            "name": "ttzip_create",
            "description": "Create a new archive (ZIP, 7z, TAR, Snappy, Brotli) from source files or directories",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "archive_path": { "type": "string", "description": "Destination archive output path" },
                    "source_paths": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "Source files/directories" },
                    "format": { "type": "string", "enum": ["zip", "7z", "tar", "brotli", "snappy"], "description": "Format override" },
                    "level": { "type": "integer", "minimum": 0, "maximum": 12, "default": 6, "description": "Compression level (0-12)" },
                    "password": { "type": "string", "description": "Optional encryption password" },
                    "threads": { "type": "integer", "minimum": 1, "maximum": 64, "default": 4, "description": "Compression threads" },
                    "volume_size": { "type": "string", "description": "Volume chunk size (e.g. '10M', '100MB')" },
                    "dry_run": { "type": "boolean", "default": false, "description": "Simulate creation" }
                },
                "required": ["archive_path", "source_paths"]
            }
        }),
        json!({
            "name": "ttzip_inspect",
            "description": "Inspect archive header metadata, compression ratio, encryption status, and volume structure",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "archive_path": { "type": "string", "description": "Absolute path to the archive file" },
                    "deep_scan": { "type": "boolean", "default": false, "description": "Perform deep header analysis" }
                },
                "required": ["archive_path"]
            }
        }),
        json!({
            "name": "ttzip_cat",
            "description": "Read content of a single archive file entry directly into context with zero temporary disk writes",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "archive_path": { "type": "string", "description": "Absolute path to the archive file" },
                    "entry_path": { "type": "string", "description": "Internal relative path of the file entry" },
                    "password": { "type": "string", "description": "Optional password" },
                    "max_bytes": { "type": "integer", "minimum": 1, "maximum": 10485760, "default": 1048576, "description": "Max bytes to return" }
                },
                "required": ["archive_path", "entry_path"]
            }
        }),
        json!({
            "name": "ttzip_verify",
            "description": "Perform integrity verification and payload CRC32/CRC64 checksum passes across all archive entries",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "archive_path": { "type": "string", "description": "Absolute path to the archive file" },
                    "password": { "type": "string", "description": "Optional decryption password" },
                    "deep": { "type": "boolean", "default": true, "description": "Full payload decompression pass" }
                },
                "required": ["archive_path"]
            }
        }),
        json!({
            "name": "ttzip_convert",
            "description": "Transcode an archive from one container format to another with target compression level",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_archive": { "type": "string", "description": "Source archive path" },
                    "destination_archive": { "type": "string", "description": "Target archive path" },
                    "target_format": { "type": "string", "enum": ["zip", "7z", "tar", "brotli", "snappy"], "description": "Format override" },
                    "level": { "type": "integer", "minimum": 0, "maximum": 12, "default": 6, "description": "Target compression level" }
                },
                "required": ["source_archive", "destination_archive"]
            }
        }),
        json!({
            "name": "ttzip_repair",
            "description": "Attempt non-destructive salvage and reconstruction on corrupted ZIP or TAR archives",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "damaged_archive": { "type": "string", "description": "Path to corrupted archive" },
                    "output_path": { "type": "string", "description": "Path for repaired archive output" },
                    "format_override": { "type": "string", "enum": ["zip", "tar"], "description": "Format override if header damaged" }
                },
                "required": ["damaged_archive", "output_path"]
            }
        }),
    ]
}

/// Dispatches tool call execution to the matching handler.
pub fn execute_tool_call(name: &str, arguments: Option<&Value>) -> McpToolResult {
    let args = arguments.cloned().unwrap_or(Value::Object(serde_json::Map::new()));
    match name {
        "ttzip_list" => handle_list(args),
        "ttzip_extract" => handle_extract(args),
        "ttzip_create" => handle_create(args),
        "ttzip_inspect" => handle_inspect(args),
        "ttzip_cat" => handle_cat(args),
        "ttzip_verify" => handle_verify(args),
        "ttzip_convert" => handle_convert(args),
        "ttzip_repair" => handle_repair(args),
        unknown => McpToolResult::err(format!("Unknown tool: {}", unknown)),
    }
}

// ---------------------------------------------------------------------------
// Tool Handlers
// ---------------------------------------------------------------------------

fn handle_list(args: Value) -> McpToolResult {
    let archive_path_str = match args.get("archive_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: archive_path", None),
    };
    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        return McpToolResult::err_structured(
            "FILE_NOT_FOUND",
            format!("Archive file not found: {}", archive_path.display()),
            Some(json!({ "archive_path": archive_path_str })),
        );
    }

    let include: Vec<String> = args
        .get("include")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let exclude: Vec<String> = args
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let page_size = args.get("page_size").and_then(|v| v.as_u64()).unwrap_or(200) as usize;

    let (_volumes, data) = match read_archive_data_auto(archive_path) {
        Ok(res) => res,
        Err(e) => return McpToolResult::err_structured(
            "ARCHIVE_READ_FAILED",
            format!("Failed to read archive: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    };

    let (format_detected, all_entries) = match parse_archive_entries(archive_path, &data) {
        Ok(res) => res,
        Err(e) => return McpToolResult::err_structured(
            "ARCHIVE_PARSE_FAILED",
            format!("Failed to parse archive: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    };

    let filtered: Vec<_> = all_entries
        .into_iter()
        .filter(|e| pattern_matches(&e.relative_path, &include, &exclude))
        .collect();

    let total_entries = filtered.len();
    let total_uncompressed: u64 = filtered.iter().map(|e| e.uncompressed_size).sum();
    let total_compressed: u64 = filtered.iter().map(|e| e.compressed_size).sum();

    let start_idx = (page.saturating_sub(1)).saturating_mul(page_size);
    let page_entries: Vec<VfsNodeContractDto> = filtered
        .into_iter()
        .skip(start_idx)
        .take(page_size)
        .map(|e| VfsNodeContractDto {
            name: e.name,
            relative_path: e.relative_path,
            is_directory: e.is_directory,
            uncompressed_size: e.uncompressed_size,
            compressed_size: e.compressed_size,
            crc32: e.crc32,
            is_encrypted: e.is_encrypted,
            match_indices: None,
        })
        .collect();

    let result = json!({
        "archive_path": archive_path_str,
        "format": format_detected.as_str(),
        "total_entries": total_entries,
        "total_uncompressed_bytes": total_uncompressed,
        "total_compressed_bytes": total_compressed,
        "page": page,
        "page_size": page_size,
        "returned_entries_count": page_entries.len(),
        "entries": page_entries
    });

    McpToolResult::ok_json(&result).unwrap_or_else(McpToolResult::err)
}

fn handle_extract(args: Value) -> McpToolResult {
    let archive_path_str = match args.get("archive_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: archive_path", None),
    };
    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        return McpToolResult::err_structured(
            "FILE_NOT_FOUND",
            format!("Archive not found: {}", archive_path.display()),
            Some(json!({ "archive_path": archive_path_str })),
        );
    }

    let dest_str = args.get("destination").and_then(|v| v.as_str());
    let dest_path = dest_str.map(PathBuf::from);

    // Defensive check: ensure destination sandbox directory is valid
    if let Some(ref d) = dest_path {
        if let Err(e) = fs::create_dir_all(d) {
            return McpToolResult::err_structured(
                "IO_ERROR",
                format!("Failed to create destination directory {}: {}", d.display(), e),
                Some(json!({ "destination": d.display().to_string(), "error": e.to_string() })),
            );
        }
    }

    let password = args.get("password").and_then(|v| v.as_str());
    let threads = args.get("threads").and_then(|v| v.as_u64()).unwrap_or(4) as u32;
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

    let include: Vec<String> = args
        .get("include")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let exclude: Vec<String> = args
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let start = Instant::now();
    let extract_res = execute_extract(CliExtractParams {
        archive_path,
        output_dir: dest_path.as_deref(),
        password,
        threads,
        verbose: false,
        dry_run,
        include: &include,
        exclude: &exclude,
    });

    match extract_res {
        Ok(_) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let res = json!({
                "status": if dry_run { "simulated" } else { "completed" },
                "archive_path": archive_path_str,
                "destination": dest_str.unwrap_or("."),
                "dry_run": dry_run,
                "elapsed_ms": elapsed_ms
            });
            McpToolResult::ok_json(&res).unwrap_or_else(McpToolResult::err)
        }
        Err(e) => {
            let err_lower = e.to_lowercase();
            let code = if err_lower.contains("password") {
                "PASSWORD_REQUIRED"
            } else if err_lower.contains("traversal") || err_lower.contains("zip slip") {
                "PATH_TRAVERSAL_BLOCKED"
            } else if err_lower.contains("corrupt") {
                "CORRUPTED_ARCHIVE"
            } else {
                "EXTRACTION_FAILED"
            };
            McpToolResult::err_structured(
                code,
                format!("Extraction error: {}", e),
                Some(json!({ "archive_path": archive_path_str, "error": e })),
            )
        }
    }
}

fn handle_create(args: Value) -> McpToolResult {
    let archive_path_str = match args.get("archive_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: archive_path", None),
    };
    let source_paths_arr = match args.get("source_paths").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return McpToolResult::err_structured("INVALID_PARAMS", "Missing or empty required parameter: source_paths", None),
    };

    let archive_path = PathBuf::from(archive_path_str);
    let sources: Vec<PathBuf> = source_paths_arr
        .iter()
        .filter_map(|v| v.as_str().map(PathBuf::from))
        .collect();

    let format = args.get("format").and_then(|v| v.as_str());
    let level = args.get("level").and_then(|v| v.as_u64()).unwrap_or(6) as u8;
    let password = args.get("password").and_then(|v| v.as_str());
    let threads = args.get("threads").and_then(|v| v.as_u64()).unwrap_or(4) as u32;
    let volume_size = args.get("volume_size").and_then(|v| v.as_str());
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

    if dry_run {
        return McpToolResult::ok_json(&json!({
            "status": "simulated",
            "archive_path": archive_path_str,
            "source_count": sources.len(),
            "level": level
        }))
        .unwrap_or_else(McpToolResult::err);
    }

    let start = Instant::now();
    let create_res = execute_create_silent(
        &archive_path,
        &sources,
        format,
        level,
        password,
        threads,
        volume_size,
    );

    match create_res {
        Ok(_) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let file_size = fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0);
            let res = json!({
                "status": "created",
                "archive_path": archive_path_str,
                "compressed_size_bytes": file_size,
                "elapsed_ms": elapsed_ms
            });
            McpToolResult::ok_json(&res).unwrap_or_else(McpToolResult::err)
        }
        Err(e) => McpToolResult::err_structured(
            "CREATION_FAILED",
            format!("Archive creation error: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    }
}

fn handle_inspect(args: Value) -> McpToolResult {
    let archive_path_str = match args.get("archive_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: archive_path", None),
    };
    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        return McpToolResult::err_structured(
            "FILE_NOT_FOUND",
            format!("Archive not found: {}", archive_path.display()),
            Some(json!({ "archive_path": archive_path_str })),
        );
    }

    let (volumes, data) = match read_archive_data_auto(archive_path) {
        Ok(res) => res,
        Err(e) => return McpToolResult::err_structured(
            "ARCHIVE_READ_FAILED",
            format!("Failed to read archive: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    };

    let (format_detected, entries) = match parse_archive_entries(archive_path, &data) {
        Ok(res) => res,
        Err(e) => return McpToolResult::err_structured(
            "ARCHIVE_PARSE_FAILED",
            format!("Failed to parse archive: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    };

    let file_size = fs::metadata(archive_path).map(|m| m.len()).unwrap_or(0);
    let total_uncompressed: u64 = entries.iter().map(|e| e.uncompressed_size).sum();
    let total_compressed: u64 = entries.iter().map(|e| e.compressed_size).sum();
    let dir_count = entries.iter().filter(|e| e.is_directory).count();
    let file_count = entries.len() - dir_count;
    let is_encrypted = entries.iter().any(|e| e.is_encrypted);

    let ratio = if total_uncompressed > 0 {
        ((total_compressed as f64) / (total_uncompressed as f64)) * 100.0
    } else {
        100.0
    };

    let res = json!({
        "archive_path": archive_path_str,
        "format": format_detected.as_str(),
        "file_size_bytes": file_size,
        "file_size_formatted": format_bytes(file_size),
        "total_entries": entries.len(),
        "file_count": file_count,
        "directory_count": dir_count,
        "total_uncompressed_bytes": total_uncompressed,
        "total_uncompressed_formatted": format_bytes(total_uncompressed),
        "compression_ratio_percent": format!("{:.1}%", ratio),
        "is_encrypted": is_encrypted,
        "is_multi_volume": volumes.len() > 1,
        "volume_count": volumes.len()
    });

    McpToolResult::ok_json(&res).unwrap_or_else(McpToolResult::err)
}

fn handle_cat(args: Value) -> McpToolResult {
    let archive_path_str = match args.get("archive_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: archive_path", None),
    };
    let entry_path = match args.get("entry_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: entry_path", None),
    };

    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        return McpToolResult::err_structured(
            "FILE_NOT_FOUND",
            format!("Archive not found: {}", archive_path.display()),
            Some(json!({ "archive_path": archive_path_str })),
        );
    }

    // Clamp max_bytes to [1, 10MB] to match schema upper bound
    let max_bytes = (args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(1048576) as usize).clamp(1, 10 * 1024 * 1024);
    let password = args.get("password").and_then(|v| v.as_str());

    let (_volumes, data) = match read_archive_data_auto(archive_path) {
        Ok(res) => res,
        Err(e) => return McpToolResult::err_structured(
            "ARCHIVE_READ_FAILED",
            format!("Failed to read archive: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    };

    let (_fmt, entries) = match parse_archive_entries(archive_path, &data) {
        Ok(res) => res,
        Err(e) => return McpToolResult::err_structured(
            "ARCHIVE_PARSE_FAILED",
            format!("Failed to parse archive: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    };

    let entry = entries.iter().find(|e| e.relative_path == entry_path || e.name == entry_path);
    let entry = match entry {
        Some(e) => e,
        None => return McpToolResult::err_structured(
            "ENTRY_NOT_FOUND",
            format!("Entry '{}' not found in archive", entry_path),
            Some(json!({ "archive_path": archive_path_str, "entry_path": entry_path })),
        ),
    };

    if entry.is_directory {
        return McpToolResult::err_structured(
            "IS_DIRECTORY",
            format!("Entry '{}' is a directory", entry_path),
            Some(json!({ "archive_path": archive_path_str, "entry_path": entry_path })),
        );
    }

    // Safety defense: prohibit decompressing unbounded entries (>64MB) into memory to prevent OOM
    const MAX_IN_MEMORY_DECOMPRESS_BYTES: u64 = 64 * 1024 * 1024;
    if entry.uncompressed_size > MAX_IN_MEMORY_DECOMPRESS_BYTES {
        return McpToolResult::err_structured(
            "PAYLOAD_TOO_LARGE",
            format!(
                "Entry uncompressed size ({} bytes) exceeds in-memory safety limit ({} bytes). Use ttzip_extract instead.",
                entry.uncompressed_size, MAX_IN_MEMORY_DECOMPRESS_BYTES
            ),
            Some(json!({
                "entry_path": entry_path,
                "uncompressed_size": entry.uncompressed_size,
                "max_allowed_bytes": MAX_IN_MEMORY_DECOMPRESS_BYTES
            })),
        );
    }

    // Zero-disk direct in-memory stream extraction via unified microkernel
    match extract_single_entry_memory(archive_path, Some(&entry.relative_path), -1, password) {
        Ok(bytes) => {
            let truncated = bytes.len() > max_bytes;
            let slice = if truncated { &bytes[..max_bytes] } else { &bytes };
            let content_str = String::from_utf8_lossy(slice).to_string();
            let res = json!({
                "archive_path": archive_path_str,
                "entry_path": entry_path,
                "size_bytes": bytes.len(),
                "returned_bytes": slice.len(),
                "truncated": truncated,
                "content": content_str
            });
            McpToolResult::ok_json(&res).unwrap_or_else(McpToolResult::err)
        }
        Err(TTZipStatus::ErrInvalidPassword) => McpToolResult::err_structured(
            "PASSWORD_REQUIRED",
            format!("Password required or invalid for entry '{}'", entry_path),
            Some(json!({ "archive_path": archive_path_str, "entry_path": entry_path })),
        ),
        Err(TTZipStatus::ErrFileNotFound) => McpToolResult::err_structured(
            "ENTRY_NOT_FOUND",
            format!("Entry '{}' not found in archive", entry_path),
            Some(json!({ "archive_path": archive_path_str, "entry_path": entry_path })),
        ),
        Err(TTZipStatus::ErrSecurityViolation) => McpToolResult::err_structured(
            "SECURITY_VIOLATION",
            format!("Path traversal security violation for entry '{}'", entry_path),
            Some(json!({ "archive_path": archive_path_str, "entry_path": entry_path })),
        ),
        Err(status) => McpToolResult::err_structured(
            "DECOMPRESSION_FAILED",
            format!("Failed to decompress entry '{}': {:?}", entry_path, status),
            Some(json!({ "archive_path": archive_path_str, "entry_path": entry_path, "status_code": status.to_i32() })),
        ),
    }
}

fn handle_verify(args: Value) -> McpToolResult {
    let archive_path_str = match args.get("archive_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: archive_path", None),
    };
    let archive_path = Path::new(archive_path_str);
    if !archive_path.exists() {
        return McpToolResult::err_structured(
            "FILE_NOT_FOUND",
            format!("Archive not found: {}", archive_path.display()),
            Some(json!({ "archive_path": archive_path_str })),
        );
    }

    let password = args.get("password").and_then(|v| v.as_str());
    let deep = args.get("deep_scan").or_else(|| args.get("deep")).and_then(|v| v.as_bool()).unwrap_or(true);

    match crate::cli::handlers::check::check_archive_integrity(archive_path, password, deep) {
        Ok(dto) => {
            if dto.is_valid {
                McpToolResult::ok_json(&dto).unwrap_or_else(McpToolResult::err)
            } else {
                McpToolResult::err_structured(
                    "INTEGRITY_CHECK_FAILED",
                    format!("Archive integrity check failed with {} corrupted entries", dto.corrupted_entries),
                    Some(json!(dto)),
                )
            }
        }
        Err(e) => McpToolResult::err_structured(
            "CORRUPTED_ARCHIVE",
            format!("Archive verification error: {}", e),
            Some(json!({ "archive_path": archive_path_str, "error": e })),
        ),
    }
}

fn handle_convert(args: Value) -> McpToolResult {
    let src_str = match args.get("source_archive").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: source_archive", None),
    };
    let dst_str = match args.get("destination_archive").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: destination_archive", None),
    };

    let src = Path::new(src_str);
    let dst = Path::new(dst_str);
    let format = args.get("target_format").and_then(|v| v.as_str());
    let level = args.get("level").and_then(|v| v.as_u64()).unwrap_or(6) as u8;

    match execute_convert_silent(src, dst, format, level) {
        Ok(_) => {
            let res = json!({
                "status": "converted",
                "source_archive": src_str,
                "destination_archive": dst_str,
                "level": level
            });
            McpToolResult::ok_json(&res).unwrap_or_else(McpToolResult::err)
        }
        Err(e) => McpToolResult::err_structured(
            "CONVERSION_FAILED",
            format!("Conversion failed: {}", e),
            Some(json!({ "source_archive": src_str, "destination_archive": dst_str, "error": e })),
        ),
    }
}

fn handle_repair(args: Value) -> McpToolResult {
    let damaged_str = match args.get("damaged_archive").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: damaged_archive", None),
    };
    let output_str = match args.get("output_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::err_structured("INVALID_PARAMS", "Missing required parameter: output_path", None),
    };

    let damaged = Path::new(damaged_str);
    let output = Path::new(output_str);
    let format_override = args.get("format_override").and_then(|v| v.as_str());

    match execute_repair_silent(damaged, output, format_override) {
        Ok(salvaged_entries) => {
            let res = json!({
                "success": true,
                "damaged_archive": damaged_str,
                "repaired_archive": output_str,
                "salvaged_entries": salvaged_entries
            });
            McpToolResult::ok_json(&res).unwrap_or_else(McpToolResult::err)
        }
        Err(e) => McpToolResult::err_structured(
            "REPAIR_FAILED",
            format!("Repair failed: {}", e),
            Some(json!({ "damaged_archive": damaged_str, "output_path": output_str, "error": e })),
        ),
    }
}
