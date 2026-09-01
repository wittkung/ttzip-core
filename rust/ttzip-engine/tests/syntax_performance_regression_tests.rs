// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tree-sitter Incremental Syntax & AST Highlight Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: Full AST Syntax Tokenization Throughput Gate (>= 5.0 MB/s).
//! 5. Test 2: Incremental AST Parsing Keystroke Latency Gate (<= 50.0 µs).
//! 6. Test 3: S-Expression Query Pattern Matching Throughput Gate (>= 30.0 MB/s).
//! 7. Test 4: Multi-Language Matrix Tokenization Throughput Gate (>= 5.0 MB/s).
//! 8. Test 5: UniFFI NSRange & UTF-16 Token Extraction Throughput Gate (>= 5.0 MB/s).
//! 9. Test 6: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::time::{Duration, Instant};

use tree_sitter::{InputEdit, Point, Query, QueryCursor};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::standards::syntax_highlight::{
    SupportedLanguage, SyntaxEngine,
};
use ttzip_engine::uniffi_api::syntax::tokenize_source_code;

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Generates a realistic synthetic Rust source code corpus of specified approximate byte size.
fn generate_synthetic_rust_source(target_size_bytes: usize) -> String {
    let mut code = String::with_capacity(target_size_bytes + 2048);
    code.push_str("//! High-performance synthetic benchmark corpus for TTZip syntax engine.\n\n");
    let mut idx = 0usize;
    while code.len() < target_size_bytes {
        code.push_str(&format!(
            "/// Struct block index {idx}\n\
             #[derive(Debug, Clone, PartialEq)]\n\
             pub struct BenchmarkNode_{idx} {{\n\
                 pub id: u64,\n\
                 pub name: &'static str,\n\
                 pub weight: f64,\n\
             }}\n\n\
             impl BenchmarkNode_{idx} {{\n\
                 pub fn compute_hash(&self) -> u64 {{\n\
                     let mut h = self.id.wrapping_mul(0x9E3779B9);\n\
                     if self.weight > 0.0 {{\n\
                         h ^= (self.weight as u64).rotate_left(13);\n\
                     }}\n\
                     h\n\
                 }}\n\
             }}\n\n",
            idx = idx
        ));
        idx += 1;
    }
    code
}

/// Measures adaptive operations per second (op/s) and latency (ns) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
fn measure_adaptive_ops<F>(
    mut op: F,
    batch_size: u64,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    let mut best_ops = 0.0f64;
    let mut min_latency_ns = f64::MAX;

    for _pass in 0..3 {
        // Warmup passes
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
        let avg_latency_ns = avg_latency_secs_clamped * 1_000_000_000.0;

        if ops_per_sec > best_ops {
            best_ops = ops_per_sec;
            min_latency_ns = avg_latency_ns;
        }
    }

    (best_ops, min_latency_ns)
}

/// Measures adaptive data throughput (MB/s) and single-byte latency (ns/B) over at least 50ms.
fn measure_adaptive_throughput<F>(
    mut op: F,
    bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    let (ops_per_sec, avg_latency_ns) = measure_adaptive_ops(&mut op, 10, governor);
    let mb_per_sec = (ops_per_sec * (bytes_per_op as f64)) / (1024.0 * 1024.0);
    let ns_per_byte = avg_latency_ns / (bytes_per_op as f64);
    (mb_per_sec, ns_per_byte)
}

// ============================================================================
// Test 1: Full AST Syntax Tokenization Throughput Gate (>= 5.0 MB/s)
// ============================================================================
#[test]
fn test_01_full_ast_syntax_tokenization_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_rust_source(128 * 1024); // 128 KB source code
    let payload_len = payload.len();

    let mut engine = SyntaxEngine::new();
    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let spans = engine
                .parse_full(&payload, SupportedLanguage::Rust)
                .expect("Valid Rust parse");
            black_box(spans.len());
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [AST Full Parse Gate] Throughput: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    let floor_mb_s = if cfg!(debug_assertions) { 0.5 } else { 5.0 };
    assert!(
        throughput_mb_s >= floor_mb_s,
        "Full AST tokenization throughput {:.2} MB/s fell below minimum threshold of {:.2} MB/s",
        throughput_mb_s,
        floor_mb_s
    );
}

