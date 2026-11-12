// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! POSIX.1-2001 PAX Extended Header parsing, formatting, and numeric conversions.
//!
//! Handles arbitrarily long filenames (`path`), large sizes (`size`), high-precision timestamps
//! (`mtime`), link targets (`linkpath`), and arbitrary UTF-8 metadata keywords.

use std::collections::HashMap;

/// Parsed PAX metadata attributes representation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaxAttributes {
    pub path: Option<String>,
    pub linkpath: Option<String>,
    pub size: Option<u64>,
    pub mtime_secs: Option<i64>,
    pub mtime_nanos: Option<u32>,
    pub uid: Option<u64>,
    pub gid: Option<u64>,
    pub uname: Option<String>,
    pub gname: Option<String>,
    pub raw_map: HashMap<String, String>,
}

/// Parses PAX Extended Header payload into `PaxAttributes`.
pub fn parse_pax_data(data: &[u8]) -> PaxAttributes {
    let mut attrs = PaxAttributes::default();
    let mut cursor = 0;

    while cursor < data.len() {
        let Some((record_len, kv_opt)) = next_pax_record(&data[cursor..]) else {
            break;
        };
        cursor += record_len;
        if let Some((key, val)) = kv_opt {
            apply_pax_attribute(&mut attrs, key, val);
        }
    }

    attrs
}

fn next_pax_record(remaining: &[u8]) -> Option<(usize, Option<(String, String)>)> {
    let space_pos = remaining.iter().position(|&b| b == b' ')?;
    let len_str = std::str::from_utf8(&remaining[..space_pos]).ok()?;
    let record_len: usize = len_str.parse().ok()?;
    if record_len <= space_pos || record_len > remaining.len() {
        return None;
    }

    let record_bytes = &remaining[..record_len];
    let kv_bytes = if record_bytes.ends_with(b"\n") {
        &record_bytes[space_pos + 1..record_bytes.len() - 1]
    } else {
        &record_bytes[space_pos + 1..]
    };

    let kv = (|| {
        let eq_pos = kv_bytes.iter().position(|&b| b == b'=')?;
        let key = std::str::from_utf8(&kv_bytes[..eq_pos]).ok()?.to_string();
        let val = std::str::from_utf8(&kv_bytes[eq_pos + 1..]).ok()?.to_string();
        Some((key, val))
    })();

    Some((record_len, kv))
}

fn apply_pax_attribute(attrs: &mut PaxAttributes, key: String, val: String) {
    match key.as_str() {
        "path" => attrs.path = Some(val.clone()),
        "linkpath" => attrs.linkpath = Some(val.clone()),
        "size" => attrs.size = val.parse::<u64>().ok().or(attrs.size),
        "mtime" => {
            let (secs, nanos) = parse_pax_timestamp(&val);
            attrs.mtime_secs = Some(secs);
            attrs.mtime_nanos = Some(nanos);
        }
        "uid" => attrs.uid = val.parse::<u64>().ok().or(attrs.uid),
        "gid" => attrs.gid = val.parse::<u64>().ok().or(attrs.gid),
        "uname" => attrs.uname = Some(val.clone()),
        "gname" => attrs.gname = Some(val.clone()),
        _ => {}
    }
    attrs.raw_map.insert(key, val);
}

/// Parses PAX timestamp formatted as `<seconds>[.<nanoseconds>]`.
pub fn parse_pax_timestamp(val: &str) -> (i64, u32) {
    if let Some((secs_str, frac_str)) = val.split_once('.') {
        let secs = secs_str.parse::<i64>().unwrap_or(0);
        let mut nanos_padded = frac_str.to_string();
        if nanos_padded.len() > 9 {
            nanos_padded.truncate(9);
        } else {
            while nanos_padded.len() < 9 {
                nanos_padded.push('0');
            }
        }
        let nanos = nanos_padded.parse::<u32>().unwrap_or(0);
        (secs, nanos)
    } else {
        (val.parse::<i64>().unwrap_or(0), 0)
    }
}

/// Formats a single PAX record line: `"%d %s=%s\n"`.
pub fn format_pax_record(key: &str, val: &str) -> Vec<u8> {
    // Length includes length digits, space (1), key, '=', val, newline (1)
    let base_len = 1 + key.len() + 1 + val.len() + 1;
    let mut total_len = base_len + 2; // starting estimate for 1-2 digit length

    loop {
        let len_str = total_len.to_string();
        let actual_len = len_str.len() + base_len;
        if actual_len == total_len {
            break;
        }
        total_len = actual_len;
    }

    format!("{} {}={}\n", total_len, key, val).into_bytes()
}

/// Serializes multiple key-value pairs into a complete PAX extended header payload.
pub fn build_pax_payload(records: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in records {
        out.extend_from_slice(&format_pax_record(k, v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pax_record_roundtrip() {
        let payload = build_pax_payload(&[
            ("path", "very/long/nested/path/to/my_file.txt"),
            ("size", "8589934592"),
            ("mtime", "1700000000.500000000"),
        ]);

        let attrs = parse_pax_data(&payload);
        assert_eq!(attrs.path.as_deref(), Some("very/long/nested/path/to/my_file.txt"));
        assert_eq!(attrs.size, Some(8589934592));
        assert_eq!(attrs.mtime_secs, Some(1700000000));
        assert_eq!(attrs.mtime_nanos, Some(500000000));
    }
}
