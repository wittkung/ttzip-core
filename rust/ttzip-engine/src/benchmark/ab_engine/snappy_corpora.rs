// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Google Snappy 12 Industrial Heterogeneous Benchmark Corpora Integration.
//!
//! Directly mirrors Google Snappy's canonical production test suite defined in
//! `vendor/snappy/snappy_test_data.h` and `vendor/snappy/testdata/`:
//! 1. `html`: 102,400 B (Google Web Search Result HTML)
//! 2. `urls`: 702,087 B (10,000 Index Crawl URLs)
//! 3. `jpg`: 123,093 B (Fireworks Photographic JPEG)
//! 4. `jpg_200`: 200 B (JPEG Header / Small Packet Micro-Chunk)
//! 5. `pdf`: 102,400 B (100KB Academic Vector PDF Paper)
//! 6. `html4`: 409,600 B (4x Concatenated Web HTML Payload)
//! 7. `txt1`: 152,089 B (Alice in Wonderland English Prose)
//! 8. `txt2`: 125,179 B (Shakespeare's As You Like It Play)
//! 9. `txt3`: 426,754 B (Technical and Educational Prose)
//! 10. `txt4`: 481,861 B (Milton's Paradise Lost Epic Verse)
//! 11. `pb`: 118,588 B (Google Maps Geographic Protocol Buffer)
//! 12. `gaviota`: 184,320 B (Gaviota Chess Endgame Tablebase)

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::benchmark::ab_engine::corpus_provider::{BenchmarkCorpusProvider, CorpusRegistry};
use crate::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};

// ============================================================================
// Snappy Industrial Corpus Classification
// ============================================================================

/// Identifies one of the 12 canonical Google Snappy industrial test datasets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnappyCorpusKind {
    /// Web Search Result HTML (`html`, ~100KB).
    Html,
    /// 10,000 Crawled Web URLs (`urls.10K`, ~700KB).
    Urls10K,
    /// Photographic JPEG Image (`fireworks.jpeg`, ~120KB).
    Jpg,
    /// Small 200-byte Packet Slice of JPEG Image (`fireworks.jpeg`, 200B).
    Jpg200,
    /// 100KB Academic Vector PDF Document (`paper-100k.pdf`, ~100KB).
    Pdf,
    /// 4x Concatenated Web HTML (`html_x_4`, ~400KB).
    Html4,
    /// Alice in Wonderland English Prose (`alice29.txt`, ~150KB).
    Txt1Alice,
    /// Shakespeare's As You Like It (`asyoulik.txt`, ~125KB).
    Txt2AsYouLikeIt,
    /// Technical and Educational Prose (`lcet10.txt`, ~420KB).
    Txt3Lcet10,
    /// Milton's Paradise Lost Epic Verse (`plrabn12.txt`, ~480KB).
    Txt4ParadiseLost,
    /// Google Maps Geographic Protocol Buffer (`geo.protodata`, ~118KB).
    ProtobufGeo,
    /// Gaviota Chess Endgame Tablebase (`kppkn.gtb`, ~180KB).
    GaviotaTablebase,
    /// Aggregate composite combining all 12 datasets sequentially.
    AllCombined,
}

