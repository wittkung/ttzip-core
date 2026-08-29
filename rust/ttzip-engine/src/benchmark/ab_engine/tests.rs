// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit test suite for declarative A/B benchmark target registry and driver adapters.

use std::sync::Arc;

use crate::benchmark::ab_engine::{
    AbBaselineSnapshot, AbEngineOrchestrator, AbOrchestratorConfig,
    AsciiTableReporter, JsonTelemetryReporter, MarkdownCommentReporter,
};
use crate::benchmark::ab_engine::target::{
    glob_match, BenchmarkTarget, CodecMode, CodecTargetAdapter, ContainerMode,
    ContainerTargetAdapter, CryptoMode, CryptoTargetAdapter, MetricUnit, TargetCategory,
    TargetRegistry,
};
use crate::benchmark::codecs_driver::{
    CodecBenchmarkDriver, DeflateBenchmarkDriver, ZstdBenchmarkDriver,
};
use crate::benchmark::container_driver::{
    ContainerBenchmarkDriver, TarContainerDriver, ZipContainerDriver,
};
use crate::benchmark::crypto_driver::{
    Blake3BenchmarkDriver, Crc32BenchmarkDriver, CryptoBenchmarkDriver,
    VaultAesGcmBenchmarkDriver,
};

#[test]
fn test_glob_match_patterns() {
    assert!(glob_match("*", "codec/zstd/compress/l3"));
    assert!(glob_match("**", "anything"));
    assert!(glob_match("codec/*", "codec/zstd/compress/l3"));
    assert!(glob_match("codec/zstd/*", "codec/zstd/compress/l3"));
    assert!(glob_match("codec/zstd/*", "codec/zstd/decompress/l1"));
    assert!(!glob_match("codec/zstd/*", "codec/deflate/compress/l3"));

    assert!(glob_match("crypto/*", "crypto/blake3/digest"));
    assert!(glob_match("crypto/*/digest", "crypto/crc32/digest"));
    assert!(!glob_match("crypto/*/digest", "crypto/blake3/verify"));

    assert!(glob_match("*zstd*", "codec/zstd_ldm/compress/l1"));
    assert!(glob_match("container/?ip/*", "container/zip/create"));
    assert!(!glob_match("container/?ip/*", "container/tar/create"));
}

#[test]
fn test_target_registry_multi_pattern_filtering() {
    let registry = TargetRegistry::default_full();

    // Single pattern
    let lzfse = registry.filter_targets("codec/lzfse/*");
    assert_eq!(lzfse.len(), 4); // 2 levels * 2 modes

    // Comma-separated multi-pattern
    let multi = registry.filter_targets("crypto/blake3/*,crypto/crc32/*");
    assert_eq!(multi.len(), 4); // 2 for blake3 + 2 for crc32

    // Multi-category pattern
    let composite = registry.filter_targets("codec/lzfse/*,crypto/vault_chacha20_poly1305/*");
    assert_eq!(composite.len(), 6); // 4 for lzfse + 2 for chacha
}

#[test]
fn test_codec_target_adapter_roundtrip() {
    let driver: Arc<dyn CodecBenchmarkDriver> = Arc::new(ZstdBenchmarkDriver);
    let compress_target = CodecTargetAdapter::new(Arc::clone(&driver), 3, CodecMode::Compress);
    let decompress_target = CodecTargetAdapter::new(driver, 3, CodecMode::Decompress);

    assert_eq!(compress_target.descriptor().category, TargetCategory::Codec);
    assert_eq!(compress_target.descriptor().unit, MetricUnit::BytesPerSec);
    assert_eq!(compress_target.descriptor().uri, "codec/zstd/compress/l3");

    let payload = b"The quick brown fox jumps over the lazy dog. 1234567890 repeating payload...";
    let comp_out = compress_target.execute_pass(payload).expect("compression pass");
    assert!(comp_out.output_bytes > 0);
    assert!(comp_out.duration_nanos > 0);
    assert!(comp_out.extra_metric.is_some());

    let decomp_out = decompress_target.execute_pass(payload).expect("decompression pass");
    assert_eq!(decomp_out.output_bytes, payload.len());
    assert!(decomp_out.duration_nanos > 0);
}

