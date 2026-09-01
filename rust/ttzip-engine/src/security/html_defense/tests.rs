// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;

#[test]
fn test_sensitive_html_buffer_zeroize() {
    let mut buf = SensitiveHtmlBuffer::from_str("<h1>Confidential Payload</h1>");
    assert_eq!(buf.as_str().unwrap(), "<h1>Confidential Payload</h1>");
    assert!(!buf.is_empty());
    assert_eq!(buf.len(), 29);

    buf.clear_and_zeroize();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
}

#[test]
fn test_sanitizer_strips_script_and_events() {
    let pipeline = HtmlSecurityPipeline::default();
    let raw = r#"<html><head><script>alert('XSS')</script></head><body><div onclick="bad()">Click</div><a href="javascript:steal()">Link</a></body></html>"#;
    let res = pipeline.sanitize_html(raw).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(!out.contains("<script"));
    assert!(!out.contains("alert("));
    assert!(!out.contains("onclick"));
    assert!(!out.contains("javascript:"));
    assert!(res.report.sanitizer.stripped_tags_count > 0);
    assert!(res.report.sanitizer.stripped_events_count > 0);
    assert!(res.report.sanitizer.neutralized_protocols_count > 0);
}

#[test]
fn test_network_sandbox_neutralizes_external_urls() {
    let pipeline = HtmlSecurityPipeline::default();
    let raw = r#"<div><a href="https://evil.com/leak">Leak</a><img src="http://tracker.com/pixel.png"><img src="./local.png"></div>"#;
    let res = pipeline.sanitize_html(raw).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(!out.contains("https://evil.com/leak"));
    assert!(!out.contains("http://tracker.com/pixel.png"));
    assert!(out.contains("ttzip-vfs://local.png"));
    assert!(out.contains("Content-Security-Policy"));
    assert_eq!(res.report.network_sandbox.neutralized_external_links_count, 2);
    assert_eq!(res.report.network_sandbox.rewritten_vfs_links_count, 1);
}

#[test]
fn test_tag_depth_limit_enforced() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        max_depth: 4,
        ..HtmlDefenseOptions::default()
    });

    let deep_html = "<div><div><div><div><div><span>Deep</span></div></div></div></div></div>";
    let err = pipeline.sanitize_html(deep_html).unwrap_err();
    match err {
        HtmlDefenseError::TagDepthLimitExceeded { depth, max_depth } => {
            assert_eq!(depth, 5);
            assert_eq!(max_depth, 4);
        }
        other => panic!("Expected TagDepthLimitExceeded, got {other:?}"),
    }
}

#[test]
fn test_attribute_quota_limits_enforced() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        max_attributes_per_element: 3,
        ..HtmlDefenseOptions::default()
    });

    let many_attrs = r#"<div a="1" b="2" c="3" d="4">Content</div>"#;
    let err = pipeline.sanitize_html(many_attrs).unwrap_err();
    match err {
        HtmlDefenseError::AttributeCountExceeded { count, max } => {
            assert_eq!(count, 4);
            assert_eq!(max, 3);
        }
        other => panic!("Expected AttributeCountExceeded, got {other:?}"),
    }
}

#[test]
fn test_memory_truncation_threshold() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        memory_truncation_threshold: 64,
        ..HtmlDefenseOptions::default()
    });

    let long_html = format!("<div>{}</div>", "A".repeat(128));
    let res = pipeline.sanitize_html(&long_html).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(res.report.was_truncated);
    assert!(out.contains("ttzip-security-truncated-banner"));
}
