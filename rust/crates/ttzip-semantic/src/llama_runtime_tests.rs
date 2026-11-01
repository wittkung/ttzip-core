// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Controlled runtime lifecycle tests. Never open a real model or native backend.

use super::*;
use crate::engine::{
    SemanticFileEvent, SemanticIngestPhase, SemanticScopeDescriptor, SemanticScopeKind,
    SemanticSearchCoordinator, SemanticSearchRequest,
};
use crate::scoring::DeterministicEmbeddingProvider;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[test]
fn model_state_is_a_read_even_when_a_model_path_exists() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir.path().join("controlled.gguf");
    std::fs::write(&path, b"GGUF").expect("write tiny invalid fixture");
    let runtime = LlamaRuntimeEngine::new(LlamaRuntimeConfig {
        model_path: path,
        ..LlamaRuntimeConfig::default()
    });
    assert_eq!(runtime.model_state(), SemanticModelState::NotInstalled);
    assert_eq!(runtime.state(), SemanticModelState::NotInstalled);
    assert!(runtime
        .session
        .lock()
        .expect("session lock")
        .bridge_ptr
        .is_null());
}

#[test]
fn published_state_is_readable_while_native_session_is_busy() {
    let runtime = Arc::new(LlamaRuntimeEngine::new(LlamaRuntimeConfig::default()));
    let session = runtime.session.lock().expect("session lock");
    runtime.publish_state(SemanticModelState::Verifying);
    let reader = Arc::clone(&runtime);
    let (sent, received) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        sent.send((reader.state(), reader.model_state()))
            .expect("send state");
    });
    let observed = received.recv_timeout(Duration::from_secs(2));
    drop(session);
    thread.join().expect("reader exits");
    assert_eq!(
        observed.expect("state read must not wait for session"),
        (SemanticModelState::Verifying, SemanticModelState::Verifying,)
    );
}

#[test]
fn explicit_prepare_publishes_failure_and_unload_publishes_uninstalled() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir.path().join("invalid.gguf");
    std::fs::write(&path, b"GGUF").expect("write tiny invalid fixture");
    let runtime = LlamaRuntimeEngine::new(LlamaRuntimeConfig {
        model_path: path,
        ..LlamaRuntimeConfig::default()
    });
    let error = runtime
        .verify_and_prepare()
        .expect_err("tiny file must fail before native init");
    assert!(matches!(error, SemanticModelError::SizeMismatch { .. }));
    assert_eq!(runtime.model_state(), SemanticModelState::Failed(error));
    {
        let mut session = runtime.session.lock().expect("session lock");
        session.state = SemanticModelState::Ready(SemanticModelInfo::pinned(
            SemanticComputeBackend::CpuFallback,
        ));
        runtime.publish_state(session.state.clone());
    }
    runtime.unload();
    assert_eq!(runtime.model_state(), SemanticModelState::NotInstalled);
}

struct ExplicitPreparationProvider {
    inner: DeterministicEmbeddingProvider,
    ready: AtomicBool,
    preparation_calls: AtomicUsize,
    fail_preparation: bool,
}

impl ExplicitPreparationProvider {
    fn new(fail_preparation: bool) -> Self {
        Self {
            inner: DeterministicEmbeddingProvider::ready(),
            ready: AtomicBool::new(false),
            preparation_calls: AtomicUsize::new(0),
            fail_preparation,
        }
    }
}

impl EmbeddingProvider for ExplicitPreparationProvider {
    fn model_state(&self) -> SemanticModelState {
        if self.ready.load(Ordering::SeqCst) {
            self.inner.model_state()
        } else {
            SemanticModelState::NotInstalled
        }
    }

    fn prepare(&self) -> SemanticModelState {
        self.preparation_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_preparation {
            return SemanticModelState::Failed(SemanticModelError::RuntimeFailed {
                reason: "controlled preparation failure".into(),
            });
        }
        self.ready.store(true, Ordering::SeqCst);
        self.model_state()
    }