#[test]
fn test_crypto_target_adapter_digest_and_encryption() {
    // 1. Hash primitive (Blake3)
    let b3_driver: Arc<dyn CryptoBenchmarkDriver> = Arc::new(Blake3BenchmarkDriver);
    let b3_digest = CryptoTargetAdapter::new(Arc::clone(&b3_driver), CryptoMode::Process);
    let b3_verify = CryptoTargetAdapter::new(b3_driver, CryptoMode::VerifyOrDecrypt);

    assert_eq!(b3_digest.descriptor().uri, "crypto/blake3/digest");
    assert_eq!(b3_verify.descriptor().uri, "crypto/blake3/verify");

    let payload = b"Cryptographic verification test payload for A/B engine Layer 1.";
    let digest_out = b3_digest.execute_pass(payload).expect("digest pass");
    assert_eq!(digest_out.output_bytes, 32);
    assert!(digest_out.duration_nanos > 0);

    let verify_out = b3_verify.execute_pass(payload).expect("verify pass");
    assert_eq!(verify_out.output_bytes, payload.len());

    // 2. Authenticated Cipher primitive (Vault AES-GCM)
    let gcm_driver: Arc<dyn CryptoBenchmarkDriver> = Arc::new(VaultAesGcmBenchmarkDriver);
    let gcm_enc = CryptoTargetAdapter::new(Arc::clone(&gcm_driver), CryptoMode::Process);
    let gcm_dec = CryptoTargetAdapter::new(gcm_driver, CryptoMode::VerifyOrDecrypt);

    assert_eq!(gcm_enc.descriptor().uri, "crypto/vault_aes_gcm/encrypt");
    assert_eq!(gcm_dec.descriptor().uri, "crypto/vault_aes_gcm/decrypt");

    let enc_out = gcm_enc.execute_pass(payload).expect("encrypt pass");
    assert!(enc_out.output_bytes > payload.len());

    let dec_out = gcm_dec.execute_pass(payload).expect("decrypt pass");
    assert_eq!(dec_out.output_bytes, payload.len());
}

#[test]
fn test_container_target_adapter_create_and_extract() {
    let zip_driver: Arc<dyn ContainerBenchmarkDriver> = Arc::new(ZipContainerDriver);
    let zip_create = ContainerTargetAdapter::new(
        Arc::clone(&zip_driver),
        ContainerMode::Create,
        6,
        None,
        None,
    );
    let zip_extract = ContainerTargetAdapter::new(
        zip_driver,
        ContainerMode::Extract,
        6,
        None,
        None,
    );

    assert_eq!(zip_create.descriptor().uri, "container/zip/create");
    assert_eq!(zip_extract.descriptor().uri, "container/zip/extract");

    let payload = b"Container TAR/ZIP archive benchmark payload.";
    let create_out = zip_create.execute_pass(payload).expect("create zip pass");
    assert!(create_out.output_bytes > 0);
    assert!(create_out.duration_nanos > 0);

    let extract_out = zip_extract.execute_pass(payload).expect("extract zip pass");
    assert_eq!(extract_out.output_bytes, payload.len());
    assert_eq!(extract_out.extra_metric, Some(1.0)); // 1 entry extracted
}

#[test]
fn test_target_registry_registration_and_lookup() {
    let mut registry = TargetRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    let deflate = Arc::new(CodecTargetAdapter::new(
        Arc::new(DeflateBenchmarkDriver),
        6,
        CodecMode::Compress,
    ));
    let crc32 = Arc::new(CryptoTargetAdapter::new(
        Arc::new(Crc32BenchmarkDriver),
        CryptoMode::Process,
    ));
    let tar = Arc::new(ContainerTargetAdapter::new(
        Arc::new(TarContainerDriver),
        ContainerMode::Create,
        0,
        None,
        None,
    ));

    registry.register_target(Arc::clone(&deflate) as Arc<dyn BenchmarkTarget>);
    registry.register_target(Arc::clone(&crc32) as Arc<dyn BenchmarkTarget>);
    registry.register_target(Arc::clone(&tar) as Arc<dyn BenchmarkTarget>);

    assert_eq!(registry.len(), 3);

    let looked_up = registry.get_target("codec/deflate/compress/l6");
    assert!(looked_up.is_some());
    assert_eq!(looked_up.unwrap().descriptor().name, "Deflate L6 [compress]");

    assert!(registry.get_target("non_existent_uri").is_none());

    let codec_targets = registry.filter_targets("codec/*");
    assert_eq!(codec_targets.len(), 1);
    assert_eq!(codec_targets[0].descriptor().uri, "codec/deflate/compress/l6");

    let all_targets = registry.filter_targets("*");
    assert_eq!(all_targets.len(), 3);
}

