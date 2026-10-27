// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pinned EmbeddingGemma 2 GGUF metadata, checksum verification, prompt templates,
//! and Matryoshka 128-D L2-normalized INT8 quantization math.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use thiserror::Error;

pub const PINNED_MODEL_ID: &str = "google/embeddinggemma-2";
pub const PINNED_MODEL_REVISION: &str = "914f7f8";
pub const PINNED_GGUF_REPO: &str = "ggml-org/embeddinggemma-2-GGUF";
pub const PINNED_GGUF_REVISION: &str = "bfcd298762cc34d0357ece5ebdd31791a3a374d8";
pub const PINNED_GGUF_FILENAME: &str = "embeddinggemma-2-Q8_0.gguf";
pub const PINNED_GGUF_SIZE_BYTES: u64 = 309_855_456;
pub const PINNED_GGUF_SHA256: &str =
    "2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135";
pub const PINNED_LLAMA_CPP_TAG: &str = "b11461";
pub const PINNED_LLAMA_CPP_COMMIT: &str = "4d756bc72bf00a4aacf410ae15a2d315f3db400d";
pub const PINNED_LLAMA_CPP_NO_LOGITS_PATCH_SHA256: &str =
    "33c8c4812a4307aef3f398e5ba622edcc54ef421054edd98ca0f71906b9a2d7e";
