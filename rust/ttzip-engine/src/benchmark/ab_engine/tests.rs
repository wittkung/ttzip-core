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
        max_allowed_regression: 5.0,
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
        warmup_rounds: 3,
        measurement_rounds: 10,
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
            1048576,
            &config,
        )
        .expect("run ab benchmark on crc32");

    assert_eq!(report.total_targets, 1);
    assert_eq!(report.passed_targets, 1);
    assert!(report.overall_passed);
    assert_eq!(report.corpus_uri, "synthetic:zipf_text");
    assert_eq!(report.corpus_size_bytes, 1048576);
    assert_eq!(report.items[0].descriptor.uri, "crypto/crc32/digest");
    assert!(report.items[0].throughput_a_mbs > 0.0);
    assert!(report.items[0].throughput_b_mbs > 0.0);
}

#[test]
fn test_ab_orchestrator_baseline_snapshot_roundtrip_and_comparison() {
    let orchestrator = AbEngineOrchestrator::new();
    let config = AbOrchestratorConfig {
        warmup_rounds: 4,
        measurement_rounds: 8,
        max_allowed_regression: 5.0,
        p_value_threshold: 0.05,
        hampel_filter: true,
        hampel_k: 3.0,
        target_rse_pct: 2.0,
    };

    let report = orchestrator
        .run_ab_benchmark("crypto/blake3/digest", "synthetic:dna", 262144, &config)
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
            262144,
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
        warmup_rounds: 4,
        measurement_rounds: 8,
        max_allowed_regression: 5.0,
        p_value_threshold: 0.05,
        hampel_filter: true,
        hampel_k: 3.0,
        target_rse_pct: 2.0,
    };

    let report = orchestrator
        .run_ab_benchmark("crypto/xxh3_64/digest", "synthetic:noise", 1048576, &config)
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
    assert!(json_out.contains("\"overall_passed\""));

    // 3. Markdown Comment Reporter
    let md_out = MarkdownCommentReporter::render(&report);
    assert!(md_out.contains("## 🚀 TTZip Declarative A/B Benchmark Report"));
    assert!(md_out.contains("`crypto/xxh3_64/digest`"));
    assert!(md_out.contains("<details>"));
    assert!(md_out.contains("Degrees of Freedom"));
}

#[test]
fn test_timing_wait_for_next_tick_and_stopwatch() {
    use crate::benchmark::ab_engine::timing::{
        estimate_clock_resolution_nanos, get_hardware_monotonic_nanos, time_aligned_closure,
        wait_for_next_tick, wait_for_next_tick_instant, HardwareMonotonicStopwatch,
    };
    use std::time::Duration;

    let t1 = get_hardware_monotonic_nanos();
    assert!(t1 > 0);

    let tick1 = wait_for_next_tick();
    let tick2 = wait_for_next_tick();
    assert!(tick2 > tick1);

    let inst1 = wait_for_next_tick_instant();
    let inst2 = wait_for_next_tick_instant();
    assert!(inst2 > inst1);

    let mut sw = HardwareMonotonicStopwatch::new();
    std::thread::sleep(Duration::from_millis(3));
    assert!(sw.elapsed_nanos() >= 2_000_000);
    assert!(sw.elapsed_millis() >= 2.0);

    let lap = sw.lap_nanos();
    assert!(lap >= 2_000_000);

    sw.reset_aligned();
    assert!(sw.elapsed_nanos() < lap);

    let (sum, elapsed) = time_aligned_closure(|| {
        (1..=50_000).fold(0u64, |acc, x| acc.wrapping_add(std::hint::black_box(x)))
    });
    assert_eq!(sum, 1250025000);
    assert!(elapsed > 0);

    let res = estimate_clock_resolution_nanos(16);
    assert!(res > 0.0);
}

