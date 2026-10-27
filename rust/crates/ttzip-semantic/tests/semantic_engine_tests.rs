// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unit and integration tests for `ttzip-semantic` model verification,
//! `b11461` C-ABI layout offsets, quantization math on the declared reference
//! vector fixture (`digup_reference_vectors_v1.json`), and real GGUF runtime
//! inference vs. standalone no-model honest lexical degradation.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use ttzip_semantic::{
    bundled_production_dylib_candidates, current_phys_footprint_bytes,
    expected_pinned_b11461_layout_snapshot, format_code_query_prompt, format_document_prompt,
    format_query_prompt, is_permitted_production_dylib_candidate, quantize_mrl128_int8,
    strip_code_query_prefix, validate_abi_layout_snapshot, validate_non_zero_int8_vector,
    verify_gguf_model_file, verify_pinned_b11461_abi_layout, verify_reference_vector_parity,
    AbiHeaderStatus, CheckpointStatus, DeterministicEmbeddingProvider, DigUpReferenceFile,
    EmbeddingProvider, LlamaDylibLoadPolicy, LlamaRuntimeConfig, LlamaRuntimeEngine,
    ReferenceVectorEntry, SemanticIngestPhase, SemanticMatchKind, SemanticModelError,
    SemanticModelState, SemanticScopeDescriptor, SemanticScopeKind, SemanticSearchCoordinator,
    SemanticSearchRequest, PINNED_GGUF_SHA256, PINNED_GGUF_SIZE_BYTES, PINNED_MODEL_FINGERPRINT,
    TEST_DOUBLE_MODEL_FINGERPRINT, TT_EMBEDDING_MAGIC, TT_EMBEDDING_SCHEMA_VERSION_2,
    TT_SECTION_TYPE_EMBEDDINGS_V2, TT_SECTION_TYPE_SUBTREE_END_IDS,
};

fn load_declared_reference_fixture() -> Vec<ReferenceVectorEntry> {
    let candidates = [
        PathBuf::from(
            "products/ttzip/core/rust/crates/ttzip-semantic/tests/fixtures/digup_reference_vectors_v1.json",
        ),
        PathBuf::from("tests/fixtures/digup_reference_vectors_v1.json"),
    ];
    let fixture_path = candidates.iter().find(|p| p.exists()).unwrap_or_else(|| {
        panic!(
            "Declared Bazel test fixture digup_reference_vectors_v1.json is missing; \
                 synthetic fallback is forbidden"
        )
    });
    let raw = fs::read_to_string(fixture_path).expect("read digup_reference_vectors_v1.json");
    let parsed: DigUpReferenceFile =
        serde_json::from_str(&raw).expect("parse digup_reference_vectors_v1.json");
    assert_eq!(parsed.made_with, "llama.cpp b11461");
    assert_eq!(parsed.vector_version, 1);
    let entries = parsed.into_entries();
    assert!(
        entries.len() >= 5,
        "Expected at least 5 reference vectors in declared fixture"
    );
    entries
}

/// Resolves the test runtime configuration without reading user home directory
/// data when running in the default standalone no-model lane (`TTZIP_REQUIRE_REAL_GGUF != "1"`).
/// When `TTZIP_REQUIRE_REAL_GGUF == "1"`, resolves `TTZIP_REAL_GGUF_MODEL_PATH` if
/// explicitly provided, or falls back to `LlamaRuntimeConfig::default_for_storage_dir`.
fn resolve_test_gguf_config(sandbox_storage_dir: &std::path::Path) -> LlamaRuntimeConfig {
    if std::env::var("TTZIP_REQUIRE_REAL_GGUF").as_deref() == Ok("1") {
        if let Ok(explicit_path) = std::env::var("TTZIP_REAL_GGUF_MODEL_PATH") {
            return LlamaRuntimeConfig {
                model_path: PathBuf::from(explicit_path),
                ..LlamaRuntimeConfig::default()
            };
        }
        return LlamaRuntimeConfig::default_for_storage_dir(sandbox_storage_dir);
    }
    LlamaRuntimeConfig {
        model_path: sandbox_storage_dir
            .join("models")
            .join("embeddinggemma-2-Q8_0.gguf"),
        ..LlamaRuntimeConfig::default()
    }
}

