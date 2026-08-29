// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified declarative benchmark corpus provider abstraction and registry.
//!
//! Provides decoupled, extensible, and thread-safe corpus generators:
//! 1. `BenchmarkCorpusProvider`: Core trait for corpus suppliers.
//! 2. `SyntheticCorpusProvider`: Wrapper for the 8 mathematical synthetic generators.
//! 3. `RealWorldCorpusProvider`: Wrapper for real-world multi-modal fixtures (Silesia 12, Mach-O, PDF, 4K images).
//! 4. `CorpusRegistry`: URI-based resolution and registry lookup (`synthetic:dna`, `silesia:dickens`, `real:macho`).

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use crate::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use crate::benchmark::multimodal_loader::{MultimodalCorpusLoader, SILESIA_STANDARD_FILES};
use crate::types::TTZipStatus;

// ============================================================================
// Core BenchmarkCorpusProvider Trait
// ============================================================================

/// Unified trait for benchmark corpus providers (synthetic & real-world).
pub trait BenchmarkCorpusProvider: Send + Sync {
    /// Canonical identifier for this corpus (e.g., "synthetic:zipf_text", "silesia:dickens").
    fn corpus_id(&self) -> &'static str;

    /// Human-readable descriptive name of the corpus.
    fn display_name(&self) -> &'static str {
        self.corpus_id()
    }

    /// Technical description of the dataset characteristics.
    fn description(&self) -> &'static str {
        ""
    }

    /// Whether this provider generates mathematical synthetic data.
    fn is_synthetic(&self) -> bool {
        true
    }

    /// Generates or loads corpus byte buffer matching the requested size in bytes.
    ///
    /// If `size_bytes` is 0, returns the full default asset size or standard chunk.
    fn generate(&self, size_bytes: usize) -> Vec<u8>;
}

// ============================================================================
// Synthetic Corpus Provider (8 Mathematical Models)
// ============================================================================

/// Provider for mathematical synthetic benchmark corpora.
#[derive(Debug, Clone)]
pub struct SyntheticCorpusProvider {
    corpus_type: BenchmarkCorpusType,
    id: &'static str,
    name: &'static str,
    desc: &'static str,
}

impl SyntheticCorpusProvider {
    /// Creates a new synthetic provider for a given `BenchmarkCorpusType`.
    pub fn new(corpus_type: BenchmarkCorpusType) -> Self {
        let (id, desc) = match corpus_type {
            BenchmarkCorpusType::TextData | BenchmarkCorpusType::Calgary => (
                "synthetic:zipf_text",
                "Zipf power-law natural language text with 128-word vocabulary",
            ),
            BenchmarkCorpusType::ShortMatch => (
                "synthetic:short_match",
                "Rotating 8-slot short match pattern pool with intermittent RLE runs",
            ),
            BenchmarkCorpusType::Dna => (
                "synthetic:dna",
                "4-symbol DNA alphabet stressing deep hash collisions",
            ),
            BenchmarkCorpusType::Noise | BenchmarkCorpusType::Random => (
                "synthetic:noise",
                "Incompressible XorShift128+ white noise (~7.999 bits/byte entropy)",
            ),
            BenchmarkCorpusType::Literals => (
                "synthetic:literals",
                "High-entropy Huffman-coded literals (~6.5 bits/byte entropy)",
            ),
            BenchmarkCorpusType::MachOBinary | BenchmarkCorpusType::Binary => (
                "synthetic:macho",
                "64-bit Mach-O binary with ARM64 instructions and DWARF symbol records",
            ),
            BenchmarkCorpusType::RealisticRgb => (
                "synthetic:realistic_rgb",
                "24-bit RGB raster with 2D spatial gradients and per-pixel noise",
            ),
            BenchmarkCorpusType::StripedRgb => (
                "synthetic:striped_rgb",
                "24-bit RGB raster with 3 solid R/G/B stripes producing long matches",
            ),
            BenchmarkCorpusType::Xml => (
                "synthetic:xml",
                "Structured XML benchmark records with nested tags and attributes",
            ),
            BenchmarkCorpusType::Silesia => (
                "synthetic:silesia",
                "Composite multi-modal synthetic chunk interleaving text, binary, and RGB",
            ),
        };

        Self {
            corpus_type,
            id,
            name: corpus_type.name(),
            desc,
        }
    }

