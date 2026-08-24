// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::sync::Arc;

use super::catalog::{get_string_or_fallback, lookup, AppLanguage};
use super::formatting::{format_bytes, format_throughput, localize_error, ByteSizeStandard};

/// Thread-safe localization engine for cross-platform SDKs.
#[derive(Default, uniffi::Object)]
pub struct TTZipLocalizationEngine;

#[uniffi::export]
impl TTZipLocalizationEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    /// Retrieve localized string by key for given language with English fallback.
    pub fn get_string(&self, key: &str, lang: AppLanguage) -> String {
        get_string_or_fallback(key, lang).to_string()
    }

    /// Check whether key exists in dictionary.
    pub fn has_key(&self, key: &str, lang: AppLanguage) -> bool {
        lookup(key, lang).is_some()
    }

    /// Format byte size according to standard and language delimiters.
    pub fn format_bytes(&self, bytes: i64, standard: ByteSizeStandard, lang: AppLanguage) -> String {
        format_bytes(bytes, standard, lang)
    }

    /// Format throughput rate in MB/s according to language delimiters.
    pub fn format_throughput(&self, mb_per_sec: f64, lang: AppLanguage) -> String {
        format_throughput(mb_per_sec, lang)
    }

    /// Translate error code and optional arguments to localized string.
    pub fn localize_error(
        &self,
        error_code: i32,
        param1: Option<String>,
        param2: Option<String>,
        lang: AppLanguage,
    ) -> String {
        localize_error(
            error_code,
            param1.as_deref(),
            param2.as_deref(),
            lang,
        )
    }
}
