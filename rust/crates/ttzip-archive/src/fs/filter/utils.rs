// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Utility parsers for human-readable size, duration specs, and glob matchers.

use crate::fs::filter::expr::ComparisonOp;
use globset::{GlobBuilder, GlobMatcher};

/// Compiles a GlobMatcher from pattern with case-insensitivity.
pub fn build_glob_matcher(pattern: &str) -> Option<GlobMatcher> {
    let glob_str = if !pattern.contains('*') && !pattern.contains('?') {
        format!("*{}*", pattern)
    } else {
        pattern.to_string()
    };

    GlobBuilder::new(&glob_str)
        .case_insensitive(true)
        .literal_separator(false)
        .build()
        .map(|g| g.compile_matcher())
        .ok()
}

/// Parses human-readable size specifier into byte count.
pub fn parse_size(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut num_end = 0;
    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() || c == '.' {
            num_end = i + c.len_utf8();
        } else {
            break;
        }
    }

    let num_str = &trimmed[..num_end];
    let unit_str = trimmed[num_end..].trim().to_ascii_uppercase();
    let val: f64 = num_str.parse().ok()?;

    let multiplier: f64 = match unit_str.as_str() {
        "B" | "" => 1.0,
        "K" | "KB" => 1024.0,
        "M" | "MB" => 1024.0 * 1024.0,
        "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };

    Some((val * multiplier) as u64)
}

fn split_numeric_prefix(trimmed: &str) -> (&str, &str) {
    let mut num_end = 0;
    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() || c == '.' {
            num_end = i + c.len_utf8();
        } else {
            break;
        }
    }
    (&trimmed[..num_end], trimmed[num_end..].trim())
}

fn duration_unit_seconds(unit: &str) -> Option<f64> {
    match unit {
        "s" | "sec" => Some(1.0),
        "m" | "min" => Some(60.0),
        "h" | "hr" => Some(3600.0),
        "d" | "day" | "days" => Some(86400.0),
        "w" | "week" | "weeks" => Some(7.0 * 86400.0),
        "mth" | "month" | "months" => Some(30.0 * 86400.0),
        "y" | "yr" | "years" => Some(365.0 * 86400.0),
        _ => None,
    }
}

const fn invert_relative_date_op(default_op: ComparisonOp) -> ComparisonOp {
    match default_op {
        ComparisonOp::LessThan | ComparisonOp::LessThanOrEqual => ComparisonOp::GreaterThanOrEqual,
        ComparisonOp::GreaterThan | ComparisonOp::GreaterThanOrEqual => {
            ComparisonOp::LessThanOrEqual
        }
        ComparisonOp::Equals => ComparisonOp::Equals,
        ComparisonOp::NotEquals => ComparisonOp::NotEquals,
    }
}

/// Parses duration/date specifier into epoch timestamp and adjusted comparison operator.
pub fn parse_date_spec(
    spec: &str,
    default_op: ComparisonOp,
    ref_epoch_secs: i64,
) -> Option<(i64, ComparisonOp)> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (num_str, unit_raw) = split_numeric_prefix(trimmed);
    if !num_str.is_empty() && !unit_raw.is_empty() {
        if let Ok(val) = num_str.parse::<f64>() {
            let unit_secs = duration_unit_seconds(&unit_raw.to_ascii_lowercase())?;
            let target_time = ref_epoch_secs - ((val * unit_secs) as i64);
            return Some((target_time, invert_relative_date_op(default_op)));
        }
    }

    trimmed.parse::<i64>().ok().map(|epoch| (epoch, default_op))
}
