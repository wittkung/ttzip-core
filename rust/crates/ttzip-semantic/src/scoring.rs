// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Deterministic concept embedding provider and hybrid lexical + semantic segment scoring.

use crate::index::PersistentScopeIndex;
use crate::model::{
    format_code_query_prompt, format_document_prompt, format_query_prompt, quantize_mrl128_int8,
    SemanticComputeBackend, SemanticModelError, SemanticModelInfo, SemanticModelState,
    PINNED_MODEL_FINGERPRINT, TEST_DOUBLE_MODEL_FINGERPRINT,
};
use crate::{SemanticMatchKind, SemanticScopeDescriptor, SemanticSearchHit};
use std::collections::BTreeMap;

pub trait EmbeddingProvider: Send + Sync {
    fn model_state(&self) -> SemanticModelState;
    fn model_fingerprint(&self) -> u64;
    fn encode_query(
        &self,
        query: &str,
        is_code_query: bool,
    ) -> Result<[i8; 128], SemanticModelError>;
    fn encode_document(
        &self,
        relative_path: Option<&str>,
        chunk_text: &str,
    ) -> Result<[i8; 128], SemanticModelError>;
}

/// Deterministic test-double embedding provider labeled `DeterministicTestDouble`.
/// Used ONLY in explicit control-flow tests; forbidden in production constructors.
/// Always carries `TEST_DOUBLE_MODEL_FINGERPRINT` (never `PINNED_MODEL_FINGERPRINT`).
pub struct DeterministicEmbeddingProvider {
    state: SemanticModelState,
    fingerprint: u64,
}

const CONCEPT_CLUSTERS: &[(&[&str], usize, f32)] = &[
    (
        &[
            "tax",
            "invoice",
            "receipt",
            "billing",
            "deduction",
            "fiscal",
            "税务",
            "发票",
            "报销",
            "账单",
            "税单",
            "收据",
        ],
        0,
        5.0,
    ),
    (
        &[
            "database",
            "connection",
            "timeout",
            "pool",
            "socket",
            "retry",
            "backoff",
            "postgres",
            "sql",
            "数据库",
            "连接",
            "超时",
            "重试",
            "连接池",
        ],
        8,
        5.0,
    ),
    (
        &[
            "archive",
            "decompress",
            "extract",
            "zip",
            "lzma",
            "deflate",
            "zstd",
            "password",
            "encrypted",
            "解压",
            "归档",
            "压缩",
            "密码",
            "加密",
        ],
        16,
        5.0,
    ),
    (
        &[
            "authentication",
            "oauth",
            "jwt",
            "token",
            "login",
            "session",
            "credential",
            "鉴权",
            "登录",
            "令牌",
            "认证",
            "会话",
        ],
        24,
        5.0,
    ),
    (
        &[
            "memory",
            "budget",
            "leak",
            "allocation",
            "rss",
            "arena",
            "mmap",
            "cache",
            "内存",
            "预算",
            "分配",
            "缓存",
            "映射",
        ],
        32,
        5.0,
    ),
    (
        &[
            "audio",
            "video",
            "subtitle",
            "waveform",
            "playback",
            "broll",
            "media",
            "音频",
            "视频",
            "字幕",
            "波形",
            "播放器",
        ],
        40,
        5.0,
    ),
];

impl DeterministicEmbeddingProvider {
    pub fn ready() -> Self {
        Self {
            state: SemanticModelState::Ready(SemanticModelInfo::pinned(
                SemanticComputeBackend::DeterministicTestDouble,
            )),
            fingerprint: TEST_DOUBLE_MODEL_FINGERPRINT,
        }
    }

    pub fn not_installed() -> Self {
        Self {
            state: SemanticModelState::NotInstalled,
            fingerprint: TEST_DOUBLE_MODEL_FINGERPRINT,
        }
    }

    pub fn with_fingerprint(fingerprint: u64) -> Self {
        let safe_fingerprint = if fingerprint == PINNED_MODEL_FINGERPRINT {
            TEST_DOUBLE_MODEL_FINGERPRINT
        } else {
            fingerprint
        };
        let mut info = SemanticModelInfo::pinned(SemanticComputeBackend::DeterministicTestDouble);
        info.model_fingerprint = safe_fingerprint;
        Self {
            state: SemanticModelState::Ready(info),
            fingerprint: safe_fingerprint,
        }
    }

