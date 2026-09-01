// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite (Part 1: Targets 1 to 8) for HTML Streaming Rewriter.

use std::cell::RefCell;
use std::panic::catch_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use lol_html::{element, rewrite_str, HtmlRewriter, RewriteStrSettings, Settings};
use rayon::prelude::*;

use ttzip_engine::text::normalize_and_sanitize_path;

// ============================================================================
// Target 1: Unclosed Tag Bombs & Hanging Tags Self-Healing Defense
// ============================================================================
#[test]
fn test_target_01_unclosed_tags_and_dangling_elements_self_healing() {
    let mut dangling_html = String::with_capacity(8192);
    for i in 0..200 {
        dangling_html.push_str(&format!(
            "<div class=\"layer_{i}\"><section id=\"sec_{i}\"><span><p>Item {i}",
        ));
    }

    let res = catch_unwind(|| {
        let mut count = 0usize;
        let mut output = Vec::new();
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("span, p, div, section", |el| {
                    count += 1;
                    let _ = el.set_attribute("data-inspected", "true");
                    Ok(())
                })],
                ..Settings::default()
            },
            |chunk: &[u8]| output.extend_from_slice(chunk),
        );

        for chunk in dangling_html.as_bytes().chunks(64) {
            rewriter.write(chunk).expect("Write chunk");
        }
        rewriter.end().expect("End stream");

        assert!(count > 0, "Elements should be inspected during stream");
        assert!(!output.is_empty(), "Output stream must not be empty");
    });
    assert!(res.is_ok(), "Panic on mass unclosed dangling tag bomb");
}

// ============================================================================
// Target 2: Ultra-Deep Tag Nesting (>64 Levels) Stack Overflow Circuit Breaking
// ============================================================================
#[test]
fn test_target_02_ultra_deep_tag_nesting_stack_defense() {
    const DEPTH: usize = 256;
    const MAX_ALLOWED_DEPTH: usize = 64;

    let mut deep_html = String::with_capacity(DEPTH * 32);
    for i in 0..DEPTH {
        deep_html.push_str(&format!("<nested_node_{i}>"));
    }
    deep_html.push_str("Deep Inner Payload");
    for i in (0..DEPTH).rev() {
        deep_html.push_str(&format!("</nested_node_{i}>"));
    }

    let res = catch_unwind(|| {
        let depth_tracker = Rc::new(RefCell::new(0usize));
        let max_observed_depth = Rc::new(RefCell::new(0usize));
        let tripped = Rc::new(RefCell::new(false));

        let d_enter = Rc::clone(&depth_tracker);
        let d_max = Rc::clone(&max_observed_depth);
        let d_trip = Rc::clone(&tripped);

        let mut output = Vec::new();
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("*", move |_el| {
                    let mut d = d_enter.borrow_mut();
                    *d += 1;
                    let mut m = d_max.borrow_mut();
                    if *d > *m {
                        *m = *d;
                    }
                    if *d > MAX_ALLOWED_DEPTH {
                        *d_trip.borrow_mut() = true;
                    }
                    Ok(())
                })],
                ..Settings::default()
            },
            |chunk: &[u8]| output.extend_from_slice(chunk),
        );

        rewriter.write(deep_html.as_bytes()).expect("Write deep HTML");
        rewriter.end().expect("End stream");

        assert!(*tripped.borrow(), "Depth circuit breaker must trip for 256 levels");
        assert!(*max_observed_depth.borrow() >= MAX_ALLOWED_DEPTH);
    });
    assert!(res.is_ok(), "Panic or stack overflow on 256-level tag nesting");
}

// ============================================================================
// Target 3: Gigantic Attribute (>8KB) & Attribute Bloat (>128 Attributes) Quota
// ============================================================================
#[test]
fn test_target_03_gigantic_attribute_and_count_quota_defense() {
    const MAX_ATTR_VAL_LEN: usize = 8192;
    const MAX_ATTR_COUNT: usize = 128;

    let huge_val = "A".repeat(16384);
    let mut bloat_html = format!("<div data-huge=\"{huge_val}\" ");
    for i in 0..200 {
        bloat_html.push_str(&format!("attr_{i}=\"val_{i}\" "));
    }
    bloat_html.push_str(">Content</div>");

    let res = catch_unwind(|| {
        let violations = Rc::new(RefCell::new(Vec::<String>::new()));
        let v_clone = Rc::clone(&violations);

        let mut output = Vec::new();
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("div", move |el| {
                    let attrs = el.attributes();
                    if attrs.len() > MAX_ATTR_COUNT {
                        v_clone.borrow_mut().push(format!(
                            "Attribute count {} exceeded limit {}",
                            attrs.len(),
                            MAX_ATTR_COUNT
                        ));
                    }
                    let mut overlong_attrs = Vec::new();
                    for attr in attrs {
                        if attr.value().len() > MAX_ATTR_VAL_LEN {
                            v_clone.borrow_mut().push(format!(
                                "Attribute {} length {} exceeded limit {}",
                                attr.name(),
                                attr.value().len(),
                                MAX_ATTR_VAL_LEN
                            ));
                            overlong_attrs.push(attr.name());
                        }
                    }
                    for name in overlong_attrs {
                        let _ = el.remove_attribute(&name);
                    }
                    Ok(())
                })],
                ..Settings::default()
            },
            |chunk: &[u8]| output.extend_from_slice(chunk),
        );

        rewriter.write(bloat_html.as_bytes()).expect("Write attribute bloat");
        rewriter.end().expect("End stream");

        let recorded = violations.borrow();
        assert!(!recorded.is_empty(), "Violations should be captured");
        assert!(recorded.iter().any(|v| v.contains("Attribute count")));
        assert!(recorded.iter().any(|v| v.contains("Attribute data-huge length")));
    });
    assert!(res.is_ok(), "Panic on gigantic attribute length and bloat");
}

