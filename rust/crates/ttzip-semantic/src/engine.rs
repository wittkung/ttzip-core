// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Request-ID isolated hybrid semantic search coordinator, incremental scope indexer,
//! and cooperative cancellation manager.

use crate::chunker::is_excluded_relative_path;
use crate::index::{IndexLoadOutcome, PersistentScopeIndex};
use crate::llama_ffi::{LlamaRuntimeConfig, LlamaRuntimeEngine};
use crate::model::{
    strip_code_query_prefix, validate_non_zero_int8_vector, SemanticModelError, SemanticModelState,
    PINNED_MODEL_FINGERPRINT,
};
use crate::scoring::{
    compute_lexical_score, score_hybrid_scope_index, EmbeddingProvider, HybridScoringQuery,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

mod directory;
mod ingest;

pub use crate::scoring::{DeterministicEmbeddingProvider, EmbeddingProvider as _};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticScopeKind {
    AuthorizedDirectory,
    OpenArchive,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticScopeDescriptor {
    pub scope_id: String,
    pub kind: SemanticScopeKind,
    pub root_path: String,
    pub display_name: String,
    pub is_authorized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticIngestPhase {
    Idle,
    Scanning,
    ChunkingAndEmbedding,
    Committing,
    Completed,
    Cancelled,
    DegradedLexicalOnly,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticIndexProgress {
    pub scope_id: String,
    pub phase: SemanticIngestPhase,
    pub processed_files: u32,
    pub total_files: u32,
    pub indexed_segments: u32,
    pub index_generation: u64,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticFileEvent {
    CreatedOrModified {
        relative_path: String,
        archive_entry_path: Option<String>,
        bytes: Vec<u8>,
        mtime_ns: i64,
    },
    Removed {
        relative_path: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticMatchKind {
    LexicalOnly,
    PathSemantic,
    ContentSemantic,
    HybridPathAndContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticSearchRequest {
    pub request_id: u64,
    pub window_id: String,
    pub query: String,
    pub scope_ids: Vec<String>,
    pub top_k: u32,
    pub lexical_weight: f32,
    pub semantic_weight: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticSearchHit {
    pub scope_id: String,
    pub source_kind: SemanticScopeKind,
    pub container_or_root_path: String,
    pub display_path: String,
    pub archive_entry_path: Option<String>,
    pub snippet: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub match_kind: SemanticMatchKind,
    pub lexical_score: f32,
    pub semantic_score: f32,
    pub combined_score: f32,
    pub index_generation: u64,
    pub model_fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticSearchResponse {
    pub request_id: u64,
    pub window_id: String,
    pub is_cancelled: bool,
    pub degraded_lexical_only: bool,
    pub hits: Vec<SemanticSearchHit>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticOpenRequest {
    pub scope_id: String,
    pub source_kind: SemanticScopeKind,
    pub outer_path: String,
    pub archive_entry_path: Option<String>,
    pub line_number: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ScopeRunIdentity {
    pub auth_epoch: u64,
    pub run_token: u64,
}

pub(super) struct ManagedScope {
    pub descriptor: SemanticScopeDescriptor,
    pub index: PersistentScopeIndex,
    pub progress: SemanticIndexProgress,
    pub cancel_flag: Arc<AtomicBool>,
    pub auth_epoch: u64,
    pub active_run_token: u64,
}

/// Thread-safe coordinator managing authorized scopes, persistent `.ttvec` v2 indices,
/// incremental indexing, and `request_id`-isolated hybrid search.
pub struct SemanticSearchCoordinator {
    provider: Arc<dyn EmbeddingProvider>,
    storage_dir: PathBuf,
    scopes: RwLock<BTreeMap<String, ManagedScope>>,
    latest_window_request: RwLock<BTreeMap<String, u64>>,
    cancelled_requests: RwLock<HashSet<u64>>,
    next_sequence: AtomicU64,
}

impl SemanticSearchCoordinator {
    /// Creates a coordinator with an explicit embedding provider.
    pub fn new(provider: Arc<dyn EmbeddingProvider>, storage_dir: PathBuf) -> Self {
        Self {
            provider,
            storage_dir,
            scopes: RwLock::new(BTreeMap::new()),
            latest_window_request: RwLock::new(BTreeMap::new()),
            cancelled_requests: RwLock::new(HashSet::new()),
            next_sequence: AtomicU64::new(1),
        }
    }

    /// Canonical production constructor backed exclusively by [`LlamaRuntimeEngine`].
    /// Never instantiates [`DeterministicEmbeddingProvider`].
    pub fn new_production(storage_dir: PathBuf) -> Self {
        let config = LlamaRuntimeConfig::default_for_storage_dir(&storage_dir);
        let provider = Arc::new(LlamaRuntimeEngine::new(config));
        Self::new(provider, storage_dir)
    }

    /// Explicit test-only constructor backed by [`DeterministicEmbeddingProvider`].
    /// Persists indices under `.testdbl.ttvec` / `.testdbl.ttidx` with `TEST_DOUBLE_MODEL_FINGERPRINT`.
    pub fn new_for_testing_with_test_double(storage_dir: PathBuf) -> Self {
        let provider = Arc::new(DeterministicEmbeddingProvider::ready());
        Self::new(provider, storage_dir)
    }

    pub fn model_state(&self) -> SemanticModelState {
        self.provider.model_state()
    }

    /// Explicitly prepares the model runtime. Status inspection never performs this work.
    pub fn prepare_model_runtime(&self) -> SemanticModelState {
        self.provider.prepare()
    }

    pub fn set_provider(&mut self, provider: Arc<dyn EmbeddingProvider>) {
        self.provider = provider;
    }

    pub(super) fn scope_paths(&self, scope_id: &str) -> (PathBuf, PathBuf) {
        let safe_name: String = scope_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if self.provider.model_fingerprint() == PINNED_MODEL_FINGERPRINT {
            let ttvec = self.storage_dir.join(format!("{}.ttvec", safe_name));
            let ttidx = self.storage_dir.join(format!("{}.ttidx", safe_name));
            (ttvec, ttidx)
        } else {
            let ttvec = self
                .storage_dir
                .join(format!("{}.testdbl.ttvec", safe_name));
            let ttidx = self
                .storage_dir
                .join(format!("{}.testdbl.ttidx", safe_name));
            (ttvec, ttidx)
        }
    }

    /// Registers or updates an authorized scope and loads any persisted `.ttvec` v2 index from disk.
    pub fn register_scope(&self, descriptor: SemanticScopeDescriptor) -> IndexLoadOutcome {
        let fp = compute_scope_fingerprint(&descriptor.scope_id, &descriptor.root_path);
        let mut index = PersistentScopeIndex::new(descriptor.scope_id.clone(), fp);
        index.model_fingerprint = self.provider.model_fingerprint();

        let (ttvec_path, ttidx_path) = self.scope_paths(&descriptor.scope_id);
        let outcome =
            index.load_from_disk(&ttvec_path, &ttidx_path, self.provider.model_fingerprint());

        let progress = SemanticIndexProgress {
            scope_id: descriptor.scope_id.clone(),
            phase: match outcome {
                IndexLoadOutcome::Loaded { .. } => SemanticIngestPhase::Completed,
                IndexLoadOutcome::NeedsRebuild { .. } => SemanticIngestPhase::Idle,
            },
            processed_files: index.files().count() as u32,
            total_files: index.files().count() as u32,
            indexed_segments: index.segments().len() as u32,
            index_generation: index.index_generation,
            error_message: None,
        };

        let auth_epoch = self.next_sequence.fetch_add(1, Ordering::SeqCst);
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if let Some(prev) = guard.get(&descriptor.scope_id) {
            prev.cancel_flag.store(true, Ordering::SeqCst);
        }
        guard.insert(
            descriptor.scope_id.clone(),
            ManagedScope {
                descriptor,
                index,
                progress,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                auth_epoch,
                active_run_token: 0,
            },
        );
        outcome
    }

    /// Revokes an authorized scope, cancels any active indexing, purges memory, and deletes disk files.
    pub fn revoke_scope(&self, scope_id: &str) -> bool {
        let removed = {
            let mut guard = self.scopes.write().expect("scopes lock poisoned");
            if let Some(mut managed) = guard.remove(scope_id) {
                managed.cancel_flag.store(true, Ordering::SeqCst);
                managed.active_run_token = 0;
                managed.index.clear();
                true
            } else {
                false
            }
        };
        let (ttvec, ttidx) = self.scope_paths(scope_id);
        let _ = fs::remove_file(ttvec);
        let _ = fs::remove_file(ttidx);
        removed
    }

    /// Signals cooperative cancellation for an active scope indexing job.
    pub fn cancel_indexing(&self, scope_id: &str) {
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if let Some(managed) = guard.get_mut(scope_id) {
            managed.cancel_flag.store(true, Ordering::SeqCst);
            managed.active_run_token = 0;
            managed.progress.phase = SemanticIngestPhase::Cancelled;
        }
    }

    /// Returns a cancellation handle for a scope.
    pub fn scope_cancellation_flag(&self, scope_id: &str) -> Option<Arc<AtomicBool>> {
        let guard = self.scopes.read().expect("scopes lock poisoned");
        guard.get(scope_id).map(|m| Arc::clone(&m.cancel_flag))
    }

    /// Returns current indexing progress for a scope.
    pub fn scope_progress(&self, scope_id: &str) -> Option<SemanticIndexProgress> {
        let guard = self.scopes.read().expect("scopes lock poisoned");
        guard.get(scope_id).map(|m| m.progress.clone())
    }

    /// Indexes an authorized directory on disk, respecting excluded paths, content tiers,
    /// incremental `mtime_ns + size + blake3` checks, and cooperative cancellation.
    pub fn index_directory_scope(
        &self,
        scope_id: &str,
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        let (root_path, run_identity, cancel_flag) = self.prepare_directory_scope_scan(scope_id)?;

        let mut discovered_files = Vec::new();
        let mut had_directory_read_error = false;
        collect_authorized_files(
            &root_path,
            &root_path,
            &mut discovered_files,
            &cancel_flag,
            &mut had_directory_read_error,
        );
        if !self.is_run_still_active(scope_id, run_identity, &cancel_flag) {
            return Ok(self.cancelled_progress_snapshot(scope_id, run_identity));
        }
        if had_directory_read_error {
            return self.fail_directory_scope_scan(scope_id, &root_path, run_identity);
        }

        let had_stream_read_error = AtomicBool::new(false);
        let events = self.directory_file_event_stream(
            scope_id,
            discovered_files,
            &cancel_flag,
            &had_stream_read_error,
            had_directory_read_error,
        );

        let event_count = events.remaining_event_count();
        self.apply_ingest_event_iter_for_run(
            scope_id,
            events,
            event_count,
            run_identity,
            &cancel_flag,
            Some((&root_path, &had_stream_read_error)),
        )
    }

    /// Applies incremental file creation, modification, or deletion events to a scope index
    /// and persists `.ttvec` v2 + `.ttidx` to disk.
    pub fn apply_file_events(
        &self,
        scope_id: &str,
        events: &[SemanticFileEvent],
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        let (run_identity, cancel_flag) = {
            let mut guard = self.scopes.write().expect("scopes lock poisoned");
            let Some(managed) = guard.get_mut(scope_id) else {
                return Err(SemanticModelError::RuntimeFailed {
                    reason: format!("Unknown scope_id: {}", scope_id),
                });
            };
            managed.cancel_flag.store(true, Ordering::SeqCst);
            let new_cancel = Arc::new(AtomicBool::new(false));
            managed.cancel_flag = Arc::clone(&new_cancel);
            let run_token = self.next_sequence.fetch_add(1, Ordering::SeqCst);
            managed.active_run_token = run_token;
            (
                ScopeRunIdentity {
                    auth_epoch: managed.auth_epoch,
                    run_token,
                },
                new_cancel,
            )
        };
        self.apply_file_events_for_run(scope_id, events, run_identity, &cancel_flag)
    }

    fn prepare_query_embedding(&self, query: &str, is_code: bool) -> Option<[i8; 128]> {
        if !matches!(self.provider.prepare(), SemanticModelState::Ready(_)) {
            return None;
        }
        let embedding = self.provider.encode_query(query, is_code).ok()?;
        validate_non_zero_int8_vector(&embedding).ok()?;
        Some(embedding)
    }

    /// Explicitly cancels a specific search `request_id`.
    pub fn cancel_search(&self, request_id: u64) {
        let mut guard = self
            .cancelled_requests
            .write()
            .expect("cancelled_requests lock poisoned");
        guard.insert(request_id);
    }

    fn is_request_cancelled(&self, window_id: &str, request_id: u64) -> bool {
        if self
            .cancelled_requests
            .read()
            .expect("cancelled_requests lock poisoned")
            .contains(&request_id)
        {
            return true;
        }
        self.latest_window_request
            .read()
            .expect("latest_window_request lock poisoned")
            .get(window_id)
            .is_some_and(|&latest| latest > request_id)
    }

    fn register_window_request(&self, request: &SemanticSearchRequest) {
        let mut win_guard = self
            .latest_window_request
            .write()
            .expect("latest_window_request lock poisoned");
        let entry = win_guard.entry(request.window_id.clone()).or_insert(0);
        if request.request_id > *entry {
            *entry = request.request_id;
        }
    }

    /// Executes a `request_id` and `window_id` isolated hybrid search across authorized scopes.
    pub fn search(&self, request: &SemanticSearchRequest) -> SemanticSearchResponse {
        self.register_window_request(request);

        if self.is_request_cancelled(&request.window_id, request.request_id) {
            return empty_search_response(request, true, false);
        }

        let (is_code_query, clean_query) = strip_code_query_prefix(&request.query);
        if clean_query.is_empty() {
            return empty_search_response(request, false, false);
        }

        let query_vec_opt = self.prepare_query_embedding(clean_query, is_code_query);
        let degraded_lexical_only = query_vec_opt.is_none();

        let scopes_guard = self.scopes.read().expect("scopes lock poisoned");
        let mut candidate_hits = Vec::new();

        for (scope_id, managed) in scopes_guard.iter() {
            if !managed.descriptor.is_authorized {
                continue;
            }
            if !request.scope_ids.is_empty() && !request.scope_ids.contains(scope_id) {
                continue;
            }
            if self.is_request_cancelled(&request.window_id, request.request_id) {
                return empty_search_response(request, true, degraded_lexical_only);
            }

            if degraded_lexical_only || managed.index.segments().is_empty() {
                score_lexical_only_scope(managed, clean_query, &mut candidate_hits);
                continue;
            }

            let query_vec = query_vec_opt.expect("checked Some above");
            score_hybrid_scope_index(
                &managed.descriptor,
                &managed.index,
                HybridScoringQuery {
                    clean_query,
                    is_code_query,
                    query_vec: &query_vec,
                    lexical_weight: request.lexical_weight,
                    semantic_weight: request.semantic_weight,
                },
                &mut candidate_hits,
            );
        }

        if self.is_request_cancelled(&request.window_id, request.request_id) {
            return empty_search_response(request, true, degraded_lexical_only);
        }

        candidate_hits.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.display_path.cmp(&b.display_path))
        });
        let top_k = (request.top_k as usize).clamp(1, 200);
        candidate_hits.truncate(top_k);

        SemanticSearchResponse {
            request_id: request.request_id,
            window_id: request.window_id.clone(),
            is_cancelled: false,
            degraded_lexical_only,
            hits: candidate_hits,
        }
    }

    /// Constructs a `SemanticOpenRequest` from a hit, preserving `archive_entry_path` and `line_start`.
    pub fn build_open_request(&self, hit: &SemanticSearchHit) -> SemanticOpenRequest {
        let outer_path = match hit.source_kind {
            SemanticScopeKind::OpenArchive => hit.container_or_root_path.clone(),
            SemanticScopeKind::AuthorizedDirectory => Path::new(&hit.container_or_root_path)
                .join(&hit.display_path)
                .to_string_lossy()
                .into_owned(),
        };
        SemanticOpenRequest {
            scope_id: hit.scope_id.clone(),
            source_kind: hit.source_kind,
            outer_path,
            archive_entry_path: hit.archive_entry_path.clone(),
            line_number: hit.line_start,
        }
    }
}

fn empty_search_response(
    request: &SemanticSearchRequest,
    is_cancelled: bool,
    degraded_lexical_only: bool,
) -> SemanticSearchResponse {
    SemanticSearchResponse {
        request_id: request.request_id,
        window_id: request.window_id.clone(),
        is_cancelled,
        degraded_lexical_only,
        hits: Vec::new(),
    }
}

fn score_lexical_only_scope(
    managed: &ManagedScope,
    clean_query: &str,
    out_hits: &mut Vec<SemanticSearchHit>,
) {
    let root = Path::new(&managed.descriptor.root_path);
    let mut files_on_disk = Vec::new();
    let dummy_cancel = AtomicBool::new(false);
    let mut dummy_read_error = false;
    if root.exists() {
        collect_authorized_files(
            root,
            root,
            &mut files_on_disk,
            &dummy_cancel,
            &mut dummy_read_error,
        );
    }
    for (rel_path, _, _, _) in files_on_disk {
        let lex = compute_lexical_score(clean_query, &rel_path, "");
        if lex > 0.05 {
            out_hits.push(SemanticSearchHit {
                scope_id: managed.descriptor.scope_id.clone(),
                source_kind: managed.descriptor.kind,
                container_or_root_path: managed.descriptor.root_path.clone(),
                display_path: rel_path,
                archive_entry_path: None,
                snippet: None,
                line_start: None,
                line_end: None,
                match_kind: SemanticMatchKind::LexicalOnly,
                lexical_score: lex,
                semantic_score: 0.0,
                combined_score: lex,
                index_generation: managed.index.index_generation,
                model_fingerprint: managed.index.model_fingerprint,
            });
        }
    }
}

fn compute_scope_fingerprint(scope_id: &str, root_path: &str) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(scope_id.as_bytes());
    hasher.update(b":");
    hasher.update(root_path.as_bytes());
    let bytes = hasher.finalize();
    u64::from_le_bytes(bytes.as_bytes()[0..8].try_into().unwrap_or([0u8; 8]))
}

pub(super) fn collect_authorized_files(
    root: &Path,
    current: &Path,
    out: &mut Vec<(String, PathBuf, u64, i64)>,
    cancel_flag: &AtomicBool,
    had_directory_read_error: &mut bool,
) {
    if cancel_flag.load(Ordering::SeqCst) {
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        *had_directory_read_error = true;
        return;
    };
    let mut sorted_entries = Vec::new();
    for entry_res in entries {
        let Ok(entry) = entry_res else {
            *had_directory_read_error = true;
            return;
        };
        sorted_entries.push(entry);
    }
    sorted_entries.sort_by_key(|e| e.file_name());

    for entry in sorted_entries {
        if cancel_flag.load(Ordering::SeqCst) {
            return;
        }
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if is_excluded_relative_path(&rel_str) {
            continue;
        }
        let Ok(ft) = entry.file_type() else {
            *had_directory_read_error = true;
            return;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            collect_authorized_files(
                root,
                &path,
                out,
                cancel_flag,
                had_directory_read_error,
            );
            if *had_directory_read_error {
                return;
            }
        } else if ft.is_file() {
            let Ok(meta) = entry.metadata() else {
                *had_directory_read_error = true;
                return;
            };
            if is_dataless_metadata(&meta) {
                continue;
            }
            let mtime_ns = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(0);
            out.push((rel_str, path, meta.len(), mtime_ns));
        }
    }
}

#[cfg(target_os = "macos")]
fn is_dataless_metadata(meta: &fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    const SF_DATALESS_FLAG: u32 = 0x4000_0000;
    (meta.st_flags() & SF_DATALESS_FLAG) != 0
}

#[cfg(not(target_os = "macos"))]
fn is_dataless_metadata(_meta: &fs::Metadata) -> bool {
    false
}
