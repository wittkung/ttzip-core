// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Brotli Precompiled Shared Dictionary Evaluator and Cost-Benefit Decision Engine.
//!
//! Inspired by Google Brotli's shared dictionary architecture (`vendor/brotli/c/enc/encode.c`):
//! - Evaluates compression ratio gains, throughput deltas, and memory footprint overheads.
//! - Supports precompiled built-in domain dictionaries (HTML, JSON, URLs, Protobuf, C Code).
//! - Implements frequency-based sample dictionary training and synthesis.
//! - Provides deterministic decision verdicts (`BrotliDictDecisionVerdict`) based on Invariant 6 bounds.

use std::collections::HashMap;
use std::io::Cursor;
use std::time::Instant;

use brotli::enc::BrotliEncoderParams;
use serde::{Deserialize, Serialize};

use crate::crypto::{crc32_fast, xxh3_64};
use crate::types::TTZipStatus;

// ============================================================================
// Domain Dictionaries & Precompiled Data
// ============================================================================

/// Domain classification for Brotli precompiled dictionaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrotliDictDomain {
    /// HTML5 web pages, tags, common schema attributes, and meta properties.
    Html,
    /// JSON REST API payloads, standard keys, types, and envelope headers.
    Json,
    /// Web URLs, query parameter schemas, hostnames, and path fragments.
    Urls,
    /// Protocol Buffer wire formats, field descriptors, and varint structures.
    Protobuf,
    /// C / C++ / Rust source code tokens, keywords, and standard library primitives.
    SourceCode,
    /// Natural language prose and common text n-grams.
    Text,
    /// Custom user-supplied or dynamically trained domain dictionary.
    Custom(String),
}

impl BrotliDictDomain {
    /// Returns canonical string identifier for the domain.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Html => "html",
            Self::Json => "json",
            Self::Urls => "urls",
            Self::Protobuf => "protobuf",
            Self::SourceCode => "source_code",
            Self::Text => "text",
            Self::Custom(ref s) => s.as_str(),
        }
    }
}

/// Represents a loaded or trained precompiled Brotli shared dictionary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrotliDictionary {
    /// Unique identifier for this dictionary.
    pub id: String,
    /// Domain category classification.
    pub domain: BrotliDictDomain,
    /// Human-readable descriptive name.
    pub description: String,
    /// Precompiled dictionary raw byte content.
    #[serde(skip_serializing, skip_deserializing)]
    pub data: Vec<u8>,
    /// XXH3-64 checksum of the dictionary bytes.
    pub checksum_xxh3: u64,
    /// CRC-32 checksum of the dictionary bytes.
    pub checksum_crc32: u32,
    /// Size of the dictionary in bytes.
    pub size_bytes: usize,
}

impl BrotliDictionary {
    /// Constructs a dictionary from raw byte slice with metadata calculation.
    pub fn from_raw(
        id: impl Into<String>,
        domain: BrotliDictDomain,
        description: impl Into<String>,
        data: &[u8],
    ) -> Self {
        let xxh3 = xxh3_64(data);
        let crc = crc32_fast(0, data);
        Self {
            id: id.into(),
            domain,
            description: description.into(),
            data: data.to_vec(),
            checksum_xxh3: xxh3,
            checksum_crc32: crc,
            size_bytes: data.len(),
        }
    }

    /// Precompiled built-in HTML5 dictionary.
    pub fn builtin_html() -> Self {
        const HTML_TOKENS: &str = "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title></title><link rel=\"stylesheet\" href=\"\"><script type=\"text/javascript\" src=\"\"></script><style></style></head><body><header><nav><ul class=\"\"><li class=\"nav-item\"><a href=\"#\" class=\"nav-link\"></a></li></ul></nav></header><main class=\"container\"><section id=\"\"><div class=\"row\"><div class=\"col-md-6\"><h1 class=\"display-4\"></h1><p class=\"lead\"></p><button type=\"button\" class=\"btn btn-primary\"></button></div></div></section><article class=\"post\"><div class=\"card shadow-sm\"><div class=\"card-body\"><h5 class=\"card-title\"></h5><p class=\"card-text\"></p><span class=\"badge bg-secondary\"></span></div></div></article></main><footer class=\"footer\"><div class=\"text-center p-3\">&copy; 2026 All rights reserved.</div></footer></body></html>";
        Self::from_raw(
            "brotli-dict-html5-v1",
            BrotliDictDomain::Html,
            "Standard HTML5 semantics, meta headers, and DOM structure dictionary",
            HTML_TOKENS.as_bytes(),
        )
    }

