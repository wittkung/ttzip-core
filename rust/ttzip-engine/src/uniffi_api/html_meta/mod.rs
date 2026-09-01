// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for HTML Preview, DOM Transformation, and VFS Routing.
//!
//! Provides in-memory HTML probing, Lol-HTML stream rewriting, DOM sanitization, and
//! `ttzip-vfs://` asset URL resolution for Swift 6 WebKit preview pipelines.

pub mod rewriter;
pub mod service;
pub mod types;

pub use rewriter::{
    extract_resources_from_html, probe_html_format, resolve_vfs_uri, sanitize_html_markup,
    transform_html_vfs,
};
pub use service::{
    uniffi_extract_html_resources, uniffi_html_service_new, uniffi_probe_html_bytes,
    uniffi_probe_html_file, uniffi_rewrite_html_vfs, uniffi_sanitize_html, UniFFIHtmlService,
};
pub use types::{
    UniFFIHtmlError, UniFFIHtmlFormat, UniFFIHtmlResourceLink, UniFFIHtmlSanitizationPolicy,
    UniFFIHtmlTransformResult,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_html_service_e2e() {
        let service = UniFFIHtmlService::new();
        let sample_html = r#"<!DOCTYPE html>
<html>
<head>
    <title>E2E HTML Test</title>
    <link rel="stylesheet" href="theme.css">
    <script>console.log('danger')</script>
</head>
<body>
    <h1>Header 1</h1>
    <p>Sample paragraph with words.</p>
    <img src="./assets/pic.jpg" alt="Photo">
</body>
</html>"#;

        // 1. Probe
        let fmt = service
            .probe_bytes(sample_html.as_bytes().to_vec(), Some("index.html".to_string()))
            .expect("Probe failed");
        assert_eq!(fmt, UniFFIHtmlFormat::Html);

        // 2. Resource extraction
        let resources = service
            .extract_resources(sample_html.to_string())
            .expect("Resource extraction failed");
        assert_eq!(resources.len(), 2); // link (theme.css), img (./assets/pic.jpg)
        assert!(resources.iter().any(|r| r.tag_name == "img" && r.original_uri == "./assets/pic.jpg"));

        // 3. VFS rewrite with default sanitization
        let policy = UniFFIHtmlSanitizationPolicy::default();
        let res = service
            .rewrite_vfs(sample_html.to_string(), "archive.zip/web/".to_string(), policy)
            .expect("Rewrite failed");

        assert_eq!(res.title.as_deref(), Some("E2E HTML Test"));
        assert!(!res.transformed_html.contains("<script"));
        assert!(res.transformed_html.contains("ttzip-vfs://archive.zip/web/theme.css"));
        assert!(res.transformed_html.contains("ttzip-vfs://archive.zip/web/assets/pic.jpg"));
        assert!(res.metrics_chars > 0);
        assert!(res.metrics_words > 0);
    }
}
