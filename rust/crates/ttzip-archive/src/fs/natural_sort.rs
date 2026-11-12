// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-performance zero-allocation natural string comparator and sorter.

use std::cmp::Ordering;

fn consume_digit_run<I: Iterator<Item = char>>(
    chars: &mut std::iter::Peekable<I>,
) -> (usize, [u8; 32], usize) {
    let mut zeros = 0usize;
    while chars.peek() == Some(&'0') {
        zeros += 1;
        chars.next();
    }

    let mut digits = [0u8; 32];
    let mut len = 0usize;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            if len < 32 {
                digits[len] = c as u8;
            }
            len += 1;
            chars.next();
        } else {
            break;
        }
    }
    (zeros, digits, len)
}

fn cmp_digit_runs<I: Iterator<Item = char>>(
    chars_a: &mut std::iter::Peekable<I>,
    chars_b: &mut std::iter::Peekable<I>,
) -> Ordering {
    let (zeros_a, digits_a, len_a) = consume_digit_run(chars_a);
    let (zeros_b, digits_b, len_b) = consume_digit_run(chars_b);

    if len_a != len_b {
        return len_a.cmp(&len_b);
    }

    let cmp_len = len_a.min(32);
    match digits_a[..cmp_len].cmp(&digits_b[..cmp_len]) {
        Ordering::Equal if len_a == 0 && zeros_a != zeros_b => zeros_a.cmp(&zeros_b),
        other => other,
    }
}

fn cmp_char_case_insensitive(ca: char, cb: char) -> Ordering {
    if ca.is_ascii() && cb.is_ascii() {
        return (ca as u8).to_ascii_lowercase().cmp(&(cb as u8).to_ascii_lowercase());
    }

    let mut it_a = ca.to_lowercase();
    let mut it_b = cb.to_lowercase();
    loop {
        match (it_a.next(), it_b.next()) {
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => continue,
                ord => return ord,
            },
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

/// Compares two strings using natural sort ordering:
/// - Contiguous digit chunks are compared as numbers rather than lexicographically.
/// - Alphabetical characters are compared case-insensitively with case-sensitive tie breaking.
/// - Zero-allocation and bounds-safe.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut chars_a = a.chars().peekable();
    let mut chars_b = b.chars().peekable();

    while let (Some(&ca), Some(&cb)) = (chars_a.peek(), chars_b.peek()) {
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let ord = cmp_digit_runs(&mut chars_a, &mut chars_b);
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            let ord = cmp_char_case_insensitive(ca, cb);
            if ord != Ordering::Equal {
                return ord;
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

    #[test]
    fn test_natural_sort_pure_zeros() {
        let mut files = vec!["file00.txt", "file0.txt", "file000.txt", "file01.txt"];
        natural_sort(&mut files);
        assert_eq!(
            files,
            vec!["file0.txt", "file00.txt", "file000.txt", "file01.txt"]
        );

        let mut items = vec!["00_a", "0_b"];
        natural_sort(&mut items);
        assert_eq!(items, vec!["0_b", "00_a"]);
    }
}