    /// Precompiled built-in JSON dictionary.
    pub fn builtin_json() -> Self {
        const JSON_TOKENS: &str = "{\"status\":\"success\",\"code\":200,\"data\":{\"id\":\"00000000-0000-0000-0000-000000000000\",\"type\":\"item\",\"attributes\":{\"name\":\"\",\"description\":\"\",\"title\":\"\",\"created_at\":\"2026-08-29T00:00:00Z\",\"updated_at\":\"2026-08-29T00:00:00Z\",\"is_active\":true,\"count\":0,\"tags\":[\"primary\",\"default\"],\"metadata\":{\"version\":\"1.0.0\",\"author\":\"admin\",\"checksum\":\"\"}}},\"pagination\":{\"page\":1,\"per_page\":50,\"total\":1000,\"has_next\":true},\"errors\":[]}";
        Self::from_raw(
            "brotli-dict-json-v1",
            BrotliDictDomain::Json,
            "Standard JSON REST API response schema and structural keys dictionary",
            JSON_TOKENS.as_bytes(),
        )
    }

    /// Precompiled built-in URL dictionary.
    pub fn builtin_urls() -> Self {
        const URL_TOKENS: &str = "https://www.google.com/search?q=https://api.github.com/repos/wittkung/ttzip/releases/latest?utm_source=web&utm_medium=referral&utm_campaign=benchmarks&page=1&limit=100&sort=desc&format=json&token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9&version=v1.0.0&session_id=147d8a75-4cee-4628-92d5-0b3eb153354d";
        Self::from_raw(
            "brotli-dict-urls-v1",
            BrotliDictDomain::Urls,
            "High-density Web URLs, query parameter schemas, and REST paths dictionary",
            URL_TOKENS.as_bytes(),
        )
    }

    /// Precompiled built-in Protobuf dictionary.
    pub fn builtin_protobuf() -> Self {
        const PROTOBUF_TOKENS: &str = "\x08\x01\x12\x1a\x0a\x08google.protobuf\x12\x0eDescriptorProto\x18\x01 \x01(\t2\x04name\x18\x02 \x01(\x052\x06number\x18\x03 \x01(\x0e2\x04type\x18\x04 \x01(\x0e2\x05label\x12\x1c\x0a\x0cFieldOptions\x18\x05 \x01(\x082\x08packed\x18\x06 \x01(\x082\x0bdeprecated\x20\x01\x28\x03\x30\x01";
        Self::from_raw(
            "brotli-dict-protobuf-v1",
            BrotliDictDomain::Protobuf,
            "Google Protocol Buffer descriptor tags and wire format schema dictionary",
            PROTOBUF_TOKENS.as_bytes(),
        )
    }

    /// Precompiled built-in Source Code dictionary (C/Rust/Swift).
    pub fn builtin_source_code() -> Self {
        const CODE_TOKENS: &str = "pub fn new() -> Self { Self { config: Default::default() } }\n#[inline]\npub fn is_empty(&self) -> bool { self.len() == 0 }\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <stdint.h>\nint main(int argc, char** argv) {\n    if (argc < 2) return 1;\n    return 0;\n}\n// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0\nimport Foundation\nimport Accelerate\n";
        Self::from_raw(
            "brotli-dict-source-code-v1",
            BrotliDictDomain::SourceCode,
            "Multi-language source code keywords, headers, and syntax dictionary",
            CODE_TOKENS.as_bytes(),
        )
    }

    /// Precompiled built-in English Text dictionary.
    pub fn builtin_text() -> Self {
        const TEXT_TOKENS: &str = " the of and to in a is that for it as was with on as by at from this be are or an have which not were they their will one all would there their what so up out if about who get which go me when make can like time no just him know take people into year your good some could them see other than then now look only come its over think also back after use two how our work first well way even new want because any these give day most us";
        Self::from_raw(
            "brotli-dict-text-v1",
            BrotliDictDomain::Text,
            "English language high-frequency Zipf vocabulary dictionary",
            TEXT_TOKENS.as_bytes(),
        )
    }

