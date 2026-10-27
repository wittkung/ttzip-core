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