    /// Zipf natural language text generator.
    pub fn zipf_text() -> Self {
        Self::new(BenchmarkCorpusType::TextData)
    }

    /// Short match pool generator.
    pub fn short_match() -> Self {
        Self::new(BenchmarkCorpusType::ShortMatch)
    }

    /// DNA 4-symbol collision generator.
    pub fn dna() -> Self {
        Self::new(BenchmarkCorpusType::Dna)
    }

    /// Incompressible XorShift128+ white noise generator.
    pub fn noise() -> Self {
        Self::new(BenchmarkCorpusType::Noise)
    }

    /// High-entropy literals generator.
    pub fn literals() -> Self {
        Self::new(BenchmarkCorpusType::Literals)
    }

    /// Mach-O binary with ARM64 code and DWARF records generator.
    pub fn macho_binary() -> Self {
        Self::new(BenchmarkCorpusType::MachOBinary)
    }

    /// Realistic 24-bit RGB smooth gradient generator.
    pub fn realistic_rgb() -> Self {
        Self::new(BenchmarkCorpusType::RealisticRgb)
    }

    /// Striped 3-channel RGB raster generator.
    pub fn striped_rgb() -> Self {
        Self::new(BenchmarkCorpusType::StripedRgb)
    }

    /// XML structured markup generator.
    pub fn xml() -> Self {
        Self::new(BenchmarkCorpusType::Xml)
    }

    /// Returns a list of all 8 standard mathematical synthetic providers.
    pub fn all_standard_synthetic() -> Vec<Self> {
        vec![
            Self::zipf_text(),
            Self::short_match(),
            Self::dna(),
            Self::noise(),
            Self::literals(),
            Self::macho_binary(),
            Self::realistic_rgb(),
            Self::striped_rgb(),
            Self::xml(),
        ]
    }
}

impl BenchmarkCorpusProvider for SyntheticCorpusProvider {
    fn corpus_id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.desc
    }

    fn is_synthetic(&self) -> bool {
        true
    }

    fn generate(&self, size_bytes: usize) -> Vec<u8> {
        let size = if size_bytes == 0 { 64 * 1024 } else { size_bytes };
        BenchmarkCorpusGenerator::generate(self.corpus_type, size)
    }
}

// ============================================================================
// Real-World Corpus Provider (Silesia 12, Mach-O, PDF, 4K Images)
// ============================================================================

/// Real-world asset classification for loader targeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealWorldAssetKind {
    SilesiaEntity(&'static str),
    SilesiaAll,
    MachOArchive,
    PdfDocument,
    ImageSample,
}

/// Provider for real-world multi-modal benchmark datasets.
#[derive(Debug, Clone)]
pub struct RealWorldCorpusProvider {
    kind: RealWorldAssetKind,
    id: &'static str,
    name: &'static str,
    desc: &'static str,
}

