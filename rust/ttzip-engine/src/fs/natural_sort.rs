// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance zero-allocation natural string comparator and sorter.

use std::cmp::Ordering;

/// Compares two strings using natural sort ordering:
/// - Contiguous digit chunks are compared as numbers rather than lexicographically.
/// - Alphabetical characters are compared case-insensitively with case-sensitive tie breaking.
/// - Zero-allocation and bounds-safe.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut chars_a = a.chars().peekable();
    let mut chars_b = b.chars().peekable();

    while let (Some(&ca), Some(&cb)) = (chars_a.peek(), chars_b.peek()) {
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let mut zeros_a = 0usize;
            while let Some(&c) = chars_a.peek() {
                if c == '0' {
                    zeros_a += 1;
                    chars_a.next();
                } else {
                    break;
                }
            }

            let mut zeros_b = 0usize;
            while let Some(&c) = chars_b.peek() {
                if c == '0' {
                    zeros_b += 1;
                    chars_b.next();
                } else {
                    break;
                }
            }

            let mut digits_a = [0u8; 32];
            let mut len_a = 0usize;
            while let Some(&c) = chars_a.peek() {
                if c.is_ascii_digit() {
                    if len_a < 32 {
                        digits_a[len_a] = c as u8;
                    }
                    len_a += 1;
                    chars_a.next();
                } else {
                    break;
                }
            }

            let mut digits_b = [0u8; 32];
            let mut len_b = 0usize;
            while let Some(&c) = chars_b.peek() {
                if c.is_ascii_digit() {
                    if len_b < 32 {
                        digits_b[len_b] = c as u8;
                    }
                    len_b += 1;
                    chars_b.next();
                } else {
                    break;
                }
            }

            if len_a != len_b {
                return len_a.cmp(&len_b);
            }

            let cmp_len = len_a.min(32);
            match digits_a[..cmp_len].cmp(&digits_b[..cmp_len]) {
                Ordering::Equal => {
                    // Equal numerical value; if both were pure zeros, compare zero counts
                    if len_a == 0 && zeros_a != zeros_b {
                        // More leading zeros comes after fewer leading zeros or tie break
                    }
                }
                other => return other,
            }
        } else {
            let mut it_a = ca.to_lowercase();
            let mut it_b = cb.to_lowercase();
            let mut diff = Ordering::Equal;
            loop {
                match (it_a.next(), it_b.next()) {
                    (Some(x), Some(y)) => match x.cmp(&y) {
                        Ordering::Equal => continue,
                        ord => {
                            diff = ord;
                            break;
                        }
                    },
                    (None, None) => break,
                    (None, Some(_)) => {
                        diff = Ordering::Less;
                        break;
                    }
                    (Some(_), None) => {
                        diff = Ordering::Greater;
                        break;
                    }
                }
            }
            if diff != Ordering::Equal {
                return diff;
            }
            chars_a.next();
            chars_b.next();
        }
    }

    match (chars_a.next(), chars_b.next()) {
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (Some(_), Some(_)) | (None, None) => a.cmp(b),
    }
}

/// Sorts a slice of string-like items in place using natural ordering.
pub fn natural_sort<T: AsRef<str>>(items: &mut [T]) {
    items.sort_by(|a, b| natural_cmp(a.as_ref(), b.as_ref()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_sort_ordering() {
        let mut files = vec![
            "file10.txt",
            "file1.txt",
            "file2.txt",
            "file20.txt",
            "file03.txt",
            "file0.txt",
        ];
        natural_sort(&mut files);
        assert_eq!(
            files,
            vec![
                "file0.txt",
                "file1.txt",
                "file2.txt",
                "file03.txt",
                "file10.txt",
                "file20.txt"
            ]
        );
    }

    #[test]
    fn test_natural_sort_case_and_versions() {
        let mut versions = vec!["v1.10.0", "v1.2.0", "V1.2.1", "v1.1.0"];
        natural_sort(&mut versions);
        assert_eq!(versions, vec!["v1.1.0", "v1.2.0", "V1.2.1", "v1.10.0"]);
    }
}
