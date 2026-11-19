// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Per-file incremental freshness checks, scope run-identity commit arbitration,
//! and path/content segment embedding.

use super::{
    ManagedScope, ScopeRunIdentity, SemanticFileEvent, SemanticIndexProgress, SemanticIngestPhase,
    SemanticSearchCoordinator,
};
use crate::chunker::{
    chunk_text_content, classify_content_tier, extract_safe_utf8_text, is_excluded_relative_path,
    ContentSupportTier, DEFAULT_CHUNK_CHARS, DEFAULT_CHUNK_OVERLAP_CHARS,
};
use crate::index::{FileIndexInput, PersistentScopeIndex};
use crate::model::{validate_non_zero_int8_vector, SemanticModelError, SemanticModelState};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

type BuiltSegment = (bool, u32, u32, u8, String, [i8; 128]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum IngestFileEvent {
    CreatedOrModified {
        relative_path: String,
        archive_entry_path: Option<String>,
        bytes: Vec<u8>,
        file_size: u64,
        tier: ContentSupportTier,
        mtime_ns: i64,
    },
    Removed {
        relative_path: String,
    },
}

impl From<&SemanticFileEvent> for IngestFileEvent {
    fn from(event: &SemanticFileEvent) -> Self {
        match event {
            SemanticFileEvent::CreatedOrModified {
                relative_path,
                archive_entry_path,
                bytes,
                mtime_ns,
            } => {
                let file_size = bytes.len() as u64;
                let tier = classify_content_tier(relative_path, file_size.max(1), false);
                let bounded_bytes = if matches!(tier, ContentSupportTier::Tier1PathOnly) {
                    Vec::new()
                } else {
                    bytes.clone()
                };
                Self::CreatedOrModified {
                    relative_path: relative_path.clone(),
                    archive_entry_path: archive_entry_path.clone(),
                    bytes: bounded_bytes,
                    file_size,
                    tier,
                    mtime_ns: *mtime_ns,
                }
            }
            SemanticFileEvent::Removed { relative_path } => Self::Removed {
                relative_path: relative_path.clone(),
            },
        }
    }
}

pub(super) struct FileIngestInput<'a> {
    pub relative_path: &'a str,
    pub archive_entry_path: Option<String>,
    pub bytes: &'a [u8],
    pub file_size: u64,
    pub tier: ContentSupportTier,
    pub mtime_ns: i64,
}

struct IngestBatchContext<'a> {
    scope_id: &'a str,
    run_identity: ScopeRunIdentity,
    cancel_flag: &'a AtomicBool,
    working_index: PersistentScopeIndex,
    model_ready: Option<bool>,
    saw_removed: bool,
}

