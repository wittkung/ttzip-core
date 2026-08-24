// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Pure Safe Rust Unified Internationalization (i18n) & CLDR Formatting Engine.
//!
//! Provides compile-time zero-allocation static dictionaries across 7 languages,
//! CLDR-compliant byte size / throughput formatters, and UniFFI exportable bindings.

pub mod catalog;
pub mod catalogs;
pub mod engine;
pub mod formatting;

#[cfg(test)]
pub mod tests;

pub use catalog::{get_string_or_fallback, lookup, AppLanguage};
pub use engine::TTZipLocalizationEngine;
pub use formatting::{format_bytes, format_throughput, localize_error, ByteSizeStandard};