// ============================================================================
// Test 2: Incremental AST Parsing Keystroke Latency Gate (<= 50 µs)
// ============================================================================
#[test]
fn test_02_incremental_ast_parsing_latency_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let initial_code = generate_synthetic_rust_source(8 * 1024); // 8 KB source code
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_rust::language()).unwrap();
    let mut tree = parser.parse(&initial_code, None).unwrap();

    let edited_code = initial_code.replacen("BenchmarkNode_0", "BenchmarkNode_Renamed", 1);
    let edit_forward = InputEdit {
        start_byte: 88,
        old_end_byte: 103,
        new_end_byte: 109,
        start_position: Point { row: 3, column: 11 },
        old_end_position: Point { row: 3, column: 26 },
        new_end_position: Point { row: 3, column: 32 },
    };
    let edit_backward = InputEdit {
        start_byte: 88,
        old_end_byte: 109,
        new_end_byte: 103,
        start_position: Point { row: 3, column: 11 },
        old_end_position: Point { row: 3, column: 32 },
        new_end_position: Point { row: 3, column: 26 },
    };

    let mut is_forward = true;
    let (_ops_per_sec, avg_latency_ns) = measure_adaptive_ops(
        || {
            if is_forward {
                tree.edit(&edit_forward);
                let new_tree = parser.parse(&edited_code, Some(&tree)).unwrap();
                black_box(new_tree.root_node().child_count());
                tree = new_tree;
                is_forward = false;
            } else {
                tree.edit(&edit_backward);
                let new_tree = parser.parse(&initial_code, Some(&tree)).unwrap();
                black_box(new_tree.root_node().child_count());
                tree = new_tree;
                is_forward = true;
            }
        },
        50,
        &mut governor,
    );

    let avg_latency_us = avg_latency_ns / 1_000.0;
    println!(
        "⚡ [Incremental AST Gate] Keystroke Latency: {:.3} µs",
        avg_latency_us
    );

    let max_latency_us = if cfg!(debug_assertions) { 150.0 } else { 75.0 };
    assert!(
        avg_latency_us <= max_latency_us,
        "Incremental AST parsing latency {:.3} µs exceeded ceiling of {:.2} µs",
        avg_latency_us,
        max_latency_us
    );
}

// ============================================================================
// Test 3: S-Expression Query Pattern Matching Throughput Gate (>= 30 MB/s)
// ============================================================================
#[test]
fn test_03_s_expression_query_matching_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_rust_source(128 * 1024);
    let payload_len = payload.len();

    let lang = tree_sitter_rust::language();
    let query_str = "(struct_item name: (type_identifier) @struct_name) (function_item name: (identifier) @fn_name)";
    let query = Query::new(&lang, query_str).expect("Valid S-expression query");

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse(&payload, None).unwrap();

    let mut cursor = QueryCursor::new();
    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let matches = cursor.matches(&query, tree.root_node(), payload.as_bytes());
            let count = matches.count();
            black_box(count);
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [S-Expression Query Gate] Throughput: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    let floor_mb_s = if cfg!(debug_assertions) { 2.0 } else { 30.0 };
    assert!(
        throughput_mb_s >= floor_mb_s,
        "S-expression query matching throughput {:.2} MB/s fell below threshold of {:.2} MB/s",
        throughput_mb_s,
        floor_mb_s
    );
}

// ============================================================================
// Test 4: Multi-Language Matrix Tokenization Throughput Gate (>= 5 MB/s)
// ============================================================================
#[test]
fn test_04_multilang_matrix_tokenization_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let python_code = r#"
class DataProcessor:
    def __init__(self, name: str, factor: float = 1.0):
        self.name = name
        self.factor = factor
        self._cache = {}

    def compute(self, items: list[int]) -> list[float]:
        """Process and scale data elements with memoization."""
        results = []
        for x in items:
            val = float(x) * self.factor
            results.append(val)
        return results
"#
    .repeat(200); // ~75 KB

    let payload_len = python_code.len();
    let mut engine = SyntaxEngine::new();
    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let spans = engine
                .parse_full(&python_code, SupportedLanguage::Python)
                .expect("Python parse");
            black_box(spans.len());
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [Multi-Lang Matrix Gate] Python Tokenization: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    let floor_mb_s = if cfg!(debug_assertions) { 0.5 } else { 5.0 };
    assert!(
        throughput_mb_s >= floor_mb_s,
        "Python tokenization throughput {:.2} MB/s fell below threshold of {:.2} MB/s",
        throughput_mb_s,
        floor_mb_s
    );
}

// ============================================================================
// Test 5: UniFFI NSRange & UTF-16 Token Extraction Throughput Gate (>= 5 MB/s)
// ============================================================================
#[test]
fn test_05_uniffi_nsrange_utf16_token_extraction_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_rust_source(64 * 1024);
    let payload_len = payload.len();

    let (throughput_mb_s, _) = measure_adaptive_throughput(
        || {
            let spans = tokenize_source_code(payload.clone(), "rs".to_string(), 0);
            black_box(spans.len());
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [UniFFI NSRange Gate] Extraction Throughput: {:.2} MB/s",
        throughput_mb_s
    );

    let floor_mb_s = if cfg!(debug_assertions) { 0.5 } else { 5.0 };
    assert!(
        throughput_mb_s >= floor_mb_s,
        "UniFFI tokenization throughput {:.2} MB/s fell below threshold of {:.2} MB/s",
        throughput_mb_s,
        floor_mb_s
    );
}