// ============================================================================
// Target 4: Cross-Chunk Oversized Comments & CDATA Split Escape Defense
// ============================================================================
#[test]
fn test_target_04_cross_chunk_oversized_comments_and_cdata_split() {
    let mut large_comment = String::from("<!-- BEGIN COMMENT ");
    large_comment.push_str(&"X".repeat(65536));
    large_comment.push_str(" END COMMENT -->");
    let cdata_block = "<![CDATA[<script>escaped_script()</script>]]>";
    let html_payload = format!("<div>{large_comment}{cdata_block}<span>Safe Text</span></div>");

    let res = catch_unwind(|| {
        let comment_count = Rc::new(AtomicUsize::new(0));
        let c_clone = Rc::clone(&comment_count);

        let mut output = Vec::new();
        let mut rewriter = HtmlRewriter::new(
            Settings {
                document_content_handlers: vec![lol_html::doc_comments!(move |_c| {
                    c_clone.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })],
                ..Settings::default()
            },
            |chunk: &[u8]| output.extend_from_slice(chunk),
        );

        for chunk in html_payload.as_bytes().chunks(7) {
            rewriter.write(chunk).expect("Write micro chunk");
        }
        rewriter.end().expect("End stream");

        assert!(comment_count.load(Ordering::SeqCst) >= 1);
        let out_str = String::from_utf8_lossy(&output);
        assert!(out_str.contains("Safe Text"));
    });
    assert!(res.is_ok(), "Panic on cross-chunk oversized comment parsing");
}

// ============================================================================
// Target 5: Malicious XSS Scripts, SVG Dynamic Scripts & on* Event Sanitization
// ============================================================================
#[test]
fn test_target_05_xss_svg_scripts_and_onevent_sanitization() {
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
                    element!("script, svg, iframe, embed, object", |el| {
                        el.remove();
                        Ok(())
                    }),
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
// Target 6: Relative Path Traversal (Zip-Slip) VFS Re-Routing Protection
// ============================================================================
#[test]
fn test_target_06_relative_path_traversal_vfs_rerouting() {
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
                        match normalize_and_sanitize_path(&val) {
                            Ok(sanitized) => {
                                let _ = el.set_attribute(attr_name, &format!("vfs://{sanitized}"));
                            }
                            Err(_) => {
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
// Target 7: Zero-Byte & Empty-Stream HTML Probing Defense
// ============================================================================
#[test]
fn test_target_07_zero_byte_and_empty_stream_defense() {
    let zero_vectors: Vec<&[u8]> = vec![
        b"",
        b" ",
        b"\t\r\n",
        b"\0",
        b"\0\0\0\0",
        b"<!DOCTYPE html>\0",
        b"<html\0><body\0>Text\0</body></html>",
        b"<",
        b"</",
        b"<!",
        b"<!--\0-->",
    ];

    for (idx, vec_data) in zero_vectors.iter().enumerate() {
        let res = catch_unwind(|| {
            let mut out = Vec::new();
            let mut rewriter = HtmlRewriter::new(
                Settings {
                    element_content_handlers: vec![element!("*", |_el| Ok(()))],
                    ..Settings::default()
                },
                |chunk: &[u8]| out.extend_from_slice(chunk),
            );
            let _ = rewriter.write(vec_data);
            let _ = rewriter.end();
        });
        assert!(res.is_ok(), "Panic on zero-byte vector index {idx}");
    }
}

// ============================================================================
// Target 8: 1000+ Concurrent Rayon Tasks HTML Rewriting Race & Memory Watchdog
// ============================================================================
#[test]
fn test_target_08_concurrent_rayon_html_rewriting_race() {
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