#[test]
fn test_thermal_throttle_governor_70s_threshold() {
    use crate::benchmark::ab_engine::thermal::{
        ThermalThrottleGovernor, ACTIVE_PERIOD_MICROS, COOL_PERIOD_SECS, DEFAULT_COOL_PERIOD,
    };
    use std::time::Duration;

    assert_eq!(ACTIVE_PERIOD_MICROS, 70_000_000);
    assert_eq!(COOL_PERIOD_SECS, 10);
    assert_eq!(DEFAULT_COOL_PERIOD, Duration::from_secs(10));

    let mut gov = ThermalThrottleGovernor::new();
    assert_eq!(gov.accumulated_micros(), 0);
    assert_eq!(gov.remaining_active_micros(), 70_000_000);

    // Accumulate 50s (no cooling)
    let s1 = gov.record_active_micros(50_000_000);
    assert!(s1.is_none());
    assert_eq!(gov.accumulated_micros(), 50_000_000);
    assert_eq!(gov.remaining_active_micros(), 20_000_000);
    assert!(!gov.is_cooling_needed());

    // Accumulate another 25s (total 75s -> triggers 10s cooldown, remainder 5s)
    let s2 = gov.record_active_micros(25_000_000);
    assert_eq!(s2, Some(Duration::from_secs(10)));
    assert_eq!(gov.accumulated_micros(), 5_000_000);
    assert_eq!(gov.total_cooldowns_triggered(), 1);
    assert_eq!(gov.total_active_micros(), 75_000_000);

    // Test with scaled down thresholds (100us threshold)
    let mut mini_gov = ThermalThrottleGovernor::with_thresholds(100, Duration::from_millis(1));
    for _ in 0..5 {
        mini_gov.record_active_micros(30);
    }
    // 5 * 30 = 150us -> 1 cooldown triggered, remainder 50us
    assert_eq!(mini_gov.total_cooldowns_triggered(), 1);
    assert_eq!(mini_gov.accumulated_micros(), 50);
}

#[test]
fn test_timed_fn_benchmark_engine_adaptive_and_filtering() {
    use crate::benchmark::ab_engine::timed_fn::{
        TimedFnBenchmarkEngine, TimedFnConfig, DEFAULT_TARGET_DURATION,
    };
    use std::time::Duration;

    assert_eq!(DEFAULT_TARGET_DURATION, Duration::from_millis(1000));

    let engine = TimedFnBenchmarkEngine::new(TimedFnConfig {
        target_duration: Duration::from_millis(5),
        num_rounds: 5,
        probe_runs: 1,
        min_loops: 1,
        max_loops: 50_000,
        enable_hampel: true,
        hampel_k: 3.0,
        rising_edge_sync: true,
    });

    let mut checksum = 0u64;
    let res = engine.bench(|| {
        for i in 0..50 {
            checksum = checksum.wrapping_add(std::hint::black_box(i));
        }
        std::hint::black_box(checksum);
    });

    assert_eq!(res.num_rounds, 5);
    assert!(res.estimated_loops_per_round >= 1);
    assert!(res.best_round_duration_ns > 0);
    assert!(res.best_ns_per_iteration > 0.0);
    assert!(res.mean_ns_per_iteration > 0.0);
    assert!(res.median_ns_per_iteration > 0.0);
    assert!(res.best_ns_per_iteration <= res.mean_ns_per_iteration * 1.5);
    assert_eq!(res.round_durations_ns.len(), 5);
    assert!(res.clean_per_iteration_ns.len() <= 5);

    let tp = res.throughput_mbs_from_best(1024 * 1024);
    assert!(tp > 0.0);
    let cpb = res.calc_cpb_from_best(1024, 3.2);
    assert!(cpb > 0.0);
}