    fn embed_prompt(&self, formatted_prompt: &str) -> Result<[i8; 128], SemanticModelError> {
        if !matches!(self.state, SemanticModelState::Ready(_)) {
            return Err(SemanticModelError::ModelFileMissing {
                expected_path: "not_installed".to_string(),
            });
        }
        let body = formatted_prompt
            .split_once("| query:")
            .map(|(_, q)| q.trim())
            .or_else(|| {
                formatted_prompt
                    .split_once("| text:")
                    .map(|(_, t)| t.trim())
            })
            .unwrap_or(formatted_prompt);
        if body.is_empty() {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }

        let lower = formatted_prompt.to_lowercase();
        let mut raw = vec![0.0f32; 128];

        for (keywords, base_dim, weight) in CONCEPT_CLUSTERS {
            let hits = keywords.iter().filter(|&&kw| lower.contains(kw)).count();
            if hits > 0 {
                let boost = *weight * (1.0 + (hits as f32).ln());
                for offset in 0..8 {
                    raw[base_dim + offset] += boost * if offset % 2 == 0 { 1.0 } else { -0.7 };
                }
            }
        }

        if lower.starts_with("task: code retrieval") {
            raw[48] += 1.5;
        }

        for token in body
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| s.len() >= 2)
        {
            let h = blake3::hash(token.as_bytes());
            let bytes = h.as_bytes();
            let idx1 = 56 + ((bytes[0] as usize) % 72);
            let idx2 = 56 + ((bytes[1] as usize) % 72);
            let sign1 = if bytes[2] & 1 == 0 { 1.0 } else { -1.0 };
            let sign2 = if bytes[3] & 1 == 0 { 0.8 } else { -0.8 };
            raw[idx1] += sign1 * 1.2;
            raw[idx2] += sign2 * 0.9;
        }

        if raw.iter().all(|&v| v.abs() < 1e-6) {
            let h = blake3::hash(body.as_bytes());
            for (i, b) in h.as_bytes().iter().enumerate() {
                raw[64 + i] = (*b as f32 - 128.0) / 64.0 + 0.1;
            }
        }

        quantize_mrl128_int8(&raw)
    }
}

impl EmbeddingProvider for DeterministicEmbeddingProvider {
    fn model_state(&self) -> SemanticModelState {
        self.state.clone()
    }

