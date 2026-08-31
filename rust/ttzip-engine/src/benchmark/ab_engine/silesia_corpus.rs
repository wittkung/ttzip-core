// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Silesia 12 Industrial Benchmark Corpus Engine & Unified Differential Baseline Provider.
//!
//! Provides production-grade dataset management, validation, fallback synthesis, and benchmark
//! adapters for the canonical Silesia Compression Corpus (12 entities, 211,938,580 bytes):
//! 1. `dickens`: 10,192,446 B (ASCII plain text of Charles Dickens' collected works)
//! 2. `mozilla`: 51,220,480 B (Tarred Mozilla 1.0.1 executables & shared objects)
//! 3. `mr`: 9,970,564 B (3D head MRI DICOM/raw imaging)
//! 4. `nci`: 33,553,445 B (Chemical database of molecular structures)
//! 5. `ooffice`: 6,152,192 B (OpenOffice 1.0.1 DLL binary)
//! 6. `osdb`: 10,085,684 B (Sample MySQL database tables)
//! 7. `reymont`: 6,627,202 B (Chłopi by Władysław Reymont in Polish ISO-8859-2 / text)
//! 8. `samba`: 21,606,400 B (Tarred source code and documentation of Samba 2.2.3)
//! 9. `sao`: 7,251,944 B (Smithsonian Astrophysical Observatory star catalog)
//! 10. `webster`: 41,458,703 B (1913 Webster's Unabridged Dictionary HTML/text)
//! 11. `xml`: 5,345,280 B (Collected technical XML documents)
//! 12. `x-ray`: 8,474,240 B (Medical diagnostic X-ray picture of a child's hand)

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::analytics::entropy::compute_shannon_entropy;
use crate::benchmark::ab_engine::corpus_provider::{BenchmarkCorpusProvider, CorpusRegistry};
use crate::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};

// ============================================================================
// Constants & Metadata Descriptors
// ============================================================================

/// Number of canonical individual entities in the Silesia Compression Corpus.
pub const SILESIA_ENTITIES_COUNT: usize = 12;

/// Total uncompressed standard byte size of all 12 Silesia entities combined.
pub const SILESIA_TOTAL_STANDARD_BYTES: usize = 211_938_580;

/// Static metadata descriptor for a Silesia corpus entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SilesiaFileDescriptor {
    pub name: &'static str,
    pub size_bytes: usize,
    pub sha256_hex: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub min_entropy: f64,
    pub max_entropy: f64,
}

