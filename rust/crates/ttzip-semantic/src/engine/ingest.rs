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
        let guard = self.scopes.read().expect("scopes lock poisoned");
        Self::can_commit_locked(guard.get(scope_id), run_identity, cancel_flag)
    }

    pub(super) fn prepare_directory_scope_scan(
        &self,
        scope_id: &str,
    ) -> Result<(PathBuf, ScopeRunIdentity, Arc<AtomicBool>), SemanticModelError> {
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
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
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if let Some(managed) = guard.get_mut(scope_id) {
            if managed.auth_epoch == run_identity.auth_epoch
                && managed.active_run_token == run_identity.run_token
            {
                managed.progress.phase = SemanticIngestPhase::Failed;
                managed.progress.error_message = Some(reason.clone());
            }
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
        let mut model_ready: Option<bool> = None;
        let mut saw_removed = false;
        let mut working_index = self.prepare_run_working_index(
            scope_id, event_count, run_identity, cancel_flag,
        )?;

        for (idx, event) in events.into_iter().enumerate() {
            if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
                return Ok(self.cancelled_progress_snapshot(scope_id, run_identity));
            }
            if let Some(early_finish) = self.apply_single_ingest_event_step(
                scope_id,
                idx,
                event,
                &mut working_index,
                &mut model_ready,
                &mut saw_removed,
                run_identity,
                cancel_flag,
            )? {
                return Ok(early_finish);
            }
        }

        if let Some((root_path, had_stream_read_error)) = directory_read_guard {
            if had_stream_read_error.load(Ordering::SeqCst) {
                return self.fail_directory_scope_scan(scope_id, root_path, run_identity);
            }
        }

        let resolved_model_ready = match model_ready {
            Some(ready) => ready,
            None if saw_removed => true,
            None => matches!(self.provider.prepare(), SemanticModelState::Ready(_)),
        };

        self.finalize_ingest_commit(
            scope_id,
            working_index,
            run_identity,
            cancel_flag,
            resolved_model_ready,
        )
    }

    fn apply_single_ingest_event_step(
        &self,
        scope_id: &str,
        idx: usize,
        event: IngestFileEvent,
        working_index: &mut PersistentScopeIndex,
        model_ready: &mut Option<bool>,
        saw_removed: &mut bool,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> Result<Option<SemanticIndexProgress>, SemanticModelError> {
        match event {
            IngestFileEvent::Removed { relative_path } => {
                *saw_removed = true;
                working_index.remove_file(&relative_path);
                self.record_step_progress_if_active(
                    scope_id,
                    idx,
                    working_index,
                    run_identity,
                    cancel_flag,
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
                let is_ready = *model_ready.get_or_insert_with(|| {
                    matches!(self.provider.prepare(), SemanticModelState::Ready(_))
                });
                if !is_ready {
                    let degraded = self.finalize_ingest_commit(
                        scope_id,
                        working_index.clone(),
                        run_identity,
                        cancel_flag,
                        false,
                    )?;
                    return Ok(Some(degraded));
                }
                let committed_step = self.ingest_single_file(
                    scope_id,
                    idx,
                    FileIngestInput {
                        relative_path: &relative_path,
                        archive_entry_path,
                        bytes: &bytes,
                        file_size,
                        tier,
                        mtime_ns,
                    },
                    working_index,
                    run_identity,
                    cancel_flag,
                )?;
                if !committed_step {
                    return Ok(Some(self.cancelled_progress_snapshot(scope_id, run_identity)));
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
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        let Some(managed) = guard.get_mut(scope_id) else {
            return Err(SemanticModelError::RuntimeFailed {
                reason: format!("Unknown scope_id: {}", scope_id),
            });
        };
        if Self::can_commit_locked(Some(managed), run_identity, cancel_flag) {
            if let Some(managed) = guard.get_mut(scope_id) {
                managed.progress.phase = SemanticIngestPhase::ChunkingAndEmbedding;
                managed.progress.total_files = event_count as u32;
                managed.progress.processed_files = 0;
                return Ok(managed.index.clone());
            }
        }
        Ok(guard
            .get(scope_id)
            .map(|m| m.index.clone())
            .expect("checked Some above"))
    }

    pub(super) fn cancelled_progress_snapshot(
        &self,
        scope_id: &str,
        run_identity: ScopeRunIdentity,
    ) -> SemanticIndexProgress {
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if let Some(managed) = guard.get_mut(scope_id) {
            if managed.auth_epoch == run_identity.auth_epoch
                && (managed.active_run_token == run_identity.run_token
                    || managed.active_run_token == 0)
            {
                managed.progress.phase = SemanticIngestPhase::Cancelled;
                return managed.progress.clone();
            }
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
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if !Self::can_commit_locked(guard.get(scope_id), run_identity, cancel_flag) {
            if let Some(managed) = guard.get_mut(scope_id) {
                if managed.auth_epoch == run_identity.auth_epoch
                    && (managed.active_run_token == run_identity.run_token
                        || managed.active_run_token == 0)
                {
                    managed.progress.phase = SemanticIngestPhase::Cancelled;
                    return Ok(managed.progress.clone());
                }
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

        let mut hasher = blake3::Hasher::new();
        hasher.update(relative_path.as_bytes());
        if matches!(tier, ContentSupportTier::Tier1PathOnly) && bytes.is_empty() {
            hasher.update(&file_size.to_le_bytes());
        } else {
            hasher.update(bytes);
        }
        let content_blake3 = *hasher.finalize().as_bytes();

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

        let Some(built_segments) = self.build_file_segments(
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

        working_index.upsert_file_segments(
            FileIndexInput {
                relative_path: relative_path.to_string(),
                archive_entry_path,
                mtime_ns,
                file_size,
                content_blake3,
                tier,
            },
            built_segments,
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
        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if Self::can_commit_locked(guard.get(scope_id), run_identity, cancel_flag) {
            if let Some(managed) = guard.get_mut(scope_id) {
                managed.progress.processed_files = (idx + 1) as u32;
                managed.progress.indexed_segments = working_index.segments().len() as u32;
                managed.progress.index_generation = working_index.index_generation;
            }
        }
    }

    fn build_file_segments(
        &self,
        scope_id: &str,
        relative_path: &str,
        bytes: &[u8],
        tier: ContentSupportTier,
        run_identity: ScopeRunIdentity,
        cancel_flag: &AtomicBool,
    ) -> Result<Option<Vec<BuiltSegment>>, SemanticModelError> {
        if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
            return Ok(None);
        }
        let mut built_segments = Vec::new();
        let path_vec = self.provider.encode_document(None, relative_path)?;
        if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
            return Ok(None);
        }
        validate_non_zero_int8_vector(&path_vec)?;
        built_segments.push((true, 0, 0, 0, relative_path.to_string(), path_vec));

        if !matches!(
            tier,
            ContentSupportTier::Tier2PlainText | ContentSupportTier::Tier2SourceCode
        ) {
            return Ok(Some(built_segments));
        }
        let Some(text) = extract_safe_utf8_text(bytes) else {
            return Ok(Some(built_segments));
        };
        let is_code = tier == ContentSupportTier::Tier2SourceCode;
        let chunks = chunk_text_content(
            &text,
            is_code,
            DEFAULT_CHUNK_CHARS,
            DEFAULT_CHUNK_OVERLAP_CHARS,
        );
        for ch in chunks {
            if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
                return Ok(None);
            }
            let vec_i8 = self
                .provider
                .encode_document(Some(relative_path), &ch.text)?;
            if !self.is_run_still_active(scope_id, run_identity, cancel_flag) {
                return Ok(None);
            }
            validate_non_zero_int8_vector(&vec_i8)?;
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

        Ok(Some(built_segments))
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        SemanticIngestPhase, SemanticScopeDescriptor, SemanticScopeKind, SemanticSearchCoordinator,
    };
    use crate::model::{SemanticModelError, SemanticModelState};
    use crate::scoring::{DeterministicEmbeddingProvider, EmbeddingProvider};
    use std::fs;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    struct GatedEmbeddingProvider {
        inner: DeterministicEmbeddingProvider,
        doc_call_count: AtomicUsize,
        gate_on_call: usize,
        entered_tx: Mutex<Option<mpsc::Sender<()>>>,
        release_rx: Mutex<Option<mpsc::Receiver<()>>>,
    }

    impl GatedEmbeddingProvider {
        fn new(
            gate_on_call: usize,
        ) -> (Arc<Self>, mpsc::Receiver<()>, mpsc::Sender<()>) {
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let provider = Arc::new(Self {
                inner: DeterministicEmbeddingProvider::ready(),
                doc_call_count: AtomicUsize::new(0),
                gate_on_call,
                entered_tx: Mutex::new(Some(entered_tx)),
                release_rx: Mutex::new(Some(release_rx)),
            });
            (provider, entered_rx, release_tx)
        }
    }

    impl EmbeddingProvider for GatedEmbeddingProvider {
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
            let call_idx = self.doc_call_count.fetch_add(1, Ordering::SeqCst) + 1;
            if call_idx == self.gate_on_call {
                if let Some(tx) = self.entered_tx.lock().expect("entered_tx").take() {
                    let _ = tx.send(());
                }
                if let Some(rx) = self.release_rx.lock().expect("release_rx").take() {
                    let _ = rx.recv_timeout(Duration::from_secs(2));
                }
            }
            self.inner.encode_document(relative_path, chunk_text)
        }
    }

    #[test]
    fn test_s2_b_encode_a_suspended_cancel_start_b_release_a_no_resurrection_or_partial_commit() {
        let tmp = TempDir::new().expect("tempdir");
        let ws = tmp.path().join("ws");
        let store = tmp.path().join("store");
        fs::create_dir_all(&ws).expect("mkdir ws");
        fs::create_dir_all(&store).expect("mkdir store");

        let long_text = (0..60)
            .map(|i| format!("Line {}: database connection retry backoff chunk content.\n", i))
            .collect::<String>();
        fs::write(ws.join("multi_chunk.txt"), &long_text).expect("write multi_chunk.txt");

        let (provider, entered_rx, release_tx) = GatedEmbeddingProvider::new(3);
        let coordinator = Arc::new(SemanticSearchCoordinator::new(provider, store));
        let scope = SemanticScopeDescriptor {
            scope_id: "dir:s2_b".to_string(),
            kind: SemanticScopeKind::AuthorizedDirectory,
            root_path: ws.to_string_lossy().into_owned(),
            display_name: "ws".to_string(),
            is_authorized: true,
        };
        coordinator.register_scope(scope);

        let coord_a = Arc::clone(&coordinator);
        let (done_tx, done_rx) = mpsc::channel();
        let handle_a = thread::spawn(move || {
            let res = coord_a.index_directory_scope("dir:s2_b");
            let _ = done_tx.send(());
            res
        });

        if entered_rx.recv_timeout(Duration::from_secs(2)).is_err() {
            let _ = release_tx.send(());
            panic!("Run A timed out entering gated encode_document on chunk 2");
        }
        let flag_a: Arc<AtomicBool> = coordinator
            .scope_cancellation_flag("dir:s2_b")
            .expect("flag A");

        coordinator.cancel_indexing("dir:s2_b");
        assert!(flag_a.load(Ordering::SeqCst), "Flag A must be true after cancel");

        let prog_b = coordinator
            .index_directory_scope("dir:s2_b")
            .expect("Run B must succeed");
        assert_eq!(prog_b.phase, SemanticIngestPhase::Completed);
        assert!(
            flag_a.load(Ordering::SeqCst),
            "Starting Run B must NEVER resurrect Run A's cancellation flag"
        );

        let _ = release_tx.send(());
        assert!(
            done_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "Run A must complete within bounded timeout after gate release"
        );
        let prog_a = handle_a.join().expect("join A").expect("Run A result");
        assert_eq!(prog_a.phase, SemanticIngestPhase::Cancelled);

        let final_prog = coordinator.scope_progress("dir:s2_b").expect("final progress");
        assert_eq!(final_prog.phase, SemanticIngestPhase::Completed);
        assert_eq!(final_prog.indexed_segments, prog_b.indexed_segments);
        assert!(final_prog.indexed_segments >= 3);
    }

    #[test]
    fn test_s2_c_cancel_revoke_reauthorize_blocks_stale_commit() {
        let tmp = TempDir::new().expect("tempdir");
        let ws = tmp.path().join("ws_c");
        let store = tmp.path().join("store_c");
        fs::create_dir_all(&ws).expect("mkdir ws_c");
        fs::create_dir_all(&store).expect("mkdir store_c");
        fs::write(ws.join("stale_from_a.txt"), "stale body from run A").expect("write stale");

        let (provider, entered_rx, release_tx) = GatedEmbeddingProvider::new(2);
        let coordinator = Arc::new(SemanticSearchCoordinator::new(provider, store));

        let scope = SemanticScopeDescriptor {
            scope_id: "dir:s2_c".to_string(),
            kind: SemanticScopeKind::AuthorizedDirectory,
            root_path: ws.to_string_lossy().into_owned(),
            display_name: "ws_c".to_string(),
            is_authorized: true,
        };
        coordinator.register_scope(scope.clone());

        let coord_a = Arc::clone(&coordinator);
        let (done_tx, done_rx) = mpsc::channel();
        let handle_a = thread::spawn(move || {
            let res = coord_a.index_directory_scope("dir:s2_c");
            let _ = done_tx.send(());
            res
        });

        if entered_rx.recv_timeout(Duration::from_secs(2)).is_err() {
            let _ = release_tx.send(());
            panic!("Run A timed out entering encode_document");
        }

        coordinator.cancel_indexing("dir:s2_c");
        assert!(coordinator.revoke_scope("dir:s2_c"));
        fs::remove_file(ws.join("stale_from_a.txt")).expect("remove stale file");
        coordinator.register_scope(scope);

        let _ = release_tx.send(());
        assert!(
            done_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "Run A must complete within bounded timeout after gate release"
        );
        let prog_a = handle_a.join().expect("join A").expect("res A");
        let post_c = coordinator.scope_progress("dir:s2_c").expect("progress C");
        assert_eq!(prog_a.phase, SemanticIngestPhase::Cancelled);
        assert_eq!(
            post_c.indexed_segments, 0,
            "Retired Run A must never write into re-authorized Scope C"
        );
    }
}