#[test]
fn test_lz4_timeloop_engine_integration_and_best_of_six() {
    use crate::benchmark::ab_engine::timeloop::{
        Lz4TimeLoopBenchEngine, TimeLoopConfig, NB_TESTS, TIMELOOP_MICROS,
    };

    assert_eq!(TIMELOOP_MICROS, 1_900_000);
    assert_eq!(NB_TESTS, 6);

    let config = TimeLoopConfig::new(3_000, 6).with_warmup(1);
    let engine = Lz4TimeLoopBenchEngine::with_config(config);
    let payload = vec![0x5Au8; 32 * 1024]; // 32 KB

    let mut state = 123456789u64;
    let stats = engine.benchmark_timeloop(&payload, 6, || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        std::hint::black_box(state);
    });

    assert_eq!(stats.runs, 6);
    assert_eq!(stats.passes.len(), 6);
    assert!(stats.best_throughput_mbs > 0.0);
    assert!(stats.best_pass_index >= 1 && stats.best_pass_index <= 6);
    assert!(stats.mean_throughput_mbs > 0.0);
    assert!(stats.median_throughput_mbs > 0.0);
    assert!(stats.max_throughput_mbs >= stats.min_throughput_mbs);
    assert!((stats.best_throughput_mbs - stats.max_throughput_mbs).abs() < 1e-6);

    // Verify all 6 passes succeeded and accumulated correct loop counts
    for (idx, pass) in stats.passes.iter().enumerate() {
        assert_eq!(pass.pass_index, idx + 1);
        assert!(pass.loop_count > 0);
        assert_eq!(pass.total_bytes, pass.loop_count * (payload.len() as u64));
        assert!(pass.elapsed_micros >= 2_000.0);
        assert!(pass.throughput_mbs > 0.0);
    }
}

#[test]
fn test_guarded_buffer_mmu_end_alignment_and_codecs() {
    use crate::benchmark::ab_engine::guarded_buffer::GuardedBuffer;
    use crate::codecs::deflate::{deflate_compress, deflate_decompress};
    use crate::codecs::lz4::{lz4_compress, lz4_decompress};
    use crate::codecs::zstd::{zstd_compress, zstd_decompress};

    let payload = b"MMU Hardware Guard Page & Buffer Security Test String for TTZip Kernel Benchmark Engine.";
    
    // 1. Deflate codec check
    let mut deflate_comp = vec![0u8; 1024];
    let comp_sz = deflate_compress(payload, &mut deflate_comp, 6).expect("deflate compress");
    let mut gbuf_deflate = GuardedBuffer::new(payload.len());
    let dec_sz = deflate_decompress(&deflate_comp[..comp_sz], gbuf_deflate.as_mut_slice())
        .expect("deflate decompress into GuardedBuffer");
    assert_eq!(dec_sz, payload.len());
    assert_eq!(&gbuf_deflate[..], payload);

    // 2. LZ4 codec check
    let mut lz4_comp = vec![0u8; 1024];
    let comp_sz_lz4 = lz4_compress(payload, &mut lz4_comp).expect("lz4 compress");
    let mut gbuf_lz4 = GuardedBuffer::new(payload.len());
    let dec_sz_lz4 = lz4_decompress(&lz4_comp[..comp_sz_lz4], gbuf_lz4.as_mut_slice())
        .expect("lz4 decompress into GuardedBuffer");
    assert_eq!(dec_sz_lz4, payload.len());
    assert_eq!(&gbuf_lz4[..], payload);

    // 3. Zstd codec check
    let mut zstd_comp = vec![0u8; 1024];
    let comp_sz_zstd = zstd_compress(payload, &mut zstd_comp, 3).expect("zstd compress");
    let mut gbuf_zstd = GuardedBuffer::new(payload.len());
    let dec_sz_zstd = zstd_decompress(&zstd_comp[..comp_sz_zstd], gbuf_zstd.as_mut_slice())
        .expect("zstd decompress into GuardedBuffer");
    assert_eq!(dec_sz_zstd, payload.len());
    assert_eq!(&gbuf_zstd[..], payload);
}