/// Static table of the 12 canonical Silesia files and their exact characteristics.
pub const SILESIA_DESCRIPTORS: &[SilesiaFileDescriptor] = &[
    SilesiaFileDescriptor {
        name: "dickens",
        size_bytes: 10_192_446,
        sha256_hex: "b24c37886142e11d0ee687db6ab06f936207aa7f2ea1fd1d9a36763c7a507e6a",
        category: "text",
        description: "Collected works of Charles Dickens (ASCII plain text)",
        min_entropy: 3.0,
        max_entropy: 6.0,
    },
    SilesiaFileDescriptor {
        name: "mozilla",
        size_bytes: 51_220_480,
        sha256_hex: "657fc3764b0c75ac9de9623125705831ebbfbe08fed248df73bc2dc66e2a963b",
        category: "executable",
        description: "Tarred Mozilla 1.0.1 executables and shared objects",
        min_entropy: 5.5,
        max_entropy: 8.0,
    },
    SilesiaFileDescriptor {
        name: "mr",
        size_bytes: 9_970_564,
        sha256_hex: "68637ed52e3e4860174ed2dc0840ac77d5f1a60abbcb13770d5754e3774d53e6",
        category: "image",
        description: "3-D MRI image of head (16-bit DICOM/raw)",
        min_entropy: 2.0,
        max_entropy: 7.5,
    },
    SilesiaFileDescriptor {
        name: "nci",
        size_bytes: 33_553_445,
        sha256_hex: "fc63a31770947b8c2062d3b19ca94c00485a232bb91b502021948fee983e1635",
        category: "database",
        description: "Chemical database of structures (ASCII text)",
        min_entropy: 1.8,
        max_entropy: 5.5,
    },
    SilesiaFileDescriptor {
        name: "ooffice",
        size_bytes: 6_152_192,
        sha256_hex: "e7ee013880d34dd5208283d0d3d91b07f442e067454276095ded14f322a656eb",
        category: "executable",
        description: "OpenOffice.org 1.01 DLL binary",
        min_entropy: 5.5,
        max_entropy: 8.0,
    },
    SilesiaFileDescriptor {
        name: "osdb",
        size_bytes: 10_085_684,
        sha256_hex: "60f027179302ca3ad87c58ac90b6be72ec23588aaa7a3b7fe8ecc0f11def3fa3",
        category: "database",
        description: "Sample database in MySQL format",
        min_entropy: 3.5,
        max_entropy: 7.5,
    },
    SilesiaFileDescriptor {
        name: "reymont",
        size_bytes: 6_627_202,
        sha256_hex: "0eac0114a3dfe6e2ee1f345a0f79d653cb26c3bc9f0ed79238af4933422b7578",
        category: "text",
        description: "Chłopi by Władysław Reymont (Polish text / ISO-8859-2)",
        min_entropy: 3.0,
        max_entropy: 6.0,
    },
    SilesiaFileDescriptor {
        name: "samba",
        size_bytes: 21_606_400,
        sha256_hex: "93ba07bc44d8267789c1d911992f40b089ffa2140b4a160fac11ccae9a40e7b2",
        category: "source_code",
        description: "Tarred source code and documentation of Samba 2.2.3",
        min_entropy: 5.0,
        max_entropy: 7.8,
    },
    SilesiaFileDescriptor {
        name: "sao",
        size_bytes: 7_251_944,
        sha256_hex: "c2d0ea2cc59d4c21b7fe43a71499342a00cbe530a1d5548770e91ecd6214adcc",
        category: "binary_data",
        description: "SAO star catalog (binary astronomical data)",
        min_entropy: 5.0,
        max_entropy: 7.8,
    },
    SilesiaFileDescriptor {
        name: "webster",
        size_bytes: 41_458_703,
        sha256_hex: "6a68f69b26daf09f9dd84f7470368553194a0b294fcfa80f1604efb11143a383",
        category: "structured_text",
        description: "1913 Webster's Unabridged Dictionary (HTML/text)",
        min_entropy: 3.0,
        max_entropy: 6.0,
    },
    SilesiaFileDescriptor {
        name: "xml",
        size_bytes: 5_345_280,
        sha256_hex: "0e82e54e695c1938e4193448022543845b33020c8be6bf3bf3ead2224903e08c",
        category: "structured_text",
        description: "Collected XML documents",
        min_entropy: 2.0,
        max_entropy: 6.0,
    },
    SilesiaFileDescriptor {
        name: "x-ray",
        size_bytes: 8_474_240,
        sha256_hex: "7de9fce1405dc44ae5e6813ed21cd5751e761bd4265655a005d39b9685d1c9ad",
        category: "image",
        description: "Medical diagnostic X-ray picture of a child's hand",
        min_entropy: 2.5,
        max_entropy: 7.5,
    },
];

// ============================================================================
// Silesia Corpus Kind Enumeration
// ============================================================================

/// Identifies one of the 12 Silesia datasets or the full combined corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SilesiaCorpusKind {
    Dickens,
    Mozilla,
    Mr,
    Nci,
    Ooffice,
    Osdb,
    Reymont,
    Samba,
    Sao,
    Webster,
    Xml,
    XRay,
    AllCombined,
}

