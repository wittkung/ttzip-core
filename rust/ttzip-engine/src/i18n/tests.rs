// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

#[cfg(test)]
mod tests {
    use crate::i18n::catalog::AppLanguage;
    use crate::i18n::catalogs;
    use crate::i18n::engine::TTZipLocalizationEngine;
    use crate::i18n::formatting::ByteSizeStandard;
    use std::collections::HashSet;

    #[test]
    fn test_all_catalogs_have_equal_keys_and_no_missing_entries() {
        let en_keys: HashSet<&str> = catalogs::en::STRINGS.iter().map(|&(k, _)| k).collect();
        assert_eq!(en_keys.len(), 415, "English catalog should have exactly 415 keys");

        let languages = [
            AppLanguage::ZhHans,
            AppLanguage::ZhHant,
            AppLanguage::Ja,
            AppLanguage::De,
            AppLanguage::Fr,
            AppLanguage::Es,
        ];

        for lang in &languages {
            let cat_keys: HashSet<&str> = lang_slice(lang).iter().map(|&(k, _)| k).collect();
            let missing: Vec<_> = en_keys.difference(&cat_keys).collect();
            assert!(
                missing.is_empty(),
                "Language {:?} is missing keys: {:?}",
                lang,
                missing
            );
            assert_eq!(
                cat_keys.len(),
                en_keys.len(),
                "Language {:?} key count mismatch",
                lang
            );
        }
    }

    fn lang_slice(lang: &AppLanguage) -> &'static [(&'static str, &'static str)] {
        match lang {
            AppLanguage::En => catalogs::en::STRINGS,
            AppLanguage::ZhHans => catalogs::zh_hans::STRINGS,
            AppLanguage::ZhHant => catalogs::zh_hant::STRINGS,
            AppLanguage::Ja => catalogs::ja::STRINGS,
            AppLanguage::De => catalogs::de::STRINGS,
            AppLanguage::Fr => catalogs::fr::STRINGS,
            AppLanguage::Es => catalogs::es::STRINGS,
        }
    }

    #[test]
    fn test_zero_alloc_lookup_performance() {
        let engine = TTZipLocalizationEngine::new();
        let val = engine.get_string("common.ok", AppLanguage::ZhHans);
        assert_eq!(val, "好");

        let val_de = engine.get_string("common.cancel", AppLanguage::De);
        assert_eq!(val_de, "Abbrechen");

        let val_fr = engine.get_string("sidebar.vault", AppLanguage::Fr);
        assert_eq!(val_fr, "Coffre-fort");
    }

    #[test]
    fn test_byte_size_formatter_delimiters() {
        let engine = TTZipLocalizationEngine::new();
        let bytes = 1536 * 1000; // 1.5 MB in SI

        let en_str = engine.format_bytes(bytes, ByteSizeStandard::MetricSI, AppLanguage::En);
        assert!(en_str.contains('.'), "English should have '.'");
        assert!(en_str.contains("MB"));

        let de_str = engine.format_bytes(bytes, ByteSizeStandard::MetricSI, AppLanguage::De);
        assert!(de_str.contains(','), "German should have ','");
        assert!(de_str.contains("MB"));
    }

    #[test]
    fn test_error_localization() {
        let engine = TTZipLocalizationEngine::new();
        let err_en = engine.localize_error(-1, None, None, AppLanguage::En);
        assert!(!err_en.is_empty());

        let err_zh = engine.localize_error(-1, None, None, AppLanguage::ZhHans);
        assert!(!err_zh.is_empty());
    }
}