#[test]
fn test_header_quota_guard_and_four_billion_bomb() {
    use crate::benchmark::ab_engine::header_quota_guard::{
        validate_header_entry_count, HeaderQuotaGuard, HeaderSecurityError,
    };

    // Valid header
    assert!(validate_header_entry_count(50, 100).is_ok());

    // 4-billion entry attack in tiny header
    let bomb_res = validate_header_entry_count(4_000_000_000, 64);
    assert!(matches!(
        bomb_res,
        Err(HeaderSecurityError::HeaderOomBombDetected { declared_files: 4_000_000_000, .. })
    ));

    // Custom quota
    let custom = HeaderQuotaGuard::new(1024 * 1024, 128); // 1 MB quota
    assert!(custom.validate(1000, 500).is_ok());
    assert!(custom.validate(100_000, 50_000).is_err());
}

#[test]
fn test_micro_chunk_stream_validator_all_codecs_and_ladder_steps() {
    use crate::benchmark::ab_engine::micro_chunk::{
        MicroChunkCodec, MicroChunkStreamValidator, MICRO_CHUNK_STEPS, STAIRCASE_CHUNK_PATTERN,
    };

    assert_eq!(MICRO_CHUNK_STEPS, &[1, 2, 3, 7, 15, 259, 1024, 4096]);
    assert_eq!(STAIRCASE_CHUNK_PATTERN, &[1, 3, 7, 2, 15, 259, 1, 7, 1024, 3, 4096]);

    let payload = b"TTZip Micro-Buffer Streaming Decompression Extreme Step Test Payload 2026. \
        Repeating content to ensure state machine transitions across multiple blocks and buffers: \
        ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-=[]{}|;':,./<>? \
        The quick brown fox jumps over the lazy dog repeatedly.";

    // 1. Validate all built-in streaming codecs across all steps
    let reports = MicroChunkStreamValidator::validate_all_codecs(payload)
        .expect("validate all codecs");
    assert_eq!(reports.len(), 4);

    for report in &reports {
        assert!(
            report.all_passed,
            "Codec {} failed micro-chunk validation",
            report.codec.name()
        );
        assert_eq!(report.step_results.len(), MICRO_CHUNK_STEPS.len());
        for res in &report.step_results {
            assert!(res.passed, "Step {} failed for {}", res.chunk_size, report.codec.name());
            assert_eq!(res.bytes_decompressed, payload.len());
            assert!(res.read_iterations > 0);
        }
        let staircase = report.staircase_result.as_ref().expect("staircase result");
        assert!(staircase.passed);
        assert_eq!(staircase.bytes_decompressed, payload.len());
    }

    // 2. Custom flate2 Deflate/Gzip streaming decompressor test in 1..259 byte micro chunks
    use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
    use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
    use flate2::Compression;
    use std::io::{Cursor, Write};

    // Deflate
    let mut def_enc = DeflateEncoder::new(Vec::new(), Compression::default());
    def_enc.write_all(payload).expect("deflate write");
    let def_comp = def_enc.finish().expect("deflate finish");
    for &step in &[1, 3, 7, 259] {
        let dec = DeflateDecoder::new(Cursor::new(&def_comp));
        let res = MicroChunkStreamValidator::validate_reader(
            dec,
            payload,
            step,
            MicroChunkCodec::RawPassthrough,
        );
        assert!(res.passed);
        assert_eq!(res.bytes_decompressed, payload.len());
    }

    // Gzip
    let mut gz_enc = GzEncoder::new(Vec::new(), Compression::default());
    gz_enc.write_all(payload).expect("gz write");
    let gz_comp = gz_enc.finish().expect("gz finish");
    for &step in &[1, 2, 7, 259] {
        let dec = GzDecoder::new(Cursor::new(&gz_comp));
        let res = MicroChunkStreamValidator::validate_reader(
            dec,
            payload,
            step,
            MicroChunkCodec::RawPassthrough,
        );
        assert!(res.passed);
        assert_eq!(res.bytes_decompressed, payload.len());
    }

    // Zlib
    let mut zl_enc = ZlibEncoder::new(Vec::new(), Compression::default());
    zl_enc.write_all(payload).expect("zlib write");
    let zl_comp = zl_enc.finish().expect("zlib finish");
    for &step in &[1, 3, 15, 259] {
        let dec = ZlibDecoder::new(Cursor::new(&zl_comp));
        let res = MicroChunkStreamValidator::validate_reader(
            dec,
            payload,
            step,
            MicroChunkCodec::RawPassthrough,
        );
        assert!(res.passed);
        assert_eq!(res.bytes_decompressed, payload.len());
    }
}

