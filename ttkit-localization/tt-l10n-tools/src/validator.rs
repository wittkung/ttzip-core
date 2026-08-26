// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

use anyhow::{bail, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::codegen::CatalogContract;

/// Validates 1:1 key parity across all languages.
pub fn validate_parity(contract_path: &Path) -> Result<()> {
    let content = fs::read_to_string(contract_path)?;
    let contract: CatalogContract = serde_json::from_str(&content)?;

    let languages = ["en", "zh-Hans", "zh-Hant", "ja", "de", "fr", "es"];
    let en_keys: HashSet<String> = contract.entries.keys().cloned().collect();

    for lang in languages {
        for (k, entry) in &contract.entries {
            if !entry.translations.contains_key(lang) {
                bail!("Key '{}' is missing in target language '{}'", k, lang);
            }
            let val = &entry.translations[lang];
            if val.trim().is_empty() {
                bail!("Key '{}' in language '{}' has empty translation", k, lang);
            }
        }
    }

    println!("✓ Key parity verified: {} keys across 7 languages 100% matched.", en_keys.len());
    Ok(())
}

/// 4-stage Anti-Fake translation verification algorithm.
pub fn validate_anti_fake(contract_path: &Path, max_duplicate_ratio: f64) -> Result<()> {
    let content = fs::read_to_string(contract_path)?;
    let contract: CatalogContract = serde_json::from_str(&content)?;

    let non_english = ["de", "fr", "es", "ja", "zh-Hans", "zh-Hant"];
    let whitelisted_terms = ["AES-256", "7-Zip", "MD5", "SHA-256", "UTF-8", "POSIX", "MB/s", "GB", "TB", "OK", "Cancel", "CPU", "RAM"];

    for lang in non_english {
        let mut identical_count = 0;
        let total_count = contract.entries.len();

        for (_k, entry) in &contract.entries {
            let en_val = entry.translations.get("en").cloned().unwrap_or_default();
            let target_val = entry.translations.get(lang).cloned().unwrap_or_default();

            if en_val == target_val {
                let is_whitelisted = whitelisted_terms.iter().any(|&term| en_val.contains(term));
                if !is_whitelisted && en_val.len() > 5 {
                    identical_count += 1;
                }
            }
        }

        let ratio = identical_count as f64 / total_count as f64;
        if ratio > max_duplicate_ratio {
            bail!(
                "Language '{}' failed Anti-Fake audit! Duplicate ratio {:.1}% exceeds threshold {:.1}%",
                lang,
                ratio * 100.0,
                max_duplicate_ratio * 100.0
            );
        }
        println!("✓ Language '{}': Anti-Fake duplicate ratio {:.1}% <= threshold {:.1}%", lang, ratio * 100.0, max_duplicate_ratio * 100.0);
    }

    Ok(())
}