impl SnappyCorpusKind {
    /// Returns the canonical dataset filename in `vendor/snappy/testdata/`.
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Urls10K => "urls.10K",
            Self::Jpg => "fireworks.jpeg",
            Self::Jpg200 => "fireworks.jpeg",
            Self::Pdf => "paper-100k.pdf",
            Self::Html4 => "html_x_4",
            Self::Txt1Alice => "alice29.txt",
            Self::Txt2AsYouLikeIt => "asyoulik.txt",
            Self::Txt3Lcet10 => "lcet10.txt",
            Self::Txt4ParadiseLost => "plrabn12.txt",
            Self::ProtobufGeo => "geo.protodata",
            Self::GaviotaTablebase => "kppkn.gtb",
            Self::AllCombined => "all_snappy_corpora",
        }
    }

    /// Returns the canonical URI identifier (e.g., `snappy:html`, `snappy:pb`).
    pub fn canonical_id(&self) -> &'static str {
        match self {
            Self::Html => "snappy:html",
            Self::Urls10K => "snappy:urls",
            Self::Jpg => "snappy:jpg",
            Self::Jpg200 => "snappy:jpg_200",
            Self::Pdf => "snappy:pdf",
            Self::Html4 => "snappy:html4",
            Self::Txt1Alice => "snappy:txt1",
            Self::Txt2AsYouLikeIt => "snappy:txt2",
            Self::Txt3Lcet10 => "snappy:txt3",
            Self::Txt4ParadiseLost => "snappy:txt4",
            Self::ProtobufGeo => "snappy:pb",
            Self::GaviotaTablebase => "snappy:gaviota",
            Self::AllCombined => "snappy:all",
        }
    }

    /// Human-readable descriptive name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Html => "Google Snappy / HTML (Web Search)",
            Self::Urls10K => "Google Snappy / URLs (10K Web URLs)",
            Self::Jpg => "Google Snappy / JPG (Fireworks Photo)",
            Self::Jpg200 => "Google Snappy / JPG_200 (200B Header Packet)",
            Self::Pdf => "Google Snappy / PDF (Paper 100K)",
            Self::Html4 => "Google Snappy / HTML4 (HTML x 4 Concatenated)",
            Self::Txt1Alice => "Google Snappy / TXT1 (Alice in Wonderland)",
            Self::Txt2AsYouLikeIt => "Google Snappy / TXT2 (As You Like It)",
            Self::Txt3Lcet10 => "Google Snappy / TXT3 (Lcet10 Technical)",
            Self::Txt4ParadiseLost => "Google Snappy / TXT4 (Paradise Lost)",
            Self::ProtobufGeo => "Google Snappy / PB (Geo Protobuf)",
            Self::GaviotaTablebase => "Google Snappy / Gaviota (Chess Tablebase)",
            Self::AllCombined => "Google Snappy / Complete 12 Industrial Suite",
        }
    }

    /// Detailed description of the dataset characteristics.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Html => "Google search HTML page with rich tags, styles, and scripts (102.4 KB)",
            Self::Urls10K => "10,000 crawled URLs stressing long common prefixes and query parameters (702.1 KB)",
            Self::Jpg => "Photographic JPEG compressed raster testing incompressible binary behavior (123.1 KB)",
            Self::Jpg200 => "200-byte small packet slice testing latency on micro-chunk transfers (200 B)",
            Self::Pdf => "Academic paper PDF with text streams, xref offsets, and metadata tables (102.4 KB)",
            Self::Html4 => "4x concatenated HTML page stressing mid-range backward LZ77 matches (409.6 KB)",
            Self::Txt1Alice => "Classic Alice in Wonderland English prose with Zipf word distributions (152.1 KB)",
            Self::Txt2AsYouLikeIt => "Shakespeare's theatrical script with repetitive character headers (125.2 KB)",
            Self::Txt3Lcet10 => "Technical literature with domain terms and structured sentences (426.8 KB)",
            Self::Txt4ParadiseLost => "Milton's poetic verse with archaic vocabulary and rhythmic meter (481.9 KB)",
            Self::ProtobufGeo => "Google Maps geographic Protocol Buffer wire-format records (118.6 KB)",
            Self::GaviotaTablebase => "Gaviota chess endgame bitboard tablebase binary matrix (184.3 KB)",
            Self::AllCombined => "Full 12-dataset Google Snappy industrial production benchmark suite (~3.3 MB)",
        }
    }

    /// Size limit in bytes (0 for full file).
    pub fn size_limit(&self) -> usize {
        match self {
            Self::Jpg200 => 200,
            _ => 0,
        }
    }

    /// Canonical standard uncompressed size in bytes.
    pub fn standard_size_bytes(&self) -> usize {
        match self {
            Self::Html => 102_400,
            Self::Urls10K => 702_087,
            Self::Jpg => 123_093,
            Self::Jpg200 => 200,
            Self::Pdf => 102_400,
            Self::Html4 => 409_600,
            Self::Txt1Alice => 152_089,
            Self::Txt2AsYouLikeIt => 125_179,
            Self::Txt3Lcet10 => 426_754,
            Self::Txt4ParadiseLost => 481_861,
            Self::ProtobufGeo => 118_588,
            Self::GaviotaTablebase => 184_320,
            Self::AllCombined => 3_308_771,
        }
    }
}