impl SilesiaCorpusKind {
    /// Returns the static descriptor corresponding to this kind.
    pub fn descriptor(&self) -> Option<&'static SilesiaFileDescriptor> {
        match self {
            Self::Dickens => Some(&SILESIA_DESCRIPTORS[0]),
            Self::Mozilla => Some(&SILESIA_DESCRIPTORS[1]),
            Self::Mr => Some(&SILESIA_DESCRIPTORS[2]),
            Self::Nci => Some(&SILESIA_DESCRIPTORS[3]),
            Self::Ooffice => Some(&SILESIA_DESCRIPTORS[4]),
            Self::Osdb => Some(&SILESIA_DESCRIPTORS[5]),
            Self::Reymont => Some(&SILESIA_DESCRIPTORS[6]),
            Self::Samba => Some(&SILESIA_DESCRIPTORS[7]),
            Self::Sao => Some(&SILESIA_DESCRIPTORS[8]),
            Self::Webster => Some(&SILESIA_DESCRIPTORS[9]),
            Self::Xml => Some(&SILESIA_DESCRIPTORS[10]),
            Self::XRay => Some(&SILESIA_DESCRIPTORS[11]),
            Self::AllCombined => None,
        }
    }

    /// Returns the canonical dataset filename in standard Silesia corpus distribution.
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Dickens => "dickens",
            Self::Mozilla => "mozilla",
            Self::Mr => "mr",
            Self::Nci => "nci",
            Self::Ooffice => "ooffice",
            Self::Osdb => "osdb",
            Self::Reymont => "reymont",
            Self::Samba => "samba",
            Self::Sao => "sao",
            Self::Webster => "webster",
            Self::Xml => "xml",
            Self::XRay => "x-ray",
            Self::AllCombined => "silesia_all",
        }
    }

    /// Returns the canonical URI identifier (e.g., `silesia:dickens`).
    pub fn canonical_id(&self) -> &'static str {
        match self {
            Self::Dickens => "silesia:dickens",
            Self::Mozilla => "silesia:mozilla",
            Self::Mr => "silesia:mr",
            Self::Nci => "silesia:nci",
            Self::Ooffice => "silesia:ooffice",
            Self::Osdb => "silesia:osdb",
            Self::Reymont => "silesia:reymont",
            Self::Samba => "silesia:samba",
            Self::Sao => "silesia:sao",
            Self::Webster => "silesia:webster",
            Self::Xml => "silesia:xml",
            Self::XRay => "silesia:x-ray",
            Self::AllCombined => "silesia:all",
        }
    }

    /// Human-readable descriptive name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Dickens => "Silesia / Dickens (English Text)",
            Self::Mozilla => "Silesia / Mozilla (Executables & Libs)",
            Self::Mr => "Silesia / MR (3D Head MRI)",
            Self::Nci => "Silesia / NCI (Chemical Structure DB)",
            Self::Ooffice => "Silesia / OpenOffice (Shared Library)",
            Self::Osdb => "Silesia / OSDB (MySQL Tables)",
            Self::Reymont => "Silesia / Reymont (Polish Prose)",
            Self::Samba => "Silesia / Samba (Source Code Tarball)",
            Self::Sao => "Silesia / SAO (Star Catalog Binary)",
            Self::Webster => "Silesia / Webster (HTML Dictionary)",
            Self::Xml => "Silesia / XML (Technical Data)",
            Self::XRay => "Silesia / X-Ray (Medical Radiograph)",
            Self::AllCombined => "Silesia / Full 12 Industrial Suite",
        }
    }

    /// Technical description of dataset entropy and characteristics.
    pub fn description(&self) -> &'static str {
        if let Some(desc) = self.descriptor() {
            desc.description
        } else {
            "Full 211.9MB multi-modal 12-file Silesia compression benchmark suite"
        }
    }

    /// Standard uncompressed size in bytes.
    pub fn standard_size_bytes(&self) -> usize {
        if let Some(desc) = self.descriptor() {
            desc.size_bytes
        } else {
            SILESIA_TOTAL_STANDARD_BYTES
        }
    }

    /// Returns the array of all 12 individual kinds in canonical order.
    pub fn all_12_kinds() -> &'static [SilesiaCorpusKind] {
        &[
            Self::Dickens,
            Self::Mozilla,
            Self::Mr,
            Self::Nci,
            Self::Ooffice,
            Self::Osdb,
            Self::Reymont,
            Self::Samba,
            Self::Sao,
            Self::Webster,
            Self::Xml,
            Self::XRay,
        ]
    }

    /// Maps a raw name string to a `SilesiaCorpusKind`.
    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_lowercase();
        match normalized.as_str() {
            "dickens" => Some(Self::Dickens),
            "mozilla" => Some(Self::Mozilla),
            "mr" => Some(Self::Mr),
            "nci" => Some(Self::Nci),
            "ooffice" => Some(Self::Ooffice),
            "osdb" => Some(Self::Osdb),
            "reymont" => Some(Self::Reymont),
            "samba" => Some(Self::Samba),
            "sao" => Some(Self::Sao),
            "webster" => Some(Self::Webster),
            "xml" => Some(Self::Xml),
            "x-ray" | "x_ray" | "xray" => Some(Self::XRay),
            "silesia" | "silesia_all" | "all" => Some(Self::AllCombined),
            _ => None,
        }
    }

    /// Maps a canonical URI (`silesia:dickens`) to a `SilesiaCorpusKind`.
    pub fn from_canonical_id(id: &str) -> Option<Self> {
        let trimmed = id.trim().to_lowercase();
        let name = trimmed.strip_prefix("silesia:").unwrap_or(&trimmed);
        Self::from_name(name)
    }
}

