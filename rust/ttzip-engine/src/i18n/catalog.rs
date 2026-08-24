// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::catalogs;

/// Strongly-typed application language identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum AppLanguage {
    En,
    ZhHans,
    ZhHant,
    Ja,
    De,
    Fr,
    Es,
}

impl AppLanguage {
    pub fn bcp47(&self) -> &'static str {
        match self {
            AppLanguage::En => "en",
            AppLanguage::ZhHans => "zh-Hans",
            AppLanguage::ZhHant => "zh-Hant",
            AppLanguage::Ja => "ja",
            AppLanguage::De => "de",
            AppLanguage::Fr => "fr",
            AppLanguage::Es => "es",
        }
    }

    pub fn from_bcp47(code: &str) -> Self {
        let clean = code.trim().to_lowercase();
        if clean.starts_with("zh-hant") || clean.starts_with("zh-tw") || clean.starts_with("zh-hk") {
            AppLanguage::ZhHant
        } else if clean.starts_with("zh") {
            AppLanguage::ZhHans
        } else if clean.starts_with("ja") {
            AppLanguage::Ja
        } else if clean.starts_with("de") {
            AppLanguage::De
        } else if clean.starts_with("fr") {
            AppLanguage::Fr
        } else if clean.starts_with("es") {
            AppLanguage::Es
        } else {
            AppLanguage::En
        }
    }

    fn slice(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            AppLanguage::En => catalogs::en::STRINGS,
            AppLanguage::ZhHans => catalogs::zh_hans::STRINGS,
            AppLanguage::ZhHant => catalogs::zh_hant::STRINGS,
            AppLanguage::Ja => catalogs::ja::STRINGS,
            AppLanguage::De => catalogs::de::STRINGS,
            AppLanguage::Fr => catalogs::fr::STRINGS,
            AppLanguage::Es => catalogs::es::STRINGS,
        }
    }
}

/// Zero-allocation O(log N) binary search lookup directly in .rodata slice.
#[inline]
pub fn lookup(key: &str, lang: AppLanguage) -> Option<&'static str> {
    let slice = lang.slice();
    slice
        .binary_search_by_key(&key, |&(k, _)| k)
        .ok()
        .map(|idx| slice[idx].1)
}

/// Lookup localized string with fallback to English, then returning the raw key if absent.
#[inline]
pub fn get_string_or_fallback(key: &str, lang: AppLanguage) -> &'static str {
    if let Some(val) = lookup(key, lang) {
        return val;
    }
    if lang != AppLanguage::En {
        if let Some(val) = lookup(key, AppLanguage::En) {
            return val;
        }
    }
    ""
}