impl RealWorldCorpusProvider {
    /// Creates a provider for an individual Silesia entity.
    pub fn silesia_entity(entity_name: &'static str) -> Self {
        let (id, name, desc): (&'static str, &'static str, &'static str) = match entity_name {
            "dickens" => ("silesia:dickens", "Silesia / Dickens (English Text)", "Charles Dickens collected works text"),
            "mozilla" => ("silesia:mozilla", "Silesia / Mozilla (Executable)", "Netscape/Mozilla tar executable archive"),
            "mr" => ("silesia:mr", "Silesia / MR (Medical Imaging)", "Medical magnetic resonance raw image data"),
            "nci" => ("silesia:nci", "Silesia / NCI (Chemical Database)", "National Cancer Institute chemical database"),
            "ooffice" => ("silesia:ooffice", "Silesia / OpenOffice (Shared Lib)", "OpenOffice 1.0 shared library dynamic code"),
            "osdb" => ("silesia:osdb", "Silesia / OSDB (Database Table)", "Open Source Database benchmark sample tables"),
            "reymont" => ("silesia:reymont", "Silesia / Reymont (Polish Text)", "Wladyslaw Reymont text in ISO-8859-2 encoding"),
            "samba" => ("silesia:samba", "Silesia / Samba (Tar Source)", "Samba source distribution tarball"),
            "sao" => ("silesia:sao", "Silesia / SAO (Star Catalog)", "Smithsonian Astrophysical Observatory star catalog"),
            "webster" => ("silesia:webster", "Silesia / Webster (HTML Dictionary)", "1913 Webster dictionary HTML text"),
            "xml" => ("silesia:xml", "Silesia / XML (Structured Data)", "XML structured technical dataset"),
            "x-ray" => ("silesia:x-ray", "Silesia / X-Ray (Medical Image)", "Medical X-ray raw 8-bit image data"),
            _ => ("silesia:custom", "Silesia / Custom Entity", "Silesia multi-modal entity"),
        };

        Self {
            kind: RealWorldAssetKind::SilesiaEntity(entity_name),
            id,
            name,
            desc,
        }
    }

    /// Creates a provider aggregating all 12 Silesia standard files.
    pub fn silesia_all() -> Self {
        Self {
            kind: RealWorldAssetKind::SilesiaAll,
            id: "silesia:all",
            name: "Silesia Corpus (Complete 12 Entities)",
            desc: "Full 211.9MB multi-modal Silesia benchmark standard dataset",
        }
    }

    /// Creates a provider for the Fat Mach-O universal static library (`libTTZipVendor.a`).
    pub fn macho_vendor() -> Self {
        Self {
            kind: RealWorldAssetKind::MachOArchive,
            id: "real:macho",
            name: "Fat Mach-O Universal Archive (libTTZipVendor.a)",
            desc: "Universal macOS ARM64/x86_64 static library archive with object tables",
        }
    }

    /// Creates a provider for a vector PDF document.
    pub fn test_pdf() -> Self {
        Self {
            kind: RealWorldAssetKind::PdfDocument,
            id: "real:pdf",
            name: "Vector PDF Document (test.pdf)",
            desc: "Complex vector PDF document with embedded fonts, streams, and xref tables",
        }
    }

    /// Creates a provider for 4K / high-resolution image samples.
    pub fn image_sample() -> Self {
        Self {
            kind: RealWorldAssetKind::ImageSample,
            id: "real:image",
            name: "4K / Hi-Res Image Sample",
            desc: "High-resolution photographic raster dataset",
        }
    }

    /// Returns all standard real-world providers (12 Silesia entities, SilesiaAll, Mach-O, PDF, Image).
    pub fn all_standard_real_world() -> Vec<Self> {
        let mut providers = Vec::with_capacity(16);
        for &(name, _, _) in SILESIA_STANDARD_FILES {
            providers.push(Self::silesia_entity(name));
        }
        providers.push(Self::silesia_all());
        providers.push(Self::macho_vendor());
        providers.push(Self::test_pdf());
        providers.push(Self::image_sample());
        providers
    }
}

impl BenchmarkCorpusProvider for RealWorldCorpusProvider {
    fn corpus_id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.desc
    }

    fn is_synthetic(&self) -> bool {
        false
    }

    fn generate(&self, size_bytes: usize) -> Vec<u8> {
        let loader = MultimodalCorpusLoader::global();
        match self.kind {
            RealWorldAssetKind::SilesiaEntity(name) => {
                let entry = loader.load_silesia_file(name);
                slice_or_tile_buffer(&entry.data, size_bytes)
            }
            RealWorldAssetKind::SilesiaAll => {
                let entries = loader.load_all_silesia();
                let mut combined = Vec::new();
                for entry in &entries {
                    combined.extend_from_slice(&entry.data);
                    if size_bytes > 0 && combined.len() >= size_bytes {
                        combined.truncate(size_bytes);
                        return combined;
                    }
                }
                if size_bytes > 0 && combined.len() < size_bytes {
                    slice_or_tile_buffer(&combined, size_bytes)
                } else {
                    combined
                }
            }
            RealWorldAssetKind::MachOArchive => {
                let limit = if size_bytes > 0 { Some(size_bytes) } else { None };
                let entry = loader.load_macho_vendor_archive(limit);
                slice_or_tile_buffer(&entry.data, size_bytes)
            }
            RealWorldAssetKind::PdfDocument => {
                let limit = if size_bytes > 0 { Some(size_bytes) } else { None };
                let entry = loader.load_test_pdf(limit);
                slice_or_tile_buffer(&entry.data, size_bytes)
            }
            RealWorldAssetKind::ImageSample => {
                let entries = loader.load_image_samples(1);
                if let Some(first) = entries.first() {
                    slice_or_tile_buffer(&first.data, size_bytes)
                } else {
                    BenchmarkCorpusGenerator::gen_realistic_rgb_data(size_bytes.max(64 * 1024))
                }
            }
        }
    }
}

