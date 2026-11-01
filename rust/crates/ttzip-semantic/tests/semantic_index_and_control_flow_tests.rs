// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unit and integration tests for `ttzip-semantic` content chunking,
//! `.ttvec` v2 persistent index integrity, and `SemanticSearchCoordinator`
//! control-flow lifecycle using the isolated `DeterministicEmbeddingProvider`.

use std::fs;
use tempfile::TempDir;
use ttzip_semantic::{
    chunk_text_content, classify_content_tier, is_excluded_relative_path, ContentSupportTier,
    DeterministicEmbeddingProvider, EmbeddingProvider, IndexLoadOutcome, PersistentScopeIndex,
    SemanticFileEvent, SemanticIngestPhase, SemanticScopeDescriptor, SemanticScopeKind,
    SemanticSearchCoordinator, SemanticSearchRequest, PINNED_MODEL_FINGERPRINT,
    TEST_DOUBLE_MODEL_FINGERPRINT,
};

#[test]
fn test_path_filtering_and_chunking_with_line_numbers() {
    assert!(is_excluded_relative_path(".git/config"));
    assert!(is_excluded_relative_path("node_modules/pkg/index.js"));
    assert!(is_excluded_relative_path("secrets/.env.production"));
    assert!(is_excluded_relative_path("keys/server.pem"));
    assert!(!is_excluded_relative_path("docs/architecture.md"));
    assert!(!is_excluded_relative_path("src/engine.rs"));

    assert_eq!(
        classify_content_tier("docs/scan.pdf", 4096, false),
        ContentSupportTier::Tier1PathOnly
    );
    assert_eq!(
        classify_content_tier("media/clip.mp4", 102400, false),
        ContentSupportTier::Tier1PathOnly
    );
    assert_eq!(
        classify_content_tier("notes/readme.md", 2048, false),
        ContentSupportTier::Tier2PlainText
    );
    assert_eq!(
        classify_content_tier("src/lib.rs", 2048, false),
        ContentSupportTier::Tier2SourceCode
    );

    let mut long_code = String::new();
    for i in 1..=80 {
        if i == 10 {
            long_code.push_str("pub fn reconnect_database_with_backoff() {\n");
        } else {
            long_code.push_str(&format!(
                "    let step_{} = \"exponential_backoff_retry_policy_line_{}\";\n",
                i, i
            ));
        }
    }
    let chunks = chunk_text_content(&long_code, true, 600, 120);
    assert!(chunks.len() >= 4, "Expected multiple overlapping chunks");
    assert_eq!(chunks[0].line_start, 1);
    assert!(chunks[0].line_end >= 8);
    assert_eq!(chunks[0].definition_strength, 2);
    assert!(chunks[1].line_start <= chunks[0].line_end);
}

#[test]
fn test_persistent_index_v2_save_reload_corruption_and_fingerprint_rejection() {
    let tmp = TempDir::new().expect("tempdir");
    let ttvec = tmp.path().join("scope_a.testdbl.ttvec");
    let ttidx = tmp.path().join("scope_a.testdbl.ttidx");

    let provider = DeterministicEmbeddingProvider::ready();
    let v_path = provider
        .encode_document(None, "notes/finance.md")
        .expect("encode path");
    let v_body = provider
        .encode_document(
            Some("notes/finance.md"),
            "Quarterly tax deduction and invoice ledger",
        )
        .expect("encode body");

    let mut index = PersistentScopeIndex::new("scope_a", 0xABCD_1234);
    index.model_fingerprint = provider.model_fingerprint();
    index
        .upsert_file_segments(
            ttzip_semantic::index::FileIndexInput {
                relative_path: "notes/finance.md".to_string(),
                archive_entry_path: None,
                mtime_ns: 1_700_000_000,
                file_size: 128,
                content_blake3: [7u8; 32],
                tier: ContentSupportTier::Tier2PlainText,
            },
            vec![
                (true, 0, 0, 0, "notes/finance.md".to_string(), v_path),
                (
                    false,
                    1,
                    12,
                    0,
                    "Quarterly tax deduction and invoice ledger".to_string(),
                    v_body,
                ),
            ],
        )
        .expect("upsert segments");
    index.save_to_disk(&ttvec, &ttidx).expect("save to disk");

    // 1. Reload with matching TEST_DOUBLE_MODEL_FINGERPRINT -> Loaded
    let mut reloaded = PersistentScopeIndex::new("scope_a", 0xABCD_1234);
    let outcome = reloaded.load_from_disk(&ttvec, &ttidx, TEST_DOUBLE_MODEL_FINGERPRINT);
    assert_eq!(
        outcome,
        IndexLoadOutcome::Loaded {
            index_generation: index.index_generation,
            file_count: 1,
            segment_count: 2,
        }
    );

    // 2. Production loader expecting PINNED_MODEL_FINGERPRINT MUST reject test-double index
    let mut prod_index = PersistentScopeIndex::new("scope_a", 0xABCD_1234);
    let prod_outcome = prod_index.load_from_disk(&ttvec, &ttidx, PINNED_MODEL_FINGERPRINT);
    assert!(
        matches!(prod_outcome, IndexLoadOutcome::NeedsRebuild { .. }),
        "Production index loader must reject TEST_DOUBLE_MODEL_FINGERPRINT"
    );

    // 3. Corrupt a byte in the vector payload -> BLAKE3 mismatch -> NeedsRebuild
    let mut raw_bytes = fs::read(&ttvec).expect("read .ttvec");
    let last_idx = raw_bytes.len() - 1;
    raw_bytes[last_idx] ^= 0xFF;
    fs::write(&ttvec, &raw_bytes).expect("write corrupted .ttvec");

    let mut corrupted = PersistentScopeIndex::new("scope_a", 0xABCD_1234);
    let corrupt_outcome = corrupted.load_from_disk(&ttvec, &ttidx, TEST_DOUBLE_MODEL_FINGERPRINT);
    assert!(matches!(
        corrupt_outcome,
        IndexLoadOutcome::NeedsRebuild { .. }
    ));
}

