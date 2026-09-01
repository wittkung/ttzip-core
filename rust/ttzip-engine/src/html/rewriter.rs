// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust high-throughput streaming HTML rewriter built on `lol_html`.
//!
//! Provides zero-copy chunk-by-chunk HTML transformation, VFS resource URL rewriting,
//! CSS/JS injection, security sanitization, and structured text extraction.

use crate::html::types::{
    HtmlError, HtmlFormat, HtmlResourceLink, HtmlResult, HtmlSanitizationPolicy, HtmlTransformStats,
};
use crate::html::vfs_router::HtmlVfsResourceRouter;
use lol_html::html_content::ContentType;
use lol_html::{element, text, HtmlRewriter, OutputSink, Settings};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Output sink for `lol_html` that appends transformed byte chunks into a shared buffer.
struct SharedBufferSink {
    buffer: Arc<Mutex<Vec<u8>>>,
    stats: Arc<Mutex<HtmlTransformStats>>,
}

impl OutputSink for SharedBufferSink {
    fn handle_chunk(&mut self, chunk: &[u8]) {
        if let Ok(mut buf) = self.buffer.lock() {
            buf.extend_from_slice(chunk);
        }
        if let Ok(mut st) = self.stats.lock() {
            st.bytes_out += chunk.len();
        }
    }
}

/// Builder for constructing a configured [`TTZipHtmlRewriter`].
#[derive(Debug, Clone)]
pub struct TTZipHtmlRewriterBuilder {
    archive_id: String,
    base_dir: String,
    policy: HtmlSanitizationPolicy,
    custom_scheme: String,
    custom_styles: Vec<String>,
    custom_scripts: Vec<String>,
    strip_tags: Vec<String>,
    extract_text_selectors: Vec<String>,
}

impl TTZipHtmlRewriterBuilder {
    /// Creates a new rewriter builder with default settings for the specified archive and path.
    #[must_use]
    pub fn new(archive_id: impl Into<String>, html_path: &str) -> Self {
        Self {
            archive_id: archive_id.into(),
            base_dir: html_path.to_string(),
            policy: HtmlSanitizationPolicy::default(),
            custom_scheme: crate::html::vfs_router::DEFAULT_VFS_SCHEME.to_string(),
            custom_styles: Vec::new(),
            custom_scripts: Vec::new(),
            strip_tags: Vec::new(),
            extract_text_selectors: Vec::new(),
        }
    }

    /// Sets the security sanitization policy.
    #[must_use]
    pub fn sanitization_policy(mut self, policy: HtmlSanitizationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Sets a custom VFS URI scheme prefix (e.g. `my-vfs`).
    #[must_use]
    pub fn custom_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.custom_scheme = scheme.into();
        self
    }

    /// Appends custom CSS stylesheet content to be injected into the `<head>` section.
    #[must_use]
    pub fn add_custom_style(mut self, css: impl Into<String>) -> Self {
        self.custom_styles.push(css.into());
        self
    }

    /// Appends custom JavaScript content to be injected into the `<head>` section (if policy permits).
    #[must_use]
    pub fn add_custom_script(mut self, js: impl Into<String>) -> Self {
        self.custom_scripts.push(js.into());
        self
    }

    /// Appends a tag name to be completely stripped from the output stream.
    #[must_use]
    pub fn strip_tag(mut self, tag: impl Into<String>) -> Self {
        self.strip_tags.push(tag.into());
        self
    }

    /// Requests structured text extraction for elements matching the specified CSS selector.
    #[must_use]
    pub fn extract_text(mut self, selector: impl Into<String>) -> Self {
        self.extract_text_selectors.push(selector.into());
        self
    }

    /// Builds and initializes the streaming HTML rewriter.
    pub fn build(self) -> HtmlResult<TTZipHtmlRewriter> {
        TTZipHtmlRewriter::from_builder(self)
    }
}

/// Pure Safe Rust streaming HTML rewriter with VFS routing and policy enforcement.
pub struct TTZipHtmlRewriter {
    rewriter: Option<HtmlRewriter<'static, SharedBufferSink>>,
    output_buffer: Arc<Mutex<Vec<u8>>>,
    stats: Arc<Mutex<HtmlTransformStats>>,
    links: Arc<Mutex<Vec<HtmlResourceLink>>>,
    extracted_texts: Arc<Mutex<HashMap<String, String>>>,
    format: HtmlFormat,
    is_finished: bool,
}

impl TTZipHtmlRewriter {
    /// Creates a new rewriter with standard parameters.
    pub fn new(
        archive_id: impl Into<String>,
        html_path: &str,
        policy: HtmlSanitizationPolicy,
    ) -> HtmlResult<Self> {
        TTZipHtmlRewriterBuilder::new(archive_id, html_path)
            .sanitization_policy(policy)
            .build()
    }