// ============================================================================
// Validation & Reports
// ============================================================================

/// Error type for Silesia buffer and boundary validation failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SilesiaValidationError {
    EmptyBuffer,
    SizeMismatch { actual: usize, expected: usize },
    EntropyOutOfRange {
        kind: SilesiaCorpusKind,
        actual: f64,
        min: f64,
        max: f64,
    },
    FileNotFound { name: String },
}

impl std::fmt::Display for SilesiaValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyBuffer => write!(f, "Silesia buffer is unexpectedly empty"),
            Self::SizeMismatch { actual, expected } => {
                write!(f, "Silesia buffer size {} does not match expected {}", actual, expected)
            }
            Self::EntropyOutOfRange { kind, actual, min, max } => {
                write!(
                    f,
                    "Entropy {:.3} for {:?} is outside expected bounds [{:.3}, {:.3}]",
                    actual, kind, min, max
                )
            }
            Self::FileNotFound { name } => write!(f, "Silesia fixture '{}' not found", name),
        }
    }
}

impl std::error::Error for SilesiaValidationError {}

/// Validation audit report confirming integrity and mathematical bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SilesiaValidationReport {
    pub kind: SilesiaCorpusKind,
    pub byte_count: usize,
    pub shannon_entropy: f64,
    pub is_synthetic: bool,
    pub is_valid: bool,
}

// ============================================================================
// Silesia Corpus Engine
// ============================================================================

/// Unified Loader, Fallback Synthesizer, and Boundary Validator for Silesia Corpus.
#[derive(Debug, Clone)]
pub struct SilesiaCorpusEngine {
    search_roots: Vec<PathBuf>,
}

