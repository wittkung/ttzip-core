// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-Copy Streaming HTML Parser, VFS Rewriter, and DOM Sanitizer.

use std::cell::RefCell;
use std::rc::Rc;

use lol_html::{element, text, HtmlRewriter, OutputSink, Settings};

use super::types::{
    UniFFIHtmlError, UniFFIHtmlFormat, UniFFIHtmlResourceLink, UniFFIHtmlSanitizationPolicy,
    UniFFIHtmlTransformResult,
};

// ============================================================================
// Format Sniffer & Prober
// ============================================================================

/// Probes the format classification of an HTML or markup byte buffer.
pub fn probe_html_format(bytes: &[u8], file_name: Option<&str>) -> UniFFIHtmlFormat {
    if let Some(name) = file_name {
        let lower = name.to_lowercase();
        if lower.ends_with(".mhtml") || lower.ends_with(".mht") {
            return UniFFIHtmlFormat::Mhtml;
        }
        if lower.ends_with(".xhtml") || lower.ends_with(".xht") {
            return UniFFIHtmlFormat::Xhtml;
        }
        if lower.ends_with(".svg") || lower.ends_with(".svgz") {
            return UniFFIHtmlFormat::Svg;
        }
        if lower.ends_with(".html") || lower.ends_with(".htm") {
            return UniFFIHtmlFormat::Html;
        }
    }

    if bytes.is_empty() {
        return UniFFIHtmlFormat::Unknown;
    }

    // Skip UTF-8 / UTF-16 BOMs
    let slice = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };

    let sample_len = slice.len().min(4096);
    let sample = String::from_utf8_lossy(&slice[..sample_len]).to_lowercase();
    let trimmed = sample.trim_start();

    if trimmed.starts_with("mime-version:")
        || trimmed.starts_with("from: <saved by")
        || trimmed.contains("content-type: multipart/related")
        || trimmed.contains("content-type: message/rfc822")
    {
        return UniFFIHtmlFormat::Mhtml;
    }

    if trimmed.starts_with("<svg") || trimmed.contains("<svg xmlns=") {
        return UniFFIHtmlFormat::Svg;
    }

    if trimmed.starts_with("<?xml") && (trimmed.contains("xhtml") || trimmed.contains("<!doctype html")) {
        return UniFFIHtmlFormat::Xhtml;
    }

    if trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html") || trimmed.contains("<html") {
        return UniFFIHtmlFormat::Html;
    }

    if trimmed.contains("<div")
        || trimmed.contains("<p")
        || trimmed.contains("<span")
        || trimmed.contains("<body")
        || trimmed.contains("<head")
        || trimmed.contains("<table")
        || trimmed.contains("<h1")
        || trimmed.contains("<section")
    {
        return UniFFIHtmlFormat::HtmlFragment;
    }

    UniFFIHtmlFormat::Unknown
}

// ============================================================================
// URI Normalization & VFS Resolution Helpers
// ============================================================================

/// Normalizes relative or absolute URI into `ttzip-vfs://` virtual scheme.
pub fn resolve_vfs_uri(original: &str, base_vfs_prefix: &str) -> (Option<String>, bool) {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        return (None, false);
    }

    if trimmed.starts_with('#')
        || trimmed.starts_with("data:")
        || trimmed.starts_with("blob:")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("tel:")
        || trimmed.starts_with("javascript:")
    {
        return (None, false);
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("//") {
        return (None, true);
    }

    if trimmed.starts_with("ttzip-vfs://") {
        return (Some(trimmed.to_string()), false);
    }

    let prefix = base_vfs_prefix.trim_matches('/');
    if prefix.is_empty() {
        let clean = trimmed.trim_start_matches('/');
        return (Some(format!("ttzip-vfs://{clean}")), false);
    }

    // Split base prefix into archive base and directory path
    let base_dir = if let Some(last_slash) = prefix.rfind('/') {
        if prefix.ends_with(".html") || prefix.ends_with(".htm") || prefix.ends_with(".xhtml") {
            &prefix[..last_slash]
        } else {
            prefix
        }
    } else if prefix.ends_with(".html") || prefix.ends_with(".htm") || prefix.ends_with(".xhtml") {
        ""
    } else {
        prefix
    };

    let target_path = if trimmed.starts_with('/') {
        // Path relative to archive root
        trimmed.trim_start_matches('/').to_string()
    } else if base_dir.is_empty() {
        trimmed.to_string()
    } else {
        format!("{base_dir}/{trimmed}")
    };

    // Normalize `/./` and `/../` path segments safely
    let mut segments = Vec::new();
    for seg in target_path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            segments.pop();
        } else {
            segments.push(seg);
        }
    }

    let normalized_path = segments.join("/");
    (Some(format!("ttzip-vfs://{normalized_path}")), false)
}

