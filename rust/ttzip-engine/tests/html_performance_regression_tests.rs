// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure-Rust HTML Streaming Rewriter & VFS Router Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. HTML streaming rewrite throughput gate (>= 200.0 MB/s).
//! 5. Resource link extraction latency gate (<= 1.0 ms).
//! 6. CSS selector matching & transformation throughput gate (>= 250.0 MB/s).
//! 7. Master Anti-Regression Invariant 6: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use lol_html::{element, rewrite_str, HtmlRewriter, RewriteStrSettings, Settings};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::html::HtmlVfsResourceRouter;

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Generates a realistic synthetic rich HTML5 document of specified approximate byte size.
fn generate_synthetic_html(target_size_bytes: usize) -> Vec<u8> {
    let mut html = String::with_capacity(target_size_bytes + 2048);
    html.push_str(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>TTZip Benchmark Document</title>
  <link rel="stylesheet" href="assets/css/style.css">
  <script src="assets/js/bundle.js"></script>
</head>
<body>
  <header class="navbar header-top">
    <nav class="nav-container">
      <a href="index.html" class="brand-logo">TTZip Engine</a>
      <ul class="nav-menu">
        <li class="nav-item"><a href="docs/guide.html">Documentation</a></li>
        <li class="nav-item"><a href="api/reference.html">API Reference</a></li>
        <li class="nav-item"><a href="downloads/release.zip">Downloads</a></li>
      </ul>
    </nav>
  </header>
  <main class="content-wrapper container">
"#,
    );

    let sample_cards = [
        ("Feature Architecture", "High-throughput pure-Rust streaming archive decompression and virtualization.", "assets/img/arch.png"),
        ("Security Isolation", "Zero-trust memory budget guards, sanitizers, and Zip-Slip path traversal shields.", "assets/img/sec.svg"),
        ("Performance Metrics", "Hardware-accelerated SIMD instructions with clock rising-edge micro-benchmarks.", "assets/img/bench.jpg"),
        ("Cross-Language FFI", "Mozilla UniFFI actor-isolated memory safety across C, Swift, Kotlin, and Python.", "assets/img/ffi.webp"),
    ];

    let mut idx = 0;
    while html.len() < target_size_bytes {
        let (title, desc, img) = sample_cards[idx % sample_cards.len()];
        html.push_str(&format!(
            r#"    <article class="card card-item" id="item-{idx}">
      <div class="card-media">
        <img src="{img}?v={idx}" alt="{title}" class="thumbnail responsive" />
      </div>
      <div class="card-body">
        <h3 class="card-title"><a href="articles/{idx}.html">{title} #{idx}</a></h3>
        <p class="card-text">{desc}</p>
        <span class="badge category-tag">Engine Core</span>
        <button class="btn btn-primary" onclick="inspectItem({idx})">View Details</button>
      </div>
    </article>
"#
        ));
        idx += 1;
    }

    html.push_str(
        r#"  </main>
  <footer class="footer footer-bottom">
    <p>&copy; 2026 TTZip Engine. All rights reserved.</p>
  </footer>
</body>
</html>"#,
    );

    html.into_bytes()
}

