// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use super::catalog::{get_string_or_fallback, AppLanguage};

/// Byte sizing standard specification.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ByteSizeStandard {
    MetricSI,
    BinaryIEC,
}

/// Format decimal number with language-appropriate delimiter.
fn format_decimal(value: f64, decimals: usize, lang: AppLanguage) -> String {
    let raw = format!("{:.1$}", value, decimals);
    match lang {
        AppLanguage::De | AppLanguage::Fr | AppLanguage::Es => raw.replace('.', ","),
        _ => raw,
    }
}

/// Format byte sizes according to SI/IEC standards and locale delimiters.
pub fn format_bytes(bytes: i64, standard: ByteSizeStandard, lang: AppLanguage) -> String {
    if bytes < 0 {
        return "0 B".to_string();
    }
    let b = bytes as f64;

    match standard {
        ByteSizeStandard::MetricSI => {
            const KB: f64 = 1000.0;
            const MB: f64 = 1000.0 * 1000.0;
            const GB: f64 = 1000.0 * 1000.0 * 1000.0;
            const TB: f64 = 1000.0 * 1000.0 * 1000.0 * 1000.0;

            if b >= TB {
                format!("{} TB", format_decimal(b / TB, 2, lang))
            } else if b >= GB {
                format!("{} GB", format_decimal(b / GB, 2, lang))
            } else if b >= MB {
                format!("{} MB", format_decimal(b / MB, 1, lang))
            } else if b >= KB {
                format!("{} KB", format_decimal(b / KB, 1, lang))
            } else {
                format!("{} B", bytes)
            }
        }
        ByteSizeStandard::BinaryIEC => {
            const KIB: f64 = 1024.0;
            const MIB: f64 = 1024.0 * 1024.0;
            const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
            const TIB: f64 = 1024.0 * 1024.0 * 1024.0 * 1024.0;

            if b >= TIB {
                format!("{} TiB", format_decimal(b / TIB, 2, lang))
            } else if b >= GIB {
                format!("{} GiB", format_decimal(b / GIB, 2, lang))
            } else if b >= MIB {
                format!("{} MiB", format_decimal(b / MIB, 1, lang))
            } else if b >= KIB {
                format!("{} KiB", format_decimal(b / KIB, 1, lang))
            } else {
                format!("{} B", bytes)
            }
        }
    }
}

/// Format throughput rate in MB/s according to language conventions.
pub fn format_throughput(mb_per_sec: f64, lang: AppLanguage) -> String {
    format!("{} MB/s", format_decimal(mb_per_sec, 1, lang))
}

/// Localize archive error code with parameter interpolation.
pub fn localize_error(
    error_code: i32,
    param1: Option<&str>,
    param2: Option<&str>,
    lang: AppLanguage,
) -> String {
    match error_code {
        -1 => {
            let tmpl = get_string_or_fallback("error.file_not_found", lang);
            if let Some(p) = param1 {
                tmpl.replace("%1$@", p).replace("%@", p)
            } else {
                tmpl.to_string()
            }
        }
        -2 => {
            let tmpl = get_string_or_fallback("error.read_error", lang);
            if let Some(p) = param1 {
                tmpl.replace("%1$@", p).replace("%@", p)
            } else {
                tmpl.to_string()
            }
        }
        -3 => get_string_or_fallback("error.unsupported_format", lang).to_string(),
        -4 => get_string_or_fallback("error.password_required", lang).to_string(),
        -5 => get_string_or_fallback("error.incorrect_password", lang).to_string(),
        -6 => {
            let tmpl = get_string_or_fallback("error.unsupported_encryption", lang);
            let mut res = tmpl.to_string();
            if let Some(m) = param1 {
                res = res.replace("%1$@", m);
            }
            if let Some(p) = param2 {
                res = res.replace("%2$@", p);
            }
            res
        }
        -7 => {
            let tmpl = get_string_or_fallback("error.corrupt_data", lang);
            if let Some(p) = param1 {
                format!("{} ({})", tmpl, p)
            } else {
                tmpl.to_string()
            }
        }
        -8 => get_string_or_fallback("error.operation_cancelled", lang).to_string(),
        _ => {
            let tmpl = get_string_or_fallback("error.engine_failure", lang);
            let mut res = tmpl.replace("%1$d", &error_code.to_string());
            if let Some(msg) = param1 {
                res = res.replace("%2$@", msg);
            }
            res
        }
    }
}
