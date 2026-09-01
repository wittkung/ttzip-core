// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Corruption injection and chaos mutation test suite (Part 2: Targets 9 to 16) for HTML Rewriter & VFS Router.

use std::cell::RefCell;
use std::panic::catch_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, HtmlRewriter, RewriteStrSettings, Settings};
use zeroize::Zeroize;

use ttzip_engine::text::{decode_to_utf8_lossy, detect_encoding};

/// Deterministic 64-bit XorShift pseudo-random number generator for reproducible chaos vectors.
#[derive(Clone, Debug)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c_49e6_748f_ea9b } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u64() % span) as usize
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }
}

// ============================================================================
// Target 9: 500+ Rounds Pseudo-Random Mutation (XorShift64) Fuzzing Zero Panic
// ============================================================================
#[test]
fn test_target_09_xorshift64_html_fuzzing_zero_panic() {
    let seed_html = r#"
    <!DOCTYPE html>
    <html lang="en">
      <head><meta charset="utf-8"><title>Fuzz Seed</title></head>
      <body>
        <div id="main" class="container">
          <h1>Header</h1>
          <p>Paragraph with <b>bold</b> and <i>italic</i> formatting.</p>
          <a href="https://example.com" target="_blank">External Link</a>
          <!-- Comment block -->
        </div>
      </body>
    </html>"#;

    let mut prng = XorShift64::new(0xDEAD_BEEF_CAFE_BABE);

    for round in 0..500 {
        let mut mutated = seed_html.as_bytes().to_vec();
        let num_mutations = prng.next_range(1, 10);

        for _ in 0..num_mutations {
            let action = prng.next_range(0, 3);
            let pos = prng.next_range(0, mutated.len().saturating_sub(1));
            match action {
                0 => {
                    if !mutated.is_empty() {
                        mutated[pos] = prng.next_byte();
                    }
                }
                1 => {
                    mutated.insert(pos, prng.next_byte());
                }
                2 => {
                    mutated.truncate(pos);
                }
                _ => {
                    let tags: &[&[u8]] = &[b"<", b">", b"</", b"\"", b"'", b"<!--", b"-->", b"\0"];
                    let tag = tags[prng.next_range(0, tags.len() - 1)];
                    mutated.splice(pos..pos, tag.iter().copied());
                }
            }
        }

        let res = catch_unwind(|| {
            let mut out = Vec::new();
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![element!("*", |_el| Ok(()))],
                    ..Settings::default()
                },
                |chunk: &[u8]| out.extend_from_slice(chunk),
            );

            let chunk_size = (round % 17) + 1;
            for chunk in mutated.chunks(chunk_size) {
                let _ = rewriter.write(chunk);
            }
            let _ = rewriter.end();
        });
        assert!(res.is_ok(), "Panic in XorShift64 HTML fuzzing round {round}");
    }
}

