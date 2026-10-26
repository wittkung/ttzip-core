// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

/// Required fixture corpus selected once before parallel tests begin reading it.
/// Managed runs use only the BUILD-provided runfiles root; a manual upstream root
/// must be explicitly requested and is never reported as owned acceptance evidence.
pub struct FixtureCorpus {
    pub root: PathBuf,
    pub managed: bool,
    pub filenames: Vec<String>,
}

pub fn fixture_corpus() -> &'static FixtureCorpus {
    static CORPUS: std::sync::OnceLock<FixtureCorpus> = std::sync::OnceLock::new();
    CORPUS.get_or_init(|| {
        if let Some(root) = std::env::var_os("TTZIP_MANUAL_LIBARCHIVE_FIXTURE_DIR") {
            let root = PathBuf::from(root);
            assert!(root.is_dir(), "explicit manual fixture directory is missing: {}", root.display());
            eprintln!("MANUAL libarchive fixture source: {} (not managed acceptance)", root.display());
            return FixtureCorpus { root, managed: false, filenames: Vec::new() };
        }
        let root = std::env::var_os("TTZIP_LIBARCHIVE_FIXTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                let local_tp = manifest_dir.join("../../third_party/testdata/libarchive-corrupted");
                if local_tp.join("manifest.json").exists() {
                    return local_tp;
                }
                panic!("BUILD must provide TTZIP_LIBARCHIVE_FIXTURE_DIR or repository-local third_party/testdata/libarchive-corrupted; personal HOME/compile directories are never probed");
            });
        let data = fs::read(root.join("manifest.json")).expect("required managed fixture manifest");
        let manifest: serde_json::Value = serde_json::from_slice(&data).expect("fixture manifest JSON");
        assert_eq!(manifest["pinned_commit"], "03b426003adf12c1eb4c34e72bedb4c2bf1ba8d3");
        assert_eq!(manifest["official_repository"], "https://github.com/libarchive/libarchive");
        let entries = manifest["fixtures"].as_array().expect("required fixture list");
        assert_eq!(entries.len(), 26, "all 26 named corpus assets are required");
        let mut filenames = Vec::new();
        for entry in entries {
            let filename = format!("{}.uu", entry["owned_name"].as_str().expect("fixture name"));
            assert_eq!(PathBuf::from(&filename).components().count(), 1);
            let bytes = fs::read(root.join(&filename)).unwrap_or_else(|e|
                panic!("required owned fixture {filename} is missing: {e}"));
            use sha2::{Digest, Sha256};
            assert_eq!(hex::encode(Sha256::digest(&bytes)),
                entry["raw_sha256"].as_str().expect("fixture SHA256"),
                "owned fixture identity differs: {filename}");
            filenames.push(filename);
        }
        filenames.sort();
        filenames.dedup();
        assert_eq!(filenames.len(), 26, "owned fixture list must be unique");
        eprintln!("MANAGED libarchive fixture source: {} pin=03b426003adf12c1eb4c34e72bedb4c2bf1ba8d3 count=26", root.display());
        FixtureCorpus { root, managed: true, filenames }
    })
}

/// Every requested corpus asset is mandatory. Missing/invalid assets fail instead
/// of silently skipping a specialized test or reducing the corpus sweep.
pub fn load_libarchive_asset(filename: &str) -> Vec<u8> {
    let corpus = fixture_corpus();
    let uu_name = format!("{filename}.uu");
    if corpus.managed {
        assert!(corpus.filenames.contains(&uu_name), "fixture outside owned manifest: {filename}");
    } else {
        let direct = corpus.root.join(filename);
        if direct.is_file() {
            return fs::read(direct).expect("read explicit manual fixture");
        }
    }
    let bytes = fs::read(corpus.root.join(&uu_name))
        .unwrap_or_else(|e| panic!("required fixture {uu_name} is missing: {e}"));
    uudecode(&bytes).unwrap_or_else(|| panic!("required fixture {uu_name} is not UU encoded"))
}

/// Writes byte payload to temporary directory and returns handle with full path.
pub fn write_temp_archive(filename: &str, data: &[u8]) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = dir.path().join(filename);
    fs::write(&path, data).expect("failed to write temp archive file");
    (dir, path)
}
