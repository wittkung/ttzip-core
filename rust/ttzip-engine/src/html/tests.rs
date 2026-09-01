// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for HTML format detection, streaming rewriting,
//! CSS3 selector engine, VFS resource routing, and security policy sanitization.

use crate::html::*;
use std::collections::HashMap;

#[test]
fn test_html_format_detection() {
    let html5_sample = b"<!DOCTYPE html><html><head><title>Test</title></head><body>Hello</body></html>";
    assert_eq!(HtmlFormat::detect(html5_sample), HtmlFormat::Html5);

    let xhtml_sample = b"<?xml version=\"1.0\" encoding=\"utf-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>Doc</title></head><body>Content</body></html>";
    assert_eq!(HtmlFormat::detect(xhtml_sample), HtmlFormat::XHtml);

    let fragment_sample = b"<div class=\"chapter\"><h1>Title</h1><p>Paragraph</p></div>";
    assert_eq!(HtmlFormat::detect(fragment_sample), HtmlFormat::Fragment);

    let unknown_sample = b"\x00\x01\x02\x03\x04\x05binarypayload";
    assert_eq!(HtmlFormat::detect(unknown_sample), HtmlFormat::Unknown);

    assert_eq!(HtmlFormat::detect(b""), HtmlFormat::Unknown);
}

#[test]
fn test_vfs_path_normalization_rfc3986() {
    // Current directory resolution
    assert_eq!(
        normalize_rfc3986_path("pages/chapter1", "./images/fig1.png"),
        "pages/chapter1/images/fig1.png"
    );

    // Parent directory traversal
    assert_eq!(
        normalize_rfc3986_path("pages/chapter1", "../shared/style.css"),
        "pages/shared/style.css"
    );

    // Double parent directory traversal
    assert_eq!(
        normalize_rfc3986_path("ops/xhtml/sub", "../../assets/logo.svg"),
        "ops/assets/logo.svg"
    );

    // Root-relative path
    assert_eq!(
        normalize_rfc3986_path("pages/chapter1", "/root_assets/main.js"),
        "root_assets/main.js"
    );

    // Empty base directory
    assert_eq!(
        normalize_rfc3986_path("", "images/pic.jpg"),
        "images/pic.jpg"
    );

    // Windows backslash path normalization
    assert_eq!(
        normalize_rfc3986_path(r"pages\chapter1", r"..\css\theme.css"),
        "pages/css/theme.css"
    );
}

#[test]
fn test_vfs_path_traversal_prevention() {
    // Attempting to escape above archive root must be clamped and sanitized
    assert_eq!(
        normalize_rfc3986_path("pages", "../../../../etc/passwd"),
        "etc/passwd"
    );

    assert_eq!(
        normalize_rfc3986_path("", "../../secret.key"),
        "secret.key"
    );
}

#[test]
fn test_extract_parent_directory() {
    assert_eq!(extract_parent_directory("index.html"), "");
    assert_eq!(extract_parent_directory("pages/chapter1/index.html"), "pages/chapter1");
    assert_eq!(extract_parent_directory("/OPS/xhtml/c1.xhtml"), "OPS/xhtml");
    assert_eq!(extract_parent_directory(r"assets\css\main.css"), "assets/css");
}

#[test]
fn test_vfs_router_single_url_routing() {
    let router = HtmlVfsResourceRouter::new("arc-1001", "books/ch1/index.html");
    assert_eq!(router.archive_id(), "arc-1001");
    assert_eq!(router.base_dir(), "books/ch1");

    // Relative image link
    let routed = router.route_url("images/pic.png");
    assert_eq!(
        routed.as_deref(),
        Some("ttzip-vfs://arc-1001/books/ch1/images/pic.png")
    );

    // Relative parent traversal
    let routed_parent = router.route_url("../styles/main.css");
    assert_eq!(
        routed_parent.as_deref(),
        Some("ttzip-vfs://arc-1001/books/styles/main.css")
    );

    // Query and fragment preserved
    let routed_query = router.route_url("images/pic.png?v=2#details");
    assert_eq!(
        routed_query.as_deref(),
        Some("ttzip-vfs://arc-1001/books/ch1/images/pic.png?v=2#details")
    );

    // External URLs must NOT be rewritten
    assert_eq!(router.route_url("https://example.com/logo.png"), None);
    assert_eq!(router.route_url("http://example.com/style.css"), None);
    assert_eq!(router.route_url("data:image/png;base64,iVBORw0KGgo="), None);
    assert_eq!(router.route_url("#chapter2"), None);
    assert_eq!(router.route_url("//cdn.example.com/lib.js"), None);
}

#[test]
fn test_vfs_router_srcset_handling() {
    let router = HtmlVfsResourceRouter::new("my-archive", "html/sub/page.html");
    let srcset = "img-small.jpg 320w, ../shared/img-medium.jpg 640w, /root-img.jpg 1024w";

    let rewritten = router.route_srcset(srcset);
    assert_eq!(
        rewritten,
        "ttzip-vfs://my-archive/html/sub/img-small.jpg 320w, ttzip-vfs://my-archive/html/shared/img-medium.jpg 640w, ttzip-vfs://my-archive/root-img.jpg 1024w"
    );
}

