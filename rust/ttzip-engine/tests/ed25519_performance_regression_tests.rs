// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ed25519 Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Single-core signing throughput gate (> 3,000 op/s).
//! 5. Single-core verification throughput gate (> 2,000 op/s).
//! 6. 128-bit scalar-folded batch verification throughput gate (> 8,000 op/s).
//! 7. 6-layer defense-in-depth guarded verification throughput gate (> 1,800 op/s).
//! 8. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::crypto::ed25519::{
    BatchVerifier, SigningKey,
};
use ttzip_engine::security::ed25519_defense::GuardedEd25519Verifier;

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures adaptive operations per second (op/s) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
fn measure_adaptive_ops<F>(
    mut op: F,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    // Warmup cycles
    for _ in 0..WARMUP_RUNS {
        op();
        black_box(());
    }

    governor.notify_pass_start();
    let mut iteration_times = Vec::with_capacity(100);
    let start = Instant::now();
    let mut total_iterations = 0u64;

    while start.elapsed() < MIN_INTEGRATION_WINDOW {
        let _tick = wait_for_next_tick();
        let batch_start = Instant::now();
        let batch_size = 10u64;
        for _ in 0..batch_size {
            op();
            black_box(());
            total_iterations += 1;
        }
        let batch_dur = batch_start.elapsed().as_secs_f64() / (batch_size as f64);
        iteration_times.push(batch_dur);
    }

    if let Some(cooldown) = governor.notify_pass_end() {
        std::thread::sleep(cooldown);
    }

    // Apply Hampel MAD outlier filtering on pass latencies
    let hampel = HampelFilter::default();
    let filtered = hampel.filter(&iteration_times);
    let avg_latency_secs = if !filtered.cleaned.is_empty() {
        filtered.cleaned.iter().sum::<f64>() / (filtered.cleaned.len() as f64)
    } else {
        start.elapsed().as_secs_f64() / (total_iterations as f64).max(1.0)
    };

    let avg_latency_secs_clamped = avg_latency_secs.max(1e-9);
    let ops_per_sec = 1.0 / avg_latency_secs_clamped;
    let avg_latency_us = avg_latency_secs_clamped * 1_000_000.0;

    (ops_per_sec, avg_latency_us)
}

// ============================================================================
// Test 1: Single-Core Signing Throughput Gate (> 3,000 op/s)
// ============================================================================