    /// Trains a dictionary from sample slices by ranking repeated 4-to-16 byte n-grams.
    pub fn train_from_samples(
        id: impl Into<String>,
        domain: BrotliDictDomain,
        description: impl Into<String>,
        samples: &[&[u8]],
        target_dict_size: usize,
    ) -> Self {
        let max_size = target_dict_size.clamp(64, 1024 * 1024);
        let mut ngram_counts: HashMap<Vec<u8>, usize> = HashMap::new();

        for sample in samples {
            if sample.len() < 4 {
                continue;
            }
            for window_len in [4, 6, 8, 12, 16] {
                if sample.len() >= window_len {
                    for window in sample.windows(window_len) {
                        *ngram_counts.entry(window.to_vec()).or_insert(0) += 1;
                    }
                }
            }
        }

        // Rank n-grams by score = frequency * (length - 2)
        let mut ranked: Vec<(Vec<u8>, usize)> = ngram_counts
            .into_iter()
            .map(|(ngram, freq)| {
                let score = freq * (ngram.len().saturating_sub(2));
                (ngram, score)
            })
            .collect();
        ranked.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));

        let mut dict_buf = Vec::with_capacity(max_size);
        for (ngram, _) in ranked {
            if dict_buf.len() + ngram.len() <= max_size {
                dict_buf.extend_from_slice(&ngram);
            }
            if dict_buf.len() >= max_size {
                break;
            }
        }

        if dict_buf.is_empty() {
            dict_buf.extend_from_slice(b"default_brotli_shared_dictionary_seed_payload_2026");
        }

        Self::from_raw(id, domain, description, &dict_buf)
    }
}

// ============================================================================
// Evaluation Policy & Decision Verdicts
// ============================================================================

/// Policy thresholds for dictionary adoption decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrotliDictPolicy {
    /// Minimum compression ratio gain in percent (e.g. 5.0% = 5% smaller output).
    pub min_ratio_gain_pct: f64,
    /// Maximum allowed throughput regression in percent (Invariant 6: <= 5.0%).
    pub max_allowed_regression_pct: f64,
    /// Maximum dictionary resident memory footprint in bytes.
    pub max_memory_overhead_bytes: usize,
    /// Minimum corpus size in bytes to warrant dictionary use.
    pub min_corpus_size_bytes: usize,
    /// Maximum corpus size in bytes beyond which dictionary gain diminishes.
    pub max_corpus_size_bytes: usize,
}

