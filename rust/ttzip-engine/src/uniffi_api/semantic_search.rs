// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Interface v1 for TTZip Semantic Search & Indexing (`ttzip-semantic`).
//!
//! Ownership: Q5 owns this file (`uniffi_api/semantic_search.rs`).
//! Registration in `uniffi_api/mod.rs` and `ttzip-engine/Cargo.toml` + `BUILD.bazel`
//! is delivered as an integration patch to H (`specs/232-embedding-semantic-search/contracts/handoff_patches.md`).

use std::path::PathBuf;
use std::sync::Arc;
use ttzip_semantic::{
    SemanticComputeBackend, SemanticFileEvent, SemanticIndexProgress, SemanticIngestPhase,
    SemanticMatchKind, SemanticModelInfo, SemanticModelState, SemanticOpenRequest,
    SemanticScopeDescriptor, SemanticScopeKind, SemanticSearchCoordinator, SemanticSearchHit,
    SemanticSearchRequest, SemanticSearchResponse,
};

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniFFISemanticComputeBackend {
    Metal,
    CpuFallback,
    DeterministicTestDouble,
}

impl From<SemanticComputeBackend> for UniFFISemanticComputeBackend {
    fn from(value: SemanticComputeBackend) -> Self {
        match value {
            SemanticComputeBackend::Metal => Self::Metal,
            SemanticComputeBackend::CpuFallback => Self::CpuFallback,
            SemanticComputeBackend::DeterministicTestDouble => Self::DeterministicTestDouble,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFISemanticModelInfo {
    pub model_id: String,
    pub model_revision: String,
    pub gguf_filename: String,
    pub expected_bytes: u64,
    pub expected_sha256: String,
    pub llama_cpp_tag: String,
    pub llama_cpp_commit: String,
    pub model_fingerprint: u64,
    pub backend: UniFFISemanticComputeBackend,
}

impl From<SemanticModelInfo> for UniFFISemanticModelInfo {
    fn from(v: SemanticModelInfo) -> Self {
        Self {
            model_id: v.model_id,
            model_revision: v.model_revision,
            gguf_filename: v.gguf_filename,
            expected_bytes: v.expected_bytes,
            expected_sha256: v.expected_sha256,
            llama_cpp_tag: v.llama_cpp_tag,
            llama_cpp_commit: v.llama_cpp_commit,
            model_fingerprint: v.model_fingerprint,
            backend: v.backend.into(),
        }
    }
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum UniFFISemanticModelState {
    NotInstalled,
    Downloading {
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    Verifying,
    Ready {
        info: UniFFISemanticModelInfo,
    },
    Failed {
        reason: String,
    },
}

impl From<SemanticModelState> for UniFFISemanticModelState {
    fn from(s: SemanticModelState) -> Self {
        match s {
            SemanticModelState::NotInstalled => Self::NotInstalled,
            SemanticModelState::Downloading {
                bytes_downloaded,
                total_bytes,
            } => Self::Downloading {
                bytes_downloaded,
                total_bytes,
            },
            SemanticModelState::Verifying => Self::Verifying,
            SemanticModelState::Ready(info) => Self::Ready { info: info.into() },
            SemanticModelState::Failed(err) => Self::Failed {
                reason: err.to_string(),
            },
        }
    }
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniFFISemanticScopeKind {
    AuthorizedDirectory,
    OpenArchive,
}

impl From<UniFFISemanticScopeKind> for SemanticScopeKind {
    fn from(k: UniFFISemanticScopeKind) -> Self {
        match k {
            UniFFISemanticScopeKind::AuthorizedDirectory => Self::AuthorizedDirectory,
            UniFFISemanticScopeKind::OpenArchive => Self::OpenArchive,
        }
    }
}

impl From<SemanticScopeKind> for UniFFISemanticScopeKind {
    fn from(k: SemanticScopeKind) -> Self {
        match k {
            SemanticScopeKind::AuthorizedDirectory => Self::AuthorizedDirectory,
            SemanticScopeKind::OpenArchive => Self::OpenArchive,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFISemanticScopeDescriptor {
    pub scope_id: String,
    pub kind: UniFFISemanticScopeKind,
    pub root_path: String,
    pub display_name: String,
    pub is_authorized: bool,
}

impl From<UniFFISemanticScopeDescriptor> for SemanticScopeDescriptor {
    fn from(d: UniFFISemanticScopeDescriptor) -> Self {
        Self {
            scope_id: d.scope_id,
            kind: d.kind.into(),
            root_path: d.root_path,
            display_name: d.display_name,
            is_authorized: d.is_authorized,
        }
    }
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniFFISemanticIngestPhase {
    Idle,
    Scanning,
    ChunkingAndEmbedding,
    Committing,
    Completed,
    Cancelled,
    DegradedLexicalOnly,
    Failed,
}

impl From<SemanticIngestPhase> for UniFFISemanticIngestPhase {
    fn from(p: SemanticIngestPhase) -> Self {
        match p {
            SemanticIngestPhase::Idle => Self::Idle,
            SemanticIngestPhase::Scanning => Self::Scanning,
            SemanticIngestPhase::ChunkingAndEmbedding => Self::ChunkingAndEmbedding,
            SemanticIngestPhase::Committing => Self::Committing,
            SemanticIngestPhase::Completed => Self::Completed,
            SemanticIngestPhase::Cancelled => Self::Cancelled,
            SemanticIngestPhase::DegradedLexicalOnly => Self::DegradedLexicalOnly,
            SemanticIngestPhase::Failed => Self::Failed,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFISemanticIndexProgress {
    pub scope_id: String,
    pub phase: UniFFISemanticIngestPhase,
    pub processed_files: u32,
    pub total_files: u32,
    pub indexed_segments: u32,
    pub index_generation: u64,
    pub error_message: Option<String>,
}

impl From<SemanticIndexProgress> for UniFFISemanticIndexProgress {
    fn from(p: SemanticIndexProgress) -> Self {
        Self {
            scope_id: p.scope_id,
            phase: p.phase.into(),
            processed_files: p.processed_files,
            total_files: p.total_files,
            indexed_segments: p.indexed_segments,
            index_generation: p.index_generation,
            error_message: p.error_message,
        }
    }
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniFFISemanticMatchKind {
    LexicalOnly,
    PathSemantic,
    ContentSemantic,
    HybridPathAndContent,
}

impl From<SemanticMatchKind> for UniFFISemanticMatchKind {
    fn from(m: SemanticMatchKind) -> Self {
        match m {
            SemanticMatchKind::LexicalOnly => Self::LexicalOnly,
            SemanticMatchKind::PathSemantic => Self::PathSemantic,
            SemanticMatchKind::ContentSemantic => Self::ContentSemantic,
            SemanticMatchKind::HybridPathAndContent => Self::HybridPathAndContent,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct UniFFISemanticSearchRequest {
    pub request_id: u64,
    pub window_id: String,
    pub query: String,
    pub scope_ids: Vec<String>,
    pub top_k: u32,
    pub lexical_weight: f32,
    pub semantic_weight: f32,
}

impl From<UniFFISemanticSearchRequest> for SemanticSearchRequest {
    fn from(r: UniFFISemanticSearchRequest) -> Self {
        Self {
            request_id: r.request_id,
            window_id: r.window_id,
            query: r.query,
            scope_ids: r.scope_ids,
            top_k: r.top_k,
            lexical_weight: r.lexical_weight,
            semantic_weight: r.semantic_weight,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct UniFFISemanticSearchHit {
    pub scope_id: String,
    pub source_kind: UniFFISemanticScopeKind,
    pub container_or_root_path: String,
    pub display_path: String,
    pub archive_entry_path: Option<String>,
    pub snippet: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub match_kind: UniFFISemanticMatchKind,
    pub lexical_score: f32,
    pub semantic_score: f32,
    pub combined_score: f32,
    pub index_generation: u64,
    pub model_fingerprint: u64,
}

impl From<SemanticSearchHit> for UniFFISemanticSearchHit {
    fn from(h: SemanticSearchHit) -> Self {
        Self {
            scope_id: h.scope_id,
            source_kind: h.source_kind.into(),
            container_or_root_path: h.container_or_root_path,
            display_path: h.display_path,
            archive_entry_path: h.archive_entry_path,
            snippet: h.snippet,
            line_start: h.line_start,
            line_end: h.line_end,
            match_kind: h.match_kind.into(),
            lexical_score: h.lexical_score,
            semantic_score: h.semantic_score,
            combined_score: h.combined_score,
            index_generation: h.index_generation,
            model_fingerprint: h.model_fingerprint,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct UniFFISemanticSearchResponse {
    pub request_id: u64,
    pub window_id: String,
    pub is_cancelled: bool,
    pub degraded_lexical_only: bool,
    pub hits: Vec<UniFFISemanticSearchHit>,
}

impl From<SemanticSearchResponse> for UniFFISemanticSearchResponse {
    fn from(r: SemanticSearchResponse) -> Self {
        Self {
            request_id: r.request_id,
            window_id: r.window_id,
            is_cancelled: r.is_cancelled,
            degraded_lexical_only: r.degraded_lexical_only,
            hits: r.hits.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFISemanticOpenRequest {
    pub scope_id: String,
    pub source_kind: UniFFISemanticScopeKind,
    pub outer_path: String,
    pub archive_entry_path: Option<String>,
    pub line_number: Option<u32>,
}

impl From<SemanticOpenRequest> for UniFFISemanticOpenRequest {
    fn from(o: SemanticOpenRequest) -> Self {
        Self {
            scope_id: o.scope_id,
            source_kind: o.source_kind.into(),
            outer_path: o.outer_path,
            archive_entry_path: o.archive_entry_path,
            line_number: o.line_number,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFISemanticArchiveEntryInput {
    pub entry_path: String,
    pub bytes: Vec<u8>,
    pub mtime_ns: i64,
}

#[derive(uniffi::Object)]
pub struct UniFFISemanticSearchEngine {
    coordinator: SemanticSearchCoordinator,
}

#[uniffi::export]
impl UniFFISemanticSearchEngine {
    /// Canonical production constructor backed exclusively by `LlamaRuntimeEngine`.
    /// Never instantiates `DeterministicEmbeddingProvider`.
    #[uniffi::constructor]
    pub fn new(storage_dir: String) -> Arc<Self> {
        Arc::new(Self {
            coordinator: SemanticSearchCoordinator::new_production(PathBuf::from(storage_dir)),
        })
    }

    /// Explicit test-only constructor backed by `DeterministicEmbeddingProvider`
    /// (`TEST_DOUBLE_MODEL_FINGERPRINT`, `.testdbl.ttvec` / `.testdbl.ttidx`).
    #[uniffi::constructor]
    pub fn new_for_testing_with_test_double(storage_dir: String) -> Arc<Self> {
        Arc::new(Self {
            coordinator: SemanticSearchCoordinator::new_for_testing_with_test_double(
                PathBuf::from(storage_dir),
            ),
        })
    }

    pub fn model_state(&self) -> UniFFISemanticModelState {
        self.coordinator.model_state().into()
    }

    pub fn register_scope(&self, descriptor: UniFFISemanticScopeDescriptor) -> bool {
        matches!(
            self.coordinator.register_scope(descriptor.into()),
            ttzip_semantic::IndexLoadOutcome::Loaded { .. }
        )
    }

    pub fn revoke_scope(&self, scope_id: String) -> bool {
        self.coordinator.revoke_scope(&scope_id)
    }

    pub fn start_directory_indexing(&self, scope_id: String) -> UniFFISemanticIndexProgress {
        match self.coordinator.index_directory_scope(&scope_id) {
            Ok(prog) => prog.into(),
            Err(err) => UniFFISemanticIndexProgress {
                scope_id,
                phase: UniFFISemanticIngestPhase::Failed,
                processed_files: 0,
                total_files: 0,
                indexed_segments: 0,
                index_generation: 0,
                error_message: Some(err.to_string()),
            },
        }
    }

    pub fn cancel_indexing(&self, scope_id: String) {
        self.coordinator.cancel_indexing(&scope_id);
    }

    pub fn scope_progress(&self, scope_id: String) -> Option<UniFFISemanticIndexProgress> {
        self.coordinator.scope_progress(&scope_id).map(Into::into)
    }

    pub fn upsert_archive_entries(
        &self,
        scope_id: String,
        entries: Vec<UniFFISemanticArchiveEntryInput>,
    ) -> UniFFISemanticIndexProgress {
        let events: Vec<SemanticFileEvent> = entries
            .into_iter()
            .map(|e| SemanticFileEvent::CreatedOrModified {
                relative_path: e.entry_path.clone(),
                archive_entry_path: Some(e.entry_path),
                bytes: e.bytes,
                mtime_ns: e.mtime_ns,
            })
            .collect();
        match self.coordinator.apply_file_events(&scope_id, &events) {
            Ok(prog) => prog.into(),
            Err(err) => UniFFISemanticIndexProgress {
                scope_id,
                phase: UniFFISemanticIngestPhase::Failed,
                processed_files: 0,
                total_files: 0,
                indexed_segments: 0,
                index_generation: 0,
                error_message: Some(err.to_string()),
            },
        }
    }

    pub fn remove_indexed_file(&self, scope_id: String, relative_path: String) -> bool {
        self.coordinator
            .apply_file_events(
                &scope_id,
                &[SemanticFileEvent::Removed { relative_path }],
            )
            .is_ok()
    }

    pub fn search(&self, request: UniFFISemanticSearchRequest) -> UniFFISemanticSearchResponse {
        self.coordinator.search(&request.into()).into()
    }

    pub fn cancel_search(&self, request_id: u64) {
        self.coordinator.cancel_search(request_id);
    }

    pub fn build_open_request(&self, hit: UniFFISemanticSearchHit) -> UniFFISemanticOpenRequest {
        let internal_hit = SemanticSearchHit {
            scope_id: hit.scope_id,
            source_kind: hit.source_kind.into(),
            container_or_root_path: hit.container_or_root_path,
            display_path: hit.display_path,
            archive_entry_path: hit.archive_entry_path,
            snippet: hit.snippet,
            line_start: hit.line_start,
            line_end: hit.line_end,
            match_kind: match hit.match_kind {
                UniFFISemanticMatchKind::LexicalOnly => SemanticMatchKind::LexicalOnly,
                UniFFISemanticMatchKind::PathSemantic => SemanticMatchKind::PathSemantic,
                UniFFISemanticMatchKind::ContentSemantic => SemanticMatchKind::ContentSemantic,
                UniFFISemanticMatchKind::HybridPathAndContent => {
                    SemanticMatchKind::HybridPathAndContent
                }
            },
            lexical_score: hit.lexical_score,
            semantic_score: hit.semantic_score,
            combined_score: hit.combined_score,
            index_generation: hit.index_generation,
            model_fingerprint: hit.model_fingerprint,
        };
        self.coordinator.build_open_request(&internal_hit).into()
    }
}