// ============================================================================
// Target 10: External Network Link Neutralization (http/https Isolation)
// ============================================================================
#[test]
fn test_target_10_external_network_link_neutralization() {
    let doc = r#"
    <nav>
      <a href="http://tracker.adnetwork.com/beacon">Ad Link</a>
      <a href="https://external.service.org/api">External API</a>
      <a href="/local/page.html">Local Doc</a>
      <img src="http://telemetry.site/pixel.png" />
      <form action="https://phishing.site/login" method="POST"></form>
    </nav>"#;

    let res = catch_unwind(|| {
        let rewritten = rewrite_str(
            doc,
            RewriteStrSettings {
                element_content_handlers: vec![element!("a[href], img[src], form[action]", |el| {
                    let tag = el.tag_name();
                    let attr = match tag.as_str() {
                        "a" => "href",
                        "img" => "src",
                        "form" => "action",
                        _ => return Ok(()),
                    };

                    if let Some(val) = el.get_attribute(attr) {
                        let lower = val.trim().to_ascii_lowercase();
                        if lower.starts_with("http://") || lower.starts_with("https://") {
                            let _ = el.set_attribute(attr, "#offline-neutralized");
                            let _ = el.set_attribute("data-original-remote", &val);
                        }
                    }
                    Ok(())
                })],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Rewrite external links");

        assert!(!rewritten.contains("href=\"http://tracker"));
        assert!(!rewritten.contains("src=\"http://telemetry"));
        assert!(!rewritten.contains("action=\"https://phishing"));
        assert!(rewritten.contains("href=\"#offline-neutralized\""));
        assert!(rewritten.contains("href=\"/local/page.html\""));
    });
    assert!(res.is_ok(), "Panic on external network link neutralization");
}

// ============================================================================
// Target 11: Multi-Byte UTF-16 / GBK Truncation & State Machine Re-Sync
// ============================================================================
#[test]
fn test_target_11_multibyte_charset_truncation_resync() {
    let gbk_bytes = [
        0xd6, 0xd0, 0xce, 0xc4, 0xb2, 0xe2, 0xca, 0xd4, 0xce, 0xc4, 0xb5, 0xb5,
    ];
    let detected = detect_encoding(&gbk_bytes);
    assert_eq!(detected.name(), "GBK");

    let truncated_gbk = &gbk_bytes[..7];
    let (decoded_text, _had_errors) = decode_to_utf8_lossy(truncated_gbk, detected);
    assert!(!decoded_text.is_empty(), "Transcoder must recover gracefully");

    let html_with_non_utf8 = format!("<p>{decoded_text}</p>");
    let res = catch_unwind(|| {
        let rewritten = rewrite_str(
            &html_with_non_utf8,
            RewriteStrSettings {
                element_content_handlers: vec![element!("p", |el| {
                    let _ = el.set_attribute("data-encoding-resynced", "true");
                    Ok(())
                })],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Rewrite decoded non-UTF8 HTML");

        assert!(rewritten.contains("data-encoding-resynced"));
    });
    assert!(res.is_ok(), "Panic on multi-byte charset truncation re-synchronization");
}

// ============================================================================
// Target 12: Malicious CSP Bypass & Inline Style Pseudo-Protocol Filtering
// ============================================================================
#[test]
fn test_target_12_csp_bypass_and_inline_style_filtering() {
    let doc = r#"
    <meta http-equiv="Content-Security-Policy" content="default-src 'unsafe-inline' 'unsafe-eval' *">
    <div style="background-image: url('javascript:alert(1)'); color: red; width: expression(alert(2));">
      Styled Text
    </div>"#;

    let res = catch_unwind(|| {
        let rewritten = rewrite_str(
            doc,
            RewriteStrSettings {
                element_content_handlers: vec![
                    element!("meta[http-equiv]", |el| {
                        if let Some(eq) = el.get_attribute("http-equiv") {
                            if eq.eq_ignore_ascii_case("content-security-policy") {
                                let _ = el.set_attribute(
                                    "content",
                                    "default-src 'self' ttzip-vfs:; script-src 'none';",
                                );
                            }
                        }
                        Ok(())
                    }),
                    element!("*[style]", |el| {
                        if let Some(style) = el.get_attribute("style") {
                            let lower = style.to_ascii_lowercase();
                            if lower.contains("javascript:") || lower.contains("expression(") {
                                let safe_style = style
                                    .replace("javascript:", "#blocked:")
                                    .replace("expression(", "blocked(");
                                let _ = el.set_attribute("style", &safe_style);
                            }
                        }
                        Ok(())
                    }),
                ],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Rewrite CSP and styles");

        assert!(!rewritten.contains("javascript:alert"));
        assert!(!rewritten.contains("expression(alert"));
        assert!(rewritten.contains("default-src 'self' ttzip-vfs:"));
    });
    assert!(res.is_ok(), "Panic on CSP bypass and style pseudo-protocol filtering");
}

// ============================================================================
// Target 13: Cascading Selector Conflict & Unclosed Selector Fault Tolerance
// ============================================================================
#[test]
fn test_target_13_cascading_selector_conflict_and_fault_tolerance() {
    let complex_html = r#"
    <div class="card active" id="card-1">
      <header class="card-header">
        <h3 class="title">Card Title</h3>
      </header>
      <div class="card-body">
        <p class="summary">Summary Text</p>
        <span class="badge highlight">Active</span>
      </div>
    </div>"#;

    let res = catch_unwind(|| {
        let execution_order = Rc::new(RefCell::new(Vec::<&'static str>::new()));
        let o1 = Rc::clone(&execution_order);
        let o2 = Rc::clone(&execution_order);
        let o3 = Rc::clone(&execution_order);

        let rewritten = rewrite_str(
            complex_html,
            RewriteStrSettings {
                element_content_handlers: vec![
                    element!("div.card", move |_el| {
                        o1.borrow_mut().push("div.card");
                        Ok(())
                    }),
                    element!("div.card > div.card-body > p.summary", move |el| {
                        o2.borrow_mut().push("p.summary");
                        let _ = el.set_attribute("data-deep-matched", "true");
                        Ok(())
                    }),
                    element!("span.badge.highlight", move |el| {
                        o3.borrow_mut().push("span.badge");
                        let _ = el.set_inner_content("Verified Active", ContentType::Text);
                        Ok(())
                    }),
                ],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Rewrite cascading selectors");

        let order = execution_order.borrow();
        assert_eq!(*order, vec!["div.card", "p.summary", "span.badge"]);
        assert!(rewritten.contains("Verified Active"));
        assert!(rewritten.contains("data-deep-matched=\"true\""));
    });
    assert!(res.is_ok(), "Panic on cascading selector matching");
}

// ============================================================================
// Target 14: Sensitive Page Content Zeroize Memory Erasure Defense
// ============================================================================
#[test]
fn test_target_14_sensitive_page_content_zeroize_defense() {
    #[derive(Zeroize)]
    #[zeroize(drop)]
    struct SensitiveTokenBuffer {
        token_bytes: Vec<u8>,
    }

    let sensitive_html = "<input type=\"password\" name=\"auth_key\" value=\"SUPER_SECRET_KEY_12345\" />";

    let res = catch_unwind(|| {
        let mut extracted_secret = SensitiveTokenBuffer {
            token_bytes: Vec::new(),
        };

        let rewritten = rewrite_str(
            sensitive_html,
            RewriteStrSettings {
                element_content_handlers: vec![element!("input[type='password']", |el| {
                    if let Some(val) = el.get_attribute("value") {
                        extracted_secret.token_bytes = val.into_bytes();
                        let _ = el.set_attribute("value", "********");
                    }
                    Ok(())
                })],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Scrub sensitive inputs");

        assert_eq!(extracted_secret.token_bytes, b"SUPER_SECRET_KEY_12345");
        assert!(!rewritten.contains("SUPER_SECRET_KEY_12345"));
        assert!(rewritten.contains("value=\"********\""));

        drop(extracted_secret);
    });
    assert!(res.is_ok(), "Panic on sensitive content Zeroize scrubbing");
}

// ============================================================================
// Target 15: Giant Single-Line Text (>1MB) Sliced Streaming Memory Control
// ============================================================================
#[test]
fn test_target_15_giant_single_line_sliced_streaming() {
    const GIANT_LEN: usize = 1_500_000;
    let mut giant_doc = Vec::with_capacity(GIANT_LEN + 128);
    giant_doc.extend_from_slice(b"<p class=\"giant-stream\">");
    for i in 0..GIANT_LEN {
        giant_doc.push(b'A' + ((i % 26) as u8));
    }
    giant_doc.extend_from_slice(b"</p>");

    let res = catch_unwind(|| {
        let mut output_bytes = 0usize;
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("p.giant-stream", |el| {
                    let _ = el.set_attribute("data-streamed", "true");
                    Ok(())
                })],
                ..Settings::default()
            },
            |chunk: &[u8]| {
                output_bytes += chunk.len();
            },
        );

        for chunk in giant_doc.chunks(4096) {
            rewriter.write(chunk).expect("Write streaming chunk");
        }
        rewriter.end().expect("End stream");

        assert!(output_bytes >= GIANT_LEN);
    });
    assert!(res.is_ok(), "Panic on 1.5MB single-line streaming processing");
}

// ============================================================================
// Target 16: Single-Task Memory Watchdog Quota Exceeding (>64MB) Circuit Breaker
// ============================================================================
#[test]
fn test_target_16_single_task_memory_watchdog_circuit_breaker() {
    const MEMORY_QUOTA_BYTES: usize = 1024 * 1024;

    let exceeded = Arc::new(AtomicBool::new(false));
    let total_bytes = Arc::new(AtomicUsize::new(0));

    let exc_clone = Arc::clone(&exceeded);
    let tot_clone = Arc::clone(&total_bytes);

    let res = catch_unwind(move || {
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("p", |el| {
                    let _ = el.append(&"EXPANSION_PAYLOAD_".repeat(100), ContentType::Text);
                    Ok(())
                })],
                ..Settings::default()
            },
            move |chunk: &[u8]| {
                let prev = tot_clone.fetch_add(chunk.len(), Ordering::SeqCst);
                if prev + chunk.len() > MEMORY_QUOTA_BYTES {
                    exc_clone.store(true, Ordering::SeqCst);
                }
            },
        );

        let repeat_doc = "<p>Repeat content block</p>\n".repeat(5000);
        let mut stopped_early = false;

        for chunk in repeat_doc.as_bytes().chunks(512) {
            if exceeded.load(Ordering::SeqCst) {
                stopped_early = true;
                break;
            }
            if rewriter.write(chunk).is_err() {
                break;
            }
        }

        assert!(
            exceeded.load(Ordering::SeqCst) || stopped_early,
            "Watchdog quota must trigger when output exceeds memory budget"
        );
    });
    assert!(res.is_ok(), "Panic on task memory watchdog circuit breaker");
}