#[test]
fn test_huffman_dos_defense_and_degenerate_bomb_generators() {
    use crate::benchmark::ab_engine::huffman_defense::{
        generate_empty_dynamic_huffman_blocks, generate_empty_dynamic_huffman_stream_by_size,
        generate_empty_static_huffman_blocks, DeflateBitWriter, HuffmanComplexityGuard,
        HuffmanDefenseStatus, HuffmanDosDefense,
    };
    use crate::codecs::deflate::DeflateDecompressor;

    // 1. Test DeflateBitWriter
    let mut writer = DeflateBitWriter::new();
    writer.put_bits(0b101, 3);
    writer.put_bits(0b11, 2);
    writer.put_bits(0b0, 1);
    writer.put_bits(0b10, 2);
    let bytes = writer.finish();
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0], 0b10_0_11_101);

    // 2. Generate and safely decompress empty static Huffman blocks
    let static_stream = generate_empty_static_huffman_blocks(512);
    assert!(static_stream.len() >= 512);
    let mut decompressor = DeflateDecompressor::new().expect("alloc decompressor");
    let mut out_buf = vec![0u8; 1024];
    let res = decompressor.decompress_precise(&static_stream, &mut out_buf);
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 0); // 0 output bytes because all blocks are empty

    // 3. Generate and safely decompress empty dynamic Huffman blocks
    let dynamic_stream = generate_empty_dynamic_huffman_blocks(10);
    assert!(!dynamic_stream.is_empty());
    let mut decompressor_dyn = DeflateDecompressor::new().expect("alloc decompressor");
    let res_dyn = decompressor_dyn.decompress_precise(&dynamic_stream, &mut out_buf);
    assert!(res_dyn.is_ok());
    assert_eq!(res_dyn.unwrap(), 0);

    // 4. Generate dynamic stream by size
    let dyn_sized = generate_empty_dynamic_huffman_stream_by_size(256);
    assert!(dyn_sized.len() >= 256);
    let mut decompressor_sized = DeflateDecompressor::new().expect("alloc decompressor");
    let res_sized = decompressor_sized.decompress_precise(&dyn_sized, &mut out_buf);
    assert!(res_sized.is_ok());
    assert_eq!(res_sized.unwrap(), 0);

    // 5. Run full defense audit
    let summary = HuffmanDosDefense::run_full_defense_audit(512, 10, None);
    assert!(summary.all_safe);
    assert!(summary.static_report.survived);
    assert!(summary.dynamic_report.survived);
    assert_eq!(summary.static_report.status, HuffmanDefenseStatus::Safe);
    assert_eq!(summary.dynamic_report.status, HuffmanDefenseStatus::Safe);
    assert!(summary.static_report.throughput_kb_per_sec > 0.0);
    assert!(summary.dynamic_report.throughput_kb_per_sec > 0.0);

    // 6. Test complexity guard timeout intercept
    let strict_guard = HuffmanComplexityGuard {
        max_duration: std::time::Duration::from_nanos(1), // Trip immediately
        max_empty_blocks_limit: 10,
        min_throughput_kb_s: 1000.0,
    };
    let guarded_rep = HuffmanDosDefense::evaluate_libdeflate(
        "Guarded Static Test",
        &static_stream,
        100,
        10_000,
        Some(&strict_guard),
    );
    assert_eq!(guarded_rep.status, HuffmanDefenseStatus::GuardTripped);
}
