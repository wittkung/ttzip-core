// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Constant-time, zero-allocation verification of Windows NT reserved devices and NTFS system metadata.

/// Constant-time, zero-allocation verification of Windows reserved device names operating on raw byte slices.
///
/// Recognizes:
/// - Base console and printer devices: `CON`, `PRN`, `AUX`, `NUL`
/// - Serial and parallel communication ports: `COM0..=9`, `LPT0..=9`
/// - Extended system device streams: `CLOCK$`, `CONIN$`, `CONOUT$`, `CONFIG$`
/// - Unicode superscript aliases folded by Windows NT kernel: `COM¹..=³`, `LPT¹..=³`
///
/// Extracts the base stem prior to the first `.` or `:` and trims whitespace, ensuring immunity
/// against multi-level extensions (`con.txt`, `aux.json`, `nul.tar.gz`) and stream attachments (`con:stream`).
#[inline]
#[must_use]
pub const fn is_windows_reserved_device_name_bytes(bytes: &[u8]) -> bool {
    let mut start = 0;
    while start < bytes.len() && (bytes[start] == b' ' || bytes[start] == b'\t') {
        start += 1;
    }

    // Stop at the first dot or colon to extract the stem when evaluating full filenames or stream paths
    let mut end = start;
    while end < bytes.len() && bytes[end] != b'.' && bytes[end] != b':' {
        end += 1;
    }

    // Trim trailing spaces and tabs from the stem
    while end > start && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\t') {
        end -= 1;
    }

    let len = end - start;
    if len < 3 {
        return false;
    }

    let stem = bytes.split_at(end).0.split_at(start).1;

    // Windows NT raw disk volume devices: PhysicalDrive0..=99
    if len >= 14 && len <= 16 {
        let (prefix, drive_num) = stem.split_at(13);
        if prefix.eq_ignore_ascii_case(b"PhysicalDrive") {
            let mut i = 0;
            let mut all_digits = true;
            while i < drive_num.len() {
                if !drive_num[i].is_ascii_digit() {
                    all_digits = false;
                    break;
                }
                i += 1;
            }
            if all_digits {
                return true;
            }
        }
    }

    if len > 7 {
        return false;
    }

    let b0 = stem[0].to_ascii_uppercase();
    let b1 = stem[1].to_ascii_uppercase();
    let b2 = stem[2].to_ascii_uppercase();

    // Fast-path length-first partitioning
    match (len, b0, b1, b2) {
        (3, b'C', b'O', b'N')
        | (3, b'P', b'R', b'N')
        | (3, b'A', b'U', b'X')
        | (3, b'N', b'U', b'L') => true,
        (4, b'C', b'O', b'M') | (4, b'L', b'P', b'T') => stem[3].is_ascii_digit(),
        (5, b'C', b'O', b'M') | (5, b'L', b'P', b'T') => {
            stem[3] == 0xC2 && matches!(stem[4], 0xB2 | 0xB3 | 0xB9)
        }
        (6, b'C', b'O', b'N') => stem.split_at(3).1.eq_ignore_ascii_case(b"IN$"),
        (6, b'C', b'L', b'O') => stem.split_at(3).1.eq_ignore_ascii_case(b"CK$"),
        (7, b'C', b'O', b'N') => {
            stem.split_at(3).1.eq_ignore_ascii_case(b"OUT$")
                || stem.split_at(3).1.eq_ignore_ascii_case(b"FIG$")
        }
        _ => false,
    }
}

/// Constant-time, zero-allocation verification of Windows reserved device names for string slices.
#[inline]
#[must_use]
pub const fn is_windows_reserved_device_name(stem: &str) -> bool {
    is_windows_reserved_device_name_bytes(stem.as_bytes())
}

/// Checks whether a stem name matches NTFS root filesystem metadata files.
///
/// Prevents denial-of-service and filesystem driver errors caused by extracting
/// entries targeting `$Mft`, `$LogFile`, `$Volume`, `$Bitmap`, `$Boot`, etc.
#[inline]
#[must_use]
pub const fn is_ntfs_reserved_metadata(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || bytes[0] != b'$' {
        return false;
    }
    let rest = bytes.split_at(1).1;
    match bytes.len() {
        4 => rest.eq_ignore_ascii_case(b"MFT"),
        5 => rest.eq_ignore_ascii_case(b"BOOT"),
        7 => {
            rest.eq_ignore_ascii_case(b"VOLUME")
                || rest.eq_ignore_ascii_case(b"BITMAP")
                || rest.eq_ignore_ascii_case(b"SECURE")
                || rest.eq_ignore_ascii_case(b"UPCASE")
                || rest.eq_ignore_ascii_case(b"EXTEND")
        }
        8 => {
            rest.eq_ignore_ascii_case(b"LOGFILE")
                || rest.eq_ignore_ascii_case(b"BADCLUS")
        }
        _ => false,
    }
}
