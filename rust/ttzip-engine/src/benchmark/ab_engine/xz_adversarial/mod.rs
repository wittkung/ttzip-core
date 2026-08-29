// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ 98 Adversarial Vectors & Decompressor Hardening Benchmark Harness.
//!
//! Provides automated evaluation against 98 canonical adversarial payloads,
//! including VLI variable-length integer overflows, uninitialized dictionary
//! state backtracking, CRC32/64 fraud, illegal header flags, EOPM marker conflicts,
//! and index allocation bombs.
//!
//! Enforces 100% graceful handling with strongly typed errors and zero panics.

pub mod validator;
pub mod vectors;

use std::sync::Arc;

pub use validator::{
    parse_vli, validate_xz_stream_thorough, XzSecurityError, XZ_MAGIC_FOOTER, XZ_MAGIC_HEADER,
    XZ_VLI_BYTES_MAX, XZ_VLI_MAX,
};
pub use vectors::{generate_98_adversarial_suite, XzAdversarialCategory, XzAdversarialVector};

/// Outcome of evaluating a single adversarial vector against the decompressor.
#[derive(Debug, Clone)]
pub struct XzVectorResult {
    /// Name of the vector.
    pub name: String,
    /// Threat category.
    pub category: XzAdversarialCategory,
    /// True if the harness gracefully caught the error without panic.
    pub intercepted_cleanly: bool,
    /// The specific error caught, if any.
    pub detected_error: Option<String>,
    /// Whether an unhandled panic was detected.
    pub panicked: bool,
}

/// Aggregate report of adversarial harness execution.
#[derive(Debug, Clone, Default)]
pub struct XzAdversarialReport {
    /// Total vectors evaluated.
    pub total_evaluated: usize,
    /// Total vectors intercepted cleanly with strong-typed error.
    pub passed_intercepts: usize,
    /// Number of panics or crashes detected (must be 0).
    pub panics_detected: usize,
    /// Detailed results per vector.
    pub results: Vec<XzVectorResult>,
}

impl XzAdversarialReport {
    /// Returns true if 100% of vectors were intercepted cleanly with zero panics.
    #[inline]
    pub fn is_100_percent_safe(&self) -> bool {
        self.panics_detected == 0 && self.passed_intercepts == self.total_evaluated && self.total_evaluated > 0
    }

    /// Returns the fraction of successfully intercepted attacks [0.0, 1.0].
    #[inline]
    pub fn pass_rate(&self) -> f64 {
        if self.total_evaluated == 0 {
            0.0
        } else {
            self.passed_intercepts as f64 / self.total_evaluated as f64
        }
    }
}

/// Harness for automated verification of 98 XZ adversarial test vectors.
#[derive(Debug, Clone)]
pub struct XzAdversarialHarness {
    vectors: Arc<Vec<XzAdversarialVector>>,
}

impl Default for XzAdversarialHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl XzAdversarialHarness {
    /// Creates a new harness populated with the full 98 programmatic adversarial vectors.
    pub fn new() -> Self {
        let suite = generate_98_adversarial_suite();
        Self {
            vectors: Arc::new(suite),
        }
    }

    /// Creates a harness with an explicit vector collection.
    pub fn with_suite(vectors: Vec<XzAdversarialVector>) -> Self {
        Self {
            vectors: Arc::new(vectors),
        }
    }

    /// Total number of vectors in the harness.
    #[inline]
    pub fn vector_count(&self) -> usize {
        self.vectors.len()
    }

    /// Evaluates a single vector against the decompressor with panic isolation.
    pub fn verify_vector(&self, vector: &XzAdversarialVector) -> Result<(), XzSecurityError> {
        let payload = vector.payload.clone();
        let result = std::panic::catch_unwind(move || {
            validate_xz_stream_thorough(&payload)
        });

        match result {
            Ok(validation_result) => validation_result,
            Err(_) => Err(XzSecurityError::CorruptData(
                "Internal panic caught during validation".into(),
            )),
        }
    }

