// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Request-ID isolated hybrid semantic search coordinator, incremental scope indexer,
//! and cooperative cancellation manager.

use crate::chunker::{classify_content_tier, is_excluded_relative_path, ContentSupportTier};
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

mod ingest;
use ingest::FileIngestInput;

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

struct ManagedScope {
    descriptor: SemanticScopeDescriptor,
    index: PersistentScopeIndex,
    progress: SemanticIndexProgress,
    cancel_flag: Arc<AtomicBool>,
}

/// Thread-safe coordinator managing authorized scopes, persistent `.ttvec` v2 indices,
/// incremental indexing, and `request_id`-isolated hybrid search.
pub struct SemanticSearchCoordinator {
    provider: Arc<dyn EmbeddingProvider>,
    storage_dir: PathBuf,
    scopes: RwLock<BTreeMap<String, ManagedScope>>,
    latest_window_request: RwLock<BTreeMap<String, u64>>,
    cancelled_requests: RwLock<HashSet<u64>>,
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

    pub fn set_provider(&mut self, provider: Arc<dyn EmbeddingProvider>) {
        self.provider = provider;
    }

    fn scope_paths(&self, scope_id: &str) -> (PathBuf, PathBuf) {
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

        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        guard.insert(
            descriptor.scope_id.clone(),
            ManagedScope {
                descriptor,
                index,
                progress,
                cancel_flag: Arc::new(AtomicBool::new(false)),
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
        let (root_path, cancel_flag, is_authorized) = {
            let mut guard = self.scopes.write().expect("scopes lock poisoned");
            let Some(managed) = guard.get_mut(scope_id) else {
                return Err(SemanticModelError::RuntimeFailed {
                    reason: format!("Unknown scope_id: {}", scope_id),
                });
            };
            managed.cancel_flag.store(false, Ordering::SeqCst);
            managed.progress.phase = SemanticIngestPhase::Scanning;
            (
                PathBuf::from(&managed.descriptor.root_path),
                Arc::clone(&managed.cancel_flag),
                managed.descriptor.is_authorized,
            )
        };

        if !is_authorized {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Scope {} is not authorized", scope_id),
            });
        }

        let mut discovered_files = Vec::new();
        collect_authorized_files(&root_path, &root_path, &mut discovered_files, &cancel_flag);

        let mut events = Vec::new();
        let mut seen_rel_paths = HashSet::new();
        for (rel_path, full_path, file_size, mtime_ns) in discovered_files {
            if cancel_flag.load(Ordering::SeqCst) {
                break;
            }
            seen_rel_paths.insert(rel_path.clone());
            let tier = classify_content_tier(&rel_path, file_size, false);
            if tier == ContentSupportTier::Excluded {
                continue;
            }
            let bytes = match tier {
                ContentSupportTier::Tier2PlainText | ContentSupportTier::Tier2SourceCode => {
                    fs::read(&full_path).unwrap_or_default()
                }
                ContentSupportTier::Tier1PathOnly | ContentSupportTier::Excluded => Vec::new(),
            };
            events.push(SemanticFileEvent::CreatedOrModified {
                relative_path: rel_path,
                archive_entry_path: None,
                bytes,
                mtime_ns,
            });
        }

        {
            let guard = self.scopes.read().expect("scopes lock poisoned");
            if let Some(managed) = guard.get(scope_id) {
                for existing in managed.index.files() {
                    if !seen_rel_paths.contains(&existing.relative_path) {
                        events.push(SemanticFileEvent::Removed {
                            relative_path: existing.relative_path.clone(),
                        });
                    }
                }
            }
        }

        self.apply_file_events(scope_id, &events)
    }

