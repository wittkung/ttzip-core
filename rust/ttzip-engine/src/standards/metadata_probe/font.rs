// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TrueType, OpenType, and Web Font format header probing (TTF, OTF, WOFF, WOFF2).

use super::types::FontProbeResult;

/// Probes TrueType (TTF) and OpenType (OTF).
pub fn probe_ttf_otf(data: &[u8]) -> Option<FontProbeResult> {
    if data.len() < 12 {
        return None;
    }

    let tag = &data[0..4];
    let is_cff = tag == b"OTTO";
    let is_ttf = tag == b"\x00\x01\x00\x00" || tag == b"true" || tag == b"typ1";

    if !is_cff && !is_ttf {
        return None;
    }

    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let mut units_per_em = 1000u32;
    let mut num_glyphs = 0u32;
    let mut is_variable = false;
    let mut font_family = None;
    let mut font_subfamily = None;
    let mut postscript_name = None;

    let mut offset = 12;
    for _ in 0..num_tables.min(64) {
        if offset + 16 > data.len() {
            break;
        }

        let tbl_tag = &data[offset..offset + 4];
        let tbl_offset = u32::from_be_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]) as usize;
        let tbl_len = u32::from_be_bytes([data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15]]) as usize;

        if tbl_offset + tbl_len <= data.len() {
            let tbl_data = &data[tbl_offset..tbl_offset + tbl_len];
            if tbl_tag == b"head" && tbl_data.len() >= 20 {
                units_per_em = u16::from_be_bytes([tbl_data[18], tbl_data[19]]) as u32;
            } else if tbl_tag == b"maxp" && tbl_data.len() >= 6 {
                num_glyphs = u16::from_be_bytes([tbl_data[4], tbl_data[5]]) as u32;
            } else if tbl_tag == b"fvar" {
                is_variable = true;
            } else if tbl_tag == b"name" && tbl_data.len() >= 6 {
                let count = u16::from_be_bytes([tbl_data[2], tbl_data[3]]) as usize;
                let string_offset = u16::from_be_bytes([tbl_data[4], tbl_data[5]]) as usize;

                let mut n_offset = 6;
                for _ in 0..count.min(128) {
                    if n_offset + 12 > tbl_data.len() {
                        break;
                    }

                    let platform_id = u16::from_be_bytes([tbl_data[n_offset], tbl_data[n_offset + 1]]);
                    let name_id = u16::from_be_bytes([tbl_data[n_offset + 6], tbl_data[n_offset + 7]]);
                    let length = u16::from_be_bytes([tbl_data[n_offset + 8], tbl_data[n_offset + 9]]) as usize;
                    let offset_val = u16::from_be_bytes([tbl_data[n_offset + 10], tbl_data[n_offset + 11]]) as usize;

                    let str_start = string_offset + offset_val;
                    if str_start + length <= tbl_data.len() {
                        let raw_bytes = &tbl_data[str_start..str_start + length];
                        let val = if platform_id == 3 || platform_id == 0 {
                            let u16_chars: Vec<u16> = raw_bytes
                                .chunks_exact(2)
                                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                                .collect();
                            String::from_utf16_lossy(&u16_chars)
                        } else {
                            String::from_utf8_lossy(raw_bytes).to_string()
                        };

                        let val_clean = val.trim().to_string();
                        if !val_clean.is_empty() {
                            match name_id {
                                1 if font_family.is_none() => font_family = Some(val_clean),
                                2 if font_subfamily.is_none() => font_subfamily = Some(val_clean),
                                6 if postscript_name.is_none() => postscript_name = Some(val_clean),
                                _ => {}
                            }
                        }
                    }

                    n_offset += 12;
                }
            }
        }

        offset += 16;
    }

    Some(FontProbeResult {
        font_family,
        font_subfamily,
        postscript_name,
        units_per_em,
        num_glyphs,
        is_variable,
        format_flavor: if is_cff { "OpenType (CFF)".to_string() } else { "TrueType (TTF)".to_string() },
    })
}

/// Probes WOFF (Web Open Font Format 1.0).
pub fn probe_woff(data: &[u8]) -> Option<FontProbeResult> {
    if data.len() < 44 || !data.starts_with(b"wOFF") {
        return None;
    }

    let num_tables = u16::from_be_bytes([data[12], data[13]]) as u32;
    let total_sfnt_size = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

    Some(FontProbeResult {
        font_family: None,
        font_subfamily: None,
        postscript_name: None,
        units_per_em: 1000,
        num_glyphs: num_tables * 50,
        is_variable: false,
        format_flavor: format!("WOFF 1.0 (sfnt size {total_sfnt_size} bytes)"),
    })
}

/// Probes WOFF2 (Web Open Font Format 2.0).
pub fn probe_woff2(data: &[u8]) -> Option<FontProbeResult> {
    if data.len() < 48 || !data.starts_with(b"wOF2") {
        return None;
    }

    let total_sfnt_size = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

    Some(FontProbeResult {
        font_family: None,
        font_subfamily: None,
        postscript_name: None,
        units_per_em: 1000,
        num_glyphs: 0,
        is_variable: false,
        format_flavor: format!("WOFF 2.0 (uncompressed {total_sfnt_size} bytes)"),
    })
}
