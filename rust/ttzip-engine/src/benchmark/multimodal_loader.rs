// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-modal real-world file loader and corpus benchmark asset manager.
//!
//! Automatically probes the filesystem for real-world test assets:
//! - Silesia Multi-Modal Corpus (12 entities, 211.9MB)
//! - Fat Mach-O universal static archive (`libTTZipVendor.a`, ~100MB)
//! - Vector PDF document (`test.pdf`, ~38MB)
//! - 4K image samples (JPEG/PNG/BMP/HDR/PSD)
//!
//! If any file is missing (e.g. running in an isolated CI container without local fixtures),
//! it gracefully degrades to the corresponding deterministic mathematical synthetic generator.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use super::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
pub use crate::analytics::entropy::compute_shannon_entropy;

/// Standard Silesia filenames and synthetic category mappings.
pub const SILESIA_STANDARD_FILES: &[(&str, usize, BenchmarkCorpusType)] = &[
    ("dickens", 10_192_446, BenchmarkCorpusType::TextData),
    ("mozilla", 51_220_480, BenchmarkCorpusType::MachOBinary),
    ("mr", 9_970_564, BenchmarkCorpusType::RealisticRgb),
    ("nci", 33_553_445, BenchmarkCorpusType::ShortMatch),
    ("ooffice", 6_152_192, BenchmarkCorpusType::MachOBinary),
    ("osdb", 10_085_684, BenchmarkCorpusType::ShortMatch),
    ("reymont", 6_627_202, BenchmarkCorpusType::TextData),
    ("samba", 21_606_400, BenchmarkCorpusType::MachOBinary),
    ("sao", 7_251_944, BenchmarkCorpusType::Literals),
    ("webster", 41_458_703, BenchmarkCorpusType::TextData),
    ("xml", 5_345_280, BenchmarkCorpusType::Xml),
    ("x-ray", 8_474_240, BenchmarkCorpusType::RealisticRgb),
];

/// Kind of multi-modal corpus dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MultimodalCorpusKind {
    /// Silesia multi-modal corpus (12 standard files).
    Silesia,
    /// Fat Mach-O archive (`libTTZipVendor.a`).
    MachOArchive,
    /// Vector PDF document (`test.pdf`).
    PdfDocument,
    /// High-resolution / 4K image samples (JPEG/PNG/BMP/HDR/PSD).
    ImageSample,
    /// Mathematical synthetic fallback.
    Synthetic(BenchmarkCorpusType),
}

/// Metadata descriptor for a loaded corpus entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalCorpusEntry {
    pub name: String,
    pub kind: MultimodalCorpusKind,
    pub source_path: Option<PathBuf>,
    pub size_bytes: usize,
    pub shannon_entropy: f64,
    pub is_synthetic: bool,
    #[serde(skip)]
    pub data: Arc<Vec<u8>>,
}

impl MultimodalCorpusEntry {
    /// Creates a new entry from raw bytes.
    pub fn from_bytes(
        name: impl Into<String>,
        kind: MultimodalCorpusKind,
        source_path: Option<PathBuf>,
        data: Vec<u8>,
        is_synthetic: bool,
    ) -> Self {
        let entropy = compute_shannon_entropy(&data);
        let size = data.len();
        Self {
            name: name.into(),
            kind,
            source_path,
            size_bytes: size,
            shannon_entropy: entropy,
            is_synthetic,
            data: Arc::new(data),
        }
    }
}

/// Real-world and synthetic multi-modal corpus loader.
pub struct MultimodalCorpusLoader {
    cache: RwLock<HashMap<String, MultimodalCorpusEntry>>,
    probe_roots: Vec<PathBuf>,
}

impl MultimodalCorpusLoader {
    /// Creates a new loader with automatically discovered project probe roots.
    pub fn new() -> Self {
        Self::with_roots(Self::default_probe_roots())
    }