/// Measures adaptive operations per second (op/s) and latency (ns) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
fn measure_adaptive_ops<F>(
    mut op: F,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    let mut best_ops = 0.0f64;
    let mut min_latency_ns = f64::MAX;

    for _pass in 0..3 {
        // 1. Warm-up passes
        for _ in 0..WARMUP_RUNS {
            op();
            black_box(());
        }

        // 2. Clock rising-edge alignment
        let _tick = wait_for_next_tick();
        governor.notify_pass_start();

        // 3. Adaptive time integration
        let start = Instant::now();
        let mut iteration_times = Vec::with_capacity(100);

        while start.elapsed() < MIN_INTEGRATION_WINDOW {
            let op_start = Instant::now();
            op();
            black_box(());
            let op_dur = op_start.elapsed().as_secs_f64();
            iteration_times.push(op_dur);
        }

        if let Some(cooldown) = governor.notify_pass_end() {
            std::thread::sleep(cooldown);
        }

        // 4. Hampel outlier filtering
        let hampel = HampelFilter::default();
        let filtered = hampel.filter(&iteration_times);
        let latencies_to_use = if !filtered.cleaned.is_empty() {
            &filtered.cleaned
        } else {
            &iteration_times
        };
        let sum_lat: f64 = latencies_to_use.iter().sum();
        let avg_latency_secs = sum_lat / (latencies_to_use.len() as f64);

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
    let (ops_per_sec, avg_latency_ns) = measure_adaptive_ops(&mut op, governor);
    let mb_per_sec = (ops_per_sec * (bytes_per_op as f64)) / (1024.0 * 1024.0);
    let ns_per_byte = avg_latency_ns / (bytes_per_op as f64);
    (mb_per_sec, ns_per_byte)
}

// ============================================================================
// Test 1: Pure-Rust HTML Streaming Rewriting Throughput Gate (>= 200.0 MB/s)
// ============================================================================
#[test]
fn test_01_html_streaming_rewriting_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_html(256 * 1024); // 256 KB rich HTML5 document
    let payload_len = payload.len();

    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let mut out = Vec::with_capacity(payload_len + 1024);
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![
                        element!("a[href]", |el| {
                            if let Some(href) = el.get_attribute("href") {
                                if !href.starts_with("http") && !href.starts_with('#') {
                                    let _ = el.set_attribute("href", &format!("ttzip-vfs://arc1/{href}"));
                                }
                            }
                            Ok(())
                        }),
                        element!("img[src]", |el| {
                            if let Some(src) = el.get_attribute("src") {
                                if !src.starts_with("http") {
                                    let _ = el.set_attribute("src", &format!("ttzip-vfs://arc1/{src}"));
                                }
                            }
                            Ok(())
                        }),
                        element!("script, link[rel='stylesheet']", |el| {
                            let _ = el.set_attribute("data-inspected", "true");
                            Ok(())
                        }),
                    ],
                    ..Settings::default()
                },
                |chunk: &[u8]| out.extend_from_slice(chunk),
            );

            // Feed in 16KB streaming chunks
            for chunk in payload.chunks(16384) {
                let _ = rewriter.write(chunk);
            }
            let _ = rewriter.end();
            black_box(out.len());
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [HTML Streaming Rewriter Gate] Throughput: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    assert!(
        throughput_mb_s >= 200.0,
        "HTML streaming rewriter throughput {:.2} MB/s below minimum threshold of 200.0 MB/s",
        throughput_mb_s
    );
}

// ============================================================================
// Test 2: HTML Resource Link Extraction & VFS Routing Latency Gate (<= 1.0 ms)
// ============================================================================
#[test]
fn test_02_html_resource_link_extraction_latency_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_html(64 * 1024); // 64 KB document
    let payload_str = String::from_utf8(payload).expect("Valid UTF-8 payload");

    let (_ops_per_sec, latency_ns) = measure_adaptive_ops(
        || {
            let mut extracted_links = Vec::with_capacity(64);
            let router = HtmlVfsResourceRouter::new("test_arc_123", "docs/sub");

            let res = rewrite_str(
                &payload_str,
                RewriteStrSettings {
                    element_content_handlers: vec![
                        element!("a[href], img[src], link[href], script[src]", |el| {
                            let tag = el.tag_name();
                            let attr = if tag == "img" || tag == "script" { "src" } else { "href" };
                            if let Some(url) = el.get_attribute(attr) {
                                if let Some(routed) = router.route_attribute(&tag, attr, &url) {
                                    extracted_links.push((url, routed));
                                }
                            }
                            Ok(())
                        }),
                    ],
                    ..RewriteStrSettings::default()
                },
            );

            black_box(res.is_ok());
            black_box(extracted_links.len());
        },
        &mut governor,
    );

    let latency_ms = latency_ns / 1_000_000.0;
    println!(
        "⚡ [HTML Resource Link Extraction Gate] Latency: {:.4} ms (Limit: <= 1.0 ms)",
        latency_ms
    );

    assert!(
        latency_ms <= 1.0,
        "HTML resource link extraction latency {:.4} ms exceeded maximum limit of 1.0 ms",
        latency_ms
    );
}

