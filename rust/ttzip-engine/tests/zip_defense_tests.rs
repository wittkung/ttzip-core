// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::path::{Path, PathBuf};
use ttzip_engine::security::{
    detect_overlapping_entries, enclosed_name, simplified_components, validate_symlink_target,
    ExtractionQuotaGuard,
};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_enclosed_name_zip_slip_traversal_interception() {
    // 1. Classic Zip-Slip directory traversal attempts
    assert_eq!(enclosed_name("../../../../etc/passwd"), None);
    assert_eq!(enclosed_name("a/../../b"), None);
    assert_eq!(enclosed_name("sub/dir/../../../secret.key"), None);
    assert_eq!(enclosed_name(r"sub\dir\..\..\..\secret.key"), None);
    assert_eq!(enclosed_name("../../outside.txt"), None);

    // 2. Absolute POSIX paths
    assert_eq!(enclosed_name("/etc/passwd"), None);
    assert_eq!(enclosed_name("/var/log/system.log"), None);
    assert_eq!(enclosed_name("///root/secret"), None);

    // 3. Absolute Windows paths & drive letters
    assert_eq!(enclosed_name(r"C:\Windows\System32\cmd.exe"), None);
    assert_eq!(enclosed_name("C:/autoexec.bat"), None);
    assert_eq!(enclosed_name(r"\\nas\share\file.txt"), None);
    assert_eq!(enclosed_name("d:/data/payload.bin"), None);
    assert_eq!(enclosed_name(r"foo\C:\bar"), None);

    // 4. Malformed and URI schemes
    assert_eq!(enclosed_name(""), None);
    assert_eq!(enclosed_name("."), None);
    assert_eq!(enclosed_name("././."), None);
    assert_eq!(enclosed_name("file:///etc/passwd"), None);
    assert_eq!(enclosed_name("payload.bin\0extra"), None);

    // 5. Valid relative enclosed paths
    assert_eq!(enclosed_name("docs/manual.pdf"), Some(PathBuf::from("docs/manual.pdf")));
    assert_eq!(enclosed_name("a/b/../c"), Some(PathBuf::from("a/c")));
    assert_eq!(enclosed_name("nested/./deep/file.txt"), Some(PathBuf::from("nested/deep/file.txt")));
    assert_eq!(enclosed_name(r"windows\path\to\item.rs"), Some(PathBuf::from("windows/path/to/item.rs")));
}