#[test]
fn test_target_registry_default_full_loading() {
    let registry = TargetRegistry::default_full();
    assert!(!registry.is_empty());

    // Check Codec targets are present
    let zstd_targets = registry.filter_targets("codec/zstd/*");
    assert!(!zstd_targets.is_empty());

    let deflate_targets = registry.filter_targets("codec/deflate/*");
    assert!(!deflate_targets.is_empty());

    // Check Crypto targets are present
    let crypto_targets = registry.filter_targets("crypto/*");
    assert_eq!(crypto_targets.len(), 22); // 11 drivers * 2 modes (process + verify)

    // Check Container targets are present
    let container_targets = registry.filter_targets("container/*");
    assert_eq!(container_targets.len(), 16); // 8 drivers * 2 modes (create + extract)

    // Verify execution pass on a selected target from registry
    let zstd_l3 = registry
        .get_target("codec/zstd/compress/l3")
        .expect("codec/zstd/compress/l3 target exists");
    let payload = b"Testing default_full registry target execution.";
    let output = zstd_l3.execute_pass(payload).expect("execute pass on zstd_l3");
    assert!(output.output_bytes > 0);
    assert!(output.duration_nanos > 0);
}

#[test]
fn test_ab_orchestrator_config_default_and_custom() {
    let default_cfg = AbOrchestratorConfig::default();
    assert_eq!(default_cfg.warmup_rounds, 3);
    assert_eq!(default_cfg.measurement_rounds, 20);
    assert_eq!(default_cfg.max_allowed_regression, 3.0);
    assert_eq!(default_cfg.p_value_threshold, 0.05);
    assert!(default_cfg.hampel_filter);
    assert_eq!(default_cfg.hampel_k, 3.0);
    assert_eq!(default_cfg.target_rse_pct, 0.5);

    let custom_cfg = AbOrchestratorConfig {
        warmup_rounds: 1,
        measurement_rounds: 8,
        max_allowed_regression: 5.0,
        p_value_threshold: 0.01,
        hampel_filter: false,
        hampel_k: 2.5,
        target_rse_pct: 1.0,
    };
    assert_eq!(custom_cfg.warmup_rounds, 1);
    assert_eq!(custom_cfg.measurement_rounds, 8);
}

#[test]
fn test_ab_orchestrator_paired_target_interleaved_sampling() {
    let orchestrator = AbEngineOrchestrator::new();
    let zstd_l1 = CodecTargetAdapter::new(Arc::new(ZstdBenchmarkDriver), 1, CodecMode::Compress);
    let zstd_l3 = CodecTargetAdapter::new(Arc::new(ZstdBenchmarkDriver), 3, CodecMode::Compress);

    let payload = b"Interleaved A/B sampling verification with Zstd L1 vs L3 on synthetic payload.";
    let config = AbOrchestratorConfig {
        warmup_rounds: 2,
        measurement_rounds: 6,
        max_allowed_regression: 10.0,
        p_value_threshold: 0.05,
        hampel_filter: true,
        hampel_k: 3.0,
        target_rse_pct: 2.0,
    };

    let item = orchestrator
        .run_paired_target(&zstd_l1, &zstd_l3, payload, "synthetic:test", &config)
        .expect("run paired target");

    assert_eq!(item.descriptor.uri, "codec/zstd/compress/l3");
    assert_eq!(item.corpus_uri, "synthetic:test");
    assert_eq!(item.corpus_size_bytes, payload.len());
    assert!(item.stats_a.sample_count >= 6);
    assert!(item.stats_b.sample_count >= 6);
    assert!(item.throughput_a_mbs > 0.0);
    assert!(item.throughput_b_mbs > 0.0);
    assert!(item.speedup_ratio > 0.0);
}