/// Returns `true` only when the test is running in the standalone no-model lane
/// (`TTZIP_REQUIRE_REAL_GGUF != "1"`) AND `err` is strictly an absent external
/// model file (`ModelFileMissing`) or an absent unmounted `libllamacpp.dylib`
/// in standalone `cargo test`. Any corrupt model file, checksum mismatch, ABI
/// offset drift, or inference failure is NEVER treated as an environment block.
fn is_standalone_missing_external_runtime_or_model(err: &SemanticModelError) -> bool {
    if std::env::var("TTZIP_REQUIRE_REAL_GGUF").as_deref() == Ok("1") {
        return false;
    }
    match err {
        SemanticModelError::ModelFileMissing { .. } => true,
        SemanticModelError::RuntimeFailed { reason } => {
            reason == "bundled libllamacpp.dylib not found"
                || reason == "bundled libllamacpp.dylib not found or failed to dlopen"
                || reason.contains("dlopen failed for")
        }
        _ => false,
    }
}

#[test]
fn test_pinned_model_constants_and_fingerprint_isolation() {
    assert_eq!(PINNED_GGUF_SIZE_BYTES, 309_855_456);
    assert_eq!(
        PINNED_GGUF_SHA256,
        "2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135"
    );
    assert_eq!(TT_EMBEDDING_MAGIC, 0x5454_454D_4231_0000);
    assert_eq!(TT_EMBEDDING_SCHEMA_VERSION_2, 2);
    assert_eq!(TT_SECTION_TYPE_SUBTREE_END_IDS, 9);
    assert_eq!(TT_SECTION_TYPE_EMBEDDINGS_V2, 10);
    assert_ne!(
        PINNED_MODEL_FINGERPRINT, TEST_DOUBLE_MODEL_FINGERPRINT,
        "Production and test-double model fingerprints must never collide"
    );

    // DeterministicEmbeddingProvider must never impersonate PINNED_MODEL_FINGERPRINT
    let td_ready = DeterministicEmbeddingProvider::ready();
    assert_eq!(td_ready.model_fingerprint(), TEST_DOUBLE_MODEL_FINGERPRINT);
    let td_attempted_impersonation =
        DeterministicEmbeddingProvider::with_fingerprint(PINNED_MODEL_FINGERPRINT);
    assert_eq!(
        td_attempted_impersonation.model_fingerprint(),
        TEST_DOUBLE_MODEL_FINGERPRINT,
        "Test double must refuse to carry PINNED_MODEL_FINGERPRINT"
    );
}

#[test]
fn test_pinned_b11461_c_abi_layout_offsets_by_value_roundtrip_and_bundled_dylib_policy() {
    // 1. Check compiled C bridge status (Verified when upstream llama.h is mounted by H; UnboundToUpstreamHeader before H mounts it)
    let status = verify_pinned_b11461_abi_layout()
        .expect("compiled C bridge ABI layout check must not return an offset mismatch");
    if let AbiHeaderStatus::Verified(snap) = status {
        assert_eq!(*snap, expected_pinned_b11461_layout_snapshot());
    }

    // 2. Verify pinned b11461 layout contract and offset drift rejection
    let expected = expected_pinned_b11461_layout_snapshot();
    assert_eq!(
        validate_abi_layout_snapshot(&expected),
        Ok(AbiHeaderStatus::Verified(Box::new(expected)))
    );
    assert_eq!(expected.batch_size, 56);
    assert_eq!(expected.batch_align, 8);
    assert_eq!(expected.batch_offset_n_tokens, 0);
    assert_eq!(expected.batch_offset_token, 8);
    assert_eq!(expected.batch_offset_logits, 48);

    assert_eq!(expected.model_params_size, 80);
    assert_eq!(expected.model_params_align, 8);
    assert_eq!(expected.model_params_offset_n_gpu_layers, 16);
    assert_eq!(expected.model_params_offset_load_mode, 24);
    assert_eq!(
        expected.model_params_offset_lazy_mode, 28,
        "lazy_mode must sit at offset 28 between load_mode (24) and main_gpu (32)"
    );
    assert_eq!(expected.model_params_offset_main_gpu, 32);
    assert_eq!(expected.model_params_offset_tensor_split, 40);
    assert_eq!(expected.model_params_offset_kv_overrides, 64);
    assert_eq!(expected.model_params_offset_vocab_only, 72);
    assert_eq!(expected.model_params_offset_load_mtp, 77);

    assert_eq!(expected.context_params_size, 160);
    assert_eq!(expected.context_params_align, 8);
    assert_eq!(expected.context_params_offset_n_outputs_max, 20);
    assert_eq!(
        expected.context_params_offset_n_outputs_max_per_seq, 24,
        "n_outputs_max_per_seq must sit at offset 24 between n_outputs_max (20) and n_threads (28)"
    );
    assert_eq!(expected.context_params_offset_n_threads, 28);
    assert_eq!(expected.context_params_offset_pooling_type, 44);
    assert_eq!(expected.context_params_offset_attention_type, 48);
    assert_eq!(expected.context_params_offset_embeddings, 128);
    assert_eq!(expected.context_params_offset_kv_unified, 133);
    assert_eq!(expected.context_params_offset_samplers, 136);
    assert_eq!(expected.context_params_offset_n_samplers, 144);
    assert_eq!(expected.context_params_offset_ctx_other, 152);

    let mut drifted = expected;
    drifted.model_params_offset_main_gpu = 28; // simulated missing lazy_mode drift
    assert!(
        validate_abi_layout_snapshot(&drifted).is_err(),
        "Validator must reject any upstream struct offset drift"
    );

    // 3. Verify production dylib loading policy strictly permits only bundled paths
    assert_eq!(
        LlamaRuntimeConfig::default().dylib_policy,
        LlamaDylibLoadPolicy::BundledOnly
    );
    for allowed in bundled_production_dylib_candidates() {
        assert!(is_permitted_production_dylib_candidate(allowed));
    }
    assert!(
        !is_permitted_production_dylib_candidate("/opt/homebrew/lib/libllama.dylib"),
        "Homebrew libllama.dylib must never be in the production fallback path"
    );
    assert!(
        !is_permitted_production_dylib_candidate("/usr/local/lib/libllama.dylib"),
        "/usr/local libllama.dylib must never be in the production fallback path"
    );
    assert!(
        !is_permitted_production_dylib_candidate("libllama.dylib"),
        "Bare libllama.dylib must never be in the production fallback path"
    );
    assert!(
        !is_permitted_production_dylib_candidate("libllamacpp.dylib"),
        "Bare libllamacpp.dylib must never be in the production fallback path"
    );
}