#[test]
fn test_css_selector_engine_matching() {
    let mut engine = HtmlSelectorEngine::new();
    engine.register("img[src]", 1).unwrap();
    engine.register("link[rel='stylesheet']", 2).unwrap();
    engine.register("div.container#main", 3).unwrap();
    engine.register("a[href^='https://']", 4).unwrap();
    engine.register("span.badge.active", 5).unwrap();
    engine.register("p[data-role*='lead']", 6).unwrap();

    assert_eq!(engine.rule_count(), 6);

    // 1. img[src]
    let mut attrs = HashMap::new();
    attrs.insert("src", "photo.jpg");
    assert_eq!(engine.evaluate("img", |k| attrs.get(k).copied()), vec![1]);

    // 2. link[rel='stylesheet']
    attrs.clear();
    attrs.insert("rel", "stylesheet");
    attrs.insert("href", "style.css");
    assert_eq!(engine.evaluate("link", |k| attrs.get(k).copied()), vec![2]);

    // 3. div.container#main
    attrs.clear();
    attrs.insert("class", "container fluid");
    attrs.insert("id", "main");
    assert_eq!(engine.evaluate("div", |k| attrs.get(k).copied()), vec![3]);

    // 4. a[href^='https://']
    attrs.clear();
    attrs.insert("href", "https://example.com/guide");
    assert_eq!(engine.evaluate("a", |k| attrs.get(k).copied()), vec![4]);

    // 5. span.badge.active
    attrs.clear();
    attrs.insert("class", "active rounded badge");
    assert_eq!(engine.evaluate("span", |k| attrs.get(k).copied()), vec![5]);

    // 6. p[data-role*='lead']
    attrs.clear();
    attrs.insert("data-role", "article-lead-text");
    assert_eq!(engine.evaluate("p", |k| attrs.get(k).copied()), vec![6]);

    // Non-matching element
    attrs.clear();
    assert_eq!(engine.evaluate("div", |k| attrs.get(k).copied()), Vec::<usize>::new());
}

#[test]
fn test_compiled_selector_alternatives() {
    let sel = CompiledSelector::parse("img[src], script[src], link[href]").unwrap();
    assert_eq!(sel.branches().len(), 3);

    let img_attrs = [("src".to_string(), "a.png".to_string())];
    assert!(sel.matches_attributes("img", &img_attrs));

    let script_attrs = [("src".to_string(), "app.js".to_string())];
    assert!(sel.matches_attributes("script", &script_attrs));

    let link_attrs = [("href".to_string(), "site.css".to_string())];
    assert!(sel.matches_attributes("link", &link_attrs));

    let div_attrs = [("class".to_string(), "box".to_string())];
    assert!(!sel.matches_attributes("div", &div_attrs));
}

#[test]
fn test_html_streaming_rewriter_resources() {
    let html_input = br#"
<!DOCTYPE html>
<html>
<head>
    <title>Archive Document</title>
    <link rel="stylesheet" href="../css/app.css">
    <script src="./js/bundle.js"></script>
</head>
<body>
    <img src="images/banner.png" alt="Banner">
    <video poster="./media/thumb.jpg" src="./media/clip.mp4"></video>
</body>
</html>
"#;

    let (output_bytes, stats) = TTZipHtmlRewriter::rewrite_all(
        html_input,
        "archive-99",
        "docs/sub/index.html",
        HtmlSanitizationPolicy::Permissive,
    )
    .unwrap();

    let output_str = String::from_utf8(output_bytes).unwrap();

    assert!(output_str.contains("ttzip-vfs://archive-99/docs/css/app.css"));
    assert!(output_str.contains("ttzip-vfs://archive-99/docs/sub/js/bundle.js"));
    assert!(output_str.contains("ttzip-vfs://archive-99/docs/sub/images/banner.png"));
    assert!(output_str.contains("ttzip-vfs://archive-99/docs/sub/media/thumb.jpg"));
    assert!(output_str.contains("ttzip-vfs://archive-99/docs/sub/media/clip.mp4"));

    assert!(stats.resources_routed >= 5);
    assert_eq!(stats.scripts_stripped, 0);
}

