// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 2: External Network Sandbox & CSP Injection Guard.
//!
//! Enforces air-gapped network isolation on HTML previews:
//! - Neutralizes external remote links (`http://`, `https://`, `//`, `ftp://`, `ws://`, `wss://`)
//! - Rewrites intra-archive relative assets to the deterministic `ttzip-vfs://` protocol
//! - Injects a strict Content-Security-Policy (CSP) meta header blocking unauthorized fetch/connect/script.

/// Default Content-Security-Policy directive enforcing air-gapped sandbox isolation.
pub const DEFAULT_STRICT_CSP_CONTENT: &str =
    "default-src 'none'; style-src 'unsafe-inline'; img-src data: ttzip-vfs:; font-src data: ttzip-vfs:; media-src data: ttzip-vfs:; sandbox allow-same-origin;";

/// Default VFS URI scheme prefix.
pub const DEFAULT_VFS_URI_PREFIX: &str = "ttzip-vfs://";

/// Metrics and audit report for network sandboxing and link rewriting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkSandboxReport {
    /// Number of external remote URLs neutralized to placeholders.
    pub neutralized_external_links_count: usize,
    /// Number of relative/archive URLs rewritten to `ttzip-vfs://`.
    pub rewritten_vfs_links_count: usize,
    /// Whether the strict CSP meta tag was injected into the document.
    pub csp_injected: bool,
}

/// Configuration options for network sandbox behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSandboxOptions {
    /// VFS prefix to prepend to internal archive paths (default: `ttzip-vfs://`).
    pub vfs_prefix: String,
    /// Whether to inject the strict CSP `<meta>` tag.
    pub inject_csp: bool,
    /// Optional custom CSP directive string overriding the default.
    pub custom_csp: Option<String>,
    /// Whether to neutralize external links (`http/https`) to placeholders.
    pub block_external_network: bool,
}

impl Default for NetworkSandboxOptions {
    fn default() -> Self {
        Self {
            vfs_prefix: DEFAULT_VFS_URI_PREFIX.to_string(),
            inject_csp: true,
            custom_csp: None,
            block_external_network: true,
        }
    }
}

/// External network sandbox guard.
#[derive(Debug, Clone)]
pub struct ExternalNetworkSandboxGuard {
    options: NetworkSandboxOptions,
}

impl Default for ExternalNetworkSandboxGuard {
    fn default() -> Self {
        Self::new(NetworkSandboxOptions::default())
    }
}

impl ExternalNetworkSandboxGuard {
    /// Creates a new network sandbox guard with configured options.
    #[must_use]
    pub const fn new(options: NetworkSandboxOptions) -> Self {
        Self { options }
    }

    /// Returns `true` if the URI points to an external remote network resource.
    #[must_use]
    pub fn is_external_uri(uri: &str) -> bool {
        let trimmed = uri.trim();
        let lower = trimmed.to_ascii_lowercase();

        lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("//")
            || lower.starts_with("ftp://")
            || lower.starts_with("ws://")
            || lower.starts_with("wss://")
    }

    /// Rewrites or neutralizes a URI attribute according to sandbox rules.
    ///
    /// - External remote URLs (`http://...`) are neutralized to `#ttzip-blocked-external` or placeholder.
    /// - Relative in-archive paths (`./assets/style.css`, `images/pic.png`) are rewritten to `ttzip-vfs://...`.
    /// - Safe `data:` image URIs, `mailto:`, `tel:`, and fragment anchors (`#section`) are preserved.
    pub fn sanitize_and_rewrite_uri(
        &self,
        uri: &str,
        report: &mut NetworkSandboxReport,
    ) -> String {
        let trimmed = uri.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // 1. Anchors (#fragment) and already routed VFS URIs are preserved
        if trimmed.starts_with('#') || trimmed.starts_with(&self.options.vfs_prefix) {
            return trimmed.to_string();
        }

        // 2. Safe data: URIs and non-network intent schemes are preserved
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("data:") || lower.starts_with("mailto:") || lower.starts_with("tel:") {
            return trimmed.to_string();
        }

        // 3. External remote URLs are neutralized if blocking is enabled
        if self.options.block_external_network && Self::is_external_uri(trimmed) {
            report.neutralized_external_links_count =
                report.neutralized_external_links_count.saturating_add(1);
            return "#ttzip-blocked-external-url".to_string();
        }

        // 4. In-archive relative or absolute paths are rewritten to VFS scheme
        let path_clean = trimmed
            .trim_start_matches('/')
            .trim_start_matches("./");

        let mut segments = Vec::new();
        for seg in path_clean.split('/') {
            if seg.is_empty() || seg == "." {
                continue;
            }
            if seg == ".." {
                segments.pop();
            } else {
                segments.push(seg);
            }
        }
        let normalized = segments.join("/");

        report.rewritten_vfs_links_count = report.rewritten_vfs_links_count.saturating_add(1);
        format!("{}{}", self.options.vfs_prefix, normalized)
    }

    /// Generates the strict CSP `<meta>` tag string.
    #[must_use]
    pub fn generate_csp_meta_tag(&self) -> String {
        let csp = self
            .options
            .custom_csp
            .as_deref()
            .unwrap_or(DEFAULT_STRICT_CSP_CONTENT);
        format!(
            r#"<meta http-equiv="Content-Security-Policy" content="{}">"#,
            csp
        )
    }

    /// Injects CSP meta tag into the HTML document string if not already present.
    pub fn inject_csp_header(
        &self,
        html_input: &str,
        report: &mut NetworkSandboxReport,
    ) -> String {
        if !self.options.inject_csp {
            return html_input.to_string();
        }

        let csp_tag = self.generate_csp_meta_tag();

        // Check if document already has CSP injected
        if html_input.contains("http-equiv=\"Content-Security-Policy\"")
            || html_input.contains("http-equiv='Content-Security-Policy'")
        {
            report.csp_injected = true;
            return html_input.to_string();
        }

        let lower = html_input.to_ascii_lowercase();
        let mut output = String::with_capacity(html_input.len() + csp_tag.len() + 32);

        if let Some(pos) = lower.find("<head>") {
            let insert_pos = pos + 6;
            output.push_str(&html_input[..insert_pos]);
            output.push_str(&csp_tag);
            output.push_str(&html_input[insert_pos..]);
            report.csp_injected = true;
        } else if let Some(pos) = lower.find("<head ") {
            // Find closing '>' of <head ...>
            if let Some(end_head) = html_input[pos..].find('>') {
                let insert_pos = pos + end_head + 1;
                output.push_str(&html_input[..insert_pos]);
                output.push_str(&csp_tag);
                output.push_str(&html_input[insert_pos..]);
                report.csp_injected = true;
            } else {
                output.push_str(&csp_tag);
                output.push_str(html_input);
                report.csp_injected = true;
            }
        } else if let Some(pos) = lower.find("<html>") {
            let insert_pos = pos + 6;
            output.push_str(&html_input[..insert_pos]);
            output.push_str("<head>");
            output.push_str(&csp_tag);
            output.push_str("</head>");
            output.push_str(&html_input[insert_pos..]);
            report.csp_injected = true;
        } else {
            // Document has no <head> or <html>, prepend CSP meta tag at document top
            output.push_str("<head>");
            output.push_str(&csp_tag);
            output.push_str("</head>");
            output.push_str(html_input);
            report.csp_injected = true;
        }

        output
    }

    /// Returns the configured options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &NetworkSandboxOptions {
        &self.options
    }
}
