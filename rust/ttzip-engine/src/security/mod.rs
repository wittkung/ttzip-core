// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Security, sandbox isolation, path defense, and threat scanning modules.

pub mod acl;
pub mod audio_defense;
pub mod blake3_defense;
pub mod brotli_defense;
pub mod bzip2_defense;
pub mod deflate_ng_defense;
pub mod ebook_defense;
pub mod ed25519_defense;
pub mod html_defense;
pub mod image_defense;
pub mod libdeflate_defense;
pub mod license;
pub mod lzfse_defense;
pub mod media_defense;
pub mod mmap_defense;
pub mod office_defense;
pub mod path_sanitizer;
pub mod pdf_defense;
pub mod secure_extract;
pub mod snappy_defense;
pub mod syntax_defense;
pub mod system_defense;
pub mod tar_defense;
pub mod text_encoding_defense;
pub mod uniffi_defense;
pub mod xml_defense;
pub mod xz_defense;
pub mod zip_defense;
pub mod zopfli_defense;

#[cfg(test)]
mod tests;

pub use acl::*;
pub use audio_defense::*;
pub use blake3_defense::*;
pub use brotli_defense::*;
pub use bzip2_defense::*;
pub use deflate_ng_defense::*;
pub use ebook_defense::*;
pub use ed25519_defense::*;
pub use html_defense::{
    AttributeQuotaGuard, AttributeQuotaReport, ExternalNetworkSandboxGuard, HtmlDefenseError,
    HtmlDefenseOptions, HtmlDefenseReport, HtmlMemoryBudgetGuard, HtmlSanitizerGuard,
    HtmlSecurityPipeline, HtmlSecurityPipelineResult, NetworkSandboxOptions,
    NetworkSandboxReport, SanitizerReport, SensitiveHtmlBuffer, TagDepthReport,
    TagNestingDepthGuard, DEFAULT_HTML_TRUNCATION_THRESHOLD, DEFAULT_MAX_HTML_ATTRIBUTE_LEN,
    DEFAULT_MAX_HTML_ATTRIBUTES_PER_ELEMENT, DEFAULT_MAX_HTML_DEPTH,
    DEFAULT_MAX_HTML_MEMORY_BUDGET, DEFAULT_MAX_HTML_TEXT_CHUNK_LEN,
    DEFAULT_MAX_HTML_TOTAL_ATTRIBUTE_LEN, DEFAULT_MAX_UNCLOSED_TAGS, DEFAULT_STRICT_CSP_CONTENT,
    DEFAULT_VFS_URI_PREFIX, HTML_TRUNCATION_BANNER,
};
pub use image_defense::*;
pub use libdeflate_defense::*;
pub use license::*;
pub use lzfse_defense::*;
pub use media_defense::*;
pub use mmap_defense::*;
pub use office_defense::*;
pub use path_sanitizer::*;
pub use pdf_defense::*;
pub use secure_extract::*;
pub use snappy_defense::*;
pub use syntax_defense::*;
pub use system_defense::{
    AppcastSignatureGuard, BinaryDeltaBudgetOptions, BinaryDeltaMemoryBudgetGuard,
    DeltaMemoryPermit, PathTraversalOptions, PathTraversalProtectionGuard, SandboxEscapingGuard,
    SandboxEscapingOptions, SensitiveCredentialBuffer, SensitiveCredentialString,
    SystemDefenseError, SystemDefenseOptions, SystemPreflightReport, SystemSecurityPipeline,
    SystemUpdateRequest, TempDirectoryCleanupGuard, TempDirectoryGuard,
    DEFAULT_MAX_DELTA_EXPANSION_RATIO, DEFAULT_MAX_DELTA_INSTRUCTIONS,
    DEFAULT_MAX_DELTA_MEMORY_BUDGET, DEFAULT_MAX_DELTA_PATCH_SIZE,
};
pub use tar_defense::*;
pub use text_encoding_defense::*;
pub use uniffi_defense::*;
pub use xml_defense::*;
pub use xz_defense::*;
pub use zip_defense::*;
pub use zopfli_defense::*;