// ============================================================================
// Custom File Provider (Dynamic Disk Fixture)
// ============================================================================

/// Provider for dynamic custom files loaded from the local filesystem.
#[derive(Debug, Clone)]
pub struct CustomFileCorpusProvider {
    file_path: String,
}

impl CustomFileCorpusProvider {
    /// Creates a new provider pointing to an absolute or relative filesystem path.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            file_path: path.into(),
        }
    }
}

impl BenchmarkCorpusProvider for CustomFileCorpusProvider {
    fn corpus_id(&self) -> &'static str {
        "file:custom"
    }

    fn display_name(&self) -> &'static str {
        "Custom Filesystem Corpus"
    }

    fn is_synthetic(&self) -> bool {
        false
    }

    fn generate(&self, size_bytes: usize) -> Vec<u8> {
        if let Ok(bytes) = fs::read(&self.file_path) {
            slice_or_tile_buffer(&bytes, size_bytes)
        } else {
            BenchmarkCorpusGenerator::gen_incompressible_noise(size_bytes.max(1024))
        }
    }
}

// ============================================================================
// Corpus Registry
// ============================================================================

/// Thread-safe unified registry for benchmark corpus providers with URI resolution.
pub struct CorpusRegistry {
    providers: RwLock<HashMap<String, Arc<dyn BenchmarkCorpusProvider>>>,
}

