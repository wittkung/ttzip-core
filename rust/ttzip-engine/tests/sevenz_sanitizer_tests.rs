// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for 7-Zip bounded count memory safety guards
//! and safe path sanitization (Zip-Slip defense) subsystem.

use std::path::Path;
use ttzip_engine::sevenz::sanitizer::{
    bounded_count, bounded_usize, safe_join, DEFAULT_MAX_CODERS_LIMIT,
    DEFAULT_MAX_FILES_LIMIT, DEFAULT_MAX_FOLDERS_LIMIT, DEFAULT_MAX_STREAMS_LIMIT,
};
use ttzip_engine::sevenz::SevenZError;
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_bounded_count_within_limits() {
    assert_eq!(bounded_count(0, 100, "folders").unwrap(), 0);
    assert_eq!(bounded_count(50, 100, "folders").unwrap(), 50);
    assert_eq!(bounded_count(100, 100, "folders").unwrap(), 100);
    assert_eq!(
        bounded_count(10_000, DEFAULT_MAX_FILES_LIMIT, "files").unwrap(),
        10_000
    );
    assert_eq!(
        bounded_count(64, DEFAULT_MAX_CODERS_LIMIT, "coders").unwrap(),
        64
    );
    assert_eq!(
        bounded_count(500_000, DEFAULT_MAX_FOLDERS_LIMIT, "folders").unwrap(),
        500_000
    );
    assert_eq!(
        bounded_count(1_000_000, DEFAULT_MAX_STREAMS_LIMIT, "streams").unwrap(),
        1_000_000
    );
}

#[test]
fn test_bounded_count_exceeded_intercepted() {
    let res = bounded_count(101, 100, "folders");
    assert!(res.is_err());
    match res.unwrap_err() {
        SevenZError::CountLimitExceeded {
            field_name,
            value,
            limit,
        } => {
            assert_eq!(field_name, "folders");
            assert_eq!(value, 101);
            assert_eq!(limit, 100);
        }
        other => panic!("expected CountLimitExceeded, got {:?}", other),
    }

    // Huge untrusted count (e.g. 0xFFFFFFFFFFFFFFFF)
    let res_huge = bounded_count(u64::MAX, DEFAULT_MAX_FILES_LIMIT, "num_files");
    assert!(res_huge.is_err());
    match res_huge.unwrap_err() {
        SevenZError::CountLimitExceeded {
            field_name,
            value,
            limit,
        } => {
            assert_eq!(field_name, "num_files");
            assert_eq!(value, u64::MAX);
            assert_eq!(limit, DEFAULT_MAX_FILES_LIMIT);
        }
        other => panic!("expected CountLimitExceeded, got {:?}", other),
    }
}

#[test]
fn test_bounded_usize_passthrough_and_limit() {
    assert_eq!(bounded_usize(0, 4096, "buffer_size").unwrap(), 0);
    assert_eq!(bounded_usize(2048, 4096, "buffer_size").unwrap(), 2048);
    assert_eq!(bounded_usize(4096, 4096, "buffer_size").unwrap(), 4096);

    let err = bounded_usize(4097, 4096, "buffer_size").unwrap_err();
    match err {
        SevenZError::CountLimitExceeded {
            field_name,
            value,
            limit,
        } => {
            assert_eq!(field_name, "buffer_size");
            assert_eq!(value, 4097);
            assert_eq!(limit, 4096);
        }
        other => panic!("expected CountLimitExceeded, got {:?}", other),
    }
}

#[test]
fn test_sevenz_error_ttzip_status_conversions() {
    let err_count = SevenZError::CountLimitExceeded {
        field_name: "test",
        value: 999,
        limit: 10,
    };
    let status: TTZipStatus = err_count.into();
    assert_eq!(status, TTZipStatus::ErrOutOfMemory);

    let err_insecure = SevenZError::InsecurePath("bad/path".to_string());
    let status_sec: TTZipStatus = err_insecure.into();
    assert_eq!(status_sec, TTZipStatus::ErrSecurityViolation);
}

#[test]
fn test_safe_join_malicious_paths_interception() {
    let dest_root = Path::new("/var/sandbox/extract");

    let malicious_cases = [
        "../evil.txt",
        "/etc/passwd",
        r"C:\Windows\System32",
        "C:/Windows/System32",
        r"D:\secret.doc",
        "a/../../b",
        r"a\..\..\b",
        "../../../../../../../../etc/shadow",
        r"..\..\..\..\..\..\..\..\etc\shadow",
        r"\root\evil.sh",
        "//server/share/payload.dll",
        r"\\server\share\payload.dll",
        "CON",
        "CON.txt",
        "PRN",
        "AUX",
        "NUL",
        "COM1",
        "COM9",
        "LPT1",
        "sub/CON/file.txt",
        "payload.txt:hidden_stream",
        "foo\0bar.txt",
        "",
        "   ",
        ".",
        "./",
        r".\",
    ];

    for path in malicious_cases {
        let res = safe_join(dest_root, path);
        assert!(
            res.is_err(),
            "Path '{}' should be strictly rejected by safe_join, but got Ok({:?})",
            path,
            res
        );
        match res.unwrap_err() {
            SevenZError::InsecurePath(_) => {}
            other => panic!("expected InsecurePath for '{}', got {:?}", path, other),
        }
    }
}

#[test]
fn test_safe_join_valid_nested_paths() {
    let dest_root = Path::new("/var/sandbox/extract");

    // Standard relative POSIX path
    let p1 = safe_join(dest_root, "documents/report.pdf").expect("valid posix path");
    assert_eq!(p1, dest_root.join("documents/report.pdf"));

    // Windows backslash normalized to forward slash hierarchy
    let p2 = safe_join(dest_root, r"assets\images\logo.png").expect("valid backslash path");
    assert_eq!(p2, dest_root.join("assets").join("images").join("logo.png"));

    // Deep nested hierarchy
    let p3 = safe_join(dest_root, "a/b/c/d/e/f/g.txt").expect("valid deep path");
    assert_eq!(p3, dest_root.join("a/b/c/d/e/f/g.txt"));

    // Safe internal normalization (a/../b stays within root)
    let p4 = safe_join(dest_root, "a/../b/file.txt").expect("valid internal relative path");
    assert_eq!(p4, dest_root.join("b/file.txt"));

    // Leading ./ ignored
    let p5 = safe_join(dest_root, "./project/Cargo.toml").expect("valid dot prefix path");
    assert_eq!(p5, dest_root.join("project/Cargo.toml"));

    // Directory path trailing slash
    let p6 = safe_join(dest_root, "output_dir/").expect("valid directory path");
    assert_eq!(p6, dest_root.join("output_dir"));
}

#[test]
fn test_safe_join_unicode_nfc_normalization() {
    let dest_root = Path::new("/var/sandbox/extract");

    // Decomposed 'e' + combining acute accent -> composed 'é'
    let decomposed = "cafe\u{0301}/menu.txt";
    let p = safe_join(dest_root, decomposed).expect("unicode nfc path");
    assert!(p.starts_with(dest_root));
    assert!(p.to_string_lossy().contains("café"));
}