    fn model_fingerprint(&self) -> u64 {
        self.inner.model_fingerprint()
    }

    fn encode_query(&self, query: &str, code: bool) -> Result<[i8; 128], SemanticModelError> {
        assert!(
            self.ready.load(Ordering::SeqCst),
            "query requires explicit preparation"
        );
        self.inner.encode_query(query, code)
    }

    fn encode_document(
        &self,
        path: Option<&str>,
        text: &str,
    ) -> Result<[i8; 128], SemanticModelError> {
        assert!(
            self.ready.load(Ordering::SeqCst),
            "ingest requires explicit preparation"
        );
        self.inner.encode_document(path, text)
    }
}

fn request() -> SemanticSearchRequest {
    SemanticSearchRequest {
        request_id: 1,
        window_id: "controlled".into(),
        query: "database retry".into(),
        scope_ids: vec![],
        top_k: 10,
        lexical_weight: 0.5,
        semantic_weight: 0.5,
    }
}

#[test]
fn first_ingest_and_restarted_search_explicitly_prepare() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let provider = Arc::new(ExplicitPreparationProvider::new(false));
    let engine = SemanticSearchCoordinator::new(provider.clone(), dir.path().into());
    let scope = SemanticScopeDescriptor {
        scope_id: "scope".into(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: dir.path().to_string_lossy().into(),
        display_name: "controlled".into(),
        is_authorized: true,
    };
    engine.register_scope(scope.clone());
    assert_eq!(engine.model_state(), SemanticModelState::NotInstalled);
    assert_eq!(provider.preparation_calls.load(Ordering::SeqCst), 0);
    let progress = engine
        .apply_file_events(
            "scope",
            &[SemanticFileEvent::CreatedOrModified {
                relative_path: "retry.md".into(),
                archive_entry_path: None,
                bytes: b"database retry with exponential backoff".to_vec(),
                mtime_ns: 1,
            }],
        )
        .expect("first index operation");
    assert_eq!(progress.phase, SemanticIngestPhase::Completed);
    assert_eq!(provider.preparation_calls.load(Ordering::SeqCst), 1);
    drop(engine);
    let restarted_provider = Arc::new(ExplicitPreparationProvider::new(false));
    let restarted = SemanticSearchCoordinator::new(restarted_provider.clone(), dir.path().into());
    restarted.register_scope(scope);
    assert_eq!(restarted.model_state(), SemanticModelState::NotInstalled);
    let result = restarted.search(&request());
    assert!(!result.degraded_lexical_only);
    assert!(!result.hits.is_empty());
    assert_eq!(
        restarted_provider.preparation_calls.load(Ordering::SeqCst),
        1
    );
}

#[test]
fn explicit_preparation_failure_keeps_lexical_degradation() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let provider = Arc::new(ExplicitPreparationProvider::new(true));
    let engine = SemanticSearchCoordinator::new(provider.clone(), dir.path().into());
    engine.register_scope(SemanticScopeDescriptor {
        scope_id: "scope".into(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: dir.path().to_string_lossy().into(),
        display_name: "controlled".into(),
        is_authorized: true,
    });
    let result = engine
        .apply_file_events("scope", &[])
        .expect("existing lexical degradation");
    assert_eq!(result.phase, SemanticIngestPhase::DegradedLexicalOnly);
    assert!(engine.search(&request()).degraded_lexical_only);
    assert_eq!(provider.preparation_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn explicit_runtime_preparation_boundary_is_separate_from_status_reads() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let provider = Arc::new(ExplicitPreparationProvider::new(false));
    let engine = SemanticSearchCoordinator::new(provider.clone(), dir.path().into());
    assert_eq!(engine.model_state(), SemanticModelState::NotInstalled);
    assert_eq!(provider.preparation_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        engine.prepare_model_runtime(),
        SemanticModelState::Ready(_)
    ));
    assert!(matches!(engine.model_state(), SemanticModelState::Ready(_)));
    assert_eq!(provider.preparation_calls.load(Ordering::SeqCst), 1);
}
