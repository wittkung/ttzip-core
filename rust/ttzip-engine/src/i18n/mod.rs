// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Unified Internationalization (i18n) & CLDR Formatting Engine.
//!
//! Powered by standalone `tt-i18n-core` SDK.

pub use tt_i18n::catalog::{self, get_string_or_fallback, lookup, AppLanguage};
pub use tt_i18n::catalogs;
pub use tt_i18n::cldr as formatting;
pub use tt_i18n::cldr::{format_bytes, format_throughput, localize_error, ByteSizeStandard, PluralCategory};
pub use tt_i18n::engine::{self, TTLocalizationEngine as TTZipLocalizationEngine};

#[cfg(test)]
pub mod tests;
