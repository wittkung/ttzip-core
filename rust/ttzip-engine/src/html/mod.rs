// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust streaming HTML rewriter, CSS3 selector engine, and VFS resource router.
//!
//! Provides high-throughput zero-copy chunk-by-chunk HTML transformation, relative resource
//! link rewriting to `ttzip-vfs://` virtual URIs, RFC 3986 path normalization, CSS3 selector
//! matching, and multi-tier security sanitization against XSS and Zip-Slip traversal.

pub mod rewriter;
pub mod selector;
pub mod types;
pub mod vfs_router;

#[cfg(test)]
mod tests;

pub use rewriter::{TTZipHtmlRewriter, TTZipHtmlRewriterBuilder};
pub use selector::{
    AttributeMatcher, CompiledSelector, HtmlSelectorEngine, SimpleSelector,
};
pub use types::{
    HtmlError, HtmlFormat, HtmlResourceLink, HtmlResult, HtmlSanitizationPolicy,
    HtmlTransformStats,
};
pub use vfs_router::{
    extract_parent_directory, is_external_or_special_url, is_routable_resource_tag_attr,
    normalize_rfc3986_path, HtmlVfsResourceRouter, DEFAULT_VFS_SCHEME,
};
