// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Proc-Macro Export Layer.
//!
//! Provides typed, memory-safe, and Swift 6 Sendable bindings directly generated
//! from Rust business logic without manual C-ABI pointers.

pub mod archive;
pub mod audio;
pub mod benchmark;
pub mod callback;
pub mod codecs;
pub mod crypto;
pub mod defensive;
pub mod disk_scanner;
#[cfg(feature = "probe")]
pub mod document;
pub mod ebook_meta;
pub mod epub;
pub mod error_mapping;
pub mod extraction;
mod extraction_observer;
pub mod extraction_session;
mod extraction_session_types;
pub mod fs_apfs;
pub mod hasher_stream;
#[cfg(feature = "probe")]
pub mod image;
pub mod image_meta;
pub mod integrity;
pub mod logging;
pub mod memory;
pub mod mmap_stream;
pub mod plugin_auth;
pub mod preset;
pub mod semantic_search;
pub mod smart_extract;
pub mod standards;
pub mod system_meta;
pub mod text_encoding;
pub mod transaction;
pub mod types;
pub mod vault;
pub mod vfs;
pub mod vfs_paging;
pub mod vfs_loader;
pub mod vfs_search;
pub mod vfs_session_status;
pub mod xml_meta;

pub use vfs_paging::*;
pub use vfs_session_status::*;

pub use system_meta::{
    uniffi_apply_delta_patch, uniffi_calculate_tree_hash, uniffi_create_delta_patch,
    uniffi_verify_appcast_signature, UniFFIAppcastItem, UniFFIAppcastMetadata,
    UniFFIDeltaFormat, UniFFIDeltaPatchResult, UniFFISystemError, UniFFISystemService,
    TTZIP_DELTA_HEADER_LEN, TTZIP_DELTA_MAGIC,
};

pub use archive::*;
pub use audio::*;
pub use benchmark::*;
pub use callback::*;
pub use codecs::*;
pub use crypto::*;
pub use disk_scanner::*;
#[cfg(feature = "probe")]
pub use document::*;
pub use ebook_meta::{
    uniffi_extract_ebook_chapter, uniffi_extract_ebook_cover, uniffi_extract_ebook_metadata,
    uniffi_extract_ebook_resource, uniffi_extract_ebook_toc, uniffi_get_ebook_spine,
    uniffi_probe_ebook_bytes, UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat,
    UniFFIEbookMetadata, UniFFIEbookResource, UniFFIEbookService, UniFFIEbookSpineItem,
    UniFFIEbookTocNode,
};
pub use epub::*;
pub use error_mapping::*;
pub use extraction::*;
pub use extraction_session::*;
pub use fs_apfs::*;
pub use hasher_stream::*;
#[cfg(feature = "probe")]
pub use image::*;
pub use image_meta::*;
pub use integrity::*;
pub use logging::*;
pub use memory::*;
pub use mmap_stream::*;
pub use plugin_auth::*;
pub use preset::*;
pub use semantic_search::*;
pub use smart_extract::*;
pub use standards::*;
pub use text_encoding::*;
pub use transaction::*;
pub use types::*;
pub use vault::*;
pub use vfs::*;
pub use xml_meta::*;
pub use crate::archive::nested_vfs::VirtualFileStream;
pub use crate::security::license::*;

pub use crate::i18n::{AppLanguage, ByteSizeStandard, TTZipLocalizationEngine};

/// Convenient static function to retrieve localized string via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_get_string(key: String, lang: AppLanguage) -> String {
    crate::i18n::get_string_or_fallback(&key, lang).to_string()
}

/// Convenient static function to format byte sizes via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_format_bytes(bytes: i64, standard: ByteSizeStandard, lang: AppLanguage) -> String {
    crate::i18n::format_bytes(bytes, standard, lang)
}

/// Convenient static function to format throughput via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_format_throughput(mb_per_sec: f64, lang: AppLanguage) -> String {
    crate::i18n::format_throughput(mb_per_sec, lang)
}

/// Convenient static function to localize errors via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_localize_error(
    error_code: i32,
    param1: Option<String>,
    param2: Option<String>,
    lang: AppLanguage,
) -> String {
    crate::i18n::localize_error(error_code, param1.as_deref(), param2.as_deref(), lang)
}