#[test]
fn test_canonical_prompt_templates_and_utf8_safe_code_prefix_stripping() {
    assert_eq!(
        format_query_prompt("tax document"),
        "task: search result | query: tax document"
    );
    assert_eq!(
        format_code_query_prompt("retry backoff socket"),
        "task: code retrieval | query: retry backoff socket"
    );
    assert_eq!(
        format_document_prompt(None, "invoice_2026.pdf"),
        "title: none | text: invoice_2026.pdf"
    );
    assert_eq!(
        format_document_prompt(
            Some("src/net/client.rs"),
            "pub fn connect_with_backoff() {}"
        ),
        "title: src/net/client.rs | text: pub fn connect_with_backoff() {}"
    );

    let (is_code_1, clean_1) = strip_code_query_prefix("code: parse central directory");
    assert!(is_code_1);
    assert_eq!(clean_1, "parse central directory");

    // Multi-byte UTF-8 Chinese query where byte index 5 falls inside a 3-byte character
    let (is_code_zh, clean_zh) = strip_code_query_prefix("数据库连接超时重试");
    assert!(!is_code_zh);
    assert_eq!(clean_zh, "数据库连接超时重试");
}

#[test]
fn test_zero_and_non_finite_vector_rejection() {
    let all_zero_f32 = vec![0.0f32; 768];
    assert_eq!(
        quantize_mrl128_int8(&all_zero_f32),
        Err(SemanticModelError::ZeroOrNonFiniteVector)
    );

    let mut nan_f32 = vec![0.1f32; 768];
    nan_f32[12] = f32::NAN;
    assert_eq!(
        quantize_mrl128_int8(&nan_f32),
        Err(SemanticModelError::ZeroOrNonFiniteVector)
    );

    let zero_i8 = [0i8; 128];
    assert_eq!(
        validate_non_zero_int8_vector(&zero_i8),
        Err(SemanticModelError::ZeroOrNonFiniteVector)
    );
}

#[test]
fn test_mrl128_int8_quantization_math_on_declared_reference_fixture() {
    let entries = load_declared_reference_fixture();
    let report = verify_reference_vector_parity(&entries).expect("parity report");
    assert_eq!(report.total_checked, entries.len());
    assert!(
        report.min_mrl128_int8_cosine >= 0.999,
        "MRL-128 INT8 quantization must preserve cosine >= 0.999 against FP32 MRL-128, got {}",
        report.min_mrl128_int8_cosine
    );
    assert!(
        report.query_doc_long_cosine_int8 > report.query_photo_cosine_int8,
        "query must have higher MRL-128 INT8 similarity with document-long ({}) than photo-140 ({})",
        report.query_doc_long_cosine_int8,
        report.query_photo_cosine_int8
    );
}

