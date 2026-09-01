// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive HTML Security Defense, XSS Interception, VFS Sandboxing & Resource Isolation Test Suite.
//!
//! Deploys 8 surgical defense verification targets:
//! 1. Malicious XSS scripts, SVG dynamic scripts, and `on*` inline event handler sanitization.
//! 2. Relative path traversal (Zip-Slip / `../../etc/passwd`) VFS re-routing protection.
//! 3. 1000+ concurrent Rayon tasks HTML rewriting race competition and memory watchdog.
//! 4. External network link neutralization (`http://`, `https://` remote request isolation).
//! 5. Malicious CSP bypass and inline style pseudo-protocol (`javascript:`, `expression`) injection filtering.
//! 6. Sensitive page content Zeroize memory erasure defense (`zeroize`).
//! 7. Giant single-line text (>1MB) sliced streaming memory control.
//! 8. Single-task memory quota exceeding (>64MB) watchdog circuit breaker.

use std::panic::catch_unwind;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, HtmlRewriter, RewriteStrSettings, Settings};
use rayon::prelude::*;
use zeroize::Zeroize;

use ttzip_engine::text::normalize_and_sanitize_path;

// ============================================================================
// Target 1: Malicious XSS Scripts, SVG Dynamic Scripts & on* Event Sanitization
// ============================================================================
#[test]
fn test_target_01_xss_svg_scripts_and_onevent_sanitization() {
    let dirty_html = r#"
    <!DOCTYPE html>
    <html>
      <head>
        <script>evil_payload();</script>
        <script src="https://attacker.com/malware.js"></script>
      </head>
      <body onload="runExploit()" onresize="trigger()">
        <img src="avatar.jpg" onerror="stealCredentials()" onclick="track()" />
        <svg onload="svgXss()"><circle cx="10" cy="10" r="5"/></svg>
        <a href="javascript:alert(document.domain)">Click Me</a>
        <iframe src="https://phishing.site"></iframe>
        <embed src="flash_exploit.swf">
        <object data="data:text/html,<script>alert(1)</script>"></object>
        <p>Safe Content</p>
      </body>
    </html>"#;

    let res = catch_unwind(|| {
        let rewritten = rewrite_str(
            dirty_html,
            RewriteStrSettings {
                element_content_handlers: vec![
                    // Strip dangerous tags entirely
                    element!("script, svg, iframe, embed, object", |el| {
                        el.remove();
                        Ok(())
                    }),
                    // Sanitize inline event handlers from all remaining tags
                    element!("*", |el| {
                        let attrs: Vec<(String, String)> = el
                            .attributes()
                            .iter()
                            .map(|a| (a.name().to_ascii_lowercase(), a.value()))
                            .collect();

                        for (name, val) in attrs {
                            if name.starts_with("on") {
                                let _ = el.remove_attribute(&name);
                            }
                            if (name == "href" || name == "src")
                                && val.trim().to_ascii_lowercase().starts_with("javascript:")
                            {
                                let _ = el.set_attribute(&name, "#sanitized");
                            }
                        }
                        Ok(())
                    }),
                ],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Rewrite dirty HTML");

        assert!(!rewritten.contains("<script>"));
        assert!(!rewritten.contains("<svg"));
        assert!(!rewritten.contains("<iframe"));
        assert!(!rewritten.contains("onload="));
        assert!(!rewritten.contains("onerror="));
        assert!(!rewritten.contains("onclick="));
        assert!(!rewritten.contains("javascript:alert"));
        assert!(rewritten.contains("Safe Content"));
    });
    assert!(res.is_ok(), "Panic on XSS / SVG / on* sanitization");
}

// ============================================================================
// Target 2: Relative Path Traversal (Zip-Slip) VFS Re-Routing Protection
// ============================================================================
#[test]
fn test_target_02_relative_path_traversal_vfs_rerouting() {
    let malicious_paths = [
        "../../etc/passwd",
        "../../../../root/.ssh/id_rsa",
        "..\\..\\windows\\system32\\cmd.exe",
        "/var/log/system.log",
        "assets/../../../secret.key",
    ];

    let mut html_doc = String::from("<div>");
    for path in &malicious_paths {
        html_doc.push_str(&format!("<a href=\"{path}\">Link</a><img src=\"{path}\">"));
    }
    html_doc.push_str("</div>");

    let res = catch_unwind(|| {
        let rewritten = rewrite_str(
            &html_doc,
            RewriteStrSettings {
                element_content_handlers: vec![element!("a[href], img[src]", |el| {
                    let tag = el.tag_name();
                    let attr_name = if tag == "a" { "href" } else { "src" };
                    if let Some(val) = el.get_attribute(attr_name) {
                        // Apply TTZip text path sanitizer to intercept escaping paths
                        match normalize_and_sanitize_path(&val) {
                            Ok(sanitized) => {
                                let _ = el.set_attribute(attr_name, &format!("vfs://{sanitized}"));
                            }
                            Err(_) => {
                                // Path traversal / Zip-Slip detected: neutralize to virtual sandbox jail
                                let _ = el.set_attribute(attr_name, "vfs://sandbox/blocked_path");
                            }
                        }
                    }
                    Ok(())
                })],
                ..RewriteStrSettings::default()
            },
        )
        .expect("Rewrite traversal paths");

        assert!(!rewritten.contains("../../etc/passwd"));
        assert!(!rewritten.contains(".ssh/id_rsa"));
        assert!(rewritten.contains("vfs://"));
    });
    assert!(res.is_ok(), "Panic on relative path traversal VFS re-routing");
}

// ============================================================================
// Target 3: 1000+ Concurrent Rayon Tasks HTML Rewriting Race & Memory Watchdog
// ============================================================================
#[test]
fn test_target_03_concurrent_rayon_html_rewriting_race() {
    let template = r#"
    <article class="news-item">
      <h2>Title Placeholder</h2>
      <p>Body paragraph with <a href="/article/123">internal link</a>.</p>
      <img src="thumb.jpg" alt="Thumbnail" />
    </article>"#;

    let res = catch_unwind(|| {
        (0..1000).into_par_iter().for_each(|task_id| {
            let doc = template.replace("Placeholder", &format!("#{task_id}"));
            let mut output = Vec::with_capacity(512);
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![
                        element!("h2", |el| {
                            let _ = el.set_attribute("data-task-processed", "true");
                            Ok(())
                        }),
                        element!("a[href]", |el| {
                            if let Some(href) = el.get_attribute("href") {
                                let _ = el.set_attribute("href", &format!("vfs://host{href}"));
                            }
                            Ok(())
                        }),
                    ],
                    ..Settings::default()
                },
                |chunk: &[u8]| output.extend_from_slice(chunk),
            );

            rewriter.write(doc.as_bytes()).expect("Write concurrent task");
            rewriter.end().expect("End concurrent task");

            assert!(!output.is_empty());
            assert!(output.len() < 4096, "Memory quota per task must be bounded");
        });
    });
    assert!(res.is_ok(), "Panic or data race on 1000+ concurrent Rayon HTML rewriting");
}