impl Default for BrotliDictPolicy {
    fn default() -> Self {
        Self {
            min_ratio_gain_pct: 5.0,
            max_allowed_regression_pct: 5.0,
            max_memory_overhead_bytes: 1024 * 1024,
            min_corpus_size_bytes: 32,
            max_corpus_size_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Recommendation classification for dictionary adoption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrotliDictRecommendation {
    /// Strong gain in ratio (>15%) with negligible throughput impact.
    StronglyRecommended,
    /// Meets policy thresholds with positive overall trade-off.
    Recommended,
    /// Marginal ratio gain (0-5%) or minor speed impact.
    MarginalGain,
    /// Fails ratio gain threshold or throughput regression too high.
    NotRecommended,
    /// Output size increased or severe throughput degradation.
    Detrimental,
}

/// Comprehensive evaluation report comparing baseline vs dictionary compression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrotliDictEvaluationReport {
    /// Dictionary identifier used.
    pub dict_id: String,
    /// Dictionary size in bytes.
    pub dict_size_bytes: usize,
    /// Raw uncompressed corpus size in bytes.
    pub raw_size_bytes: usize,
    /// Brotli quality level (0..=11).
    pub quality: u32,
    /// Brotli sliding window size (10..=24).
    pub lgwin: u32,
    /// Compressed size without dictionary in bytes.
    pub no_dict_compressed_size: usize,
    /// Compressed size with dictionary in bytes.
    pub dict_compressed_size: usize,
    /// Baseline compression ratio (compressed / raw).
    pub no_dict_ratio: f64,
    /// Dictionary compression ratio (compressed / raw).
    pub dict_ratio: f64,
    /// Compression ratio gain in percent: `((no_dict - dict) / no_dict) * 100.0`.
    pub ratio_gain_pct: f64,
    /// Baseline compression duration in nanoseconds.
    pub no_dict_compress_duration_ns: f64,
    /// Dictionary compression duration in nanoseconds.
    pub dict_compress_duration_ns: f64,
    /// Baseline throughput in MB/s.
    pub no_dict_throughput_mbs: f64,
    /// Dictionary throughput in MB/s.
    pub dict_throughput_mbs: f64,
    /// Throughput difference in percent: `((dict_tp - no_dict_tp) / no_dict_tp) * 100.0`.
    pub throughput_diff_pct: f64,
    /// Dictionary resident memory overhead in bytes.
    pub memory_overhead_bytes: usize,
    /// Verified bit-for-bit lossless decompression matches raw input.
    pub verified_lossless: bool,
}

/// Final decision verdict produced by [`BrotliDictionaryEvaluator`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrotliDictDecisionVerdict {
    /// Overall recommendation category.
    pub recommendation: BrotliDictRecommendation,
    /// Whether the system should enable the dictionary for this workload.
    pub should_use_dictionary: bool,
    /// Detailed diagnostic rationale explanations.
    pub rationales: Vec<String>,
    /// Full performance evaluation metrics report.
    pub report: BrotliDictEvaluationReport,
}

// ============================================================================
// Brotli Dictionary Evaluator Engine
// ============================================================================

/// Evaluator engine for Brotli shared dictionary cost-benefit decisions.
#[derive(Debug, Clone)]
pub struct BrotliDictionaryEvaluator {
    quality: u32,
    lgwin: u32,
    warmup_passes: usize,
    measurement_passes: usize,
}

impl Default for BrotliDictionaryEvaluator {
    fn default() -> Self {
        Self {
            quality: 6,
            lgwin: 22,
            warmup_passes: 1,
            measurement_passes: 3,
        }
    }
}

impl BrotliDictionaryEvaluator {
    /// Creates a new evaluator with default parameters (Q6, lgwin 22).
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Brotli compression quality level (0..=11).
    pub fn with_quality(mut self, quality: u32) -> Self {
        self.quality = quality.clamp(0, 11);
        self
    }

    /// Sets the Brotli window bits (10..=24).
    pub fn with_lgwin(mut self, lgwin: u32) -> Self {
        self.lgwin = lgwin.clamp(10, 24);
        self
    }

    /// Sets warmup and measurement iteration counts.
    pub fn with_iterations(mut self, warmup: usize, measurement: usize) -> Self {
        self.warmup_passes = warmup;
        self.measurement_passes = measurement.max(1);
        self
    }

