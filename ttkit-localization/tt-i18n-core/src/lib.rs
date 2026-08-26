// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

//! Pure Safe Rust Unified Internationalization (i18n) & CLDR Formatting Engine.
//!
//! Provides compile-time zero-allocation static dictionaries across 7 languages,
//! CLDR-compliant byte size / throughput formatters, and UniFFI exportable bindings.

uniffi::setup_scaffolding!();

pub mod catalog;
pub mod catalogs;
pub mod cldr;
pub mod engine;

#[cfg(test)]
pub mod tests;

pub use catalog::{get_string_or_fallback, lookup, AppLanguage};
pub use cldr::{format_bytes, format_throughput, localize_error, ByteSizeStandard, PluralCategory};
pub use engine::TTLocalizationEngine;
