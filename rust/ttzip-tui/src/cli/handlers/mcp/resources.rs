// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! MCP Resources provider for static technical manuals and dynamic archive:// inspection.

use crate::cli::format::{format_bytes, parse_archive_entries, read_archive_data_auto};
use crate::cli::handlers::mcp::protocol::ResourceContentEntry;
use serde_json::{json, Value};
use std::path::Path;

/// Returns registered static and dynamic MCP resource templates.
pub fn get_resource_definitions() -> Vec<Value> {
    vec![
        json!({
            "uri": "ttzip://manuals/overview",
            "name": "TTZip Architecture & CLI Overview",
            "description": "General architecture, capability boundaries, supported container formats, and CLI invocation overview",
            "mimeType": "text/markdown"
        }),
        json!({
            "uri": "ttzip://manuals/format-matrix",
            "name": "TTZip Archive Format Matrix & Codec Tradeoffs",
            "description": "Comprehensive comparison of ZIP, 7z, TAR, Snappy, and Brotli with throughput and compression benchmarks",
            "mimeType": "text/markdown"
        }),
        json!({
            "uri": "ttzip://manuals/safe-extraction",
            "name": "Defensive Extraction & Anti-Tar-Bomb Guidelines",
            "description": "Best practices for AI agents to safely unpack untrusted archives, avoid path traversal, and prevent host directory pollution",
            "mimeType": "text/markdown"
        }),
        json!({
            "uri": "ttzip://manuals/troubleshooting",
            "name": "Archive Diagnostics & Troubleshooting Manual",
            "description": "Guide for diagnosing corrupted archives, missing multi-volume spans, encrypted headers, and unknown charsets",
            "mimeType": "text/markdown"
        }),
    ]
}

/// Resolves a resource URI to its textual content.
pub fn read_resource(uri: &str) -> Result<ResourceContentEntry, String> {
    if uri.starts_with("ttzip://manuals/") {
        let topic = uri.trim_start_matches("ttzip://manuals/");
        let text = match topic {
            "overview" => MANUAL_OVERVIEW,
            "format-matrix" => MANUAL_FORMAT_MATRIX,
            "safe-extraction" => MANUAL_SAFE_EXTRACTION,
            "troubleshooting" => MANUAL_TROUBLESHOOTING,
            _ => return Err(format!("Unknown manual topic: '{}'", topic)),
        };
        Ok(ResourceContentEntry {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(text.to_string()),
        })
    } else if uri.starts_with("archive://") {
        let archive_path_str = uri.trim_start_matches("archive://");
        let archive_path = Path::new(archive_path_str);
        if !archive_path.exists() {
            return Err(format!("Archive file not found: {}", archive_path.display()));
        }

        let (volumes, data) = read_archive_data_auto(archive_path)?;
        let (fmt, entries) = parse_archive_entries(archive_path, &data)?;

        let mut doc = format!(
            "# Virtual Archive Hierarchy: {}\n\n- **Format**: {}\n- **Volumes**: {}\n- **Total Entries**: {}\n\n| Type | Path | Size | Compressed | CRC32 | Encrypted |\n| :--- | :--- | :--- | :--- | :--- | :--- |\n",
            archive_path.file_name().and_then(|n| n.to_str()).unwrap_or("archive"),
            fmt.as_str(),
            volumes.len(),
            entries.len()
        );

        for e in entries.iter().take(500) {
            let type_icon = if e.is_directory { "DIR" } else { "FILE" };
            let enc_str = if e.is_encrypted { "YES" } else { "NO" };
            doc.push_str(&format!(
                "| {} | {} | {} | {} | {:08x} | {} |\n",
                type_icon,
                e.relative_path,
                format_bytes(e.uncompressed_size),
                format_bytes(e.compressed_size),
                e.crc32,
                enc_str
            ));
        }

        if entries.len() > 500 {
            doc.push_str(&format!("\n*... and {} additional entries truncated for brevity.*", entries.len() - 500));
        }

        Ok(ResourceContentEntry {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(doc),
        })
    } else {
        Err(format!("Unsupported resource URI scheme: '{}'", uri))
    }
}