#[test]
fn test_real_gguf_model_output_parity_checkpoint() {
    let entries = load_declared_reference_fixture();
    let tmp = TempDir::new().expect("tempdir");
    let missing_path = tmp.path().join("embeddinggemma-2-Q8_0.gguf");

    // 1. When GGUF file is absent, checkpoint MUST report BLOCKED with 0 dims and None cosine
    let missing_engine = LlamaRuntimeEngine::new(LlamaRuntimeConfig {
        model_path: missing_path,
        prefer_metal: true,
        ..LlamaRuntimeConfig::default()
    });
    let blocked_report = missing_engine.run_real_model_checkpoint_with_references(&entries);
    assert_eq!(
        blocked_report.status,
        CheckpointStatus::Blocked,
        "Missing GGUF model must report BLOCKED, never a fake PASS"
    );
    assert_eq!(blocked_report.output_dimension, 0);
    assert_eq!(blocked_report.min_reference_cosine, None);
    assert_eq!(missing_engine.state(), SemanticModelState::NotInstalled);

    // 2. Corrupt or truncated GGUF files must be rejected with explicit errors
    let corrupt_path = tmp.path().join("corrupt.gguf");
    fs::write(&corrupt_path, b"GGUF_truncated").expect("write corrupt gguf");
    let err = verify_gguf_model_file(&corrupt_path, PINNED_GGUF_SIZE_BYTES, PINNED_GGUF_SHA256)
        .expect_err("must reject wrong size");
    assert!(matches!(err, SemanticModelError::SizeMismatch { .. }));
    assert!(!is_standalone_missing_external_runtime_or_model(&err));
    let corrupt_engine_err = LlamaRuntimeEngine::new(LlamaRuntimeConfig {
        model_path: corrupt_path,
        ..LlamaRuntimeConfig::default()
    })
    .verify_and_prepare()
    .expect_err("corrupt model must fail verify_and_prepare");
    assert!(!is_standalone_missing_external_runtime_or_model(
        &corrupt_engine_err
    ));

    let bad_magic_path = tmp.path().join("bad_magic.gguf");
    fs::write(&bad_magic_path, b"NOT_GGUF").expect("write bad magic");
    let err_magic = verify_gguf_model_file(&bad_magic_path, 8, PINNED_GGUF_SHA256)
        .expect_err("must reject invalid magic");
    assert!(matches!(
        err_magic,
        SemanticModelError::InvalidGgufMagic { .. }
    ));
    assert!(!is_standalone_missing_external_runtime_or_model(&err_magic));

    // 3. Real GGUF model loading, 768-D vector generation, and reference vector cosine parity (>= 0.999) on Metal & CPU
    let default_cfg = resolve_test_gguf_config(tmp.path());
    let probe_engine = LlamaRuntimeEngine::new(default_cfg.clone());
    if let Err(ref prep_err) = probe_engine.verify_and_prepare() {
        if is_standalone_missing_external_runtime_or_model(prep_err) {
            let report = probe_engine.run_real_model_checkpoint_with_references(&entries);
            println!(
                "[STANDALONE_ENV_DEGRADED_TIER] status={:?} reason={}",
                report.status, report.reason
            );
            assert_eq!(report.status, CheckpointStatus::Blocked);
            return;
        }
        panic!(
            "Real GGUF model verification or runtime preparation failed (must not silently degrade): {prep_err}"
        );
    }
    assert!(
        default_cfg.model_path.exists(),
        "Expected real GGUF model at {:?}",
        default_cfg.model_path
    );
    for prefer_metal in [true, false] {
        let engine = LlamaRuntimeEngine::new(LlamaRuntimeConfig {
            model_path: default_cfg.model_path.clone(),
            prefer_metal,
            ..LlamaRuntimeConfig::default()
        });
        let report = engine.run_real_model_checkpoint_with_references(&entries);
        println!(
            "[REAL_GGUF_CHECKPOINT prefer_metal={prefer_metal}] status={:?} backend={:?} device={:?} load_ms={:.2} dims={} refs={} min_cos={:?}",
            report.status,
            report.backend,
            report.verified_backend_device,
            report.load_time_ms,
            report.output_dimension,
            report.compared_reference_count,
            report.min_reference_cosine
        );
        assert_eq!(
            report.status,
            CheckpointStatus::Pass,
            "Real GGUF inference failed (prefer_metal={prefer_metal}): {}",
            report.reason
        );
        assert_eq!(report.output_dimension, 768);
        assert_eq!(report.compared_reference_count, 5);
        assert!(
            report.min_reference_cosine.unwrap_or(0.0) >= 0.999,
            "Real GGUF cosine parity fell below 0.999: {:?}",
            report.min_reference_cosine
        );
    }
}