#[test]
fn test_ab_orchestrator_run_ab_benchmark_suite() {
    let orchestrator = AbEngineOrchestrator::new();
    let config = AbOrchestratorConfig {
        warmup_rounds: 2,
        measurement_rounds: 6,
        max_allowed_regression: 5.0,
        p_value_threshold: 0.05,
        hampel_filter: true,
        hampel_k: 3.0,
        target_rse_pct: 2.0,
    };

    let report = orchestrator
        .run_ab_benchmark(
            "crypto/crc32/digest",
            "synthetic:zipf_text",
            65536,
            &config,
        )
        .expect("run ab benchmark on crc32");

    assert_eq!(report.total_targets, 1);
    assert_eq!(report.passed_targets, 1);
    assert!(report.overall_passed);
    assert_eq!(report.corpus_uri, "synthetic:zipf_text");
    assert_eq!(report.corpus_size_bytes, 65536);
    assert_eq!(report.items[0].descriptor.uri, "crypto/crc32/digest");
    assert!(report.items[0].throughput_a_mbs > 0.0);
    assert!(report.items[0].throughput_b_mbs > 0.0);
}

#[test]
fn test_ab_orchestrator_baseline_snapshot_roundtrip_and_comparison() {
    let orchestrator = AbEngineOrchestrator::new();
    let config = AbOrchestratorConfig {
        warmup_rounds: 2,
        measurement_rounds: 6,
        max_allowed_regression: 5.0,
        p_value_threshold: 0.05,
        hampel_filter: true,
        hampel_k: 3.0,
        target_rse_pct: 2.0,
    };

    let report = orchestrator
        .run_ab_benchmark("crypto/blake3/digest", "synthetic:dna", 65536, &config)
        .expect("initial benchmark");

    // 1. Snapshot creation and JSON roundtrip
    let snapshot = report.to_baseline_snapshot(false);
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot.get("crypto/blake3/digest").is_some());

    let snapshot_json = snapshot.to_json().expect("serialize snapshot");
    assert!(snapshot_json.contains("crypto/blake3/digest"));

    let loaded_snapshot = AbBaselineSnapshot::from_json(&snapshot_json).expect("deserialize snapshot");
    assert_eq!(loaded_snapshot.len(), 1);

    // 2. Comparison against loaded baseline
    let comp_report = orchestrator
        .run_ab_benchmark_against_baseline(
            "crypto/blake3/digest",
            "synthetic:dna",
            65536,
            &loaded_snapshot,
            &config,
        )
        .expect("benchmark against baseline");

    assert_eq!(comp_report.total_targets, 1);
    assert_eq!(comp_report.passed_targets, 1);
    assert!(comp_report.overall_passed);
}

#[test]
fn test_multimodal_reporters_render_outputs() {
    let orchestrator = AbEngineOrchestrator::new();
    let config = AbOrchestratorConfig {
        warmup_rounds: 2,
        measurement_rounds: 6,
        max_allowed_regression: 5.0,
        p_value_threshold: 0.05,
        hampel_filter: true,
        hampel_k: 3.0,
        target_rse_pct: 2.0,
    };

    let report = orchestrator
        .run_ab_benchmark("crypto/xxh3_64/digest", "synthetic:noise", 65536, &config)
        .expect("benchmark for reporters");

    // 1. ASCII Table Reporter
    let ascii_color = AsciiTableReporter::render(&report);
    assert!(ascii_color.contains("TTZip Declarative A/B Performance Suite"));
    assert!(ascii_color.contains("crypto/xxh3_64/digest"));
    assert!(ascii_color.contains("Quality Gate:"));

    let ascii_plain = AsciiTableReporter::render_plain(&report);
    assert!(ascii_plain.contains("TTZip Declarative A/B Performance Suite"));
    assert!(!ascii_plain.contains("\x1b["));

    // 2. JSON Telemetry Reporter
    let json_out = JsonTelemetryReporter::render(&report);
    assert!(json_out.contains("\"schema_version\": \"1.0.0\""));
    assert!(json_out.contains("\"target_filter\": \"crypto/xxh3_64/digest\""));
    assert!(json_out.contains("\"overall_passed\": true"));

    // 3. Markdown Comment Reporter
    let md_out = MarkdownCommentReporter::render(&report);
    assert!(md_out.contains("## 🚀 TTZip Declarative A/B Benchmark Report"));
    assert!(md_out.contains("`crypto/xxh3_64/digest`"));
    assert!(md_out.contains("<details>"));
    assert!(md_out.contains("Degrees of Freedom"));
}