// ============================================================================
// Core Transformation & Rewriting Engine
// ============================================================================

struct OutputBuffer(Rc<RefCell<Vec<u8>>>);

impl OutputSink for OutputBuffer {
    fn handle_chunk(&mut self, chunk: &[u8]) {
        self.0.borrow_mut().extend_from_slice(chunk);
    }
}

/// Executes HTML transformation, DOM sanitization, resource link rewriting, and metric extraction.
pub fn transform_html_vfs(
    html: &str,
    base_vfs_prefix: &str,
    policy: &UniFFIHtmlSanitizationPolicy,
) -> Result<UniFFIHtmlTransformResult, UniFFIHtmlError> {
    let output_vec = Rc::new(RefCell::new(Vec::with_capacity(html.len() + 1024)));
    let resources = Rc::new(RefCell::new(Vec::new()));
    let title_cell = Rc::new(RefCell::new(None::<String>));
    let charset_cell = Rc::new(RefCell::new(None::<String>));
    let has_scripts_cell = Rc::new(RefCell::new(false));
    let has_styles_cell = Rc::new(RefCell::new(false));
    let text_chars_cell = Rc::new(RefCell::new(0u32));
    let text_words_cell = Rc::new(RefCell::new(0u32));

    let policy_clone = policy.clone();
    let base_prefix = base_vfs_prefix.to_string();

    let element_handlers = vec![
        // Title text capture
        text!("title", {
            let title_cell = Rc::clone(&title_cell);
            move |t| {
                let chunk = t.as_str();
                let mut cell = title_cell.borrow_mut();
                if let Some(existing) = cell.as_mut() {
                    existing.push_str(chunk);
                } else {
                    *cell = Some(chunk.to_string());
                }
                Ok(())
            }
        }),
        // Meta charset inspector
        element!("meta", {
            let charset_cell = Rc::clone(&charset_cell);
            move |el| {
                if let Some(cs) = el.get_attribute("charset") {
                    *charset_cell.borrow_mut() = Some(cs.trim().to_string());
                } else if let Some(http_equiv) = el.get_attribute("http-equiv") {
                    if http_equiv.eq_ignore_ascii_case("content-type") {
                        if let Some(content) = el.get_attribute("content") {
                            if let Some(pos) = content.to_lowercase().find("charset=") {
                                let cs = content[pos + 8..].trim().trim_matches(';').trim();
                                *charset_cell.borrow_mut() = Some(cs.to_string());
                            }
                        }
                    }
                }
                Ok(())
            }
        }),
        // Universal element inspector & sanitizer & link rewriter
        element!("*", {
            let resources = Rc::clone(&resources);
            let has_scripts = Rc::clone(&has_scripts_cell);
            let has_styles = Rc::clone(&has_styles_cell);
            let policy = policy_clone.clone();
            let base_prefix = base_prefix.clone();

            move |el| {
                let tag_name = el.tag_name().to_ascii_lowercase();

                // 1. Tag blocking check
                if policy.custom_blocked_tags.iter().any(|b| b.eq_ignore_ascii_case(&tag_name)) {
                    el.remove();
                    return Ok(());
                }

                if !policy.custom_allowed_tags.is_empty()
                    && !policy.custom_allowed_tags.iter().any(|a| a.eq_ignore_ascii_case(&tag_name))
                {
                    el.remove();
                    return Ok(());
                }

                // 2. Script removal
                if tag_name == "script" {
                    *has_scripts.borrow_mut() = true;
                    if !policy.allow_scripts {
                        el.remove();
                        return Ok(());
                    }
                }

                // 3. Iframe and embedded frame removal
                if (tag_name == "iframe" || tag_name == "frame" || tag_name == "frameset" || tag_name == "object" || tag_name == "embed")
                    && !policy.allow_iframes
                {
                    el.remove();
                    return Ok(());
                }

                // 4. Form removal
                if (tag_name == "form" || tag_name == "input" || tag_name == "button" || tag_name == "select" || tag_name == "textarea")
                    && !policy.allow_forms
                {
                    if tag_name == "form" {
                        el.remove_and_keep_content();
                    } else {
                        el.remove();
                    }
                    return Ok(());
                }

                // 5. Style element handling
                if tag_name == "style" {
                    *has_styles.borrow_mut() = true;
                    if !policy.allow_inline_styles {
                        el.remove();
                        return Ok(());
                    }
                }

                // 6. Strip inline event handlers (on*) and javascript: hrefs if scripts disabled
                let mut attrs_to_remove = Vec::new();
                let mut attr_mutations = Vec::new();

                for attr in el.attributes() {
                    let name = attr.name().to_ascii_lowercase();
                    let val = attr.value();

                    if !policy.allow_scripts && name.starts_with("on") {
                        attrs_to_remove.push(name.clone());
                        *has_scripts.borrow_mut() = true;
                        continue;
                    }

                    if !policy.allow_inline_styles && name == "style" {
                        attrs_to_remove.push(name.clone());
                        *has_styles.borrow_mut() = true;
                        continue;
                    }

                    if name == "style" {
                        *has_styles.borrow_mut() = true;
                    }

                    // Resource link detection and rewriting
                    let (res_type, is_target_attr) = match (tag_name.as_str(), name.as_str()) {
                        ("img", "src") => ("image", true),
                        ("link", "href") => {
                            let rel = el.get_attribute("rel").unwrap_or_default().to_lowercase();
                            let t = if rel.contains("stylesheet") {
                                "stylesheet"
                            } else if rel.contains("icon") {
                                "image"
                            } else if rel.contains("font") || rel.contains("preload") {
                                "font"
                            } else {
                                "link"
                            };
                            (t, true)
                        }
                        ("script", "src") => ("script", true),
                        ("audio", "src") => ("audio", true),
                        ("video", "src") => ("video", true),
                        ("video", "poster") => ("image", true),
                        ("source", "src") => ("media", true),
                        ("track", "src") => ("track", true),
                        ("iframe", "src") => ("iframe", true),
                        ("embed", "src") => ("embed", true),
                        ("a", "href") => ("link", true),
                        _ => ("", false),
                    };

                    if is_target_attr {
                        let (resolved_vfs, is_external) = resolve_vfs_uri(&val, &base_prefix);

                        resources.borrow_mut().push(UniFFIHtmlResourceLink {
                            tag_name: tag_name.clone(),
                            attribute_name: name.clone(),
                            original_uri: val.clone(),
                            resolved_vfs_uri: resolved_vfs.clone(),
                            resource_type: res_type.to_string(),
                            is_external,
                        });

                        if is_external && !policy.allow_external_resources {
                            attrs_to_remove.push(name.clone());
                        } else if let Some(vfs_uri) = resolved_vfs {
                            attr_mutations.push((name.clone(), vfs_uri));
                        }
                    }
                }

                for attr in attrs_to_remove {
                    el.remove_attribute(&attr);
                }
                for (name, val) in attr_mutations {
                    el.set_attribute(&name, &val).ok();
                }

                Ok(())
            }
        }),
        // Universal text handler for word & character metrics
        text!("*", {
            let chars_acc = Rc::clone(&text_chars_cell);
            let words_acc = Rc::clone(&text_words_cell);
            move |t| {
                let content = t.as_str();
                let char_len = content.chars().count() as u32;
                *chars_acc.borrow_mut() += char_len;
                let words = content.split_whitespace().count() as u32;
                *words_acc.borrow_mut() += words;
                Ok(())
            }
        }),
    ];

    let output_buffer = OutputBuffer(Rc::clone(&output_vec));
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: element_handlers,
            ..Settings::default()
        },
        output_buffer,
    );

    rewriter
        .write(html.as_bytes())
        .map_err(|e| UniFFIHtmlError::rewrite_err(format!("Stream write failed: {e}")))?;
    rewriter
        .end()
        .map_err(|e| UniFFIHtmlError::rewrite_err(format!("Stream finalization failed: {e}")))?;

    let transformed_bytes = Rc::try_unwrap(output_vec)
        .map_err(|_| UniFFIHtmlError::rewrite_err("Buffer ownership conflict"))?
        .into_inner();

    let transformed_html = String::from_utf8(transformed_bytes)
        .map_err(|e| UniFFIHtmlError::invalid_encoding(format!("UTF-8 output decoding failed: {e}")))?;

    let extracted_resources = Rc::try_unwrap(resources)
        .map_err(|_| UniFFIHtmlError::rewrite_err("Resource collection conflict"))?
        .into_inner();

    let title = title_cell
        .borrow()
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let charset = charset_cell
        .borrow()
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let has_scripts = *has_scripts_cell.borrow();
    let has_inline_styles = *has_styles_cell.borrow();
    let metrics_chars = *text_chars_cell.borrow();
    let metrics_words = *text_words_cell.borrow();

    Ok(UniFFIHtmlTransformResult {
        transformed_html,
        extracted_resources,
        title,
        charset,
        has_scripts,
        has_inline_styles,
        metrics_chars,
        metrics_words,
    })
}

