// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! UUDecode parser and corrupted archive fixture loader for libarchive test assets.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Decodes standard UUEncoded ASCII byte string or file into binary payload.
pub fn uudecode(input: &[u8]) -> Option<Vec<u8>> {
    let content = std::str::from_utf8(input).ok()?;
    let mut decoded = Vec::new();
    let mut started = false;

    for raw_line in content.lines() {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if line.starts_with("begin ") {
            started = true;
            continue;
        }
        if !started {
            continue;
        }
        if line == "end" || line.is_empty() || line == "`" {
            if line == "end" {
                break;
            }
            continue;
        }

        let bytes = line.as_bytes();
        let len_char = bytes[0];
        // Line length is encoded in the first character
        let line_len = (len_char.wrapping_sub(b' ') & 0x3F) as usize;
        if line_len == 0 {
            continue;
        }

        let encoded_chunks = &bytes[1..];
        let mut line_bytes = Vec::with_capacity(line_len);

        let mut i = 0;
        while i < encoded_chunks.len() && line_bytes.len() < line_len {
            let c0 = encoded_chunks.get(i).copied().unwrap_or(b' ');
            let c1 = encoded_chunks.get(i + 1).copied().unwrap_or(b' ');
            let c2 = encoded_chunks.get(i + 2).copied().unwrap_or(b' ');
            let c3 = encoded_chunks.get(i + 3).copied().unwrap_or(b' ');
            i += 4;

            let b0 = c0.wrapping_sub(b' ') & 0x3F;
            let b1 = c1.wrapping_sub(b' ') & 0x3F;
            let b2 = c2.wrapping_sub(b' ') & 0x3F;
            let b3 = c3.wrapping_sub(b' ') & 0x3F;

            let out0 = (b0 << 2) | (b1 >> 4);
            let out1 = ((b1 & 0x0F) << 4) | (b2 >> 2);
            let out2 = ((b2 & 0x03) << 6) | b3;

            line_bytes.push(out0);
            if line_bytes.len() < line_len {
                line_bytes.push(out1);
            }
            if line_bytes.len() < line_len {
                line_bytes.push(out2);
            }
        }

        decoded.extend_from_slice(&line_bytes[..line_len.min(line_bytes.len())]);
    }

    if decoded.is_empty() && !started {
        None
    } else {
        Some(decoded)
    }
}

/// Locates the vendor libarchive test asset directory.
pub fn find_vendor_libarchive_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = vec![
        manifest_dir.join("../../../vendor/libarchive/libarchive/test"),
        manifest_dir.join("../../vendor/libarchive/libarchive/test"),
        manifest_dir.join("../vendor/libarchive/libarchive/test"),
        manifest_dir.join("vendor/libarchive/libarchive/test"),
        manifest_dir.join("../../../../../../products/ttzip/vendor/libarchive/libarchive/test"),
        manifest_dir.join("../../../../../../products/ttzip/core/upstream/libarchive/pr-3388-7z-aes/libarchive/test"),
    ];

    if let Ok(home) = std::env::var("HOME") {
        let h = PathBuf::from(home);
        candidates.push(h.join(".cache/metastudyline/upstream-workspaces/upstream/libarchive/pr-3388-7z-aes/libarchive/test"));
        candidates.push(h.join(".cache/metastudyline/upstream-workspaces/upstream/libarchive/pr-3391-crc32/libarchive/test"));
        candidates.push(h.join(".cache/metastudyline/upstream-workspaces/upstream/libarchive/pr-3393-preallocate/libarchive/test"));
        candidates.push(h.join("Documents/dev/products/ttzip/vendor/libarchive/libarchive/test"));
    }

    for candidate in &candidates {
        if candidate.exists() {
            return candidate.canonicalize().ok().or_else(|| Some(candidate.clone()));
        }
    }

    None
}

/// Locates and loads a reference test asset from libarchive vendor test directory.
pub fn load_libarchive_asset(filename: &str) -> Option<Vec<u8>> {
    let base_vendor = find_vendor_libarchive_dir()?;

    let direct_path = base_vendor.join(filename);
    if direct_path.exists() {
        if let Ok(bytes) = fs::read(&direct_path) {
            return Some(bytes);
        }
    }

    let uu_path = base_vendor.join(format!("{}.uu", filename));
    if uu_path.exists() {
        if let Ok(bytes) = fs::read(&uu_path) {
            if let Some(dec) = uudecode(&bytes) {
                return Some(dec);
            }
        }
    }

    None
}

/// Writes byte payload to temporary directory and returns handle with full path.
pub fn write_temp_archive(filename: &str, data: &[u8]) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = dir.path().join(filename);
    fs::write(&path, data).expect("failed to write temp archive file");
    (dir, path)
}