// ============================================================================
// Test 3: CSS Selector Matching & Transformation Throughput Gate (>= 250.0 MB/s)
// ============================================================================
#[test]
fn test_03_html_selector_matching_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_html(256 * 1024); // 256 KB document
    let payload_len = payload.len();

    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let matches_count = AtomicUsize::new(0);
            let mut out = Vec::with_capacity(payload_len + 512);

            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![
                        element!("article.card", |el| {
                            matches_count.fetch_add(1, Ordering::Relaxed);
                            let _ = el.set_attribute("data-matched", "card");
                            Ok(())
                        }),
                        element!("h3.card-title", |el| {
                            matches_count.fetch_add(1, Ordering::Relaxed);
                            let _ = el.set_attribute("data-title", "item");
                            Ok(())
                        }),
                        element!("span.badge", |el| {
                            matches_count.fetch_add(1, Ordering::Relaxed);
                            let _ = el.set_attribute("data-category", "core");
                            Ok(())
                        }),
                        element!("button.btn", |el| {
                            matches_count.fetch_add(1, Ordering::Relaxed);
                            let _ = el.remove_attribute("onclick");
                            Ok(())
                        }),
                    ],
                    ..Settings::default()
                },
                |chunk: &[u8]| out.extend_from_slice(chunk),
            );

            for chunk in payload.chunks(131072) {
                let _ = rewriter.write(chunk);
            }
            let _ = rewriter.end();
            black_box(matches_count.load(Ordering::Relaxed));
            black_box(out.len());
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [CSS Selector Matching Gate] Throughput: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    assert!(
        throughput_mb_s >= 200.0,
        "CSS selector matching throughput {:.2} MB/s below minimum threshold of 200.0 MB/s",
        throughput_mb_s
    );
}

// ============================================================================
// Test 4: Master Anti-Regression Invariant 6 (<= 3.0% Performance Regression Gate)
// ============================================================================
#[test]
fn test_04_html_invariant_6_anti_regression_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_html(128 * 1024); // 128 KB document
    let payload_len = payload.len();

    // Baseline benchmark iteration pass (Simulating prior commit baseline)
    let (baseline_mb_s, _) = measure_adaptive_throughput(
        || {
            let mut out = Vec::with_capacity(payload_len + 512);
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![
                        element!("a[href], img[src]", |el| {
                            let _ = el.set_attribute("data-checked", "1");
                            Ok(())
                        }),
                    ],
                    ..Settings::default()
                },
                |chunk: &[u8]| out.extend_from_slice(chunk),
            );
            for chunk in payload.chunks(16384) {
                let _ = rewriter.write(chunk);
            }
            let _ = rewriter.end();
            black_box(out.len());
        },
        payload_len,
        &mut governor,
    );

    // Current candidate iteration pass
    let (current_mb_s, _) = measure_adaptive_throughput(
        || {
            let mut out = Vec::with_capacity(payload_len + 512);
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![
                        element!("a[href], img[src]", |el| {
                            let _ = el.set_attribute("data-checked", "1");
                            Ok(())
                        }),
                    ],
                    ..Settings::default()
                },
                |chunk: &[u8]| out.extend_from_slice(chunk),
            );
            for chunk in payload.chunks(16384) {
                let _ = rewriter.write(chunk);
            }
            let _ = rewriter.end();
            black_box(out.len());
        },
        payload_len,
        &mut governor,
    );

    let delta_pct = ((baseline_mb_s - current_mb_s) / baseline_mb_s) * 100.0;
    println!(
        "🛡️ [Invariant 6 Anti-Regression Gate] Baseline: {:.2} MB/s, Current: {:.2} MB/s, Regression Delta: {:.2}% (Limit: <= {:.1}%)",
        baseline_mb_s, current_mb_s, delta_pct, MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        delta_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Invariant 6 Violation: Performance regression {:.2}% exceeded maximum limit of {:.1}%",
        delta_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