#[test]
fn test_html_sanitization_strict_policy() {
    let dirty_html = br#"
<!DOCTYPE html>
<html>
<head>
    <style>body { background: red; }</style>
    <script>alert('pwned');</script>
</head>
<body onload="runPayload()">
    <h1 style="color: blue;">Title</h1>
    <a href="javascript:void(0)" onclick="evil()">Click Me</a>
    <iframe src="http://attacker.com/embed"></iframe>
    <img src="logo.png" onerror="alert(1)">
</body>
</html>
"#;

    let (output_bytes, stats) = TTZipHtmlRewriter::rewrite_all(
        dirty_html,
        "vault-01",
        "index.html",
        HtmlSanitizationPolicy::Strict,
    )
    .unwrap();

    let clean_str = String::from_utf8(output_bytes).unwrap();

    // Scripts, style tags, inline event attributes, and dangerous links must be stripped
    assert!(!clean_str.contains("<script"));
    assert!(!clean_str.contains("alert('pwned')"));
    assert!(!clean_str.contains("<style"));
    assert!(!clean_str.contains("<iframe"));
    assert!(!clean_str.contains("onload"));
    assert!(!clean_str.contains("onclick"));
    assert!(!clean_str.contains("onerror"));
    assert!(!clean_str.contains("javascript:"));
    assert!(!clean_str.contains("style=\"color: blue;\""));

    // Relative image src must still be rewritten to VFS
    assert!(clean_str.contains("ttzip-vfs://vault-01/logo.png"));

    assert!(stats.scripts_stripped > 0);
    assert!(stats.iframes_stripped > 0);
}

#[test]
fn test_html_sanitization_allow_inline_styles() {
    let html_input = br#"
<html>
<head>
    <style>.title { font-size: 20px; }</style>
    <script>evil();</script>
</head>
<body style="margin: 0;" onclick="bad()">
    <div style="padding: 10px;">Content</div>
</body>
</html>
"#;

    let (output_bytes, stats) = TTZipHtmlRewriter::rewrite_all(
        html_input,
        "vault-02",
        "index.html",
        HtmlSanitizationPolicy::AllowInlineStyles,
    )
    .unwrap();

    let output_str = String::from_utf8(output_bytes).unwrap();

    // Scripts and onclick must be stripped
    assert!(!output_str.contains("<script"));
    assert!(!output_str.contains("evil()"));
    assert!(!output_str.contains("onclick"));

    // Inline styles and style blocks must be preserved
    assert!(output_str.contains("<style>"));
    assert!(output_str.contains(".title { font-size: 20px; }"));
    assert!(output_str.contains("style=\"margin: 0;\""));
    assert!(output_str.contains("style=\"padding: 10px;\""));

    assert!(stats.scripts_stripped > 0);
}

#[test]
fn test_html_custom_styles_and_scripts_injection() {
    let html_input = b"<!DOCTYPE html><html><head><title>Custom</title></head><body><h1>Hello</h1></body></html>";

    let mut rewriter = TTZipHtmlRewriter::builder("arc-inj", "page.html")
        .sanitization_policy(HtmlSanitizationPolicy::Permissive)
        .add_custom_style("body { background: #1a1a1a; color: #fff; }")
        .add_custom_script("window.__TTZIP_PREVIEW__ = true;")
        .build()
        .unwrap();

    rewriter.rewrite_chunk(html_input).unwrap();
    let output = rewriter.finish().unwrap();
    let result_str = String::from_utf8(output).unwrap();

    assert!(result_str.contains("body { background: #1a1a1a; color: #fff; }"));
    assert!(result_str.contains("window.__TTZIP_PREVIEW__ = true;"));
}

#[test]
fn test_html_text_extraction() {
    let html_input = br#"
<!DOCTYPE html>
<html>
<head><title>Document Title</title></head>
<body>
    <h1 class="heading">Main Heading</h1>
    <p class="summary">First paragraph of text.</p>
</body>
</html>
"#;

    let mut rewriter = TTZipHtmlRewriter::builder("arc-text", "doc.html")
        .extract_text("title")
        .extract_text("h1.heading")
        .extract_text("p.summary")
        .build()
        .unwrap();

    rewriter.rewrite_chunk(html_input).unwrap();
    let _ = rewriter.finish().unwrap();

    let texts = rewriter.extracted_texts();
    assert_eq!(texts.get("title").map(|s| s.trim()), Some("Document Title"));
    assert_eq!(texts.get("h1.heading").map(|s| s.trim()), Some("Main Heading"));
    assert_eq!(
        texts.get("p.summary").map(|s| s.trim()),
        Some("First paragraph of text.")
    );
}

#[test]
fn test_html_chunk_by_chunk_streaming() {
    let mut rewriter = TTZipHtmlRewriter::new(
        "stream-arc",
        "sub/page.html",
        HtmlSanitizationPolicy::Permissive,
    )
    .unwrap();

    let chunk1 = b"<html><head><title>Stream</title></head>";
    let chunk2 = b"<body><img src=\"img1.png\">";
    let chunk3 = b"<img src=\"img2.png\"></body></html>";

    rewriter.rewrite_chunk(chunk1).unwrap();
    rewriter.rewrite_chunk(chunk2).unwrap();
    rewriter.rewrite_chunk(chunk3).unwrap();

    let output = rewriter.finish().unwrap();
    let out_str = String::from_utf8(output).unwrap();

    assert!(out_str.contains("ttzip-vfs://stream-arc/sub/img1.png"));
    assert!(out_str.contains("ttzip-vfs://stream-arc/sub/img2.png"));

    let stats = rewriter.stats();
    assert_eq!(stats.bytes_in, chunk1.len() + chunk2.len() + chunk3.len());
    assert_eq!(stats.resources_routed, 2);
    assert_eq!(rewriter.resource_links().len(), 2);
}
