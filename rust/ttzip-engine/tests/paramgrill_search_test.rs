// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and verification tests for `ParamGrillSearchEngine`.

use ttzip_engine::benchmark::ab_engine::paramgrill::*;

#[test]
fn test_hyper_param_vector_presets_and_bounds() {
    for level in [1, 3, 7, 9, 15, 19] {
        let mut p = HyperParamVector::preset(level);
        let orig = p;
        p.clamp_to_bounds();
        assert_eq!(p, orig, "Preset level {level} must already be within valid bounds");
    }

    let mut out_of_bounds = HyperParamVector {
        window_log: 99,
        chain_log: 0,
        hash_log: 99,
        search_log: 0,
        min_match: 99,
        target_length: 5000,
        strategy: 20,
        chunk_size: 12345,
    };
    out_of_bounds.clamp_to_bounds();
    assert_eq!(out_of_bounds.window_log, 31);
    assert_eq!(out_of_bounds.chain_log, 6);
    assert_eq!(out_of_bounds.hash_log, 30);
    assert_eq!(out_of_bounds.search_log, 1);
    assert_eq!(out_of_bounds.min_match, 7);
    assert_eq!(out_of_bounds.target_length, 999);
    assert_eq!(out_of_bounds.strategy, 9);
    assert!(VALID_CHUNK_SIZES.contains(&out_of_bounds.chunk_size));
}

#[test]
fn test_hyper_param_vector_manhattan_neighbors() {
    let p = HyperParamVector::preset(3);
    let neighbors = p.manhattan_neighbors();

    assert!(!neighbors.is_empty());
    assert!(neighbors.len() <= 16);

    for n in &neighbors {
        assert_ne!(&p, n);
        // Verify exactly one dimension is perturbed
        let mut diff_count = 0;
        if p.window_log != n.window_log { diff_count += 1; }
        if p.chain_log != n.chain_log { diff_count += 1; }
        if p.hash_log != n.hash_log { diff_count += 1; }
        if p.search_log != n.search_log { diff_count += 1; }
        if p.min_match != n.min_match { diff_count += 1; }
        if p.target_length != n.target_length { diff_count += 1; }
        if p.strategy != n.strategy { diff_count += 1; }
        if p.chunk_size != n.chunk_size { diff_count += 1; }
        assert_eq!(diff_count, 1, "Manhattan distance must be exactly 1");
    }
}

#[test]
fn test_hyper_param_vector_xxh64_determinism() {
    let p1 = HyperParamVector::preset(3);
    let p2 = HyperParamVector::preset(3);
    let p3 = HyperParamVector::preset(7);

    assert_eq!(p1.compute_xxh64(), p2.compute_xxh64());
    assert_ne!(p1.compute_xxh64(), p3.compute_xxh64());
}

#[test]
fn test_paramgrill_evaluation_and_memo_cache() {
    let constraints = ParamGrillSearchConstraints {
        max_evaluations: 16,
        ..Default::default()
    };
    let mut engine = ParamGrillSearchEngine::new(constraints);

    // Create synthetic text corpus
    let corpus = b"The quick brown fox jumps over the lazy dog. TTZip High-Performance Engine 2026. "
        .repeat(500);

    let params = HyperParamVector::preset(1);
    let r1 = engine.evaluate(&params, &corpus).expect("Evaluation 1");
    assert!(r1.compressed_size > 0);
    assert!(r1.compression_ratio > 1.0);
    assert_eq!(engine.cache_hits(), 0);

    // Second evaluation of same params must hit cache
    let r2 = engine.evaluate(&params, &corpus).expect("Evaluation 2");
    assert_eq!(r1, r2);
    assert_eq!(engine.cache_hits(), 1);
}

#[test]
fn test_paramgrill_hill_climbing_search_and_pareto_front() {
    let constraints = ParamGrillSearchConstraints {
        min_speed_mb_s: None,
        min_compression_ratio: None,
        max_evaluations: 24,
        max_stagnant_steps: 4,
        restarts: 1,
        alpha_speed_weight: 0.5,
        beta_ratio_weight: 0.5,
    };
    let mut engine = ParamGrillSearchEngine::new(constraints);

    let corpus = b"Deterministic testing corpus for TTZip paramgrill hyperparameter search algorithm."
        .repeat(600);

    let seeds = vec![
        HyperParamVector::preset(1),
        HyperParamVector::preset(3),
    ];

    let report = engine.search(&seeds, &corpus).expect("Search execution");
    assert!(report.total_evaluations > 0);
    assert!(!report.pareto_optimal_points.is_empty());
    assert!(report.best_solution.is_some());

    // Verify Pareto front properties: non-dominated order
    for i in 0..report.pareto_optimal_points.len().saturating_sub(1) {
        let a = &report.pareto_optimal_points[i];
        let b = &report.pareto_optimal_points[i + 1];
        assert!(
            a.compress_speed_mb_s >= b.compress_speed_mb_s,
            "Pareto front should be sorted descending by speed"
        );
        assert!(
            a.compression_ratio <= b.compression_ratio,
            "Higher compression ratio should accompany lower speed on the frontier"
        );
    }

    assert!(report.recommended_levels.contains_key(&1));
    assert!(report.recommended_levels.contains_key(&19));
}