    /// Returns a new builder instance.
    #[must_use]
    pub fn builder(archive_id: impl Into<String>, html_path: &str) -> TTZipHtmlRewriterBuilder {
        TTZipHtmlRewriterBuilder::new(archive_id, html_path)
    }

    /// Internal constructor initializing `lol_html` streaming handlers.
    fn from_builder(builder: TTZipHtmlRewriterBuilder) -> HtmlResult<Self> {
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let stats = Arc::new(Mutex::new(HtmlTransformStats::default()));
        let links = Arc::new(Mutex::new(Vec::new()));
        let extracted_texts: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::<String, String>::new()));

        let sink = SharedBufferSink {
            buffer: Arc::clone(&output_buffer),
            stats: Arc::clone(&stats),
        };

        let router = HtmlVfsResourceRouter::with_scheme(
            builder.archive_id.clone(),
            &builder.base_dir,
            builder.custom_scheme.clone(),
        );

        let mut element_handlers = Vec::new();

        // 1. Resource routing handler: img, link, script, source, video, audio, track, embed, object, iframe
        {
            let stats_ref = Arc::clone(&stats);
            let links_ref = Arc::clone(&links);
            let router_clone = router.clone();
            let policy = builder.policy;

            element_handlers.push(element!(
                "img, link, script, source, video, audio, track, embed, object, iframe, image, use",
                move |el| {
                    let tag = el.tag_name().to_ascii_lowercase();

                    // Security check: strip scripts if not permitted
                    if tag == "script" && !policy.allows_scripts() {
                        el.remove();
                        if let Ok(mut st) = stats_ref.lock() {
                            st.scripts_stripped += 1;
                        }
                        return Ok(());
                    }

                    // Security check: strip iframes/embeds if not permitted
                    if (tag == "iframe" || tag == "embed" || tag == "object") && !policy.allows_iframes() {
                        el.remove();
                        if let Ok(mut st) = stats_ref.lock() {
                            st.iframes_stripped += 1;
                        }
                        return Ok(());
                    }

                    // Attribute rewriting & sanitization
                    let attrs: Vec<(String, String)> = el
                        .attributes()
                        .iter()
                        .map(|a| (a.name(), a.value()))
                        .collect();

                    for (name, val) in attrs {
                        // Strip inline event handlers
                        if HtmlSanitizationPolicy::is_event_attribute(&name) && !policy.allows_scripts() {
                            el.remove_attribute(&name);
                            if let Ok(mut st) = stats_ref.lock() {
                                st.scripts_stripped += 1;
                            }
                            continue;
                        }

                        // Strip dangerous schemes
                        if HtmlSanitizationPolicy::is_dangerous_url_scheme(&val) && !policy.allows_scripts() {
                            el.remove_attribute(&name);
                            if let Ok(mut st) = stats_ref.lock() {
                                st.scripts_stripped += 1;
                            }
                            continue;
                        }

                        // Route relative resources to VFS
                        if let Some(rewritten) = router_clone.route_attribute(&tag, &name, &val) {
                            if let Err(e) = el.set_attribute(&name, &rewritten) {
                                return Err(format!("{e}").into());
                            }
                            if let Ok(mut st) = stats_ref.lock() {
                                st.tags_rewritten += 1;
                                st.resources_routed += 1;
                            }
                            if let Ok(mut lk) = links_ref.lock() {
                                lk.push(HtmlResourceLink::new(&val, &rewritten, &tag, &name));
                            }
                        }
                    }

                    Ok(())
                }
            ));
        }

        // 2. Global attribute sanitizer handler (cleans inline events and dangerous hrefs across all elements)
        {
            let stats_ref = Arc::clone(&stats);
            let policy = builder.policy;

            element_handlers.push(element!("*", move |el| {
                let tag = el.tag_name().to_ascii_lowercase();

                // Skip elements already fully handled
                if tag == "img"
                    || tag == "link"
                    || tag == "script"
                    || tag == "source"
                    || tag == "video"
                    || tag == "audio"
                {
                    return Ok(());
                }

                let attrs: Vec<(String, String)> = el
                    .attributes()
                    .iter()
                    .map(|a| (a.name(), a.value()))
                    .collect();

                for (name, val) in attrs {
                    // Strip event attributes
                    if HtmlSanitizationPolicy::is_event_attribute(&name) && !policy.allows_scripts() {
                        el.remove_attribute(&name);
                        if let Ok(mut st) = stats_ref.lock() {
                            st.scripts_stripped += 1;
                        }
                    }

                    // Strip dangerous schemes in links/forms
                    if (name.eq_ignore_ascii_case("href")
                        || name.eq_ignore_ascii_case("action")
                        || name.eq_ignore_ascii_case("src"))
                        && HtmlSanitizationPolicy::is_dangerous_url_scheme(&val)
                        && !policy.allows_scripts()
                    {
                        el.remove_attribute(&name);
                        if let Ok(mut st) = stats_ref.lock() {
                            st.scripts_stripped += 1;
                        }
                    }

                    // Strip inline styles if policy is Strict
                    if name.eq_ignore_ascii_case("style") && !policy.allows_inline_styles() {
                        el.remove_attribute(&name);
                    }
                }

                // Strip style tags if policy is Strict
                if tag == "style" && !policy.allows_inline_styles() {
                    el.remove();
                }

                Ok(())
            }));
        }

        // 3. User-defined stripped tags
        for strip_tag_name in builder.strip_tags {
            let selector = strip_tag_name;
            element_handlers.push(element!(&selector, |el| {
                el.remove();
                Ok(())
            }));
        }

        // 4. Custom CSS and JS injection into <head> or <body>
        if !builder.custom_styles.is_empty() || !builder.custom_scripts.is_empty() {
            let styles = builder.custom_styles.clone();
            let scripts = builder.custom_scripts.clone();
            let policy = builder.policy;

            element_handlers.push(element!("head, body", move |el| {
                for css in &styles {
                    let style_block = format!("\n<style>\n{}\n</style>\n", css);
                    el.append(&style_block, ContentType::Html);
                }
                if policy.allows_scripts() {
                    for js in &scripts {
                        let script_block = format!("\n<script>\n{}\n</script>\n", js);
                        el.append(&script_block, ContentType::Html);
                    }
                }
                Ok(())
            }));
        }

        // 5. Structured text extraction handlers
        for text_sel in builder.extract_text_selectors {
            let sel_key = text_sel.clone();
            let text_map = Arc::clone(&extracted_texts);
            element_handlers.push(text!(&text_sel, move |chunk| {
                let text_str = chunk.as_str();
                if !text_str.trim().is_empty() {
                    if let Ok(mut map) = text_map.lock() {
                        let entry: &mut String = map.entry(sel_key.clone()).or_insert_with(String::new);
                        entry.push_str(text_str);
                    }
                }
                Ok(())
            }));
        }

        let settings = Settings {
            element_content_handlers: element_handlers,
            ..Settings::default()
        };

        let rewriter = HtmlRewriter::new(settings, sink);

        Ok(Self {
            rewriter: Some(rewriter),
            output_buffer,
            stats,
            links,
            extracted_texts,
            format: HtmlFormat::Html5,
            is_finished: false,
        })
    }

    /// Ingests and rewrites a single byte slice chunk from the HTML stream.
    pub fn rewrite_chunk(&mut self, chunk: &[u8]) -> HtmlResult<()> {
        if self.is_finished {
            return Err(HtmlError::RewriteError(
                "Cannot write chunk to already finished rewriter".to_string(),
            ));
        }

        if let Ok(mut st) = self.stats.lock() {
            if st.bytes_in == 0 && !chunk.is_empty() {
                self.format = HtmlFormat::detect(chunk);
            }
            st.bytes_in += chunk.len();
        }

        if let Some(ref mut rewriter) = self.rewriter {
            rewriter
                .write(chunk)
                .map_err(|e| HtmlError::RewriteError(e.to_string()))?;
        }

        Ok(())
    }

    /// Finalizes the stream rewriting process, flushes pending tags, and returns output bytes.
    pub fn finish(&mut self) -> HtmlResult<Vec<u8>> {
        if self.is_finished {
            let buf = self.output_buffer.lock().unwrap_or_else(|e| e.into_inner());
            return Ok(buf.clone());
        }

        if let Some(rewriter) = self.rewriter.take() {
            rewriter
                .end()
                .map_err(|e| HtmlError::RewriteError(e.to_string()))?;
        }

        self.is_finished = true;
        let buf = self.output_buffer.lock().unwrap_or_else(|e| e.into_inner());
        Ok(buf.clone())
    }

    /// Returns a copy of the current transformation metrics.
    #[must_use]
    pub fn stats(&self) -> HtmlTransformStats {
        self.stats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns the list of rewritten resource link records.
    #[must_use]
    pub fn resource_links(&self) -> Vec<HtmlResourceLink> {
        self.links
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns the map of extracted text segments keyed by selector string.
    #[must_use]
    pub fn extracted_texts(&self) -> HashMap<String, String> {
        self.extracted_texts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns the detected HTML format variant.
    #[must_use]
    pub const fn format(&self) -> HtmlFormat {
        self.format
    }

    /// Convenience all-in-one function to rewrite a complete HTML byte payload.
    pub fn rewrite_all(
        input: &[u8],
        archive_id: &str,
        html_path: &str,
        policy: HtmlSanitizationPolicy,
    ) -> HtmlResult<(Vec<u8>, HtmlTransformStats)> {
        let mut rewriter = Self::new(archive_id, html_path, policy)?;
        rewriter.rewrite_chunk(input)?;
        let output = rewriter.finish()?;
        let stats = rewriter.stats();
        Ok((output, stats))
    }
}