#[test]
fn test_simplified_components_root_and_drive_cleansing() {
    // 1. POSIX absolute paths
    assert_eq!(
        simplified_components("/etc/passwd"),
        Some(PathBuf::from("etc/passwd"))
    );
    assert_eq!(
        simplified_components("///var/log/audit.log"),
        Some(PathBuf::from("var/log/audit.log"))
    );

    // 2. Windows drive letters
    assert_eq!(
        simplified_components(r"C:\foo"),
        Some(PathBuf::from("foo"))
    );
    assert_eq!(
        simplified_components(r"C:\foo\bar.txt"),
        Some(PathBuf::from("foo/bar.txt"))
    );
    assert_eq!(
        simplified_components("D:/workspace/project/main.rs"),
        Some(PathBuf::from("workspace/project/main.rs"))
    );

    // 3. UNC and extended-length prefixes
    assert_eq!(
        simplified_components(r"\\?\UNC\nas\share\doc.pdf"),
        Some(PathBuf::from("nas/share/doc.pdf"))
    );
    assert_eq!(
        simplified_components(r"\\?\C:\tools\bin.exe"),
        Some(PathBuf::from("tools/bin.exe"))
    );

    // 4. Traversal underflow neutralization
    assert_eq!(
        simplified_components("../../etc/passwd"),
        Some(PathBuf::from("etc/passwd"))
    );
    assert_eq!(
        simplified_components("a/../../b"),
        Some(PathBuf::from("b"))
    );

    // 5. Empty and invalid inputs
    assert_eq!(simplified_components(""), None);
    assert_eq!(simplified_components("/"), None);
    assert_eq!(simplified_components(r"C:\"), None);
    assert_eq!(simplified_components("foo\0bar"), None);
}

#[test]
fn test_detect_overlapping_entries_42_zip_bomb_defense() {
    // 1. 42.zip archetype: Multiple entries pointing to identical payload range [1024, 5120)
    let identical_payloads = vec![
        (1024, 4096),
        (1024, 4096),
        (1024, 4096),
        (1024, 4096),
    ];
    assert!(detect_overlapping_entries(&identical_payloads));

    // 2. Partially overlapping intervals
    let partial_overlap = vec![
        (0, 500),
        (400, 600), // Overlaps with [0, 500)
    ];
    assert!(detect_overlapping_entries(&partial_overlap));

    // 3. Fully contained interval
    let contained_overlap = vec![
        (100, 1000),
        (200, 300), // Contained inside [100, 1100)
    ];
    assert!(detect_overlapping_entries(&contained_overlap));

    // 4. Arithmetic overflow in offset + length
    let overflow_entry = vec![
        (u64::MAX - 10, 100),
    ];
    assert!(detect_overlapping_entries(&overflow_entry));

    // 5. Legitimate disjoint entries in standard archives
    let legitimate_entries = vec![
        (0, 1024),
        (1024, 2048),
        (3072, 512),
        (3584, 100),
    ];
    assert!(!detect_overlapping_entries(&legitimate_entries));

    // 6. Zero-length entries (e.g. directory records or empty files)
    let entries_with_empty = vec![
        (100, 0),
        (100, 0),
        (200, 500),
        (700, 0),
        (700, 100),
    ];
    assert!(!detect_overlapping_entries(&entries_with_empty));

    // 7. Single entry or empty archive
    assert!(!detect_overlapping_entries(&[]));
    assert!(!detect_overlapping_entries(&[(100, 500)]));
}

#[test]
fn test_validate_symlink_target_sandbox_escape_interception() {
    let sandbox = Path::new("/tmp/ttzip_sandbox");

    // 1. Escapes sandbox root via parent traversal
    assert_eq!(
        validate_symlink_target(sandbox, "../outside.txt", 10),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        validate_symlink_target(sandbox, "a/../../outside", 10),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        validate_symlink_target(sandbox, "../../etc/shadow", 10),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 2. Absolute paths
    assert_eq!(
        validate_symlink_target(sandbox, "/etc/passwd", 10),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        validate_symlink_target(sandbox, r"C:\Windows\System32", 10),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. Depth limit zero or exceeded
    assert_eq!(
        validate_symlink_target(sandbox, "valid/sub/target.txt", 0),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        validate_symlink_target(sandbox, "a/b/c/d/e/f", 3),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 4. Null bytes
    assert_eq!(
        validate_symlink_target(sandbox, "target\0evil", 10),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Valid enclosed symlink target
    let valid_res = validate_symlink_target(sandbox, "sub/target.txt", 10);
    assert_eq!(valid_res, Ok(sandbox.join("sub/target.txt")));

    let valid_relative = validate_symlink_target(sandbox, "dir1/../dir2/file.bin", 10);
    assert_eq!(valid_relative, Ok(sandbox.join("dir2/file.bin")));
}

#[test]
fn test_extraction_quota_guard_zip_bomb_expansion_ratio_breaker() {
    // Max 1GB uncompressed, max 100:1 ratio, threshold 10KB for testing
    let mut guard = ExtractionQuotaGuard::with_threshold(1024 * 1024 * 1024, 100.0, 10 * 1024);

    // 1. Normal compression extraction within healthy ratio (e.g. 5:1)
    assert_eq!(guard.track(1000, 5000), Ok(()));
    assert_eq!(guard.cumulative_compressed(), 1000);
    assert_eq!(guard.cumulative_uncompressed(), 5000);
    assert!((guard.current_ratio() - 5.0).abs() < 1e-5);

    // 2. High expansion ratio (1000:1) below threshold is tolerated during warmup
    assert_eq!(guard.track(10, 5000), Ok(())); // Total uncompressed: 10,000 <= 10,240 threshold

    // 3. Exceeding ratio past threshold trips the circuit breaker
    // Adding 1 compressed byte vs 500,000 uncompressed bytes -> ratio ~ 505:1 > 100:1
    let breach_res = guard.track(1, 500_000);
    assert_eq!(breach_res, Err(TTZipStatus::ErrSecurityViolation));

    // 4. Total uncompressed quota limit enforcement
    let mut small_quota_guard = ExtractionQuotaGuard::new(1000, 100.0);
    assert_eq!(small_quota_guard.track(100, 800), Ok(()));
    assert_eq!(
        small_quota_guard.track(50, 300), // Exceeds 1000 total quota
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Reset functionality
    guard.reset();
    assert_eq!(guard.cumulative_compressed(), 0);
    assert_eq!(guard.cumulative_uncompressed(), 0);
}