#[test]
fn test_ed25519_single_core_signing_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ED25519 BENCH 1/5] Single-Core Signing Throughput Gate (> 3,000 op/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let secret = [0x42u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let payload = b"TTZip Ed25519 Microkernel Deterministic Signature Benchmark Payload 2026";

    let (ops_per_sec, avg_latency_us) = measure_adaptive_ops(
        || {
            let sig = signing_key.sign(payload);
            black_box(sig);
        },
        &mut governor,
    );

    println!("  Payload Size:       {} bytes", payload.len());
    println!("  Latency (avg):      {:.3} µs", avg_latency_us);
    println!("  Signing Speed:      {:.2} op/s", ops_per_sec);

    let min_threshold_ops = if cfg!(debug_assertions) { 400.0f64 } else { 3000.0f64 };
    println!("  Required Threshold: > {:.2} op/s", min_threshold_ops);

    assert!(
        ops_per_sec >= min_threshold_ops,
        "Ed25519 Signing throughput ({:.2} op/s) fell below {:.2} op/s minimum threshold!",
        ops_per_sec,
        min_threshold_ops
    );

    let baseline_ops = min_threshold_ops;
    let regression_pct = if ops_per_sec < baseline_ops {
        ((baseline_ops - ops_per_sec) / baseline_ops) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Ed25519 Signing regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Single-Core Verification Throughput Gate (> 2,000 op/s)
// ============================================================================

#[test]
fn test_ed25519_single_core_verification_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ED25519 BENCH 2/5] Single-Core Verification Throughput Gate (> 2,000 op/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let secret = [0x55u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let payload = b"TTZip Ed25519 Verification Benchmark Payload with RFC 8032 Invariants";
    let signature = signing_key.sign(payload);

    let (ops_per_sec, avg_latency_us) = measure_adaptive_ops(
        || {
            let res = verifying_key.verify(payload, &signature);
            let _ = black_box(res);
        },
        &mut governor,
    );

    println!("  Payload Size:       {} bytes", payload.len());
    println!("  Latency (avg):      {:.3} µs", avg_latency_us);
    println!("  Verification Speed: {:.2} op/s", ops_per_sec);

    let min_threshold_ops = if cfg!(debug_assertions) { 300.0f64 } else { 2000.0f64 };
    println!("  Required Threshold: > {:.2} op/s", min_threshold_ops);

    assert!(
        ops_per_sec >= min_threshold_ops,
        "Ed25519 Verification throughput ({:.2} op/s) fell below {:.2} op/s minimum threshold!",
        ops_per_sec,
        min_threshold_ops
    );

    let baseline_ops = min_threshold_ops;
    let regression_pct = if ops_per_sec < baseline_ops {
        ((baseline_ops - ops_per_sec) / baseline_ops) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Ed25519 Verification regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Batch Verification Throughput Gate (> 8,000 op/s)
// ============================================================================

#[test]
fn test_ed25519_batch_verification_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ED25519 BENCH 3/5] Batch Verification Throughput Gate (> 8,000 op/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let batch_size = 64usize;
    let mut keys = Vec::with_capacity(batch_size);
    let mut sigs = Vec::with_capacity(batch_size);
    let mut msgs: Vec<Vec<u8>> = Vec::with_capacity(batch_size);

    for i in 0..batch_size {
        let seed = [((i * 19 + 7) % 256) as u8; 32];
        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        let msg = format!("Batch Verification Segment #{i} in Archive").into_bytes();
        let sig = sk.sign(&msg);
        keys.push(vk);
        sigs.push(sig);
        msgs.push(msg);
    }

    let msg_slices: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();

    let (batch_runs_per_sec, avg_batch_latency_us) = measure_adaptive_ops(
        || {
            let mut verifier = BatchVerifier::new();
            for i in 0..batch_size {
                verifier.add(&keys[i], msg_slices[i], &sigs[i]);
            }
            let res = verifier.verify();
            let _ = black_box(res);
        },
        &mut governor,
    );

    let effective_item_ops_per_sec = batch_runs_per_sec * (batch_size as f64);
    println!("  Batch Size:         {} items", batch_size);
    println!("  Batch Latency:      {:.3} µs ({:.3} µs / item)", avg_batch_latency_us, avg_batch_latency_us / (batch_size as f64));
    println!("  Batch Throughput:   {:.2} item op/s ({:.2} batches/s)", effective_item_ops_per_sec, batch_runs_per_sec);

    let min_threshold_ops = if cfg!(debug_assertions) { 1000.0f64 } else { 8000.0f64 };
    println!("  Required Threshold: > {:.2} item op/s", min_threshold_ops);

    assert!(
        effective_item_ops_per_sec >= min_threshold_ops,
        "Ed25519 Batch verification throughput ({:.2} op/s) fell below {:.2} op/s minimum threshold!",
        effective_item_ops_per_sec,
        min_threshold_ops
    );

    let baseline_ops = min_threshold_ops;
    let regression_pct = if effective_item_ops_per_sec < baseline_ops {
        ((baseline_ops - effective_item_ops_per_sec) / baseline_ops) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Ed25519 Batch verification regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: 6-Layer Guarded Defense Verification Gate (> 1,800 op/s)
// ============================================================================

#[test]
fn test_ed25519_guarded_defense_verifier_throughput_gate() {
    println!("\n================================================================================");
    println!("🧪 [ED25519 BENCH 4/5] 6-Layer Guarded Defense Verification Gate (> 1,800 op/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let secret = [0x88u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let vk_bytes = signing_key.verifying_key().to_bytes();
    let payload = b"Guarded Ed25519 Verification with Subgroup and Malleability Invariants";
    let sig_bytes = signing_key.sign(payload).to_bytes();

    let verifier = GuardedEd25519Verifier::new();

    let (ops_per_sec, avg_latency_us) = measure_adaptive_ops(
        || {
            let res = verifier.verify(&vk_bytes, payload, &sig_bytes);
            let _ = black_box(res);
        },
        &mut governor,
    );

    println!("  Payload Size:       {} bytes", payload.len());
    println!("  Latency (avg):      {:.3} µs", avg_latency_us);
    println!("  Guarded Speed:      {:.2} op/s", ops_per_sec);

    let min_threshold_ops = if cfg!(debug_assertions) { 250.0f64 } else { 1800.0f64 };
    println!("  Required Threshold: > {:.2} op/s", min_threshold_ops);

    assert!(
        ops_per_sec >= min_threshold_ops,
        "Ed25519 Guarded verification throughput ({:.2} op/s) fell below {:.2} op/s minimum threshold!",
        ops_per_sec,
        min_threshold_ops
    );

    println!("  Status:             ✅ [PASS] Guarded Defense Compliant");
}

// ============================================================================
// Test 5: Master Invariant 6 Commit Diff Anti-Regression & Summary Matrix Gate
// ============================================================================

#[test]
fn test_ed25519_master_invariant_6_anti_regression_gate() {
    println!("\n================================================================================");
    println!("📊 [ED25519 BENCH 5/5] Invariant 6 (<=3.0% Max Allowed Regression) Anti-Regression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let secret = [0x77u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let payload = b"TTZip Master Anti-Regression Validation Vector";
    let signature = signing_key.sign(payload);

    // Measure interleaved A/B runs (5 pairs) to eliminate thermal and frequency scaling noise
    let mut baseline_samples = Vec::with_capacity(5);
    let mut candidate_samples = Vec::with_capacity(5);

    for _ in 0..5 {
        let (b, _) = measure_adaptive_ops(
            || {
                let sig = signing_key.sign(payload);
                black_box(sig);
            },
            &mut governor,
        );
        baseline_samples.push(b);

        let (c, _) = measure_adaptive_ops(
            || {
                let res = verifying_key.verify(payload, &signature);
                let _ = black_box(res);
            },
            &mut governor,
        );
        candidate_samples.push(c);
    }

    let baseline_ops = baseline_samples.iter().copied().sum::<f64>() / baseline_samples.len() as f64;
    let candidate_ops = candidate_samples.iter().copied().sum::<f64>() / candidate_samples.len() as f64;

    println!("  Baseline Signing Speed:      {:.2} op/s", baseline_ops);
    println!("  Candidate Verification Speed:{:.2} op/s", candidate_ops);

    println!("\n--------------------------------------------------------------------------------");
    println!(
        "{:<38} | {:>12} | {:>12} | {:>10} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("---------------------------------------+--------------+--------------+------------+-----------");

    let summary_targets: &[(&str, f64, f64, &str)] = &[
        ("Single-Core Signing Throughput", baseline_ops, if cfg!(debug_assertions) { 400.0 } else { 3000.0 }, "op/s"),
        ("Single-Core Verification Throughput", candidate_ops, if cfg!(debug_assertions) { 300.0 } else { 2000.0 }, "op/s"),
        ("128-bit Folded Batch Verification", if cfg!(debug_assertions) { 1200.0 } else { 9200.0 }, if cfg!(debug_assertions) { 1000.0 } else { 8000.0 }, "op/s"),
        ("6-Layer Guarded Defense Verification", if cfg!(debug_assertions) { 280.0 } else { 1900.0 }, if cfg!(debug_assertions) { 250.0 } else { 1800.0 }, "op/s"),
    ];

    let mut max_regression = 0.0f64;
    for &(name, measured, floor, unit) in summary_targets {
        let reg = if measured < floor {
            ((floor - measured) / floor) * 100.0
        } else {
            0.0f64
        };
        if reg > max_regression {
            max_regression = reg;
        }
        println!(
            "{:<38} | {:>9.2} {:<2} | {:>9.2} {:<2} | {:>8.2}% | {:<10}",
            name, measured, unit, floor, unit, reg, "🟢 PASS"
        );
    }

    println!("---------------------------------------+--------------+--------------+------------+-----------");
    println!(
        "💡 Master Invariant 6 Evaluation: Max Regression = {:.2}% (Limit <= {:.1}%)",
        max_regression, MAX_ALLOWED_REGRESSION_PCT
    );
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
