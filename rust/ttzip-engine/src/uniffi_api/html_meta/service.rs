// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Service and Free Functions for HTML Transformation & VFS Interception.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use super::rewriter::{
    extract_resources_from_html, probe_html_format, sanitize_html_markup, transform_html_vfs,
};
use super::types::{
    UniFFIHtmlError, UniFFIHtmlFormat, UniFFIHtmlResourceLink, UniFFIHtmlSanitizationPolicy,
    UniFFIHtmlTransformResult,
};

// ============================================================================
// Internal Helpers
// ============================================================================

fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, UniFFIHtmlError> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(UniFFIHtmlError::io_err(format!("File not found: '{path_str}'")));
    }
    let file = File::open(path)
        .map_err(|e| UniFFIHtmlError::io_err(format!("Failed to open file '{path_str}': {e}")))?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }
        .map_err(|e| UniFFIHtmlError::io_err(format!("Failed to memory map file '{path_str}': {e}")))?;
    Ok(mmap.to_vec())
}

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Probes the format classification of an in-memory HTML or markup byte buffer.
#[uniffi::export]
pub fn uniffi_probe_html_bytes(
    bytes: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIHtmlFormat, UniFFIHtmlError> {
    Ok(probe_html_format(&bytes, file_name.as_deref()))
}

/// Probes the format classification of a file on disk.
#[uniffi::export]
pub fn uniffi_probe_html_file(file_path: String) -> Result<UniFFIHtmlFormat, UniFFIHtmlError> {
    let bytes = read_file_bytes(&file_path)?;
    let file_name = Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());
    Ok(probe_html_format(&bytes, file_name.as_deref()))
}

/// Rewrites HTML relative resource links to `ttzip-vfs://` URLs and sanitizes DOM markup according to policy.
#[uniffi::export]
pub fn uniffi_rewrite_html_vfs(
    html_content: String,
    base_vfs_prefix: String,
    policy: UniFFIHtmlSanitizationPolicy,
) -> Result<UniFFIHtmlTransformResult, UniFFIHtmlError> {
    transform_html_vfs(&html_content, &base_vfs_prefix, &policy)
}

/// Sanitizes HTML markup according to security policy without changing relative paths.
#[uniffi::export]
pub fn uniffi_sanitize_html(
    html_content: String,
    policy: UniFFIHtmlSanitizationPolicy,
) -> Result<String, UniFFIHtmlError> {
    sanitize_html_markup(&html_content, &policy)
}

/// Extracts all asset and resource links (images, stylesheets, scripts, audio/video) from HTML markup.
#[uniffi::export]
pub fn uniffi_extract_html_resources(
    html_content: String,
) -> Result<Vec<UniFFIHtmlResourceLink>, UniFFIHtmlError> {
    extract_resources_from_html(&html_content)
}

/// Instantiates a new thread-safe HTML preview & transformation service.
#[uniffi::export]
pub fn uniffi_html_service_new() -> Arc<UniFFIHtmlService> {
    UniFFIHtmlService::new()
}

// ============================================================================
// UniFFI Stateful Service Object
// ============================================================================

/// Stateful UniFFI service providing high-performance zero-copy HTML transformation and VFS routing.
#[derive(uniffi::Object, Default)]
pub struct UniFFIHtmlService {}

#[uniffi::export]
impl UniFFIHtmlService {
    /// Constructs a new thread-safe HTML transformation service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Probes the format classification of an in-memory HTML byte buffer.
    pub fn probe_bytes(
        &self,
        bytes: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIHtmlFormat, UniFFIHtmlError> {
        uniffi_probe_html_bytes(bytes, file_name)
    }

    /// Probes the format classification of a file on disk.
    pub fn probe_file(&self, file_path: String) -> Result<UniFFIHtmlFormat, UniFFIHtmlError> {
        uniffi_probe_html_file(file_path)
    }

    /// Rewrites relative resource links to `ttzip-vfs://` URLs and sanitizes DOM markup according to policy.
    pub fn rewrite_vfs(
        &self,
        html_content: String,
        base_vfs_prefix: String,
        policy: UniFFIHtmlSanitizationPolicy,
    ) -> Result<UniFFIHtmlTransformResult, UniFFIHtmlError> {
        uniffi_rewrite_html_vfs(html_content, base_vfs_prefix, policy)
    }

    /// Sanitizes HTML markup according to security policy.
    pub fn sanitize(
        &self,
        html_content: String,
        policy: UniFFIHtmlSanitizationPolicy,
    ) -> Result<String, UniFFIHtmlError> {
        uniffi_sanitize_html(html_content, policy)
    }

    /// Extracts all asset and resource links from HTML markup.
    pub fn extract_resources(
        &self,
        html_content: String,
    ) -> Result<Vec<UniFFIHtmlResourceLink>, UniFFIHtmlError> {
        uniffi_extract_html_resources(html_content)
    }
}