#[test]
fn test_production_coordinator_never_uses_test_double_and_degrades_honestly() {
    let tmp = TempDir::new().expect("tempdir");
    let workspace_root = tmp.path().join("workspace");
    let index_dir = tmp.path().join("index_store");
    fs::create_dir_all(workspace_root.join("notes")).expect("mkdir notes");
    fs::create_dir_all(workspace_root.join("src")).expect("mkdir src");
    fs::create_dir_all(workspace_root.join(".git")).expect("mkdir .git");
    fs::create_dir_all(&index_dir).expect("mkdir index_store");

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

    // Part A: Explicit missing-model coordinator degrades honestly to lexical-only and never uses test double
    let missing_provider = std::sync::Arc::new(LlamaRuntimeEngine::new(LlamaRuntimeConfig {
        model_path: tmp.path().join("nonexistent_embeddinggemma.gguf"),
        ..LlamaRuntimeConfig::default()
    }));
    let degraded_coordinator =
        SemanticSearchCoordinator::new(missing_provider, index_dir.join("degraded"));
    let scope = SemanticScopeDescriptor {
        scope_id: "prod_scope".to_string(),
        kind: SemanticScopeKind::AuthorizedDirectory,
        root_path: workspace_root.to_string_lossy().into_owned(),
        display_name: "workspace".to_string(),
        is_authorized: true,
    };
    degraded_coordinator.register_scope(scope.clone());
    assert_eq!(
        degraded_coordinator.model_state(),
        SemanticModelState::NotInstalled
    );
    let deg_progress = degraded_coordinator
        .index_directory_scope("prod_scope")
        .expect("index in degraded mode");
    assert_eq!(deg_progress.phase, SemanticIngestPhase::DegradedLexicalOnly);

    let lex_res = degraded_coordinator.search(&SemanticSearchRequest {
        request_id: 1,
        window_id: "win_1".to_string(),
        query: "alpha_memo_901".to_string(),
        scope_ids: vec!["prod_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(lex_res.degraded_lexical_only);
    assert_eq!(lex_res.hits.len(), 1);
    assert_eq!(lex_res.hits[0].match_kind, SemanticMatchKind::LexicalOnly);
    assert_eq!(lex_res.hits[0].semantic_score, 0.0);

    let sem_res = degraded_coordinator.search(&SemanticSearchRequest {
        request_id: 2,
        window_id: "win_1".to_string(),
        query: "税务报销收据".to_string(),
        scope_ids: vec!["prod_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(sem_res.degraded_lexical_only);
    assert!(
        sem_res.hits.is_empty(),
        "Production coordinator without GGUF must not fabricate semantic hits via test double"
    );

    // Part B: Real production coordinator with real GGUF model performs end-to-end English/Chinese/code semantic search
    let real_index_dir = index_dir.join("real_prod");
    fs::create_dir_all(&real_index_dir).expect("mkdir real_prod");
    let baseline_phys_bytes = current_phys_footprint_bytes();
    let real_provider = std::sync::Arc::new(LlamaRuntimeEngine::new(resolve_test_gguf_config(
        &real_index_dir,
    )));
    if let Err(ref prep_err) = real_provider.verify_and_prepare() {
        if is_standalone_missing_external_runtime_or_model(prep_err) {
            let prod_coordinator = SemanticSearchCoordinator::new(real_provider, real_index_dir);
            prod_coordinator.register_scope(scope);
            let prog = prod_coordinator
                .index_directory_scope("prod_scope")
                .expect("degraded index");
            println!("[STANDALONE_ENV_DEGRADED_TIER] phase={:?}", prog.phase);
            assert_eq!(prog.phase, SemanticIngestPhase::DegradedLexicalOnly);
            return;
        }
        panic!(
            "Real GGUF model verification or runtime preparation failed (must not silently degrade): {prep_err}"
        );
    }
    let after_load_phys_bytes = current_phys_footprint_bytes();
    let prod_coordinator =
        SemanticSearchCoordinator::new(real_provider.clone(), real_index_dir.clone());
    prod_coordinator.register_scope(scope);
    let real_progress = prod_coordinator
        .index_directory_scope("prod_scope")
        .expect("index with real GGUF model");
    let after_index_phys_bytes = current_phys_footprint_bytes();
    assert_eq!(real_progress.phase, SemanticIngestPhase::Completed);
    assert_eq!(
        real_progress.processed_files, 3,
        ".git/config must be excluded"
    );
    assert!(real_index_dir.join("prod_scope.ttvec").exists());
    assert!(!real_index_dir.join("prod_scope.testdbl.ttvec").exists());

    // 1. English synonym / zero-filename-overlap body retrieval
    let en_res = prod_coordinator.search(&SemanticSearchRequest {
        request_id: 10,
        window_id: "win_real".to_string(),
        query: "corporate fiscal billing receipt".to_string(),
        scope_ids: vec!["prod_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(!en_res.degraded_lexical_only);
    assert_eq!(en_res.hits[0].display_path, "notes/alpha_memo_901.txt");
    assert_eq!(
        en_res.hits[0].match_kind,
        SemanticMatchKind::ContentSemantic
    );
    assert_eq!(en_res.hits[0].model_fingerprint, PINNED_MODEL_FINGERPRINT);

    // 2. Chinese zero-filename-overlap body retrieval
    let zh_res = prod_coordinator.search(&SemanticSearchRequest {
        request_id: 11,
        window_id: "win_real".to_string(),
        query: "数据库连接超时退避重试".to_string(),
        scope_ids: vec!["prod_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert!(!zh_res.degraded_lexical_only);
    assert_eq!(zh_res.hits[0].display_path, "notes/zeta_record_402.md");
    assert_eq!(
        zh_res.hits[0].match_kind,
        SemanticMatchKind::ContentSemantic
    );
    assert_eq!(zh_res.hits[0].model_fingerprint, PINNED_MODEL_FINGERPRINT);

    // 3. Code retrieval ("code:" prefix) + Hybrid search ("alpha_memo_901 tax")
    let code_res = prod_coordinator.search(&SemanticSearchRequest {
        request_id: 12,
        window_id: "win_real".to_string(),
        query: "code: postgres socket retry backoff".to_string(),
        scope_ids: vec!["prod_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    assert_eq!(code_res.hits[0].display_path, "src/unit_77.rs");
    assert_eq!(code_res.hits[0].model_fingerprint, PINNED_MODEL_FINGERPRINT);

    let hyb_res = prod_coordinator.search(&SemanticSearchRequest {
        request_id: 13,
        window_id: "win_real".to_string(),
        query: "alpha_memo_901 tax deduction".to_string(),
        scope_ids: vec!["prod_scope".to_string()],
        top_k: 5,
        lexical_weight: 0.30,
        semantic_weight: 0.70,
    });
    let after_query_phys_bytes = current_phys_footprint_bytes();
    real_provider.unload();
    let after_unload_phys_bytes = current_phys_footprint_bytes();
    assert_eq!(hyb_res.hits[0].display_path, "notes/alpha_memo_901.txt");
    assert!(hyb_res.hits[0].lexical_score > 0.0);
    assert!(hyb_res.hits[0].semantic_score > 0.0);
    assert!(after_load_phys_bytes > 0 && after_unload_phys_bytes > 0);
    println!(
        "[REAL_GGUF_RETRIEVAL] en_top={} (sem={:.4}) zh_top={} (sem={:.4}) code_top={} (sem={:.4}) hyb_top={} (comb={:.4})",
        en_res.hits[0].display_path,
        en_res.hits[0].semantic_score,
        zh_res.hits[0].display_path,
        zh_res.hits[0].semantic_score,
        code_res.hits[0].display_path,
        code_res.hits[0].semantic_score,
        hyb_res.hits[0].display_path,
        hyb_res.hits[0].combined_score,
    );
    println!(
        "[REAL_GGUF_PHYS_FOOTPRINT_MIB] baseline={:.2} after_load={:.2} after_index={:.2} after_query={:.2} after_unload={:.2}",
        baseline_phys_bytes as f64 / (1024.0 * 1024.0),
        after_load_phys_bytes as f64 / (1024.0 * 1024.0),
        after_index_phys_bytes as f64 / (1024.0 * 1024.0),
        after_query_phys_bytes as f64 / (1024.0 * 1024.0),
        after_unload_phys_bytes as f64 / (1024.0 * 1024.0),
    );
}