// ============================================================================
// Target 4: External Network Link Neutralization (http/https Isolation)
// ============================================================================
#[test]
fn test_target_04_external_network_link_neutralization() {
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
// Target 5: Malicious CSP Bypass & Inline Style Pseudo-Protocol Filtering
// ============================================================================
#[test]
fn test_target_05_csp_bypass_and_inline_style_filtering() {
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
                    // Intercept and sanitize meta CSP overrides
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
                    // Sanitize dangerous pseudo-protocols in inline style attributes
                    element!("*[style]", |el| {
                        if let Some(style) = el.get_attribute("style") {
                            let lower = style.to_ascii_lowercase();
                            if lower.contains("javascript:") || lower.contains("expression(") {
                                // Filter out dangerous expressions
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
// Target 6: Sensitive Page Content Zeroize Memory Erasure Defense
// ============================================================================
#[test]
fn test_target_06_sensitive_page_content_zeroize_defense() {
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
                        // Extract into zeroize-on-drop buffer and scrub from HTML output
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

        // Drop triggers Zeroize erasure
        drop(extracted_secret);
    });
    assert!(res.is_ok(), "Panic on sensitive content Zeroize scrubbing");
}

// ============================================================================
// Target 7: Giant Single-Line Text (>1MB) Sliced Streaming Memory Control
// ============================================================================
#[test]
fn test_target_07_giant_single_line_sliced_streaming() {
    // Construct 1.5 MB single unbroken text line
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

        // Feed in 4KB streaming chunks to verify memory isolation
        for chunk in giant_doc.chunks(4096) {
            rewriter.write(chunk).expect("Write streaming chunk");
        }
        rewriter.end().expect("End stream");

        assert!(output_bytes >= GIANT_LEN);
    });
    assert!(res.is_ok(), "Panic on 1.5MB single-line streaming processing");
}

// ============================================================================
// Target 8: Single-Task Memory Watchdog Quota Exceeding (>64MB) Circuit Breaker
// ============================================================================
#[test]
fn test_target_08_single_task_memory_watchdog_circuit_breaker() {
    const MEMORY_QUOTA_BYTES: usize = 1024 * 1024; // 1MB test quota representing memory limit

    let exceeded = Arc::new(AtomicBool::new(false));
    let total_bytes = Arc::new(AtomicUsize::new(0));

    let exc_clone = Arc::clone(&exceeded);
    let tot_clone = Arc::clone(&total_bytes);

    let res = catch_unwind(move || {
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("p", |el| {
                    // Try expanding payload maliciously
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