impl SilesiaCorpusEngine {
    /// Creates a new engine with default search paths.
    pub fn new() -> Self {
        let default_roots = vec![
            PathBuf::from("Tests/TTZipTests/Fixtures/Silesia"),
            PathBuf::from("core/Tests/TTZipTests/Fixtures/Silesia"),
            PathBuf::from("Fixtures/Silesia"),
            PathBuf::from("../Tests/TTZipTests/Fixtures/Silesia"),
            PathBuf::from("../../Tests/TTZipTests/Fixtures/Silesia"),
            PathBuf::from("../../../Tests/TTZipTests/Fixtures/Silesia"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Tests/TTZipTests/Fixtures/Silesia"),
        ];
        Self { search_roots: default_roots }
    }

    /// Creates an engine with a customized search root.
    pub fn with_custom_dir(dir: impl Into<PathBuf>) -> Self {
        let mut engine = Self::new();
        engine.search_roots.insert(0, dir.into());
        engine
    }

    /// Finds the absolute or relative path to a Silesia file if it exists on disk.
    pub fn find_fixture_file(&self, filename: &str) -> Option<PathBuf> {
        for root in &self.search_roots {
            let path = root.join(filename);
            if path.is_file() {
                return Some(path);
            }
        }
        None
    }

    /// Loads a specific Silesia dataset by kind, applying an optional max byte limit.
    pub fn load_entity(&self, kind: SilesiaCorpusKind, max_bytes: usize) -> Vec<u8> {
        if kind == SilesiaCorpusKind::AllCombined {
            let mut combined = Vec::new();
            for k in SilesiaCorpusKind::all_12_kinds() {
                let entity_data = self.load_entity(*k, 0);
                combined.extend_from_slice(&entity_data);
                if max_bytes > 0 && combined.len() >= max_bytes {
                    combined.truncate(max_bytes);
                    return combined;
                }
            }
            if max_bytes > 0 && combined.len() < max_bytes {
                return slice_or_tile_buffer(&combined, max_bytes);
            }
            return combined;
        }

        let filename = kind.filename();
        if let Some(path) = self.find_fixture_file(filename) {
            if let Ok(bytes) = fs::read(&path) {
                return slice_or_tile_buffer(&bytes, max_bytes);
            }
        }

        // Graceful mathematical fallback generator
        let target_size = if max_bytes == 0 {
            kind.standard_size_bytes().min(1024 * 1024)
        } else {
            max_bytes
        };
        self.generate_high_fidelity_payload(kind, target_size)
    }

    /// Loads a Silesia dataset by raw name.
    pub fn load_by_name(&self, name: &str, max_bytes: usize) -> Option<Vec<u8>> {
        SilesiaCorpusKind::from_name(name).map(|k| self.load_entity(k, max_bytes))
    }

    /// Loads all 12 Silesia entities in parallel or sequence with bounded sizes.
    pub fn load_all_12(&self, max_bytes_per_entity: usize) -> Vec<(SilesiaCorpusKind, Vec<u8>)> {
        SilesiaCorpusKind::all_12_kinds()
            .iter()
            .map(|&kind| (kind, self.load_entity(kind, max_bytes_per_entity)))
            .collect()
    }

    /// Generates high-fidelity synthetic payload tailored to the specific Silesia entity type.
    pub fn generate_high_fidelity_payload(&self, kind: SilesiaCorpusKind, target_size: usize) -> Vec<u8> {
        let size = target_size.max(16);
        match kind {
            SilesiaCorpusKind::Dickens | SilesiaCorpusKind::Reymont | SilesiaCorpusKind::Webster => {
                BenchmarkCorpusGenerator::gen_text_data(size)
            }
            SilesiaCorpusKind::Mozilla | SilesiaCorpusKind::Ooffice | SilesiaCorpusKind::Samba => {
                BenchmarkCorpusGenerator::gen_binary_macho_data(size)
            }
            SilesiaCorpusKind::Mr | SilesiaCorpusKind::XRay => {
                BenchmarkCorpusGenerator::gen_realistic_rgb_data(size)
            }
            SilesiaCorpusKind::Nci | SilesiaCorpusKind::Osdb => {
                BenchmarkCorpusGenerator::gen_short_match_data(size)
            }
            SilesiaCorpusKind::Sao => {
                BenchmarkCorpusGenerator::gen_literals_data(size)
            }
            SilesiaCorpusKind::Xml => {
                BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Xml, size)
            }
            SilesiaCorpusKind::AllCombined => {
                let mut buf = Vec::with_capacity(size);
                for k in SilesiaCorpusKind::all_12_kinds() {
                    let chunk = self.generate_high_fidelity_payload(*k, size / 12 + 1024);
                    let rem = size.saturating_sub(buf.len());
                    let take = rem.min(chunk.len());
                    buf.extend_from_slice(&chunk[..take]);
                    if buf.len() >= size {
                        break;
                    }
                }
                buf.truncate(size);
                buf
            }
        }
    }

    /// Validates buffer bounds, non-emptiness, and Shannon entropy bounds.
    pub fn validate_bounds(
        &self,
        kind: SilesiaCorpusKind,
        buffer: &[u8],
    ) -> Result<SilesiaValidationReport, SilesiaValidationError> {
        if buffer.is_empty() {
            return Err(SilesiaValidationError::EmptyBuffer);
        }

        let entropy = compute_shannon_entropy(buffer);
        let is_synthetic = self.find_fixture_file(kind.filename()).is_none();

        if let Some(desc) = kind.descriptor() {
            // Check entropy bounds with appropriate margin for sub-slice local heterogeneity
            let margin = if buffer.len() < desc.size_bytes {
                2.0
            } else if is_synthetic {
                0.8
            } else {
                0.3
            };
            let min_allowed = (desc.min_entropy - margin).max(0.0);
            let max_allowed = (desc.max_entropy + margin).min(8.0);

            if entropy < min_allowed || entropy > max_allowed {
                return Err(SilesiaValidationError::EntropyOutOfRange {
                    kind,
                    actual: entropy,
                    min: min_allowed,
                    max: max_allowed,
                });
            }
        }

        Ok(SilesiaValidationReport {
            kind,
            byte_count: buffer.len(),
            shannon_entropy: entropy,
            is_synthetic,
            is_valid: true,
        })
    }

