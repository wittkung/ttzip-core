// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;

#[test]
fn test_sanitize_posix() {
    let home = get_default_home();
    let base = Some("/Users/test/Documents".into());
    let cases: &[(&str, Option<String>, &str)] = &[
        ("~", None, &home),
        ("~/Downloads", None, &format!("{}/Downloads", home)),
        ("~/Desktop/../Downloads", None, &format!("{}/Downloads", home)),
        ("../folder", base.clone(), "/Users/test/folder"),
        ("./child", base.clone(), "/Users/test/Documents/child"),
        ("sub/archive.7z", base.clone(), "/Users/test/Documents/sub/archive.7z"),
        (r#"/Users/test/My\ Space/File\ \(1\).zip"#, None, "/Users/test/My Space/File (1).zip"),
        (r#"/var/tmp/Special\[1\]\&\$name.tar"#, None, "/var/tmp/Special[1]&$name.tar"),
        ("file:///Users/test/Downloads/abc%20def", None, "/Users/test/Downloads/abc def"),
        ("file://localhost/Users/test/archive.zip", None, "/Users/test/archive.zip"),
        ("///var///tmp///", None, "/var/tmp"),
        ("///", None, "/"),
        ("  \"/Users/test/Documents\"  ", None, "/Users/test/Documents"),
        ("   ", None, ""),
    ];
    for (raw, b, expected) in cases {
        assert_eq!(sanitize_posix_path((*raw).into(), b.clone()), *expected);
    }
}

#[test]
fn test_extract_parent_and_prefix() {
    let home = get_default_home();
    let base = Some("/Users/test/Documents".into());
    let cases = [
        ("/var/lo", "/var", "lo"),
        ("/var/log/", "/var/log", ""),
        ("/", "/", ""),
        ("~/Down", home.as_str(), "Down"),
        ("~/Downloads/", &format!("{}/Downloads", home), ""),
        ("~", home.as_str(), ""),
        ("sub/fil", "/Users/test/Documents/sub", "fil"),
        (r#"/Users/test/My\ Sp"#, "/Users/test", "My Sp"),
        ("file:///Users/test/Downloads/abc", "/Users/test/Downloads", "abc"),
        ("", "/Users/test/Documents", ""),
    ];
    for (input, exp_parent, exp_pre) in cases {
        let res = extract_parent_and_prefix(input.into(), base.clone());
        assert_eq!(res.parent_directory, exp_parent, "Failed parent for {}", input);
        assert_eq!(res.prefix, exp_pre, "Failed prefix for {}", input);
    }
}

#[test]
fn test_scan_directory_basic_and_filtering() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path();

    let sub_dir = temp_path.join("subdir");
    std::fs::create_dir(&sub_dir).expect("create subdir");

    let file_a = temp_path.join("alpha.txt");
    std::fs::write(&file_a, b"hello world").expect("write file");

    let file_b = temp_path.join("beta.bin");
    std::fs::write(&file_b, b"12345678").expect("write file");

    let hidden = temp_path.join(".hidden");
    std::fs::write(&hidden, b"secret").expect("write hidden");

    let res = scan_directory(temp_path.to_string_lossy().to_string(), 1).expect("scan directory");

    assert_eq!(res.len(), 3);
    assert_eq!(res[0].name, "subdir");
    assert!(res[0].is_directory);
    assert_eq!(res[0].size, 0);

    assert_eq!(res[1].name, "alpha.txt");
    assert!(!res[1].is_directory);
    assert_eq!(res[1].size, 11);

    assert_eq!(res[2].name, "beta.bin");
    assert!(!res[2].is_directory);
    assert_eq!(res[2].size, 8);
}

#[test]
fn test_scan_directory_recursive_depth() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path();

    let level1 = temp_path.join("level1");
    std::fs::create_dir(&level1).expect("create level1");

    let level2 = level1.join("level2");
    std::fs::create_dir(&level2).expect("create level2");

    let deep_file = level2.join("deep.txt");
    std::fs::write(&deep_file, b"deep content").expect("write deep file");

    let depth1_items = scan_directory(temp_path.to_string_lossy().to_string(), 1).expect("scan depth 1");
    assert_eq!(depth1_items.len(), 1);
    assert_eq!(depth1_items[0].name, "level1");

    let depth2_items = scan_directory(temp_path.to_string_lossy().to_string(), 2).expect("scan depth 2");
    assert_eq!(depth2_items.len(), 2);
    assert_eq!(depth2_items[0].name, "level1");
    assert_eq!(depth2_items[1].name, "level2");

    let depth3_items = scan_directory(temp_path.to_string_lossy().to_string(), 3).expect("scan depth 3");
    assert_eq!(depth3_items.len(), 3);
}

#[test]
fn test_scan_directory_symlink_nofollow() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path();

    let target_dir = temp_path.join("target_dir");
    std::fs::create_dir(&target_dir).expect("create target_dir");

    let symlink_path = temp_path.join("link_dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target_dir, &symlink_path).expect("create symlink");

    let items = scan_directory(temp_path.to_string_lossy().to_string(), 1).expect("scan items");
    let symlink_item = items.iter().find(|i| i.name == "link_dir");
    assert!(symlink_item.is_some());
    assert!(!symlink_item.unwrap().is_directory);
}

#[test]
fn test_scan_directory_not_found() {
    let missing = "/path/to/definitely/missing/dir_12345";
    let res = scan_directory(missing.to_string(), 1);
    assert!(matches!(res, Err(TTZipError::FileNotFound { .. })));
}

#[cfg(unix)]
#[test]
fn test_scan_directory_io_error_accounting() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path();

    let ok_file = temp_path.join("visible.txt");
    std::fs::write(&ok_file, b"ok").expect("write visible");

    let locked_dir = temp_path.join("locked_subdir");
    std::fs::create_dir(&locked_dir).expect("create locked_subdir");
    std::fs::write(locked_dir.join("secret.txt"), b"secret").expect("write secret");
    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o000))
        .expect("chmod 000");

    let (items, io_errors) =
        scan_directory_with_error_count(temp_path.to_string_lossy().into_owned(), 2)
            .expect("scan with error count");

    let _ = std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o755));

    assert!(items.iter().any(|i| i.name == "visible.txt"));
    assert!(io_errors >= 1);
    assert!(last_scan_io_error_count() >= 1);
}