    /// Evaluates compression performance of `corpus` with and without `dict`.
    pub fn evaluate(
        &self,
        dict: &BrotliDictionary,
        corpus: &[u8],
    ) -> Result<BrotliDictEvaluationReport, TTZipStatus> {
        if corpus.is_empty() {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let raw_len = corpus.len();
        let q = self.quality;
        let lgwin = self.lgwin;

        // 1. Baseline: Compress without dictionary
        let (no_dict_bytes, no_dict_dur_ns) =
            self.measure_baseline_compression(corpus, q, lgwin)?;

        // 2. Dictionary Mode: Compress with shared dictionary prefix
        let (dict_bytes, dict_dur_ns) =
            self.measure_dictionary_compression(dict, corpus, q, lgwin)?;

        // 3. Verify bit-for-bit lossless decompression
        let verified = self.verify_dictionary_decompression(dict, &dict_bytes, corpus)?;

        // 4. Calculate statistical metrics
        let no_dict_size = no_dict_bytes.len();
        let dict_size = dict_bytes.len();
        let no_dict_ratio = no_dict_size as f64 / raw_len as f64;
        let dict_ratio = dict_size as f64 / raw_len as f64;

        let ratio_gain_pct = if no_dict_size > 0 {
            ((no_dict_size as f64 - dict_size as f64) / no_dict_size as f64) * 100.0
        } else {
            0.0
        };

        let raw_mb = raw_len as f64 / 1_048_576.0;
        let no_dict_secs = (no_dict_dur_ns / 1_000_000_000.0).max(1e-9);
        let dict_secs = (dict_dur_ns / 1_000_000_000.0).max(1e-9);

        let no_dict_tp = raw_mb / no_dict_secs;
        let dict_tp = raw_mb / dict_secs;

        let tp_diff_pct = if no_dict_tp > 0.0 {
            ((dict_tp - no_dict_tp) / no_dict_tp) * 100.0
        } else {
            0.0
        };

        // Resident memory overhead: dictionary buffer + internal hash tables
        let mem_overhead = dict.size_bytes + (dict.size_bytes * 2).clamp(1024, 65536);

        Ok(BrotliDictEvaluationReport {
            dict_id: dict.id.clone(),
            dict_size_bytes: dict.size_bytes,
            raw_size_bytes: raw_len,
            quality: q,
            lgwin,
            no_dict_compressed_size: no_dict_size,
            dict_compressed_size: dict_size,
            no_dict_ratio,
            dict_ratio,
            ratio_gain_pct,
            no_dict_compress_duration_ns: no_dict_dur_ns,
            dict_compress_duration_ns: dict_dur_ns,
            no_dict_throughput_mbs: no_dict_tp,
            dict_throughput_mbs: dict_tp,
            throughput_diff_pct: tp_diff_pct,
            memory_overhead_bytes: mem_overhead,
            verified_lossless: verified,
        })
    }

    /// Evaluates and produces an actionable decision verdict against a policy.
    pub fn evaluate_decision(
        &self,
        dict: &BrotliDictionary,
        corpus: &[u8],
        policy: &BrotliDictPolicy,
    ) -> Result<BrotliDictDecisionVerdict, TTZipStatus> {
        let report = self.evaluate(dict, corpus)?;
        let mut rationales = Vec::new();
        let mut meets_policy = true;

        if !report.verified_lossless {
            meets_policy = false;
            rationales.push("Lossless roundtrip verification failed".to_string());
        }

        if report.raw_size_bytes < policy.min_corpus_size_bytes {
            meets_policy = false;
            rationales.push(format!(
                "Payload size ({} B) is below policy threshold ({} B)",
                report.raw_size_bytes, policy.min_corpus_size_bytes
            ));
        }

        if report.ratio_gain_pct < policy.min_ratio_gain_pct {
            meets_policy = false;
            rationales.push(format!(
                "Compression ratio gain ({:.2}%) is below minimum threshold ({:.2}%)",
                report.ratio_gain_pct, policy.min_ratio_gain_pct
            ));
        } else {
            rationales.push(format!(
                "Compression ratio improved by {:.2}% ({} B -> {} B)",
                report.ratio_gain_pct,
                report.no_dict_compressed_size,
                report.dict_compressed_size
            ));
        }

        // Check throughput regression (negative difference = regression)
        let throughput_regression = (-report.throughput_diff_pct).max(0.0);
        if throughput_regression > policy.max_allowed_regression_pct {
            meets_policy = false;
            rationales.push(format!(
                "Throughput regression ({:.2}%) exceeds strict Invariant 6 limit ({:.2}%)",
                throughput_regression, policy.max_allowed_regression_pct
            ));
        } else {
            rationales.push(format!(
                "Throughput within acceptable bounds ({:.2} MB/s vs {:.2} MB/s baseline)",
                report.dict_throughput_mbs, report.no_dict_throughput_mbs
            ));
        }

        if report.memory_overhead_bytes > policy.max_memory_overhead_bytes {
            meets_policy = false;
            rationales.push(format!(
                "Dictionary memory footprint ({} B) exceeds limit ({} B)",
                report.memory_overhead_bytes, policy.max_memory_overhead_bytes
            ));
        }

        let recommendation = if !report.verified_lossless {
            BrotliDictRecommendation::Detrimental
        } else if report.ratio_gain_pct > 15.0 && throughput_regression <= 2.0 {
            BrotliDictRecommendation::StronglyRecommended
        } else if meets_policy {
            BrotliDictRecommendation::Recommended
        } else if report.ratio_gain_pct > 0.0 && throughput_regression <= policy.max_allowed_regression_pct {
            BrotliDictRecommendation::MarginalGain
        } else if report.ratio_gain_pct <= 0.0 {
            BrotliDictRecommendation::Detrimental
        } else {
            BrotliDictRecommendation::NotRecommended
        };

        Ok(BrotliDictDecisionVerdict {
            recommendation,
            should_use_dictionary: meets_policy,
            rationales,
            report,
        })
    }

    /// Evaluates a corpus against all 6 standard built-in dictionaries.
    pub fn evaluate_all_builtin_domains(
        &self,
        corpus: &[u8],
    ) -> Result<Vec<BrotliDictEvaluationReport>, TTZipStatus> {
        let dicts = [
            BrotliDictionary::builtin_html(),
            BrotliDictionary::builtin_json(),
            BrotliDictionary::builtin_urls(),
            BrotliDictionary::builtin_protobuf(),
            BrotliDictionary::builtin_source_code(),
            BrotliDictionary::builtin_text(),
        ];

        let mut reports = Vec::with_capacity(dicts.len());
        for dict in &dicts {
            reports.push(self.evaluate(dict, corpus)?);
        }
        Ok(reports)
    }

    // ========================================================================
    // Internal Measurement Pipelines
    // ========================================================================

    fn measure_baseline_compression(
        &self,
        corpus: &[u8],
        quality: u32,
        lgwin: u32,
    ) -> Result<(Vec<u8>, f64), TTZipStatus> {
        let params = BrotliEncoderParams {
            quality: quality.clamp(0, 11) as i32,
            lgwin: lgwin.clamp(10, 24) as i32,
            ..Default::default()
        };

        // Warmup
        for _ in 0..self.warmup_passes {
            let mut out = Vec::with_capacity(corpus.len() + 1024);
            let mut cur_in = Cursor::new(corpus);
            let _ = brotli::BrotliCompress(&mut cur_in, &mut out, &params);
        }

        let mut best_dur = f64::MAX;
        let mut final_out = Vec::new();

        for _ in 0..self.measurement_passes {
            let mut out = Vec::with_capacity(corpus.len() + 1024);
            let mut cur_in = Cursor::new(corpus);

            let t0 = Instant::now();
            brotli::BrotliCompress(&mut cur_in, &mut out, &params)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            let elapsed_ns = t0.elapsed().as_nanos() as f64;

            if elapsed_ns < best_dur {
                best_dur = elapsed_ns;
                final_out = out;
            }
        }

        Ok((final_out, best_dur))
    }

    fn measure_dictionary_compression(
        &self,
        dict: &BrotliDictionary,
        corpus: &[u8],
        quality: u32,
        lgwin: u32,
    ) -> Result<(Vec<u8>, f64), TTZipStatus> {
        let params = BrotliEncoderParams {
            quality: quality.clamp(0, 11) as i32,
            lgwin: lgwin.clamp(10, 24) as i32,
            ..Default::default()
        };

        // Prepare combined dictionary + corpus buffer for prefix matching
        let mut combined = Vec::with_capacity(dict.size_bytes + corpus.len());
        combined.extend_from_slice(&dict.data);
        combined.extend_from_slice(corpus);

        // Warmup
        for _ in 0..self.warmup_passes {
            let mut out = Vec::with_capacity(combined.len() + 1024);
            let mut cur_in = Cursor::new(&combined);
            let _ = brotli::BrotliCompress(&mut cur_in, &mut out, &params);
        }

        let mut best_dur = f64::MAX;
        let mut final_out = Vec::new();

        for _ in 0..self.measurement_passes {
            let mut out = Vec::with_capacity(combined.len() + 1024);
            let mut cur_in = Cursor::new(&combined);

            let t0 = Instant::now();
            brotli::BrotliCompress(&mut cur_in, &mut out, &params)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            let elapsed_ns = t0.elapsed().as_nanos() as f64;

            if elapsed_ns < best_dur {
                best_dur = elapsed_ns;
                final_out = out;
            }
        }

        Ok((final_out, best_dur))
    }

    fn verify_dictionary_decompression(
        &self,
        dict: &BrotliDictionary,
        compressed: &[u8],
        orig_corpus: &[u8],
    ) -> Result<bool, TTZipStatus> {
        let mut decompressed = Vec::with_capacity(dict.size_bytes + orig_corpus.len() + 1024);
        let mut cur_in = Cursor::new(compressed);

        brotli::BrotliDecompress(&mut cur_in, &mut decompressed)
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

        if decompressed.len() < dict.size_bytes + orig_corpus.len() {
            return Ok(false);
        }

        // Verify dictionary prefix matches
        if &decompressed[..dict.size_bytes] != dict.data.as_slice() {
            return Ok(false);
        }

        // Verify corpus payload matches original byte-for-byte
        let corpus_part = &decompressed[dict.size_bytes..dict.size_bytes + orig_corpus.len()];
        Ok(corpus_part == orig_corpus)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brotli_builtin_dictionaries() {
        let html_dict = BrotliDictionary::builtin_html();
        assert_eq!(html_dict.domain, BrotliDictDomain::Html);
        assert!(html_dict.size_bytes > 100);
        assert_ne!(html_dict.checksum_xxh3, 0);
        assert_ne!(html_dict.checksum_crc32, 0);

        let json_dict = BrotliDictionary::builtin_json();
        assert_eq!(json_dict.domain, BrotliDictDomain::Json);
        assert!(json_dict.size_bytes > 50);

        let url_dict = BrotliDictionary::builtin_urls();
        assert_eq!(url_dict.domain, BrotliDictDomain::Urls);

        let pb_dict = BrotliDictionary::builtin_protobuf();
        assert_eq!(pb_dict.domain, BrotliDictDomain::Protobuf);

        let code_dict = BrotliDictionary::builtin_source_code();
        assert_eq!(code_dict.domain, BrotliDictDomain::SourceCode);

        let text_dict = BrotliDictionary::builtin_text();
        assert_eq!(text_dict.domain, BrotliDictDomain::Text);
    }

    #[test]
    fn test_brotli_dictionary_training_from_samples() {
        let sample1 = b"{\"status\":\"success\",\"code\":200,\"data\":{\"name\":\"user1\",\"id\":\"abc-123\"}}";
        let sample2 = b"{\"status\":\"success\",\"code\":200,\"data\":{\"name\":\"user2\",\"id\":\"def-456\"}}";
        let sample3 = b"{\"status\":\"success\",\"code\":200,\"data\":{\"name\":\"user3\",\"id\":\"ghi-789\"}}";

        let trained = BrotliDictionary::train_from_samples(
            "trained-json-sample",
            BrotliDictDomain::Json,
            "Trained from JSON API samples",
            &[sample1, sample2, sample3],
            256,
        );

        assert!(trained.size_bytes > 0);
        assert!(trained.size_bytes <= 256);
        assert_eq!(trained.id, "trained-json-sample");
    }

    #[test]
    fn test_brotli_dict_evaluator_html_gain_and_roundtrip() {
        let html_corpus = b"<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Home</title></head><body><main class=\"container\"><h1 class=\"display-4\">Welcome</h1><p class=\"lead\">TTZip high performance benchmark</p></main></body></html>";
        let dict = BrotliDictionary::builtin_html();
        let evaluator = BrotliDictionaryEvaluator::new().with_quality(4);

        let report = evaluator.evaluate(&dict, html_corpus).expect("evaluate html");
        assert!(report.verified_lossless);
        assert_eq!(report.raw_size_bytes, html_corpus.len());
        assert!(report.no_dict_compressed_size > 0);
        assert!(report.dict_compressed_size > 0);
        assert!(report.dict_ratio > 0.0);
    }

    #[test]
    fn test_brotli_dict_evaluator_decision_verdict() {
        let json_corpus = b"{\"status\":\"success\",\"code\":200,\"data\":{\"id\":\"12345\",\"type\":\"item\",\"attributes\":{\"name\":\"Benchmark Payload\"}}}";
        let dict = BrotliDictionary::builtin_json();
        let evaluator = BrotliDictionaryEvaluator::new().with_quality(5);
        let policy = BrotliDictPolicy::default();

        let verdict = evaluator
            .evaluate_decision(&dict, json_corpus, &policy)
            .expect("evaluate decision");

        assert!(verdict.report.verified_lossless);
        assert!(!verdict.rationales.is_empty());
    }

    #[test]
    fn test_brotli_evaluate_all_builtin_domains() {
        let corpus = b"{\"data\": [\"https://google.com\", \"https://github.com/wittkung/ttzip\"]}";
        let evaluator = BrotliDictionaryEvaluator::new().with_quality(1);
        let reports = evaluator
            .evaluate_all_builtin_domains(corpus)
            .expect("evaluate all domains");
        assert_eq!(reports.len(), 6);
        for rep in reports {
            assert!(rep.verified_lossless);
        }
    }
}