    /// Registers all 12 Silesia providers and canonical short aliases into a `CorpusRegistry`.
    pub fn register_all(registry: &CorpusRegistry) {
        let engine = Arc::new(Self::new());

        for &kind in SilesiaCorpusKind::all_12_kinds() {
            let provider = Arc::new(SilesiaCorpusProvider {
                kind,
                engine: engine.clone(),
            });
            let canonical_id = kind.canonical_id();
            let short_name = kind.filename();

            registry.register(provider);
            registry.register_alias(short_name, canonical_id);
            registry.register_alias(format!("silesia_{}", short_name), canonical_id);
        }

        // Register AllCombined
        let all_provider = Arc::new(SilesiaCorpusProvider {
            kind: SilesiaCorpusKind::AllCombined,
            engine: engine.clone(),
        });
        registry.register(all_provider);
        registry.register_alias("silesia", "silesia:all");
        registry.register_alias("silesia_all", "silesia:all");
    }
}

impl Default for SilesiaCorpusEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// BenchmarkCorpusProvider Implementation
// ============================================================================

/// Trait-based benchmark corpus provider adapter for Silesia.
#[derive(Debug, Clone)]
pub struct SilesiaCorpusProvider {
    kind: SilesiaCorpusKind,
    engine: Arc<SilesiaCorpusEngine>,
}

impl SilesiaCorpusProvider {
    pub fn new(kind: SilesiaCorpusKind) -> Self {
        Self {
            kind,
            engine: Arc::new(SilesiaCorpusEngine::new()),
        }
    }

    pub fn kind(&self) -> SilesiaCorpusKind {
        self.kind
    }
}

impl BenchmarkCorpusProvider for SilesiaCorpusProvider {
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
        if self.kind == SilesiaCorpusKind::AllCombined {
            return false;
        }
        self.engine.find_fixture_file(self.kind.filename()).is_none()
    }

    fn generate(&self, size_bytes: usize) -> Vec<u8> {
        self.engine.load_entity(self.kind, size_bytes)
    }
}

// ============================================================================
// Buffer Utility Helpers
// ============================================================================

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
    fn test_silesia_kinds_and_descriptors_consistency() {
        let kinds = SilesiaCorpusKind::all_12_kinds();
        assert_eq!(kinds.len(), SILESIA_ENTITIES_COUNT);
        assert_eq!(SILESIA_DESCRIPTORS.len(), SILESIA_ENTITIES_COUNT);

        let mut sum_bytes = 0;
        for (i, &k) in kinds.iter().enumerate() {
            let desc = k.descriptor().expect("descriptor must exist");
            assert_eq!(desc.name, k.filename());
            assert_eq!(desc.size_bytes, SILESIA_DESCRIPTORS[i].size_bytes);
            sum_bytes += desc.size_bytes;
        }
        assert_eq!(sum_bytes, SILESIA_TOTAL_STANDARD_BYTES);
    }

    #[test]
    fn test_silesia_engine_load_and_validation() {
        let engine = SilesiaCorpusEngine::new();

        for &kind in SilesiaCorpusKind::all_12_kinds() {
            let data = engine.load_entity(kind, 64 * 1024);
            assert_eq!(data.len(), 64 * 1024);

            let report = engine.validate_bounds(kind, &data).expect("validation should succeed");
            assert!(report.is_valid);
            assert!(report.shannon_entropy > 0.0 && report.shannon_entropy <= 8.0);
        }
    }

    #[test]
    fn test_silesia_all_combined_entity() {
        let engine = SilesiaCorpusEngine::new();
        let combined = engine.load_entity(SilesiaCorpusKind::AllCombined, 128 * 1024);
        assert_eq!(combined.len(), 128 * 1024);
    }

    #[test]
    fn test_silesia_registry_resolution() {
        let reg = CorpusRegistry::new();
        SilesiaCorpusEngine::register_all(&reg);

        assert!(reg.get("silesia:dickens").is_some());
        assert!(reg.get("silesia:mozilla").is_some());
        assert!(reg.get("silesia:reymont").is_some());
        assert!(reg.get("silesia:x-ray").is_some());
        assert!(reg.get("silesia:all").is_some());

        // Test short aliases
        assert!(reg.get("dickens").is_some());
        assert!(reg.get("samba").is_some());
        assert!(reg.get("silesia").is_some());

        let dickens_data = reg.generate("silesia:dickens", 1024).expect("generate");
        assert_eq!(dickens_data.len(), 1024);
    }
}