// ============================================================================
// Snappy Industrial Corpus Provider
// ============================================================================

/// Provider for Google Snappy 12 industrial benchmark datasets.
#[derive(Debug, Clone)]
pub struct SnappyIndustrialCorpusProvider {
    kind: SnappyCorpusKind,
}

impl SnappyIndustrialCorpusProvider {
    /// Creates a new provider for a specific Snappy corpus kind.
    pub fn new(kind: SnappyCorpusKind) -> Self {
        Self { kind }
    }

    /// Returns the underlying corpus kind.
    pub fn kind(&self) -> SnappyCorpusKind {
        self.kind
    }

    /// Returns all 12 standard individual providers.
    pub fn all_12_providers() -> Vec<Self> {
        vec![
            Self::new(SnappyCorpusKind::Html),
            Self::new(SnappyCorpusKind::Urls10K),
            Self::new(SnappyCorpusKind::Jpg),
            Self::new(SnappyCorpusKind::Jpg200),
            Self::new(SnappyCorpusKind::Pdf),
            Self::new(SnappyCorpusKind::Html4),
            Self::new(SnappyCorpusKind::Txt1Alice),
            Self::new(SnappyCorpusKind::Txt2AsYouLikeIt),
            Self::new(SnappyCorpusKind::Txt3Lcet10),
            Self::new(SnappyCorpusKind::Txt4ParadiseLost),
            Self::new(SnappyCorpusKind::ProtobufGeo),
            Self::new(SnappyCorpusKind::GaviotaTablebase),
        ]
    }

    /// Registers all 12 Snappy industrial providers and aliases into a [`CorpusRegistry`].
    pub fn register_all(registry: &CorpusRegistry) {
        for provider in Self::all_12_providers() {
            let id = provider.kind.canonical_id();
            let short_name = provider.kind.filename();

            registry.register(Arc::new(provider.clone()));
            registry.register_alias(short_name, id);
            registry.register_alias(format!("snappy_{}", short_name), id);
        }

        // Register AllCombined
        let all_provider = Self::new(SnappyCorpusKind::AllCombined);
        registry.register(Arc::new(all_provider));
        registry.register_alias("snappy", "snappy:all");
        registry.register_alias("snappy_all", "snappy:all");

        // Specific aliases
        registry.register_alias("geo.protodata", "snappy:pb");
        registry.register_alias("protodata", "snappy:pb");
        registry.register_alias("urls.10k", "snappy:urls");
        registry.register_alias("paper-100k.pdf", "snappy:pdf");
        registry.register_alias("html_x_4", "snappy:html4");
    }

    /// Resolves the filesystem path to the vendor testdata file.
    fn find_testdata_file(&self, filename: &str) -> Option<PathBuf> {
        let candidates = [
            format!("vendor/snappy/testdata/{}", filename),
            format!("../vendor/snappy/testdata/{}", filename),
            format!("../../vendor/snappy/testdata/{}", filename),
            format!("../../../vendor/snappy/testdata/{}", filename),
            format!("../../../../vendor/snappy/testdata/{}", filename),
            format!(
                "/Users/kevintung/Documents/dev/products/ttzip/vendor/snappy/testdata/{}",
                filename
            ),
        ];

        for c in &candidates {
            let p = Path::new(c);
            if p.is_file() {
                return Some(p.to_path_buf());
            }
        }

        None
    }