#[test]
fn test_control_flow_with_test_double_and_unseen_vocabulary_boundary() {
    let tmp = TempDir::new().expect("tempdir");
    let workspace_root = tmp.path().join("workspace");
    let index_dir = tmp.path().join("index_store");
    fs::create_dir_all(workspace_root.join("notes")).expect("mkdir notes");
    fs::create_dir_all(workspace_root.join("src")).expect("mkdir src");
    fs::create_dir_all(workspace_root.join(".git")).expect("mkdir .git");

    fs::write(
        workspace_root.join("notes/alpha_memo_901.txt"),
        "Line 1: Memo.\nLine 2: Submit 2026 corporate tax deduction receipts and vendor billing invoices.",
    )
    .expect("write alpha_memo_901.txt");
    fs::write(
        workspace_root.join("notes/zeta_record_402.md"),
        "# 运维记录\n第2行：生产环境数据库连接池出现超时，已启用指数退避重试机制。",
    )
    .expect("write zeta_record_402.md");
    fs::write(
        workspace_root.join("src/unit_77.rs"),
        "pub fn establish_pool_session() {\n    // postgres database connection timeout and socket retry backoff\n}\n",
    )
    .expect("write unit_77.rs");
    fs::write(
        workspace_root.join(".git/config"),
        "tax invoice database connection secret",
    )
    .expect("write .git/config");

    let coordinator =
        SemanticSearchCoordinator::new_for_testing_with_test_double(index_dir.clone());
    let scope = SemanticScopeDescriptor {
        scope_id: "dir_workspace".to_string(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: workspace_root.to_string_lossy().into_owned(),
        display_name: "workspace".to_string(),
        is_authorized: true,
    };
    coordinator.register_scope(scope);
    let progress = coordinator
        .index_directory_scope("dir_workspace")
        .expect("index directory");
    assert_eq!(progress.phase, SemanticIngestPhase::Completed);
    assert_eq!(progress.processed_files, 3, ".git/config must be excluded");
    assert!(
        index_dir.join("dir_workspace.testdbl.ttvec").exists(),
        "Test double must write .testdbl.ttvec and never .ttvec"
    );
    assert!(!index_dir.join("dir_workspace.ttvec").exists());

    // Control-flow check: snippet extraction and line range routing
    let res_en = coordinator.search(&SemanticSearchRequest {
        request_id: 10,
        window_id: "win_main".to_string(),
        query: "corporate fiscal billing receipt".to_string(),
        scope_ids: vec!["dir_workspace".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(!res_en.is_cancelled);
    assert_eq!(res_en.hits[0].display_path, "notes/alpha_memo_901.txt");
    assert_eq!(
        res_en.hits[0].model_fingerprint,
        TEST_DOUBLE_MODEL_FINGERPRINT
    );

    // Unseen vocabulary boundary: tokens outside CONCEPT_CLUSTERS only match when token hashes overlap
    let provider = DeterministicEmbeddingProvider::ready();
    let unseen_q = provider
        .encode_query("astronomical spectroscopy ephemeris", false)
        .expect("encode unseen query");
    let unseen_exact_doc = provider
        .encode_document(
            Some("notes/astro.txt"),
            "astronomical spectroscopy ephemeris table",
        )
        .expect("encode unseen doc");
    let dot_exact: i32 = unseen_q
        .iter()
        .zip(unseen_exact_doc.iter())
        .map(|(&a, &b)| (a as i32) * (b as i32))
        .sum();
    assert!(
        dot_exact > 5000,
        "Unseen tokens with shared lexical stems still hash consistently in test double"
    );

    // Incremental deletion + archive open request + request_id superseding
    fs::remove_file(workspace_root.join("notes/alpha_memo_901.txt")).expect("remove file");
    coordinator
        .index_directory_scope("dir_workspace")
        .expect("re-index after delete");
    let res_after_del = coordinator.search(&SemanticSearchRequest {
        request_id: 13,
        window_id: "win_main".to_string(),
        query: "corporate fiscal billing receipt".to_string(),
        scope_ids: vec!["dir_workspace".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(res_after_del
        .hits
        .iter()
        .all(|h| h.display_path != "notes/alpha_memo_901.txt"));

    let archive_scope = SemanticScopeDescriptor {
        scope_id: "archive_bundle".to_string(),
        kind: SemanticScopeKind::OpenArchive,
        root_path: "/Volumes/Data/release_bundle.zip".to_string(),
        display_name: "release_bundle.zip".to_string(),
        is_authorized: true,
    };
    coordinator.register_scope(archive_scope);
    coordinator
        .apply_file_events(
            "archive_bundle",
            &[SemanticFileEvent::CreatedOrModified {
                relative_path: "internal/docs/audit_2026.txt".to_string(),
                archive_entry_path: Some("internal/docs/audit_2026.txt".to_string()),
                bytes: b"Annual tax invoice audit summary for 2026.".to_vec(),
                mtime_ns: 100,
            }],
        )
        .expect("index archive entry");
    let res_arch = coordinator.search(&SemanticSearchRequest {
        request_id: 14,
        window_id: "win_main".to_string(),
        query: "tax invoice".to_string(),
        scope_ids: vec!["archive_bundle".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    let open_req = coordinator.build_open_request(&res_arch.hits[0]);
    assert_eq!(
        open_req.archive_entry_path.as_deref(),
        Some("internal/docs/audit_2026.txt")
    );
}

#[cfg(unix)]
#[test]
fn test_partial_directory_read_error_with_non_empty_root_preserves_old_index_and_fails() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let workspace_root = temp_dir.path().join("workspace_partial_err");
    let subdir = workspace_root.join("sub");
    let index_dir = temp_dir.path().join("index_partial_err");
    fs::create_dir_all(&subdir).expect("mkdir subdir");
    fs::create_dir_all(&index_dir).expect("mkdir index");

    fs::write(workspace_root.join("keep.md"), "root_keep_token_101").expect("write keep.md");
    fs::write(subdir.join("nested.md"), "nested_preserve_token_202").expect("write nested.md");

    let coordinator = SemanticSearchCoordinator::new_for_testing_with_test_double(index_dir);
    coordinator.register_scope(SemanticScopeDescriptor {
        scope_id: "dir:partial".to_string(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: workspace_root.to_string_lossy().into_owned(),
        display_name: "workspace_partial".to_string(),
        is_authorized: true,
    });

    let initial = coordinator
        .index_directory_scope("dir:partial")
        .expect("initial full scan must succeed");
    assert_eq!(initial.phase, SemanticIngestPhase::Completed);

    fs::write(
        workspace_root.join("uncommitted_new.md"),
        "partial_new_token_303",
    )
    .expect("write uncommitted_new.md");
    fs::set_permissions(&subdir, fs::Permissions::from_mode(0o000)).expect("chmod 000 subdir");

    let rescan_err = coordinator
        .index_directory_scope("dir:partial")
        .expect_err("unreadable subdirectory must fail scan even when root has readable files");
    fs::set_permissions(&subdir, fs::Permissions::from_mode(0o755)).expect("restore chmod 755");
    assert!(rescan_err.to_string().contains("Failed to read"));

    let failed_prog = coordinator
        .scope_progress("dir:partial")
        .expect("scope progress");
    assert_eq!(failed_prog.phase, SemanticIngestPhase::Failed);
    assert_eq!(failed_prog.indexed_segments, initial.indexed_segments);

    let nested_hits = coordinator.search(&SemanticSearchRequest {
        request_id: 10,
        window_id: "win".to_string(),
        query: "nested_preserve_token_202".to_string(),
        scope_ids: vec!["dir:partial".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(
        nested_hits
            .hits
            .iter()
            .any(|h| h.display_path == "sub/nested.md"),
        "Existing indexed entries inside unreadable subdirectory must not be deleted"
    );

    let uncommitted_hits = coordinator.search(&SemanticSearchRequest {
        request_id: 11,
        window_id: "win".to_string(),
        query: "partial_new_token_303".to_string(),
        scope_ids: vec!["dir:partial".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(
        uncommitted_hits
            .hits
            .iter()
            .all(|h| h.display_path != "uncommitted_new.md"),
        "Partial scan must never commit newly discovered root files when a subdirectory fails"
    );
}

#[test]
fn test_pure_delete_events_never_prepare_model_and_remove_indexed_file() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use ttzip_semantic::{
        DeterministicEmbeddingProvider, EmbeddingProvider, SemanticModelError, SemanticModelState,
    };

    struct CountingPrepareProvider {
        inner: DeterministicEmbeddingProvider,
        prepare_calls: AtomicUsize,
    }
    impl EmbeddingProvider for CountingPrepareProvider {
        fn model_state(&self) -> SemanticModelState {
            self.inner.model_state()
        }
        fn model_fingerprint(&self) -> u64 {
            self.inner.model_fingerprint()
        }
        fn prepare(&self) -> SemanticModelState {
            self.prepare_calls.fetch_add(1, Ordering::SeqCst);
            self.inner.prepare()
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
            self.inner.encode_document(relative_path, chunk_text)
        }
    }

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let provider = Arc::new(CountingPrepareProvider {
        inner: DeterministicEmbeddingProvider::ready(),
        prepare_calls: AtomicUsize::new(0),
    });
    let coordinator = SemanticSearchCoordinator::new(
        Arc::clone(&provider) as Arc<dyn EmbeddingProvider>,
        temp_dir.path().to_path_buf(),
    );
    coordinator.register_scope(SemanticScopeDescriptor {
        scope_id: "dir:pure_del".to_string(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: temp_dir.path().to_string_lossy().into_owned(),
        display_name: "pure_del".to_string(),
        is_authorized: true,
    });

    coordinator
        .apply_file_events(
            "dir:pure_del",
            &[
                SemanticFileEvent::CreatedOrModified {
                    relative_path: "remove_me.md".to_string(),
                    archive_entry_path: None,
                    bytes: b"delete_target_token_501".to_vec(),
                    mtime_ns: 1,
                },
                SemanticFileEvent::CreatedOrModified {
                    relative_path: "keep_me.md".to_string(),
                    archive_entry_path: None,
                    bytes: b"keep_target_token_502".to_vec(),
                    mtime_ns: 1,
                },
            ],
        )
        .expect("seed index");
    assert_eq!(provider.prepare_calls.load(Ordering::SeqCst), 1);

    provider.prepare_calls.store(0, Ordering::SeqCst);
    let del_prog = coordinator
        .apply_file_events(
            "dir:pure_del",
            &[SemanticFileEvent::Removed {
                relative_path: "remove_me.md".to_string(),
            }],
        )
        .expect("pure delete must succeed without preparing model");
    assert_eq!(del_prog.phase, SemanticIngestPhase::Completed);
    assert_eq!(
        provider.prepare_calls.load(Ordering::SeqCst),
        0,
        "Pure deletion event batch must never call provider.prepare()"
    );

    let removed_hits = coordinator.search(&SemanticSearchRequest {
        request_id: 20,
        window_id: "win".to_string(),
        query: "delete_target_token_501".to_string(),
        scope_ids: vec!["dir:pure_del".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(
        removed_hits
            .hits
            .iter()
            .all(|h| h.display_path != "remove_me.md"),
        "Deleted file must be removed from search index without model preparation"
    );
}

#[test]
fn test_post_scan_body_read_failure_discards_consumed_work_and_preserves_old_index() {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use ttzip_semantic::{
        DeterministicEmbeddingProvider, EmbeddingProvider, SemanticModelError, SemanticModelState,
    };

    struct InvalidateSecondFileDuringFirstEmbedProvider {
        inner: DeterministicEmbeddingProvider,
        arm_invalidation: AtomicBool,
        first_file_encode_calls: AtomicUsize,
        second_file_path: PathBuf,
    }
    impl EmbeddingProvider for InvalidateSecondFileDuringFirstEmbedProvider {
        fn model_state(&self) -> SemanticModelState {
            self.inner.model_state()
        }
        fn model_fingerprint(&self) -> u64 {
            self.inner.model_fingerprint()
        }
        fn prepare(&self) -> SemanticModelState {
            self.inner.prepare()
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
            if self.arm_invalidation.load(Ordering::SeqCst)
                && relative_path == Some("a_first.md")
            {
                self.first_file_encode_calls.fetch_add(1, Ordering::SeqCst);
                let _ = fs::remove_file(&self.second_file_path);
            }
            self.inner.encode_document(relative_path, chunk_text)
        }
    }

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let ws = temp_dir.path().join("ws_body_fail");
    let store = temp_dir.path().join("store_body_fail");
    fs::create_dir_all(&ws).expect("mkdir ws");
    fs::create_dir_all(&store).expect("mkdir store");

    let first_path = ws.join("a_first.md");
    let second_path = ws.join("z_second.md");
    fs::write(&first_path, "preserved_first_token_601").expect("write a_first");
    fs::write(&second_path, "preserved_second_token_602").expect("write z_second");

    let provider = Arc::new(InvalidateSecondFileDuringFirstEmbedProvider {
        inner: DeterministicEmbeddingProvider::ready(),
        arm_invalidation: AtomicBool::new(false),
        first_file_encode_calls: AtomicUsize::new(0),
        second_file_path: second_path.clone(),
    });
    let coordinator = SemanticSearchCoordinator::new(
        Arc::clone(&provider) as Arc<dyn EmbeddingProvider>,
        store,
    );
    coordinator.register_scope(SemanticScopeDescriptor {
        scope_id: "dir:body_fail".to_string(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: ws.to_string_lossy().into_owned(),
        display_name: "ws_body_fail".to_string(),
        is_authorized: true,
    });

    let initial = coordinator
        .index_directory_scope("dir:body_fail")
        .expect("initial scan must succeed");
    assert_eq!(initial.phase, SemanticIngestPhase::Completed);

    // Modify both files so neither is skipped by mtime+size freshness check;
    // when a_first.md is consumed and embedded, z_second.md is invalidated before its body read.
    fs::write(&first_path, "uncommitted_consumed_work_token_603_extra_len")
        .expect("modify a_first");
    fs::write(&second_path, "transient_second_body_token_604_extra_len")
        .expect("modify z_second");
    provider.arm_invalidation.store(true, Ordering::SeqCst);

    let err = coordinator
        .index_directory_scope("dir:body_fail")
        .expect_err("body read failure on z_second.md after a_first.md was consumed must fail scan");
    assert!(err.to_string().contains("Failed to read"));
    assert!(
        provider.first_file_encode_calls.load(Ordering::SeqCst) >= 1,
        "a_first.md must have been consumed and embedded into working_index before z_second.md failed"
    );

    let failed_prog = coordinator
        .scope_progress("dir:body_fail")
        .expect("progress after body fail");
    assert_eq!(failed_prog.phase, SemanticIngestPhase::Failed);
    assert_eq!(failed_prog.indexed_segments, initial.indexed_segments);

    let first_hits = coordinator.search(&SemanticSearchRequest {
        request_id: 30,
        window_id: "win".to_string(),
        query: "preserved_first_token_601".to_string(),
        scope_ids: vec!["dir:body_fail".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(
        first_hits
            .hits
            .iter()
            .any(|h| h.display_path == "a_first.md"
                && h.snippet
                    .as_deref()
                    .is_some_and(|s| s.contains("preserved_first_token_601"))),
        "Consumed work on a_first.md must be discarded and old a_first.md index preserved"
    );

    let second_hits = coordinator.search(&SemanticSearchRequest {
        request_id: 31,
        window_id: "win".to_string(),
        query: "preserved_second_token_602".to_string(),
        scope_ids: vec!["dir:body_fail".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(
        second_hits
            .hits
            .iter()
            .any(|h| h.display_path == "z_second.md"
                && h.snippet
                    .as_deref()
                    .is_some_and(|s| s.contains("preserved_second_token_602"))),
        "Old z_second.md index entry must remain intact when its rescan body read fails"
    );
}
