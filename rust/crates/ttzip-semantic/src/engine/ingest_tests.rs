// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use crate::engine::{
    SemanticFileEvent, SemanticIngestPhase, SemanticScopeDescriptor, SemanticScopeKind,
    SemanticSearchCoordinator, SemanticSearchRequest,
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

struct SelectiveFailingProvider {
    inner: DeterministicEmbeddingProvider,
}

impl EmbeddingProvider for SelectiveFailingProvider {
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
        if query.contains("poison_query") {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        self.inner.encode_query(query, is_code_query)
    }

    fn encode_document(
        &self,
        relative_path: Option<&str>,
        chunk_text: &str,
    ) -> Result<[i8; 128], SemanticModelError> {
        if chunk_text.contains("poison_chunk") {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        self.inner.encode_document(relative_path, chunk_text)
    }
}

#[test]
fn test_skipped_error_count_tracks_failed_chunks_and_queries() {
    let tmp = TempDir::new().expect("tempdir");
    let store = tmp.path().join("store_err");
    fs::create_dir_all(&store).expect("mkdir store_err");

    let provider = Arc::new(SelectiveFailingProvider {
        inner: DeterministicEmbeddingProvider::ready(),
    });
    let coordinator = SemanticSearchCoordinator::new(provider, store);
    assert_eq!(coordinator.skipped_error_count(), 0);
    assert!(coordinator.last_embedding_error().is_none());

    let embedded = coordinator.embed_batch(
        Some("notes.md"),
        &["valid database retry chunk", "poison_chunk bad data", "valid archive lzma chunk"],
    );
    assert_eq!(embedded.len(), 2);
    assert_eq!(coordinator.skipped_error_count(), 1);
    assert!(coordinator.last_embedding_error().is_some());

    coordinator.register_scope(SemanticScopeDescriptor {
        scope_id: "dir:err_scope".to_string(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: tmp.path().to_string_lossy().into_owned(),
        display_name: "err_scope".to_string(),
        is_authorized: true,
    });

    let prog = coordinator
        .index_entries(
            "dir:err_scope",
            &[SemanticFileEvent::CreatedOrModified {
                relative_path: "mixed.md".to_string(),
                archive_entry_path: None,
                bytes: b"poison_chunk inside document body".to_vec(),
                mtime_ns: 1,
            }],
        )
        .expect("index_entries should complete while accounting skipped chunk");
    assert_eq!(prog.phase, SemanticIngestPhase::Completed);
    assert_eq!(prog.indexed_segments, 1); // path segment preserved, failed content chunk skipped
    assert_eq!(coordinator.skipped_error_count(), 2);

    let search_resp = coordinator.search(&SemanticSearchRequest {
        request_id: 1,
        window_id: "w1".to_string(),
        query: "poison_query".to_string(),
        scope_ids: vec!["dir:err_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.3,
        semantic_weight: 0.7,
    });
    assert!(search_resp.degraded_lexical_only);
    assert_eq!(coordinator.skipped_error_count(), 3);
}