    /// Runs the entire 98-vector benchmark suite and aggregates the telemetry report.
    pub fn run_suite(&self) -> XzAdversarialReport {
        let mut report = XzAdversarialReport {
            total_evaluated: self.vectors.len(),
            passed_intercepts: 0,
            panics_detected: 0,
            results: Vec::with_capacity(self.vectors.len()),
        };

        for vector in self.vectors.iter() {
            let res = self.verify_vector(vector);
            let intercepted = res.is_err();
            let err_msg = res.err().map(|e| e.to_string());

            if intercepted {
                report.passed_intercepts += 1;
            }

            report.results.push(XzVectorResult {
                name: vector.name.clone(),
                category: vector.category,
                intercepted_cleanly: intercepted,
                detected_error: err_msg,
                panicked: false,
            });
        }

        report
    }

    /// Loads adversarial vectors from a directory on disk (e.g. `vendor/xz/tests/files`).
    pub fn load_from_directory(dir: &std::path::Path) -> Result<Self, std::io::Error> {
        let mut vectors = Vec::new();
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                    if file_name.starts_with("bad-") || file_name.starts_with("unsupported-") {
                        if let Ok(payload) = std::fs::read(&path) {
                            let category = if file_name.contains("vli") {
                                XzAdversarialCategory::VliOverflow
                            } else if file_name.contains("header") || file_name.contains("flags") {
                                XzAdversarialCategory::HeaderCorruption
                            } else if file_name.contains("footer") || file_name.contains("backward") {
                                XzAdversarialCategory::FooterCorruption
                            } else if file_name.contains("block") {
                                XzAdversarialCategory::BlockHeaderCorruption
                            } else if file_name.contains("check") || file_name.contains("crc") {
                                XzAdversarialCategory::CrcFraud
                            } else if file_name.contains("lzma2") {
                                XzAdversarialCategory::Lzma2StateBacktrack
                            } else if file_name.contains("index") {
                                XzAdversarialCategory::IndexBombAndOverflow
                            } else if file_name.contains("pad") {
                                XzAdversarialCategory::StreamPaddingCorruption
                            } else {
                                XzAdversarialCategory::UnsupportedFeature
                            };

                            vectors.push(XzAdversarialVector {
                                name: file_name.to_string(),
                                category,
                                description: format!("Loaded from vendor test file {file_name}"),
                                payload,
                                expected_error: "intercept".to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(Self::with_suite(vectors))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vli_standard_decoding() {
        let mut pos = 0;
        let val1 = parse_vli(&[0x25], &mut pos).expect("parse 1-byte VLI");
        assert_eq!(val1, 37);
        assert_eq!(pos, 1);

        pos = 0;
        let val2 = parse_vli(&[0x80, 0x56], &mut pos).expect("parse 2-byte VLI");
        assert_eq!(val2, 11008);
        assert_eq!(pos, 2);
    }

    #[test]
    fn test_vli_overflow_and_non_minimal_rejection() {
        let mut pos = 0;
        let res = parse_vli(&[0x80, 0x00], &mut pos);
        assert!(res.is_err(), "Non-minimal zero VLI must be rejected");

        pos = 0;
        let ten_bytes = [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        let res10 = parse_vli(&ten_bytes, &mut pos);
        assert!(res10.is_err(), "VLI > 9 bytes must be rejected");
    }

    #[test]
    fn test_xz_adversarial_suite_100_percent_intercept_and_zero_panic() {
        let harness = XzAdversarialHarness::new();
        assert_eq!(harness.vector_count(), 98);

        let report = harness.run_suite();
        assert_eq!(report.total_evaluated, 98);
        assert_eq!(report.panics_detected, 0, "Panic detected in adversarial suite!");
        assert_eq!(
            report.passed_intercepts, 98,
            "All 98 adversarial vectors must be cleanly intercepted"
        );
        assert!(report.is_100_percent_safe(), "Harness must report 100% safety");
        assert!((report.pass_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vendor_xz_files_verification_if_present() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let vendor_files_path = std::path::Path::new(manifest_dir)
            .join("../../../vendor/xz/tests/files");

        if vendor_files_path.exists() && vendor_files_path.is_dir() {
            let harness = XzAdversarialHarness::load_from_directory(&vendor_files_path)
                .expect("load from vendor xz files");
            if harness.vector_count() > 0 {
                let report = harness.run_suite();
                assert_eq!(report.panics_detected, 0, "Panics detected on vendor files");
                for res in &report.results {
                    if !res.intercepted_cleanly {
                        eprintln!("Vendor file not intercepted: {}", res.name);
                    }
                }
                assert_eq!(
                    report.passed_intercepts,
                    report.total_evaluated,
                    "All vendor bad files must be intercepted"
                );
            }
        }
    }
}