pub const PINNED_MODEL_FINGERPRINT: u64 = 0x914F_7F80_B114_6101;
pub const TEST_DOUBLE_MODEL_FINGERPRINT: u64 = 0x5445_5354_4442_4C01;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticComputeBackend {
    Metal,
    CpuFallback,
    DeterministicTestDouble,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticModelInfo {
    pub model_id: String,
    pub model_revision: String,
    pub gguf_filename: String,
    pub expected_bytes: u64,
    pub expected_sha256: String,
    pub llama_cpp_tag: String,
    pub llama_cpp_commit: String,
    pub model_fingerprint: u64,
    pub backend: SemanticComputeBackend,
}

impl SemanticModelInfo {
    pub fn pinned(backend: SemanticComputeBackend) -> Self {
        let model_fingerprint = match backend {
            SemanticComputeBackend::DeterministicTestDouble => TEST_DOUBLE_MODEL_FINGERPRINT,
            SemanticComputeBackend::Metal | SemanticComputeBackend::CpuFallback => {
                PINNED_MODEL_FINGERPRINT
            }
        };
        Self {
            model_id: PINNED_MODEL_ID.to_string(),
            model_revision: PINNED_MODEL_REVISION.to_string(),
            gguf_filename: PINNED_GGUF_FILENAME.to_string(),
            expected_bytes: PINNED_GGUF_SIZE_BYTES,
            expected_sha256: PINNED_GGUF_SHA256.to_string(),
            llama_cpp_tag: PINNED_LLAMA_CPP_TAG.to_string(),
            llama_cpp_commit: PINNED_LLAMA_CPP_COMMIT.to_string(),
            model_fingerprint,
            backend,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticModelError {
    #[error("GGUF model file is not installed at {expected_path}")]
    ModelFileMissing { expected_path: String },
    #[error("GGUF file size mismatch: expected {expected} bytes, got {actual} bytes")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("Invalid GGUF magic header in {path}")]
    InvalidGgufMagic { path: String },
    #[error("GGUF SHA-256 mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("Model download failed: {reason}")]
    DownloadFailed { reason: String },
    #[error("llama.cpp runtime failed: {reason}")]
    RuntimeFailed { reason: String },
    #[error("Zero or non-finite embedding vector rejected")]
    ZeroOrNonFiniteVector,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticModelState {
    NotInstalled,
    Downloading {
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    Verifying,
    Ready(SemanticModelInfo),
    Failed(SemanticModelError),
}

/// Formats a general search query using EmbeddingGemma 2's canonical model-card prompt.
#[inline]
pub fn format_query_prompt(query: &str) -> String {
    format!("task: search result | query: {}", query.trim())
}

/// Formats a code search query using EmbeddingGemma 2's canonical code retrieval prompt.
#[inline]
pub fn format_code_query_prompt(clean_query: &str) -> String {
    format!("task: code retrieval | query: {}", clean_query.trim())
}

/// Formats a document or path segment using EmbeddingGemma 2's canonical document prompt.
#[inline]
pub fn format_document_prompt(relative_path: Option<&str>, chunk_text: &str) -> String {
    let title = match relative_path.map(str::trim) {
        Some(t) if !t.is_empty() => t,
        _ => "none",
    };
    format!("title: {} | text: {}", title, chunk_text.trim())
}

/// Strips a leading `"code:"` prefix (case-insensitive) and returns `(is_code_query, clean_query)`.
#[inline]
pub fn strip_code_query_prefix(query: &str) -> (bool, &str) {
    let trimmed = query.trim();
    if trimmed.len() >= 5 && trimmed.as_bytes()[..5].eq_ignore_ascii_case(b"code:") {
        (true, trimmed[5..].trim())
    } else {
        (false, trimmed)
    }
}

/// Verifies a local GGUF model file's existence, exact byte size, `"GGUF"` magic, and SHA-256.
pub fn verify_gguf_model_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<String, SemanticModelError> {
    if !path.exists() {
        return Err(SemanticModelError::ModelFileMissing {
            expected_path: path.to_string_lossy().into_owned(),
        });
    }
    let meta = std::fs::metadata(path).map_err(|e| SemanticModelError::RuntimeFailed {
        reason: e.to_string(),
    })?;
    let actual_size = meta.len();
    if actual_size != expected_size {
        return Err(SemanticModelError::SizeMismatch {
            expected: expected_size,
            actual: actual_size,
        });
    }

    let mut file = File::open(path).map_err(|e| SemanticModelError::RuntimeFailed {
        reason: e.to_string(),
    })?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|_| SemanticModelError::InvalidGgufMagic {
            path: path.to_string_lossy().into_owned(),
        })?;
    if &magic != b"GGUF" {
        return Err(SemanticModelError::InvalidGgufMagic {
            path: path.to_string_lossy().into_owned(),
        });
    }

    let mut hasher = Sha256::new();
    hasher.update(magic);
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| SemanticModelError::RuntimeFailed {
                reason: e.to_string(),
            })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let actual_hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    if !actual_hex.eq_ignore_ascii_case(expected_sha256) {
        return Err(SemanticModelError::ChecksumMismatch {
            expected: expected_sha256.to_lowercase(),
            actual: actual_hex,
        });
    }
    Ok(actual_hex)
}

/// Truncates a full (e.g. 768-D) embedding to 128 dimensions (MRL), L2-normalizes,
/// and symmetrically quantizes to `i8` (`[-127, 127]`). Rejects all-zero or non-finite inputs.
pub fn quantize_mrl128_int8(raw_embedding: &[f32]) -> Result<[i8; 128], SemanticModelError> {
    if raw_embedding.len() < 128 || raw_embedding.iter().any(|v| !v.is_finite()) {
        return Err(SemanticModelError::ZeroOrNonFiniteVector);
    }
    let slice = &raw_embedding[..128];
    let sum_sq: f64 = slice.iter().map(|&v| (v as f64) * (v as f64)).sum();
    let norm = sum_sq.sqrt();
    if norm < 1e-9 {
        return Err(SemanticModelError::ZeroOrNonFiniteVector);
    }
    let inv_norm = (1.0 / norm) as f32;
    let mut out = [0i8; 128];
    let mut any_non_zero = false;
    for i in 0..128 {
        let unit = slice[i] * inv_norm;
        let scaled = (unit * 127.0).round().clamp(-127.0, 127.0) as i8;
        if scaled != 0 {
            any_non_zero = true;
        }
        out[i] = scaled;
    }
    if !any_non_zero {
        return Err(SemanticModelError::ZeroOrNonFiniteVector);
    }
    Ok(out)
}

/// Validates that an `i8` 128-D vector is not all zeros.
#[inline]
pub fn validate_non_zero_int8_vector(vec: &[i8; 128]) -> Result<(), SemanticModelError> {
    if vec.iter().all(|&b| b == 0) {
        return Err(SemanticModelError::ZeroOrNonFiniteVector);
    }
    Ok(())
}

/// Dequantizes a 128-D `i8` vector back to unit-normalized `f32`.
pub fn dequantize_int8_to_f32(vec: &[i8; 128]) -> [f32; 128] {
    let mut floats = [0.0f32; 128];
    let mut sum_sq = 0.0f64;
    for i in 0..128 {
        let v = (vec[i] as f32) / 127.0;
        floats[i] = v;
        sum_sq += (v as f64) * (v as f64);
    }
    let norm = sum_sq.sqrt() as f32;
    if norm > 1e-9 {
        for v in &mut floats {
            *v /= norm;
        }
    }
    floats
}

/// Computes cosine similarity between two equal-length `f32` slices.
pub fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += (x as f64) * (y as f64);
        na += (x as f64) * (x as f64);
        nb += (y as f64) * (y as f64);
    }
    let denom = (na * nb).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        (dot / denom) as f32
    }
}

/// Returns the exact formatted prompt used by DigUp `ReferenceVectorTests.swift` for text reference keys.
pub fn canonical_reference_prompt(name: &str) -> Option<String> {
    match name {
        "query" => Some(format_query_prompt("payment declined error")),
        "query-bn" => Some(format_query_prompt("ইলিশ মাছ রান্না")),
        "query-ar" => Some(format_query_prompt("متى تغلق المكتبة")),
        "document" => Some(format_document_prompt(
            Some("Tokyo itinerary"),
            "Flight to Tokyo Haneda on November 3. Then the train to Kamakura to see the Great Buddha, and back for dinner in Shibuya.",
        )),
        "document-long" => {
            let body = (1..=60)
                .map(|i| {
                    format!(
                        "Note {i}: the review moved to room {} because the projector in the main hall failed again, and the team agreed to send the slides the night before.",
                        i % 7 + 2
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            Some(format_document_prompt(None, &body))
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DigUpReferenceFile {
    pub vector_version: u32,
    pub made_with: String,
    pub vectors: std::collections::BTreeMap<String, Vec<f32>>,
}

impl DigUpReferenceFile {
    pub fn into_entries(self) -> Vec<ReferenceVectorEntry> {
        self.vectors
            .into_iter()
            .map(|(name, vector)| ReferenceVectorEntry {
                kind: "reference".to_string(),
                formatted_prompt: canonical_reference_prompt(&name),
                input: name,
                budget: None,
                vector,
            })
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReferenceVectorEntry {
    pub kind: String,
    pub input: String,
    #[serde(default)]
    pub formatted_prompt: Option<String>,
    #[serde(default)]
    pub budget: Option<u32>,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceVectorParityReport {
    pub total_checked: usize,
    pub min_mrl128_int8_cosine: f32,
    pub max_mrl128_int8_cosine: f32,
    pub query_doc_long_cosine_fp32: f32,
    pub query_doc_long_cosine_int8: f32,
    pub query_photo_cosine_int8: f32,
}

/// Verifies MRL-128 truncation and INT8 quantization against DigUp's official 768-D reference vectors.
pub fn verify_reference_vector_parity(
    entries: &[ReferenceVectorEntry],
) -> Result<ReferenceVectorParityReport, SemanticModelError> {
    if entries.is_empty() {
        return Err(SemanticModelError::RuntimeFailed {
            reason: "Reference vector list is empty".to_string(),
        });
    }
    let mut min_cos = 1.0f32;
    let mut max_cos = 0.0f32;
    let mut mrl_fp32_map = Vec::with_capacity(entries.len());
    let mut mrl_int8_map = Vec::with_capacity(entries.len());

    for entry in entries {
        if entry.vector.len() != 768 {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Expected 768 dims, got {}", entry.vector.len()),
            });
        }
        let mut fp32_128 = [0.0f32; 128];
        fp32_128.copy_from_slice(&entry.vector[..128]);
        let norm: f32 = fp32_128.iter().map(|v| v * v).sum::<f32>().sqrt();
        for v in &mut fp32_128 {
            *v /= norm.max(1e-9);
        }
        let q_int8 = quantize_mrl128_int8(&entry.vector)?;
        let deq = dequantize_int8_to_f32(&q_int8);
        let cos = cosine_similarity_f32(&fp32_128, &deq);
        min_cos = min_cos.min(cos);
        max_cos = max_cos.max(cos);
        mrl_fp32_map.push((entry.input.as_str(), fp32_128));
        mrl_int8_map.push((entry.input.as_str(), deq));
    }

    let find_fp32 = |name: &str| mrl_fp32_map.iter().find(|(k, _)| *k == name).map(|(_, v)| v);
    let find_int8 = |name: &str| mrl_int8_map.iter().find(|(k, _)| *k == name).map(|(_, v)| v);

    let query_doc_long_fp32 = match (find_fp32("query"), find_fp32("document-long")) {
        (Some(a), Some(b)) => cosine_similarity_f32(a, b),
        _ => 0.0,
    };
    let query_doc_long_int8 = match (find_int8("query"), find_int8("document-long")) {
        (Some(a), Some(b)) => cosine_similarity_f32(a, b),
        _ => 0.0,
    };
    let query_photo_int8 = match (find_int8("query"), find_int8("photo-140")) {
        (Some(a), Some(b)) => cosine_similarity_f32(a, b),
        _ => 0.0,
    };

    Ok(ReferenceVectorParityReport {
        total_checked: entries.len(),
        min_mrl128_int8_cosine: min_cos,
        max_mrl128_int8_cosine: max_cos,
        query_doc_long_cosine_fp32: query_doc_long_fp32,
        query_doc_long_cosine_int8: query_doc_long_int8,
        query_photo_cosine_int8: query_photo_int8,
    })
}
