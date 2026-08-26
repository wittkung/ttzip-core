// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

use anyhow::{bail, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

use crate::codegen::CatalogContract;

/// Verifies format specifier consistency (%1$@, %d, %s, %1$.1f) across all translations.
pub fn validate_format_specifiers(contract_path: &Path) -> Result<()> {
    let content = fs::read_to_string(contract_path)?;
    let contract: CatalogContract = serde_json::from_str(&content)?;

    let languages = ["en", "zh-Hans", "zh-Hant", "ja", "de", "fr", "es"];
    let specifier_re = Regex::new(r"%(\d+\$)?[.\d]*[@sdf]")?;

    for (k, entry) in &contract.entries {
        let en_val = entry.translations.get("en").cloned().unwrap_or_default();
        let en_specifiers: Vec<String> = specifier_re
            .find_iter(&en_val.replace("%%", ""))
            .map(|m| m.as_str().to_string())
            .collect();

        for lang in languages {
            let target_val = entry.translations.get(lang).cloned().unwrap_or_default();
            let target_specifiers: Vec<String> = specifier_re
                .find_iter(&target_val.replace("%%", ""))
                .map(|m| m.as_str().to_string())
                .collect();

            if en_specifiers.len() != target_specifiers.len() {
                bail!(
                    "Key '{}' specifier count mismatch in '{}'! Expected {:?}, found {:?}",
                    k,
                    lang,
                    en_specifiers,
                    target_specifiers
                );
            }
        }
    }

    println!("✓ Format specifier safety verified across all {} keys.", contract.entries.len());
    Ok(())
}