impl SemanticSearchCoordinator {
    pub(super) fn can_commit_locked(
        managed: Option<&ManagedScope>,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> bool {
        if cancel_flag.load(Ordering::SeqCst) {
            return false;
        }
        let Some(managed) = managed else {
            return false;
        };
        managed.descriptor.is_authorized
            && managed.auth_epoch == run_identity.auth_epoch
            && managed.active_run_token == run_identity.run_token
    }

    pub(super) fn is_run_still_active(
        &self,
        scope_id: &str,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> bool {
        if cancel_flag.load(Ordering::SeqCst) {
            return false;
        }
        let guard = self.scopes.read().unwrap_or_else(|e| e.into_inner());
        Self::can_commit_locked(guard.get(scope_id), run_identity, cancel_flag)
    }

    pub(super) fn prepare_directory_scope_scan(
        &self,
        scope_id: &str,
    ) -> Result<(PathBuf, ScopeRunIdentity, Arc<AtomicBool>), SemanticModelError> {
        let mut guard = self.scopes.write().unwrap_or_else(|e| e.into_inner());
        let Some(managed) = guard.get_mut(scope_id) else {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Unknown scope_id: {}", scope_id),
            });
        };
        if !managed.descriptor.is_authorized {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Scope {} is not authorized", scope_id),
            });
        }
        managed.cancel_flag.store(true, Ordering::SeqCst);
        let new_cancel = Arc::new(AtomicBool::new(false));
        managed.cancel_flag = Arc::clone(&new_cancel);
        let run_token = self.next_sequence.fetch_add(1, Ordering::SeqCst);
        managed.active_run_token = run_token;
        managed.progress.phase = SemanticIngestPhase::Scanning;
        let identity = ScopeRunIdentity {
            auth_epoch: managed.auth_epoch,
            run_token,
        };
        Ok((
            PathBuf::from(&managed.descriptor.root_path),
            identity,
            new_cancel,
        ))
    }

    pub(super) fn fail_directory_scope_scan(
        &self,
        scope_id: &str,
        root_path: &Path,
        run_identity: ScopeRunIdentity,
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        let reason = format!(
            "Failed to read authorized directory (permission or I/O error): {}",
            root_path.display()
        );
        let mut guard = self.scopes.write().unwrap_or_else(|e| e.into_inner());
        if let Some(managed) = guard.get_mut(scope_id)
            && managed.auth_epoch == run_identity.auth_epoch
            && managed.active_run_token == run_identity.run_token
        {
            managed.progress.phase = SemanticIngestPhase::Failed;
            managed.progress.error_message = Some(reason.clone());
        }
        Err(SemanticModelError::RuntimeFailed { reason })
    }

    pub(super) fn apply_file_events_for_run(
        &self,
        scope_id: &str,
        events: &[SemanticFileEvent],
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        self.apply_ingest_event_iter_for_run(
            scope_id,
            events.iter().map(IngestFileEvent::from),
            events.len(),
            run_identity,
            cancel_flag,
            None,
        )
    }

    pub(super) fn apply_ingest_event_iter_for_run<I>(
        &self,
        scope_id: &str,
        events: I,
        event_count: usize,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
        directory_read_guard: Option<(&Path, &AtomicBool)>,
    ) -> Result<SemanticIndexProgress, SemanticModelError>
    where
        I: IntoIterator<Item = IngestFileEvent>,
    {
        let working_index = self.prepare_run_working_index(
            scope_id, event_count, run_identity, cancel_flag,
        )?;
        let mut batch = IngestBatchContext {
            scope_id,
            run_identity,
            cancel_flag,
            working_index,
            model_ready: None,
            saw_removed: false,
        };

        for (idx, event) in events.into_iter().enumerate() {
            if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
                return Ok(self.cancelled_progress_snapshot(scope_id, run_identity));
            }
            if let Some(early_finish) =
                self.apply_single_ingest_event_step(&mut batch, idx, event)?
            {
                return Ok(early_finish);
            }
        }

        if let Some((root_path, had_stream_read_error)) = directory_read_guard
            && had_stream_read_error.load(Ordering::SeqCst)
        {
            return self.fail_directory_scope_scan(scope_id, root_path, run_identity);
        }

        let resolved_model_ready = match batch.model_ready {
            Some(ready) => ready,
            None if batch.saw_removed => true,
            None => matches!(self.provider.prepare(), SemanticModelState::Ready(_)),
        };

        self.finalize_ingest_commit(
            scope_id,
            batch.working_index,
            run_identity,
            cancel_flag,
            resolved_model_ready,
        )
    }

    fn apply_single_ingest_event_step(
        &self,
        batch: &mut IngestBatchContext<'_>,
        idx: usize,
        event: IngestFileEvent,
    ) -> Result<Option<SemanticIndexProgress>, SemanticModelError> {
        match event {
            IngestFileEvent::Removed { relative_path } => {
                batch.saw_removed = true;
                batch.working_index.remove_file(&relative_path);
                self.record_step_progress_if_active(
                    batch.scope_id,
                    idx,
                    &batch.working_index,
                    batch.run_identity,
                    batch.cancel_flag,
                );
                Ok(None)
            }
            IngestFileEvent::CreatedOrModified {
                relative_path,
                archive_entry_path,
                bytes,
                file_size,
                tier,
                mtime_ns,
            } => {
                let is_ready = *batch.model_ready.get_or_insert_with(|| {
                    matches!(self.provider.prepare(), SemanticModelState::Ready(_))
                });
                if !is_ready {
                    let degraded = self.finalize_ingest_commit(
                        batch.scope_id,
                        batch.working_index.clone(),
                        batch.run_identity,
                        batch.cancel_flag,
                        false,
                    )?;
                    return Ok(Some(degraded));
                }
                let committed_step = self.ingest_single_file(
                    batch.scope_id,
                    idx,
                    FileIngestInput {
                        relative_path: &relative_path,
                        archive_entry_path,
                        bytes: &bytes,
                        file_size,
                        tier,
                        mtime_ns,
                    },
                    &mut batch.working_index,
                    batch.run_identity,
                    batch.cancel_flag,
                )?;
                if !committed_step {
                    return Ok(Some(
                        self.cancelled_progress_snapshot(batch.scope_id, batch.run_identity),
                    ));
                }
                Ok(None)
            }
        }
    }

    fn prepare_run_working_index(
        &self,
        scope_id: &str,
        event_count: usize,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> Result<PersistentScopeIndex, SemanticModelError> {
        let mut guard = self.scopes.write().unwrap_or_else(|e| e.into_inner());
        let Some(managed) = guard.get_mut(scope_id) else {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Unknown scope_id: {}", scope_id),
            });
        };
        if Self::can_commit_locked(Some(managed), run_identity, cancel_flag) {
            managed.progress.phase = SemanticIngestPhase::ChunkingAndEmbedding;
            managed.progress.total_files = event_count as u32;
            managed.progress.processed_files = 0;
        }
        Ok(managed.index.clone())
    }

    pub(super) fn cancelled_progress_snapshot(
        &self,
        scope_id: &str,
        run_identity: ScopeRunIdentity,
    ) -> SemanticIndexProgress {
        let mut guard = self.scopes.write().unwrap_or_else(|e| e.into_inner());
        if let Some(managed) = guard.get_mut(scope_id)
            && managed.auth_epoch == run_identity.auth_epoch
            && (managed.active_run_token == run_identity.run_token
                || managed.active_run_token == 0)
        {
            managed.progress.phase = SemanticIngestPhase::Cancelled;
            return managed.progress.clone();
        }
        SemanticIndexProgress {
            scope_id: scope_id.to_string(),
            phase: SemanticIngestPhase::Cancelled,
            processed_files: 0,
            total_files: 0,
            indexed_segments: 0,
            index_generation: 0,
            error_message: None,
        }
    }

    fn finalize_ingest_commit(
        &self,
        scope_id: &str,
        working_index: PersistentScopeIndex,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
        model_ready: bool,
    ) -> Result<SemanticIndexProgress, SemanticModelError> {
        let (ttvec_path, ttidx_path) = self.scope_paths(scope_id);
        let mut guard = self.scopes.write().unwrap_or_else(|e| e.into_inner());
        if !Self::can_commit_locked(guard.get(scope_id), run_identity, cancel_flag) {
            if let Some(managed) = guard.get_mut(scope_id)
                && managed.auth_epoch == run_identity.auth_epoch
                && (managed.active_run_token == run_identity.run_token
                    || managed.active_run_token == 0)
            {
                managed.progress.phase = SemanticIngestPhase::Cancelled;
                return Ok(managed.progress.clone());
            }
            return Ok(SemanticIndexProgress {
                scope_id: scope_id.to_string(),
                phase: SemanticIngestPhase::Cancelled,
                processed_files: 0,
                total_files: 0,
                indexed_segments: 0,
                index_generation: 0,
                error_message: None,
            });
        }
        let Some(managed) = guard.get_mut(scope_id) else {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Unknown scope_id: {}", scope_id),
            });
        };
        if !model_ready {
            managed.progress.phase = SemanticIngestPhase::DegradedLexicalOnly;
            return Ok(managed.progress.clone());
        }
        managed.progress.phase = SemanticIngestPhase::Committing;
        working_index
            .save_to_disk(&ttvec_path, &ttidx_path)
            .map_err(|reason| SemanticModelError::RuntimeFailed { reason })?;
        managed.index = working_index;
        managed.progress.phase = SemanticIngestPhase::Completed;
        managed.progress.indexed_segments = managed.index.segments().len() as u32;
        managed.progress.index_generation = managed.index.index_generation;
        Ok(managed.progress.clone())
    }

    pub(super) fn ingest_single_file(
        &self,
        scope_id: &str,
        idx: usize,
        file: FileIngestInput<'_>,
        working_index: &mut PersistentScopeIndex,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> Result<bool, SemanticModelError> {
        let FileIngestInput {
            relative_path,
            archive_entry_path,
            bytes,
            file_size,
            tier,
            mtime_ns,
        } = file;
        if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
            return Ok(false);
        }
        if is_excluded_relative_path(relative_path) || tier == ContentSupportTier::Excluded {
            return Ok(true);
        }

        let content_blake3 = compute_file_content_hash(relative_path, bytes, file_size, tier);
        if working_index.is_file_up_to_date(relative_path, mtime_ns, file_size, &content_blake3) {
            self.record_step_progress_if_active(
                scope_id,
                idx,
                working_index,
                run_identity,
                cancel_flag,
            );
            return Ok(true);
        }

        let Some((built_segments, had_chunk_failure)) = self.build_file_segments(
            scope_id,
            relative_path,
            bytes,
            tier,
            run_identity,
            cancel_flag,
        )?
        else {
            return Ok(false);
        };

        if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
            return Ok(false);
        }

        commit_built_file_segments(
            working_index,
            FileIndexInput {
                relative_path: relative_path.to_string(),
                archive_entry_path,
                mtime_ns,
                file_size,
                content_blake3,
                tier,
            },
            built_segments,
            had_chunk_failure,
        )?;
        self.record_step_progress_if_active(
            scope_id,
            idx,
            working_index,
            run_identity,
            cancel_flag,
        );
        Ok(true)
    }

    pub(super) fn record_step_progress_if_active(
        &self,
        scope_id: &str,
        idx: usize,
        working_index: &PersistentScopeIndex,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) {
        let mut guard = self.scopes.write().unwrap_or_else(|e| e.into_inner());
        if Self::can_commit_locked(guard.get(scope_id), run_identity, cancel_flag)
            && let Some(managed) = guard.get_mut(scope_id)
        {
            managed.progress.processed_files = (idx + 1) as u32;
            managed.progress.indexed_segments = working_index.segments().len() as u32;
            managed.progress.index_generation = working_index.index_generation;
        }
    }

    fn encode_validated_segment(
        &self,
        relative_path: Option<&str>,
        text: &str,
    ) -> Result<[i8; 128], SemanticModelError> {
        let vec_i8 = self.provider.encode_document(relative_path, text).inspect_err(|err| {
            self.record_skipped_error(err);
        })?;
        validate_non_zero_int8_vector(&vec_i8).inspect_err(|err| {
            self.record_skipped_error(err);
        })?;
        Ok(vec_i8)
    }

    fn build_file_segments(
        &self,
        scope_id: &str,
        relative_path: &str,
        bytes: &[u8],
        tier: ContentSupportTier,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> Result<Option<(Vec<BuiltSegment>, bool)>, SemanticModelError> {
        if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
            return Ok(None);
        }
        let mut built_segments = Vec::new();
        let path_vec = self.encode_validated_segment(None, relative_path)?;
        if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
            return Ok(None);
        }
        built_segments.push((true, 0, 0, 0, relative_path.to_string(), path_vec));

        if !matches!(
            tier,
            ContentSupportTier::Tier2PlainText | ContentSupportTier::Tier2SourceCode
        ) {
            return Ok(Some((built_segments, false)));
        }
        let Some(text) = extract_safe_utf8_text(bytes) else {
            return Ok(Some((built_segments, false)));
        };
        let is_code = tier == ContentSupportTier::Tier2SourceCode;
        let chunks = chunk_text_content(
            &text,
            is_code,
            DEFAULT_CHUNK_CHARS,
            DEFAULT_CHUNK_OVERLAP_CHARS,
        );
        let mut had_chunk_failure = false;
        for ch in chunks {
            if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
                return Ok(None);
            }
            let Ok(vec_i8) = self.encode_validated_segment(Some(relative_path), &ch.text) else {
                had_chunk_failure = true;
                continue;
            };
            if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
                return Ok(None);
            }
            let preview = ch.text.chars().take(240).collect::<String>();
            built_segments.push((
                false,
                ch.line_start,
                ch.line_end,
                ch.definition_strength,
                preview,
                vec_i8,
            ));
        }

        Ok(Some((built_segments, had_chunk_failure)))
    }
}