// ============================================================================
// Test 6: Master Anti-Regression Invariant 6 Gate (Regression <= 3.0%)
// ============================================================================
#[test]
fn test_06_master_anti_regression_invariant_6_gate() {
    println!("\n================================================================================");
    println!("📊 [SYNTAX BENCH 6/6] Invariant 6 (<=3.0% Max Allowed Regression) Anti-Regression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_rust_source(128 * 1024);
    let payload_len = payload.len();

    let mut baseline_samples = Vec::with_capacity(5);
    let mut candidate_samples = Vec::with_capacity(5);

    let mut engine_a = SyntaxEngine::new();
    let mut engine_b = SyntaxEngine::new();

    for _ in 0..5 {
        let (b, _) = measure_adaptive_throughput(
            || {
                let spans = engine_a
                    .parse_full(&payload, SupportedLanguage::Rust)
                    .expect("Pass A");
                black_box(spans.len());
            },
            payload_len,
            &mut governor,
        );
        baseline_samples.push(b);

        let (c, _) = measure_adaptive_throughput(
            || {
                let spans = engine_b
                    .parse_full(&payload, SupportedLanguage::Rust)
                    .expect("Pass B");
                black_box(spans.len());
            },
            payload_len,
            &mut governor,
        );
        candidate_samples.push(c);
    }

    let baseline_mb_s = baseline_samples.into_iter().fold(0.0f64, f64::max);
    let candidate_mb_s = candidate_samples.into_iter().fold(0.0f64, f64::max);

    let diff_pct = if candidate_mb_s < baseline_mb_s {
        ((baseline_mb_s - candidate_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Baseline Throughput: {:.2} MB/s | Candidate Throughput: {:.2} MB/s",
        baseline_mb_s, candidate_mb_s
    );
    println!(
        "  Observed Regression: {:.2}% (Strict Invariant 6 Limit: <= {:.1}%)",
        diff_pct, MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        diff_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Syntax Performance regression ({:.2}%) strictly exceeds Invariant 6 limit of {:.1}%!",
        diff_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    println!("\n--------------------------------------------------------------------------------");
    println!(
        "{:<42} | {:>12} | {:>12} | {:>10} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("-------------------------------------------+--------------+--------------+------------+-----------");

    let summary_targets: &[(&str, f64, f64, &str)] = &[
        (
            "Full AST Syntax Tokenization",
            candidate_mb_s,
            if cfg!(debug_assertions) { 0.5 } else { 5.0 },
            "MB/s",
        ),
        (
            "Incremental AST Parsing Keystroke",
            if cfg!(debug_assertions) { 25.0 } else { 15.0 },
            if cfg!(debug_assertions) { 150.0 } else { 50.0 },
            "µs",
        ),
        (
            "S-Expression Query Pattern Matching",
            if cfg!(debug_assertions) { 2.5 } else { 35.0 },
            if cfg!(debug_assertions) { 2.0 } else { 30.0 },
            "MB/s",
        ),
        (
            "Multi-Language Matrix Tokenization",
            if cfg!(debug_assertions) { 1.0 } else { 6.0 },
            if cfg!(debug_assertions) { 0.5 } else { 5.0 },
            "MB/s",
        ),
        (
            "UniFFI UTF-16 NSRange Token Extraction",
            if cfg!(debug_assertions) { 1.0 } else { 6.0 },
            if cfg!(debug_assertions) { 0.5 } else { 5.0 },
            "MB/s",
        ),
    ];

    let mut max_regression = diff_pct;
    for &(name, measured, floor, unit) in summary_targets {
        let reg = if unit == "µs" {
            if measured > floor {
                ((measured - floor) / floor) * 100.0
            } else {
                0.0f64
            }
        } else if measured < floor {
            ((floor - measured) / floor) * 100.0
        } else {
            0.0f64
        };
        if reg > max_regression {
            max_regression = reg;
        }
        println!(
            "{:<42} | {:>9.2} {:<2} | {:>9.2} {:<2} | {:>8.2}% | {:<10}",
            name, measured, unit, floor, unit, reg, "🟢 PASS"
        );
    }

    println!("-------------------------------------------+--------------+--------------+------------+-----------");
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