    /// Applies incremental file creation, modification, or deletion events to a scope index
    /// and persists `.ttvec` v2 + `.ttidx` to disk.
    pub fn apply_file_events(
        &self,
        scope_id: &str,
        events: &[SemanticFileEvent],
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        let cancel_flag = {
            let mut guard = self.scopes.write().expect("scopes lock poisoned");
            let Some(managed) = guard.get_mut(scope_id) else {
                return Err(SemanticModelError::RuntimeFailed {
                    reason: format!("Unknown scope_id: {}", scope_id),
                });
            };
            managed.progress.phase = SemanticIngestPhase::ChunkingAndEmbedding;
            managed.progress.total_files = events.len() as u32;
            managed.progress.processed_files = 0;
            Arc::clone(&managed.cancel_flag)
        };

        let model_ready = matches!(self.provider.model_state(), SemanticModelState::Ready(_));

        for (idx, event) in events.iter().enumerate() {
            if cancel_flag.load(Ordering::SeqCst) {
                let mut guard = self.scopes.write().expect("scopes lock poisoned");
                if let Some(managed) = guard.get_mut(scope_id) {
                    managed.progress.phase = SemanticIngestPhase::Cancelled;
                    return Ok(managed.progress.clone());
                }
            }

            match event {
                SemanticFileEvent::Removed { relative_path } => {
                    let mut guard = self.scopes.write().expect("scopes lock poisoned");
                    if let Some(managed) = guard.get_mut(scope_id) {
                        managed.index.remove_file(relative_path);
                        managed.progress.processed_files = (idx + 1) as u32;
                        managed.progress.indexed_segments = managed.index.segments().len() as u32;
                        managed.progress.index_generation = managed.index.index_generation;
                    }
                }
                SemanticFileEvent::CreatedOrModified {
                    relative_path,
                    archive_entry_path,
                    bytes,
                    mtime_ns,
                } => {
                    if !model_ready {
                        continue;
                    }
                    self.ingest_single_file(
                        scope_id,
                        idx,
                        FileIngestInput {
                            relative_path,
                            archive_entry_path: archive_entry_path.clone(),
                            bytes,
                            mtime_ns: *mtime_ns,
                        },
                        &cancel_flag,
                    )?;
                }
            }
        }

        self.finalize_ingest_commit(scope_id, &cancel_flag, model_ready)
    }

    fn finalize_ingest_commit(
        &self,
        scope_id: &str,
        cancel_flag: &AtomicBool,
        model_ready: bool,
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        let (ttvec_path, ttidx_path) = self.scope_paths(scope_id);
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        let Some(managed) = guard.get_mut(scope_id) else {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Unknown scope_id: {}", scope_id),
            });
        };
        if cancel_flag.load(Ordering::SeqCst) {
            managed.progress.phase = SemanticIngestPhase::Cancelled;
            return Ok(managed.progress.clone());
        }
        if !model_ready {
            managed.progress.phase = SemanticIngestPhase::DegradedLexicalOnly;
            return Ok(managed.progress.clone());
        }
        managed.progress.phase = SemanticIngestPhase::Committing;
        managed
            .index
            .save_to_disk(&ttvec_path, &ttidx_path)
            .map_err(|reason| SemanticModelError::RuntimeFailed { reason })?;
        managed.progress.phase = SemanticIngestPhase::Completed;
        managed.progress.indexed_segments = managed.index.segments().len() as u32;
        managed.progress.index_generation = managed.index.index_generation;
        Ok(managed.progress.clone())
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

        let query_vec_opt = match self.provider.encode_query(clean_query, is_code_query) {
            Ok(vec_i8) if validate_non_zero_int8_vector(&vec_i8).is_ok() => Some(vec_i8),
            _ => None,
        };
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
    if root.exists() {
        collect_authorized_files(root, root, &mut files_on_disk, &dummy_cancel);
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

fn collect_authorized_files(
    root: &Path,
    current: &Path,
    out: &mut Vec<(String, PathBuf, u64, i64)>,
    cancel_flag: &AtomicBool,
) {
    if cancel_flag.load(Ordering::SeqCst) {
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    let mut sorted_entries: Vec<_> = entries.filter_map(Result::ok).collect();
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
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            collect_authorized_files(root, &path, out, cancel_flag);
        } else if ft.is_file() {
            if let Ok(meta) = entry.metadata() {
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
}