fn compute_file_content_hash(
    relative_path: &str,
    bytes: &[u8],
    file_size: u64,
    tier: ContentSupportTier,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(relative_path.as_bytes());
    if matches!(tier, ContentSupportTier::Tier1PathOnly) && bytes.is_empty() {
        hasher.update(&file_size.to_le_bytes());
    } else {
        hasher.update(bytes);
    }
    *hasher.finalize().as_bytes()
}

fn commit_built_file_segments(
    working_index: &mut PersistentScopeIndex,
    mut input: FileIndexInput,
    built_segments: Vec<BuiltSegment>,
    had_chunk_failure: bool,
) -> Result<(), SemanticModelError> {
    if had_chunk_failure {
        if working_index.contains_file(&input.relative_path) {
            return Ok(());
        }
        input.content_blake3[0] ^= 0xff;
        input.mtime_ns = i64::MIN;
    }
    working_index.upsert_file_segments(input, built_segments)?;
    Ok(())
}

#[cfg(test)]
#[path = "ingest_tests.rs"]
mod tests;

#[cfg(test)]
mod retry_eligibility_tests {
    use crate::engine::{
        SemanticFileEvent, SemanticIngestPhase, SemanticScopeDescriptor, SemanticScopeKind,
        SemanticSearchCoordinator,
    };
    use crate::model::{SemanticModelError, SemanticModelState};
    use crate::scoring::{DeterministicEmbeddingProvider, EmbeddingProvider};
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;