/// Sanitizes HTML markup according to policy without altering relative resource paths.
pub fn sanitize_html_markup(
    html: &str,
    policy: &UniFFIHtmlSanitizationPolicy,
) -> Result<String, UniFFIHtmlError> {
    let result = transform_html_vfs(html, "", policy)?;
    Ok(result.transformed_html)
}

/// Extracts all referenced resource links from HTML markup.
pub fn extract_resources_from_html(
    html: &str,
) -> Result<Vec<UniFFIHtmlResourceLink>, UniFFIHtmlError> {
    let permissive_policy = UniFFIHtmlSanitizationPolicy {
        allow_scripts: true,
        allow_inline_styles: true,
        allow_external_resources: true,
        allow_forms: true,
        allow_iframes: true,
        custom_allowed_tags: Vec::new(),
        custom_blocked_tags: Vec::new(),
    };
    let result = transform_html_vfs(html, "", &permissive_policy)?;
    Ok(result.extracted_resources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_html_format() {
        assert_eq!(
            probe_html_format(b"<!DOCTYPE html><html><body><h1>Hello</h1></body></html>", Some("index.html")),
            UniFFIHtmlFormat::Html
        );
        assert_eq!(
            probe_html_format(b"<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"></html>", Some("page.xhtml")),
            UniFFIHtmlFormat::Xhtml
        );
        assert_eq!(
            probe_html_format(b"<svg viewBox=\"0 0 100 100\"><circle cx=\"50\" cy=\"50\" r=\"40\"/></svg>", Some("icon.svg")),
            UniFFIHtmlFormat::Svg
        );
        assert_eq!(
            probe_html_format(b"MIME-Version: 1.0\nContent-Type: multipart/related", Some("archive.mht")),
            UniFFIHtmlFormat::Mhtml
        );
        assert_eq!(
            probe_html_format(b"<div><p>Fragment</p></div>", None),
            UniFFIHtmlFormat::HtmlFragment
        );
    }

    #[test]
    fn test_vfs_uri_resolution() {
        let (res, ext) = resolve_vfs_uri("images/pic.png", "bundle.zip/docs/page.html");
        assert_eq!(res, Some("ttzip-vfs://bundle.zip/docs/images/pic.png".to_string()));
        assert!(!ext);

        let (res_parent, _) = resolve_vfs_uri("../assets/style.css", "bundle.zip/docs/page.html");
        assert_eq!(res_parent, Some("ttzip-vfs://bundle.zip/assets/style.css".to_string()));

        let (res_ext, ext_flag) = resolve_vfs_uri("https://example.com/logo.png", "bundle.zip/docs/page.html");
        assert_eq!(res_ext, None);
        assert!(ext_flag);
    }

    #[test]
    fn test_transform_html_vfs_rewriting() {
        let raw_html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Test Page Title</title>
    <link rel="stylesheet" href="styles/main.css">
    <script src="app.js"></script>
</head>
<body onclick="alert('hack')">
    <h1>Welcome</h1>
    <img src="images/banner.png" alt="Banner">
    <iframe src="https://evil.com"></iframe>
    <a href="chapter2.html">Next</a>
</body>
</html>"#;

        let policy = UniFFIHtmlSanitizationPolicy::default(); // scripts=false, iframes=false
        let result = transform_html_vfs(raw_html, "test.zip/site/", &policy).expect("Transformation failed");

        assert_eq!(result.title.as_deref(), Some("Test Page Title"));
        assert_eq!(result.charset.as_deref(), Some("utf-8"));
        assert!(result.has_scripts);

        // Verify script and iframe were stripped
        assert!(!result.transformed_html.contains("<script"));
        assert!(!result.transformed_html.contains("<iframe"));
        assert!(!result.transformed_html.contains("onclick"));

        // Verify VFS rewriting
        assert!(result.transformed_html.contains("ttzip-vfs://test.zip/site/styles/main.css"));
        assert!(result.transformed_html.contains("ttzip-vfs://test.zip/site/images/banner.png"));
        assert!(result.transformed_html.contains("ttzip-vfs://test.zip/site/chapter2.html"));
    }
}