impl CorpusRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Global shared singleton registry initialized with all default standard providers.
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<CorpusRegistry> = OnceLock::new();
        INSTANCE.get_or_init(Self::with_defaults)
    }

    /// Creates a registry populated with all standard synthetic and real-world providers.
    pub fn with_defaults() -> Self {
        let registry = Self::new();

        // 1. Register 8 Mathematical Synthetic Providers
        for p in SyntheticCorpusProvider::all_standard_synthetic() {
            registry.register(Arc::new(p));
        }

        // 2. Register Real-World Providers (Silesia 12, Mach-O, PDF, Image)
        for p in RealWorldCorpusProvider::all_standard_real_world() {
            registry.register(Arc::new(p));
        }

        // 3. Register Canonical Short Aliases & Manifest IDs
        registry.register_alias("text", "synthetic:zipf_text");
        registry.register_alias("text_data", "synthetic:zipf_text");
        registry.register_alias("zipf_text", "synthetic:zipf_text");
        registry.register_alias("short_match", "synthetic:short_match");
        registry.register_alias("dna", "synthetic:dna");
        registry.register_alias("noise", "synthetic:noise");
        registry.register_alias("white_noise", "synthetic:noise");
        registry.register_alias("random", "synthetic:noise");
        registry.register_alias("literals", "synthetic:literals");
        registry.register_alias("macho", "synthetic:macho");
        registry.register_alias("macho_binary", "synthetic:macho");
        registry.register_alias("binary", "synthetic:macho");
        registry.register_alias("mixed", "synthetic:macho");
        registry.register_alias("realistic_rgb", "synthetic:realistic_rgb");
        registry.register_alias("striped_rgb", "synthetic:striped_rgb");
        registry.register_alias("xml", "synthetic:xml");
        registry.register_alias("calgary", "synthetic:zipf_text");

        // Silesia Short Aliases
        registry.register_alias("silesia", "silesia:all");
        registry.register_alias("silesia_all", "silesia:all");
        for &(name, _, _) in SILESIA_STANDARD_FILES {
            let target_id = format!("silesia:{}", name);
            registry.register_alias(name, &target_id);
            registry.register_alias(format!("silesia_{}", name), &target_id);
            if name == "x-ray" {
                registry.register_alias("x_ray", &target_id);
                registry.register_alias("silesia:x_ray", &target_id);
            }
        }

        // Real-world Aliases
        registry.register_alias("pdf", "real:pdf");
        registry.register_alias("test.pdf", "real:pdf");
        registry.register_alias("real_macho", "real:macho");
        registry.register_alias("libttzipvendor.a", "real:macho");
        registry.register_alias("image", "real:image");
        registry.register_alias("4k_image", "real:image");

        registry
    }

    /// Registers a provider instance under its canonical `corpus_id()`.
    pub fn register(&self, provider: Arc<dyn BenchmarkCorpusProvider>) {
        let id = provider.corpus_id().to_lowercase();
        if let Ok(mut guard) = self.providers.write() {
            guard.insert(id, provider);
        }
    }

    /// Registers a typed provider directly.
    pub fn register_provider<P: BenchmarkCorpusProvider + 'static>(&self, provider: P) {
        self.register(Arc::new(provider));
    }

    /// Registers an alias pointing to an existing canonical corpus ID.
    pub fn register_alias(&self, alias: impl Into<String>, target_id: &str) -> bool {
        let alias_key = alias.into().to_lowercase();
        let target_key = target_id.to_lowercase();

        if let Ok(guard) = self.providers.read() {
            if let Some(target_provider) = guard.get(&target_key).cloned() {
                drop(guard);
                if let Ok(mut write_guard) = self.providers.write() {
                    write_guard.insert(alias_key, target_provider);
                    return true;
                }
            }
        }
        false
    }

    /// Resolves a provider by canonical ID, URI, or registered alias.
    ///
    /// Supports URI formats:
    /// - `synthetic:dna` or `synthetic://dna`
    /// - `silesia:dickens` or `silesia://dickens`
    /// - `real:macho` or `real://macho`
    /// - `file:///path/to/dataset.bin`
    /// - Direct short IDs: `dna`, `text`, `dickens`, `pdf`
    pub fn get(&self, uri_or_id: &str) -> Option<Arc<dyn BenchmarkCorpusProvider>> {
        let normalized = normalize_corpus_uri(uri_or_id);

        if let Ok(guard) = self.providers.read() {
            if let Some(provider) = guard.get(&normalized) {
                return Some(provider.clone());
            }
        }

        // Check if URI is a local filesystem path (`file:///...` or existing path)
        if uri_or_id.starts_with("file://") || uri_or_id.starts_with("file:") {
            let path_str = uri_or_id
                .trim_start_matches("file://")
                .trim_start_matches("file:");
            return Some(Arc::new(CustomFileCorpusProvider::new(path_str)));
        }

        if Path::new(uri_or_id).is_file() {
            return Some(Arc::new(CustomFileCorpusProvider::new(uri_or_id)));
        }

        None
    }

    /// Generates corpus bytes for the given URI/ID and size.
    pub fn generate(&self, uri_or_id: &str, size_bytes: usize) -> Result<Vec<u8>, TTZipStatus> {
        let provider = self
            .get(uri_or_id)
            .ok_or(TTZipStatus::ErrFileNotFound)?;
        Ok(provider.generate(size_bytes))
    }

    /// Returns a sorted list of all unique registered corpus IDs and aliases.
    pub fn list_ids(&self) -> Vec<String> {
        if let Ok(guard) = self.providers.read() {
            let mut ids: Vec<String> = guard.keys().cloned().collect();
            ids.sort();
            ids.dedup();
            ids
        } else {
            Vec::new()
        }
    }

    /// Returns the total number of registered IDs and aliases.
    pub fn len(&self) -> usize {
        self.providers.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Returns whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for CorpusRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Normalizes URI strings (e.g. `synthetic://dna` -> `synthetic:dna`, `silesia/dickens` -> `silesia:dickens`).
fn normalize_corpus_uri(uri: &str) -> String {
    let trimmed = uri.trim().to_lowercase();
    if let Some(rest) = trimmed.strip_prefix("synthetic://") {
        return format!("synthetic:{}", rest);
    }
    if let Some(rest) = trimmed.strip_prefix("silesia://") {
        return format!("silesia:{}", rest);
    }
    if let Some(rest) = trimmed.strip_prefix("real://") {
        return format!("real:{}", rest);
    }
    if let Some(rest) = trimmed.strip_prefix("silesia/") {
        return format!("silesia:{}", rest);
    }
    trimmed
}

/// Slices or tiles buffer to exact target size.
fn slice_or_tile_buffer(source: &[u8], target_size: usize) -> Vec<u8> {
    if target_size == 0 || source.len() == target_size {
        return source.to_vec();
    }
    if target_size < source.len() {
        return source[..target_size].to_vec();
    }

    // Tile if requested size is larger than source buffer
    let mut out = Vec::with_capacity(target_size);
    while out.len() < target_size {
        let rem = target_size - out.len();
        let chunk_len = rem.min(source.len());
        out.extend_from_slice(&source[..chunk_len]);
    }
    out
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_providers() {
        let providers = SyntheticCorpusProvider::all_standard_synthetic();
        assert_eq!(providers.len(), 9);

        for p in &providers {
            let data = p.generate(4096);
            assert_eq!(data.len(), 4096);
            assert!(p.is_synthetic());
            assert!(!p.corpus_id().is_empty());
            assert!(!p.display_name().is_empty());
        }
    }

    #[test]
    fn test_real_world_providers() {
        let providers = RealWorldCorpusProvider::all_standard_real_world();
        assert!(providers.len() >= 16);

        for p in &providers {
            let data = p.generate(1024);
            assert_eq!(data.len(), 1024);
            assert!(!p.is_synthetic());
            assert!(!p.corpus_id().is_empty());
        }
    }

    #[test]
    fn test_corpus_registry_resolution() {
        let reg = CorpusRegistry::global();

        // 1. Synthetic lookups
        assert!(reg.get("synthetic:zipf_text").is_some());
        assert!(reg.get("synthetic:dna").is_some());
        assert!(reg.get("synthetic://dna").is_some());
        assert!(reg.get("text").is_some());
        assert!(reg.get("dna").is_some());
        assert!(reg.get("noise").is_some());
        assert!(reg.get("random").is_some());

        // 2. Real-world lookups
        assert!(reg.get("silesia:dickens").is_some());
        assert!(reg.get("silesia://dickens").is_some());
        assert!(reg.get("silesia/dickens").is_some());
        assert!(reg.get("dickens").is_some());
        assert!(reg.get("silesia:all").is_some());
        assert!(reg.get("silesia").is_some());
        assert!(reg.get("real:macho").is_some());
        assert!(reg.get("real:pdf").is_some());
        assert!(reg.get("real:image").is_some());

        // 3. Data generation
        let text_bytes = reg.generate("text", 2048).unwrap();
        assert_eq!(text_bytes.len(), 2048);

        let dna_bytes = reg.generate("synthetic:dna", 1024).unwrap();
        assert_eq!(dna_bytes.len(), 1024);

        let dickens_bytes = reg.generate("silesia:dickens", 512).unwrap();
        assert_eq!(dickens_bytes.len(), 512);

        // 4. Missing ID
        assert_eq!(
            reg.generate("non_existent_corpus_id_xyz", 100),
            Err(TTZipStatus::ErrFileNotFound)
        );

        // 5. List IDs
        let ids = reg.list_ids();
        assert!(ids.contains(&"synthetic:zipf_text".to_string()));
        assert!(ids.contains(&"synthetic:dna".to_string()));
        assert!(ids.contains(&"silesia:dickens".to_string()));
        assert!(ids.contains(&"dna".to_string()));
    }
}