    /// Generates high-fidelity fallback synthetic data matching the dataset's entropy and structure.
    fn generate_high_fidelity_fallback(&self, target_size: usize) -> Vec<u8> {
        let size = if target_size == 0 {
            self.kind.standard_size_bytes()
        } else {
            target_size
        };

        match self.kind {
            SnappyCorpusKind::Html | SnappyCorpusKind::Html4 => {
                BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Xml, size)
            }
            SnappyCorpusKind::Urls10K => {
                let mut buf = Vec::with_capacity(size);
                let prefixes = [
                    "https://www.google.com/search?q=",
                    "https://github.com/wittkung/ttzip/commit/",
                    "https://en.wikipedia.org/wiki/",
                    "https://news.ycombinator.com/item?id=",
                    "http://localhost:8080/api/v1/resource/",
                ];
                let mut idx = 0;
                while buf.len() < size {
                    let p = prefixes[idx % prefixes.len()];
                    idx += 1;
                    let line = format!("{}{:08x}&param={}\n", p, idx * 31337, idx);
                    let rem = size - buf.len();
                    let slice = &line.as_bytes()[..line.len().min(rem)];
                    buf.extend_from_slice(slice);
                }
                buf
            }
            SnappyCorpusKind::Jpg | SnappyCorpusKind::Jpg200 => {
                let mut buf = Vec::with_capacity(size);
                // JPEG Header magic
                buf.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46]);
                while buf.len() < size {
                    let chunk = BenchmarkCorpusGenerator::gen_incompressible_noise(size - buf.len());
                    buf.extend_from_slice(&chunk);
                }
                buf.truncate(size);
                buf
            }
            SnappyCorpusKind::Pdf => {
                let mut buf = Vec::with_capacity(size);
                buf.extend_from_slice(b"%PDF-1.4\n%TTZip High Performance Benchmark Vector PDF\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
                while buf.len() < size {
                    let chunk = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, size - buf.len());
                    buf.extend_from_slice(&chunk);
                }
                buf.truncate(size);
                buf
            }
            SnappyCorpusKind::Txt1Alice
            | SnappyCorpusKind::Txt2AsYouLikeIt
            | SnappyCorpusKind::Txt3Lcet10
            | SnappyCorpusKind::Txt4ParadiseLost => {
                BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, size)
            }
            SnappyCorpusKind::ProtobufGeo => {
                let mut buf = Vec::with_capacity(size);
                let mut seq: u32 = 1000;
                while buf.len() < size {
                    // Simulates Protobuf wire format: Tag (field 1, varint), value, Tag (field 2, length-delimited)
                    buf.extend_from_slice(&[0x08, (seq & 0x7F) as u8 | 0x80, ((seq >> 7) & 0x7F) as u8]);
                    buf.extend_from_slice(&[0x12, 0x08]); // field 2, len 8
                    buf.extend_from_slice(&seq.to_le_bytes());
                    buf.extend_from_slice(&(!seq).to_le_bytes());
                    seq = seq.wrapping_add(13);
                }
                buf.truncate(size);
                buf
            }
            SnappyCorpusKind::GaviotaTablebase => {
                let mut buf = Vec::with_capacity(size);
                let mut state: u64 = 0x0123456789ABCDEF;
                while buf.len() < size {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    buf.extend_from_slice(&state.to_le_bytes());
                }
                buf.truncate(size);
                buf
            }
            SnappyCorpusKind::AllCombined => {
                let providers = Self::all_12_providers();
                let mut combined = Vec::with_capacity(size);
                for p in &providers {
                    combined.extend_from_slice(&p.generate(0));
                    if combined.len() >= size {
                        combined.truncate(size);
                        return combined;
                    }
                }
                combined
            }
        }
    }
}

