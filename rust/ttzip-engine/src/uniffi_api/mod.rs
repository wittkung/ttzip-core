// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Proc-Macro Export Layer.
//!
//! Provides typed, memory-safe, and Swift 6 Sendable bindings directly generated
//! from Rust business logic without manual C-ABI pointers.

pub mod archive;
pub mod audio;
pub mod benchmark;
pub mod disk_scanner;
pub mod epub;
pub mod extraction;
pub mod integrity;
pub mod smart_extract;
pub mod syntax;
pub mod transaction;
pub mod types;
pub mod vault;
pub mod vfs;

pub use archive::*;
pub use audio::*;
pub use benchmark::*;
pub use disk_scanner::*;
pub use epub::*;
pub use extraction::*;
pub use integrity::*;
pub use smart_extract::*;
pub use syntax::*;
pub use transaction::*;
pub use types::*;
pub use vault::*;
pub use vfs::*;
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