#[cfg(unix)]
#[test]
fn test_last_scan_io_error_count_isolated_across_concurrent_threads() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Barrier};

    let err_dir = tempfile::tempdir().expect("create error tempdir");
    let clean_dir = tempfile::tempdir().expect("create clean tempdir");

    std::fs::write(err_dir.path().join("visible.txt"), b"ok").expect("write visible");
    let locked_dir = err_dir.path().join("locked_subdir");
    std::fs::create_dir(&locked_dir).expect("create locked_subdir");
    std::fs::write(locked_dir.join("secret.txt"), b"secret").expect("write secret");
    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o000))
        .expect("chmod 000");

    std::fs::write(clean_dir.path().join("clean.txt"), b"clean").expect("write clean");

    let err_path = err_dir.path().to_string_lossy().into_owned();
    let clean_path = clean_dir.path().to_string_lossy().into_owned();
    let after_a_scan = Arc::new(Barrier::new(2));
    let after_b_scan = Arc::new(Barrier::new(2));

    let a_b1 = Arc::clone(&after_a_scan);
    let a_b2 = Arc::clone(&after_b_scan);
    let handle_a = std::thread::spawn(move || {
        let (_, io_errors) = scan_directory_with_error_count(err_path, 2).expect("scan err_dir");
        assert!(io_errors >= 1);
        assert_eq!(last_scan_io_error_count(), io_errors);
        a_b1.wait();
        a_b2.wait();
        assert_eq!(
            last_scan_io_error_count(),
            io_errors,
            "concurrent 0-error scan on sibling thread must not clobber TLS error count"
        );
    });

    let b_b1 = Arc::clone(&after_a_scan);
    let b_b2 = Arc::clone(&after_b_scan);
    let handle_b = std::thread::spawn(move || {
        b_b1.wait();
        let items = scan_directory(clean_path, 1).expect("scan clean_dir");
        assert_eq!(items.len(), 1);
        assert_eq!(last_scan_io_error_count(), 0);
        b_b2.wait();
    });

    let res_a = handle_a.join();
    let res_b = handle_b.join();
    let _ = std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o755));
    res_a.expect("thread A must pass");
    res_b.expect("thread B must pass");
}