impl BenchmarkCorpusProvider for SnappyIndustrialCorpusProvider {
    fn corpus_id(&self) -> &'static str {
        self.kind.canonical_id()
    }

    fn display_name(&self) -> &'static str {
        self.kind.display_name()
    }

    fn description(&self) -> &'static str {
        self.kind.description()
    }

    fn is_synthetic(&self) -> bool {
        if self.kind == SnappyCorpusKind::AllCombined {
            return false;
        }
        self.find_testdata_file(self.kind.filename()).is_none()
    }

    fn generate(&self, size_bytes: usize) -> Vec<u8> {
        if self.kind == SnappyCorpusKind::AllCombined {
            let providers = Self::all_12_providers();
            let mut combined = Vec::new();
            for p in &providers {
                combined.extend_from_slice(&p.generate(0));
                if size_bytes > 0 && combined.len() >= size_bytes {
                    combined.truncate(size_bytes);
                    return combined;
                }
            }
            if size_bytes > 0 && combined.len() < size_bytes {
                return slice_or_tile_buffer(&combined, size_bytes);
            }
            return combined;
        }

        let filename = self.kind.filename();
        let limit = self.kind.size_limit();

        if let Some(path) = self.find_testdata_file(filename) {
            if let Ok(mut bytes) = fs::read(&path) {
                if limit > 0 && bytes.len() > limit {
                    bytes.truncate(limit);
                }
                return slice_or_tile_buffer(&bytes, size_bytes);
            }
        }

        // Fallback to high-fidelity mathematical model
        let generated = self.generate_high_fidelity_fallback(size_bytes);
        if limit > 0 && generated.len() > limit {
            slice_or_tile_buffer(&generated[..limit], size_bytes)
        } else {
            slice_or_tile_buffer(&generated, size_bytes)
        }
    }
}

/// Slices or tiles buffer to exact target size.
fn slice_or_tile_buffer(source: &[u8], target_size: usize) -> Vec<u8> {
    if target_size == 0 || source.len() == target_size {
        return source.to_vec();
    }
    if target_size < source.len() {
        return source[..target_size].to_vec();
    }

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
    fn test_snappy_all_12_providers_instantiation() {
        let providers = SnappyIndustrialCorpusProvider::all_12_providers();
        assert_eq!(providers.len(), 12);

        for p in &providers {
            let data = p.generate(1024);
            assert_eq!(data.len(), 1024);
            assert!(!p.corpus_id().is_empty());
            assert!(!p.display_name().is_empty());
            assert!(!p.description().is_empty());
        }
    }

    #[test]
    fn test_snappy_default_sizes_and_limits() {
        let jpg_200 = SnappyIndustrialCorpusProvider::new(SnappyCorpusKind::Jpg200);
        let data = jpg_200.generate(0);
        assert_eq!(data.len(), 200);

        let html = SnappyIndustrialCorpusProvider::new(SnappyCorpusKind::Html);
        let html_data = html.generate(0);
        assert!(html_data.len() >= 100_000);
    }

    #[test]
    fn test_snappy_all_combined_provider() {
        let all = SnappyIndustrialCorpusProvider::new(SnappyCorpusKind::AllCombined);
        let data = all.generate(0);
        assert!(data.len() > 1_000_000);
    }

    #[test]
    fn test_snappy_registry_integration() {
        let reg = CorpusRegistry::new();
        SnappyIndustrialCorpusProvider::register_all(&reg);

        assert!(reg.get("snappy:html").is_some());
        assert!(reg.get("snappy:urls").is_some());
        assert!(reg.get("snappy:jpg").is_some());
        assert!(reg.get("snappy:jpg_200").is_some());
        assert!(reg.get("snappy:pdf").is_some());
        assert!(reg.get("snappy:html4").is_some());
        assert!(reg.get("snappy:txt1").is_some());
        assert!(reg.get("snappy:pb").is_some());
        assert!(reg.get("snappy:gaviota").is_some());
        assert!(reg.get("snappy:all").is_some());

        // Test short alias lookups
        assert!(reg.get("html").is_some());
        assert!(reg.get("urls.10k").is_some());
        assert!(reg.get("geo.protodata").is_some());
        assert!(reg.get("snappy").is_some());

        let pb_bytes = reg.generate("snappy:pb", 4096).expect("generate pb");
        assert_eq!(pb_bytes.len(), 4096);
    }
}