// ---------------------------------------------------------------------------
// Static Manual Texts
// ---------------------------------------------------------------------------

const MANUAL_OVERVIEW: &str = r#"# TTZip System Architecture & MCP Capabilities

TTZip is a zero-dependency high-throughput archiving and compression microkernel.
When running as an MCP server via `ttzip mcp`, it provides:

## Core Tools
- `ttzip_list`: Non-destructive inspection of archive table of contents.
- `ttzip_extract`: Sandboxed extraction with multi-core parallelism and Zip Slip defense.
- `ttzip_create`: Fast compression supporting ZIP, 7z, TAR, Snappy, and Brotli.
- `ttzip_inspect`: Metadata, header structure, encryption, and volume analysis.
- `ttzip_cat`: Direct in-memory stream extraction without temporary disk allocations.
- `ttzip_verify`: Deep CRC and payload integrity verification.
- `ttzip_convert`: In-process container transcoding.
- `ttzip_repair`: Non-destructive header salvage.
"#;

const MANUAL_FORMAT_MATRIX: &str = r#"# TTZip Format Matrix & Codec Tradeoffs

| Container | Codec Engine | Compression Ratio | Speed Profile | Encryption | Streaming |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **ZIP** | Deflate / Deflate64 / Store | Standard (30-40%) | Ultra-High (>1.5 GB/s) | AES-256 / PKZip | Full Random Access |
| **7z** | LZMA2 / BCJ / PPMd | Ultra-High (60-80%) | Moderate (~100-300 MB/s) | AES-256 (Headers+Data) | Solid Block Streams |
| **TAR** | Raw Posix ustar/pax | None (1:1) | Maximum (Disk Bound) | None | Pure Stream |
| **Brotli** | Brotli (RFC 7932) | High (Text/Web 70%) | High (~500 MB/s) | None | Chunked Streams |
| **Snappy** | Google Snappy Framed | Low-Medium (15-25%) | Realtime (>2.5 GB/s) | None | Framed Stream |

## Compression Level Guidelines
- Level 0: Store (zero CPU compression overhead, archive container only)
- Level 1-3: Fast (prioritizes throughput over size reduction)
- Level 6: Default (balanced Pareto trade-off for general usage)
- Level 9: Maximum (optimal for software distribution bundles)
- Level 12: Ultra LZMA2/Zopfli (maximum achievable compression ratio)
"#;

const MANUAL_SAFE_EXTRACTION: &str = r#"# Defensive Extraction & Sandboxing Rules for AI Agents

When executing extraction tasks on untrusted archives:

1. **Avoid Tar-Bombs**: Always specify an explicit destination subfolder (e.g. `destination: "extracted_payload/"`) rather than unpacking directly into the agent working directory.
2. **Zip Slip Defense**: TTZip enforces path canonicalization and rejects entries attempting directory escape (`../` or leading `/`).
3. **Inspect First**: Execute `ttzip_inspect` or `ttzip_list` before calling `ttzip_extract` to gauge total uncompressed size and verify disk availability.
4. **Targeted Cat**: If you only need to read 1-2 configuration files, call `ttzip_cat` to avoid decompressing gigabytes of data.
"#;

const MANUAL_TROUBLESHOOTING: &str = r#"# TTZip Troubleshooting & Diagnostic Protocols

## Common Issues & Remediation

1. **Password Required**: If `ttzip_extract` returns a password requirement error, re-invoke with `password: "<secret>"`.
2. **Multi-Volume Archives**: For split archives (`.7z.001`, `.z01`), ensure all volume parts reside in the same directory before extraction.
3. **Corrupted Archive Headers**: Call `ttzip_repair` to salvage uncorrupted files and reconstruct the central directory.
4. **Non-UTF-8 Encodings**: TTZip automatically normalizes legacy character encodings (GBK, Shift-JIS, CP437) to UTF-8.
"#;