    fn model_fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn encode_query(
        &self,
        query: &str,
        is_code_query: bool,
    ) -> Result<[i8; 128], SemanticModelError> {
        if query.trim().is_empty() {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        let prompt = if is_code_query {
            format_code_query_prompt(query)
        } else {
            format_query_prompt(query)
        };
        self.embed_prompt(&prompt)
    }

    fn encode_document(
        &self,
        relative_path: Option<&str>,
        chunk_text: &str,
    ) -> Result<[i8; 128], SemanticModelError> {
        if chunk_text.trim().is_empty() {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        let prompt = format_document_prompt(relative_path, chunk_text);
        self.embed_prompt(&prompt)
    }
}

struct FileBestMatch {
    path_sem_score: f32,
    best_content_sem_score: f32,
    best_snippet: Option<String>,
    best_line_start: Option<u32>,
    best_line_end: Option<u32>,
}

fn collect_best_file_matches(
    index: &PersistentScopeIndex,
    is_code_query: bool,
    query_vec: &[i8; 128],
) -> BTreeMap<u32, FileBestMatch> {
    let mut per_file: BTreeMap<u32, FileBestMatch> = BTreeMap::new();
    for seg in index.segments() {
        let Some(seg_vec) = seg.vector_array() else {
            continue;
        };
        let raw_cos = int8_dot_cosine(query_vec, &seg_vec);
        let entry = per_file.entry(seg.file_id).or_insert(FileBestMatch {
            path_sem_score: 0.0,
            best_content_sem_score: -1.0,
            best_snippet: None,
            best_line_start: None,
            best_line_end: None,
        });
        if seg.is_path_segment {
            entry.path_sem_score = entry.path_sem_score.max(raw_cos);
        } else {
            let def_bonus = if is_code_query && seg.definition_strength > 0 {
                0.04 * (seg.definition_strength as f32)
            } else {
                0.0
            };
            let adjusted = raw_cos + def_bonus;
            if adjusted > entry.best_content_sem_score {
                entry.best_content_sem_score = adjusted;
                entry.best_snippet = Some(seg.snippet_preview.clone());
                entry.best_line_start = Some(seg.line_start);
                entry.best_line_end = Some(seg.line_end);
            }
        }
    }

    per_file
}

pub(crate) struct HybridScoringQuery<'a> {
    pub clean_query: &'a str,
    pub is_code_query: bool,
    pub query_vec: &'a [i8; 128],
    pub lexical_weight: f32,
    pub semantic_weight: f32,
}

pub(crate) fn score_hybrid_scope_index(
    descriptor: &SemanticScopeDescriptor,
    index: &PersistentScopeIndex,
    query: HybridScoringQuery<'_>,
    out_hits: &mut Vec<SemanticSearchHit>,
) {
    let HybridScoringQuery {
        clean_query,
        is_code_query,
        query_vec,
        lexical_weight,
        semantic_weight,
    } = query;
    let per_file = collect_best_file_matches(index, is_code_query, query_vec);

    for (file_id, best) in per_file {
        let Some(file_rec) = index.file_by_id(file_id) else {
            continue;
        };
        let snippet_text = best.best_snippet.as_deref().unwrap_or("");
        let path_lex_score = compute_lexical_score(clean_query, &file_rec.relative_path, "");
        let lex_score = compute_lexical_score(clean_query, &file_rec.relative_path, snippet_text);
        let has_content = best.best_content_sem_score > -0.5;
        let sem_score = if has_content {
            best.best_content_sem_score.max(best.path_sem_score * 0.85)
        } else {
            best.path_sem_score
        };

        let combined = if lex_score > 0.0 {
            lexical_weight * lex_score + semantic_weight * sem_score.max(0.0)
        } else {
            semantic_weight * sem_score
        };

        if combined < 0.08 && lex_score <= 0.0 {
            continue;
        }

        let match_kind = if has_content && best.best_content_sem_score >= best.path_sem_score {
            if path_lex_score > 0.2 {
                SemanticMatchKind::HybridPathAndContent
            } else {
                SemanticMatchKind::ContentSemantic
            }
        } else {
            SemanticMatchKind::PathSemantic
        };

        out_hits.push(SemanticSearchHit {
            scope_id: descriptor.scope_id.clone(),
            source_kind: descriptor.kind,
            container_or_root_path: descriptor.root_path.clone(),
            display_path: file_rec.relative_path.clone(),
            archive_entry_path: file_rec.archive_entry_path.clone(),
            snippet: if has_content { best.best_snippet } else { None },
            line_start: if has_content {
                best.best_line_start
            } else {
                None
            },
            line_end: if has_content {
                best.best_line_end
            } else {
                None
            },
            match_kind,
            lexical_score: lex_score,
            semantic_score: sem_score,
            combined_score: combined,
            index_generation: index.index_generation,
            model_fingerprint: index.model_fingerprint,
        });
    }
}

#[inline]
pub(crate) fn int8_dot_cosine(a: &[i8; 128], b: &[i8; 128]) -> f32 {
    let mut dot = 0i32;
    let mut na = 0i32;
    let mut nb = 0i32;
    for i in 0..128 {
        let x = a[i] as i32;
        let y = b[i] as i32;
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = ((na as f64) * (nb as f64)).sqrt();
    if denom < 1e-9 {
        0.0
    } else {
        (dot as f64 / denom) as f32
    }
}

pub(crate) fn compute_lexical_score(clean_query: &str, rel_path: &str, snippet: &str) -> f32 {
    let q_lower = clean_query.to_lowercase();
    let path_lower = rel_path.to_lowercase();
    let snip_lower = snippet.to_lowercase();

    if path_lower == q_lower {
        return 1.0;
    }
    let mut score = 0.0f32;
    if path_lower.contains(&q_lower) {
        score += 0.75;
    }
    if !snip_lower.is_empty() && snip_lower.contains(&q_lower) {
        score += 0.55;
    }
    let tokens: Vec<&str> = q_lower
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect();
    if !tokens.is_empty() {
        let matched = tokens
            .iter()
            .filter(|&&t| path_lower.contains(t) || snip_lower.contains(t))
            .count();
        score += 0.45 * (matched as f32 / tokens.len() as f32);
    }
    score.min(1.0)
}