    /// Creates a loader with explicit probe roots.
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            probe_roots: roots,
        }
    }

    /// Global shared singleton loader.
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<MultimodalCorpusLoader> = OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    /// Finds candidate probe roots based on environment variables and filesystem layout.
    pub fn default_probe_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Ok(env_root) = std::env::var("TTZIP_PROJECT_ROOT") {
            let p = PathBuf::from(env_root);
            if p.exists() {
                roots.push(p);
            }
        }

        if let Ok(env_corpus) = std::env::var("TTZIP_CORPUS_ROOT") {
            let p = PathBuf::from(env_corpus);
            if p.exists() {
                roots.push(p);
            }
        }

        if let Some(proj_root) = Self::find_project_root() {
            if !roots.contains(&proj_root) {
                roots.push(proj_root);
            }
        }

        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = PathBuf::from(manifest_dir);
            if !roots.contains(&p) {
                roots.push(p.clone());
            }
            if let Some(parent) = p.parent().and_then(|p| p.parent()) {
                let parent_buf = parent.to_path_buf();
                if !roots.contains(&parent_buf) {
                    roots.push(parent_buf);
                }
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            if !roots.contains(&cwd) {
                roots.push(cwd);
            }
        }

        roots
    }

    /// Traverses upward from current directory or manifest directory looking for repository root.
    pub fn find_project_root() -> Option<PathBuf> {
        let start_paths: Vec<PathBuf> = [
            std::env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from),
            std::env::current_dir().ok(),
        ]
        .into_iter()
        .flatten()
        .collect();

        for start in start_paths {
            let mut curr = start.as_path();
            for _ in 0..8 {
                // Look for key anchor directories
                if curr.join("core/Tests/TTZipTests/Fixtures/Silesia").exists()
                    || curr.join("Tests/TTZipTests/Fixtures/Silesia").exists()
                    || curr.join("vendor/lopdf/assets/test.pdf").exists()
                {
                    // If curr is `core`, check if its parent has `vendor`
                    if curr.file_name().and_then(|n| n.to_str()) == Some("core") {
                        if let Some(parent) = curr.parent() {
                            return Some(parent.to_path_buf());
                        }
                    }
                    return Some(curr.to_path_buf());
                }
                if let Some(parent) = curr.parent() {
                    curr = parent;
                } else {
                    break;
                }
            }
        }
        None
    }

    // MARK: - Silesia Corpus Loading

    /// Loads an individual Silesia file by name, falling back to synthetic data if missing.
    pub fn load_silesia_file(&self, name: &str) -> MultimodalCorpusEntry {
        let cache_key = format!("silesia:{}", name);
        if let Ok(guard) = self.cache.read() {
            if let Some(entry) = guard.get(&cache_key) {
                return entry.clone();
            }
        }

        let entry = self.resolve_silesia_file(name);
        if let Ok(mut guard) = self.cache.write() {
            guard.insert(cache_key, entry.clone());
        }
        entry
    }

    fn resolve_silesia_file(&self, name: &str) -> MultimodalCorpusEntry {
        // Probe relative candidates
        let candidate_rel_paths = [
            format!("core/Tests/TTZipTests/Fixtures/Silesia/{}", name),
            format!("Tests/TTZipTests/Fixtures/Silesia/{}", name),
            format!("Fixtures/Silesia/{}", name),
            format!("../Tests/TTZipTests/Fixtures/Silesia/{}", name),
            format!("../../Tests/TTZipTests/Fixtures/Silesia/{}", name),
        ];

        for root in &self.probe_roots {
            for rel in &candidate_rel_paths {
                let full = root.join(rel);
                if full.is_file() {
                    if let Ok(bytes) = fs::read(&full) {
                        return MultimodalCorpusEntry::from_bytes(
                            name,
                            MultimodalCorpusKind::Silesia,
                            Some(full),
                            bytes,
                            false,
                        );
                    }
                }
            }
        }

        // Graceful fallback to synthetic data
        let (default_size, fallback_type) = SILESIA_STANDARD_FILES
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, s, t)| (*s, *t))
            .unwrap_or((4096, BenchmarkCorpusType::Silesia));

        let synthetic_data = BenchmarkCorpusGenerator::generate(fallback_type, default_size.min(1024 * 1024));
        MultimodalCorpusEntry::from_bytes(
            name,
            MultimodalCorpusKind::Synthetic(fallback_type),
            None,
            synthetic_data,
            true,
        )
    }

    /// Loads all 12 Silesia standard files.
    pub fn load_all_silesia(&self) -> Vec<MultimodalCorpusEntry> {
        SILESIA_STANDARD_FILES
            .iter()
            .map(|(name, _, _)| self.load_silesia_file(name))
            .collect()
    }

    // MARK: - Mach-O Universal Vendor Archive Loading

    /// Loads the `libTTZipVendor.a` Fat Mach-O archive, or generates synthetic binary instructions on fallback.
    pub fn load_macho_vendor_archive(&self, max_bytes: Option<usize>) -> MultimodalCorpusEntry {
        let cache_key = format!("macho:max_{:?}", max_bytes);
        if let Ok(guard) = self.cache.read() {
            if let Some(entry) = guard.get(&cache_key) {
                return entry.clone();
            }
        }

        let entry = self.resolve_macho_vendor_archive(max_bytes);
        if let Ok(mut guard) = self.cache.write() {
            guard.insert(cache_key, entry.clone());
        }
        entry
    }

    fn resolve_macho_vendor_archive(&self, max_bytes: Option<usize>) -> MultimodalCorpusEntry {
        let candidate_rel_paths = [
            "core/Frameworks/TTZipVendor.xcframework/macos-arm64_x86_64/libTTZipVendor.a",
            "core/Frameworks/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a",
            "Frameworks/TTZipVendor.xcframework/macos-arm64_x86_64/libTTZipVendor.a",
            "Frameworks/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a",
            "../../Frameworks/TTZipVendor.xcframework/macos-arm64_x86_64/libTTZipVendor.a",
            "../../Frameworks/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a",
        ];

        for root in &self.probe_roots {
            for rel in &candidate_rel_paths {
                let full = root.join(rel);
                if full.is_file() {
                    if let Ok(mut bytes) = fs::read(&full) {
                        if let Some(limit) = max_bytes {
                            if bytes.len() > limit {
                                bytes.truncate(limit);
                            }
                        }
                        return MultimodalCorpusEntry::from_bytes(
                            "libTTZipVendor.a",
                            MultimodalCorpusKind::MachOArchive,
                            Some(full),
                            bytes,
                            false,
                        );
                    }
                }
            }
        }

        // Graceful synthetic fallback
        let target_size = max_bytes.unwrap_or(1024 * 1024);
        let synthetic_data = BenchmarkCorpusGenerator::gen_binary_macho_data(target_size);
        MultimodalCorpusEntry::from_bytes(
            "libTTZipVendor.a (synthetic)",
            MultimodalCorpusKind::Synthetic(BenchmarkCorpusType::MachOBinary),
            None,
            synthetic_data,
            true,
        )
    }

    // MARK: - PDF Document Loading

    /// Loads `vendor/lopdf/assets/test.pdf`, falling back to structured short-match data if missing.
    pub fn load_test_pdf(&self, max_bytes: Option<usize>) -> MultimodalCorpusEntry {
        let cache_key = format!("pdf:test.pdf:max_{:?}", max_bytes);
        if let Ok(guard) = self.cache.read() {
            if let Some(entry) = guard.get(&cache_key) {
                return entry.clone();
            }
        }

        let entry = self.resolve_test_pdf(max_bytes);
        if let Ok(mut guard) = self.cache.write() {
            guard.insert(cache_key, entry.clone());
        }
        entry
    }

    fn resolve_test_pdf(&self, max_bytes: Option<usize>) -> MultimodalCorpusEntry {
        let candidate_rel_paths = [
            "vendor/lopdf/assets/test.pdf",
            "../vendor/lopdf/assets/test.pdf",
            "../../vendor/lopdf/assets/test.pdf",
            "../../../vendor/lopdf/assets/test.pdf",
        ];

        for root in &self.probe_roots {
            for rel in &candidate_rel_paths {
                let full = root.join(rel);
                if full.is_file() {
                    if let Ok(mut bytes) = fs::read(&full) {
                        if let Some(limit) = max_bytes {
                            if bytes.len() > limit {
                                bytes.truncate(limit);
                            }
                        }
                        return MultimodalCorpusEntry::from_bytes(
                            "test.pdf",
                            MultimodalCorpusKind::PdfDocument,
                            Some(full),
                            bytes,
                            false,
                        );
                    }
                }
            }
        }

        // Graceful fallback
        let target_size = max_bytes.unwrap_or(512 * 1024);
        let synthetic_data = BenchmarkCorpusGenerator::gen_short_match_data(target_size);
        MultimodalCorpusEntry::from_bytes(
            "test.pdf (synthetic)",
            MultimodalCorpusKind::Synthetic(BenchmarkCorpusType::ShortMatch),
            None,
            synthetic_data,
            true,
        )
    }

    // MARK: - 4K Image Sample Loading

    /// Loads image samples from `vendor/zune-image/test-images/`, or falls back to realistic RGB gradients.
    pub fn load_image_samples(&self, max_files: usize) -> Vec<MultimodalCorpusEntry> {
        let candidate_dirs = [
            "vendor/zune-image/test-images",
            "../vendor/zune-image/test-images",
            "../../vendor/zune-image/test-images",
            "../../../vendor/zune-image/test-images",
        ];

        let mut found_dir = None;
        for root in &self.probe_roots {
            for rel in &candidate_dirs {
                let full = root.join(rel);
                if full.is_dir() {
                    found_dir = Some(full);
                    break;
                }
            }
            if found_dir.is_some() {
                break;
            }
        }

        let mut entries = Vec::new();

        if let Some(dir) = found_dir {
            Self::collect_image_files(&dir, &mut entries, max_files);
        }

        if entries.is_empty() {
            // Graceful fallback to realistic RGB gradient and striped RGB images
            let rgb_grad = BenchmarkCorpusGenerator::gen_realistic_rgb_data(512 * 512 * 3);
            let rgb_stripes = BenchmarkCorpusGenerator::gen_striped_rgb_data(512 * 512 * 3);

            entries.push(MultimodalCorpusEntry::from_bytes(
                "synthetic_rgb_gradient_512x512.raw",
                MultimodalCorpusKind::Synthetic(BenchmarkCorpusType::RealisticRgb),
                None,
                rgb_grad,
                true,
            ));
            entries.push(MultimodalCorpusEntry::from_bytes(
                "synthetic_rgb_striped_512x512.raw",
                MultimodalCorpusKind::Synthetic(BenchmarkCorpusType::StripedRgb),
                None,
                rgb_stripes,
                true,
            ));
        }

        entries
    }

    fn collect_image_files(dir: &Path, out: &mut Vec<MultimodalCorpusEntry>, max_files: usize) {
        if out.len() >= max_files {
            return;
        }

        let Ok(read_dir) = fs::read_dir(dir) else {
            return;
        };

        for entry in read_dir.flatten() {
            if out.len() >= max_files {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                Self::collect_image_files(&path, out, max_files);
            } else if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "hdr" | "psd" | "qoi") {
                    if let Ok(bytes) = fs::read(&path) {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image")
                            .to_string();
                        out.push(MultimodalCorpusEntry::from_bytes(
                            name,
                            MultimodalCorpusKind::ImageSample,
                            Some(path),
                            bytes,
                            false,
                        ));
                    }
                }
            }
        }
    }
}

impl Default for MultimodalCorpusLoader {
    fn default() -> Self {
        Self::new()
    }
}
