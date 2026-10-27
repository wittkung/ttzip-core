// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust Unified Internationalization (i18n) & CLDR Formatting Engine.
//!
pub mod catalog;
pub mod catalogs;
pub mod engine;
pub mod formatting;

#[cfg(test)]
pub mod tests;

pub use catalog::{get_string_or_fallback, lookup, AppLanguage};
pub use engine::TTZipLocalizationEngine;
pub use formatting::{format_bytes, format_throughput, localize_error, ByteSizeStandard};