    struct TogglePoisonProvider {
        inner: DeterministicEmbeddingProvider,
        fail_poison: AtomicBool,
    }

    impl EmbeddingProvider for TogglePoisonProvider {
        fn model_state(&self) -> SemanticModelState {
            self.inner.model_state()
        }

        fn model_fingerprint(&self) -> u64 {
            self.inner.model_fingerprint()
        }

        fn encode_query(
            &self,
            query: &str,
            is_code_query: bool,
        ) -> Result<[i8; 128], SemanticModelError> {
            self.inner.encode_query(query, is_code_query)
        }

        fn encode_document(
            &self,
            relative_path: Option<&str>,
            chunk_text: &str,
        ) -> Result<[i8; 128], SemanticModelError> {
            if self.fail_poison.load(Ordering::SeqCst) && chunk_text.contains("poison_chunk") {
                return Err(SemanticModelError::ZeroOrNonFiniteVector);
            }
            self.inner.encode_document(relative_path, chunk_text)
        }
    }

    #[test]
    fn test_failed_chunk_preserves_existing_segments_and_keeps_file_retry_eligible() {
        let tmp = TempDir::new().expect("tempdir");
        let store = tmp.path().join("store_retry");
        fs::create_dir_all(&store).expect("mkdir store_retry");

        let provider = Arc::new(TogglePoisonProvider {
            inner: DeterministicEmbeddingProvider::ready(),
            fail_poison: AtomicBool::new(false),
        });
        let coordinator = SemanticSearchCoordinator::new(Arc::clone(&provider) as _, store);
        coordinator.register_scope(SemanticScopeDescriptor {
            scope_id: "dir:retry_scope".to_string(),
            kind: SemanticScopeKind::AuthorizedDirectory,
            root_path: tmp.path().to_string_lossy().into_owned(),
            display_name: "retry_scope".to_string(),
            is_authorized: true,
        });

        let v1_body = (0..40)
            .map(|i| format!("Line {i}: initial healthy indexed content segment data.\n"))
            .collect::<String>();
        let prog_v1 = coordinator
            .index_entries(
                "dir:retry_scope",
                &[SemanticFileEvent::CreatedOrModified {
                    relative_path: "existing.md".to_string(),
                    archive_entry_path: None,
                    bytes: v1_body.into_bytes(),
                    mtime_ns: 10,
                }],
            )
            .expect("v1 index");
        assert_eq!(prog_v1.phase, SemanticIngestPhase::Completed);
        assert!(prog_v1.indexed_segments >= 3);

        provider.fail_poison.store(true, Ordering::SeqCst);
        let v2_body = format!(
            "{}\npoison_chunk transient failure tail\n",
            (0..40)
                .map(|i| format!("Line {i}: updated document body before transient failure.\n"))
                .collect::<String>()
        );
        let v2_event = SemanticFileEvent::CreatedOrModified {
            relative_path: "existing.md".to_string(),
            archive_entry_path: None,
            bytes: v2_body.into_bytes(),
            mtime_ns: 20,
        };
        let prog_v2_fail = coordinator
            .index_entries("dir:retry_scope", std::slice::from_ref(&v2_event))
            .expect("v2 failing index");
        assert_eq!(coordinator.skipped_error_count(), 1);
        assert_eq!(
            prog_v2_fail.indexed_segments, prog_v1.indexed_segments,
            "existing full segments must not be overwritten with partial segments on chunk failure"
        );

        let new_event = SemanticFileEvent::CreatedOrModified {
            relative_path: "new_partial.md".to_string(),
            archive_entry_path: None,
            bytes: b"poison_chunk in newly discovered file".to_vec(),
            mtime_ns: 30,
        };
        let prog_new_fail = coordinator
            .index_entries("dir:retry_scope", std::slice::from_ref(&new_event))
            .expect("new file failing index");
        assert_eq!(coordinator.skipped_error_count(), 2);
        let segments_after_partial = prog_new_fail.indexed_segments;

        provider.fail_poison.store(false, Ordering::SeqCst);
        let prog_retry = coordinator
            .index_entries("dir:retry_scope", &[v2_event, new_event])
            .expect("retry after transient failure clears");
        assert!(
            prog_retry.indexed_segments > segments_after_partial,
            "unchanged files with prior chunk failures must be re-indexed on subsequent scan"
        );
    }
}
