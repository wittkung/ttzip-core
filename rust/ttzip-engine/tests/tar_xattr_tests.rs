// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration tests for SCHILY.xattr / LIBARCHIVE.xattr extended attributes
//! parsing, serialization roundtrip, and macOS/Linux OS-native filesystem bridge.

use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use ttzip_engine::tar::xattr::*;
use ttzip_engine::tar::PaxRecord;

#[test]
fn test_schily_and_libarchive_pax_extraction_and_roundtrip() {
    // 1. Prepare raw PAX records containing a mix of SCHILY, LIBARCHIVE, and standard POSIX fields
    let pax_records = vec![
        PaxRecord::new("path", b"var/log/syslog.log".as_slice()),
        PaxRecord::new("mtime", b"1700000000.123456789".as_slice()),
        PaxRecord::new("SCHILY.xattr.user.author", b"Witt Kung".as_slice()),
        PaxRecord::new("SCHILY.xattr.user.project", b"TTZip Microkernel".as_slice()),
        PaxRecord::new("LIBARCHIVE.xattr.security.selinux", b"system_u:object_r:var_log_t:s0".as_slice()),
        PaxRecord::new("LIBARCHIVE.xattr.system.posix_acl_access", b"user::rw-,group::r--,other::r--".as_slice()),
        PaxRecord::new("size", b"1048576".as_slice()),
    ];

    // 2. Extract xattrs
    let xattrs = extract_xattrs_from_pax(&pax_records);
    assert_eq!(xattrs.len(), 4, "Expected exactly 4 xattr records extracted");

    assert_eq!(xattrs[0].name(), "user.author");
    assert_eq!(xattrs[0].as_str(), Some("Witt Kung"));
    assert!(xattrs[0].is_linux_attribute());
    assert!(!xattrs[0].is_macos_attribute());

    assert_eq!(xattrs[1].name(), "user.project");
    assert_eq!(xattrs[1].as_str(), Some("TTZip Microkernel"));

    assert_eq!(xattrs[2].name(), XATTR_LINUX_SELINUX);
    assert_eq!(xattrs[2].as_str(), Some("system_u:object_r:var_log_t:s0"));
    assert!(xattrs[2].is_linux_attribute());

    assert_eq!(xattrs[3].name(), XATTR_LINUX_POSIX_ACL_ACCESS);
    assert_eq!(xattrs[3].as_str(), Some("user::rw-,group::r--,other::r--"));

    // 3. Format back into standard PAX record byte stream
    let serialized_bytes = format_xattr_pax_records(&xattrs);
    assert!(!serialized_bytes.is_empty());

    // 4. Verify each record has valid PAX format: "<length> SCHILY.xattr.<key>=<value>\n"
    let serialized_str = std::str::from_utf8(&serialized_bytes).expect("PAX payload must be UTF-8");
    assert!(serialized_str.contains("SCHILY.xattr.user.author=Witt Kung\n"));
    assert!(serialized_str.contains("SCHILY.xattr.user.project=TTZip Microkernel\n"));
    assert!(serialized_str.contains("SCHILY.xattr.security.selinux=system_u:object_r:var_log_t:s0\n"));
    assert!(serialized_str.contains("SCHILY.xattr.system.posix_acl_access=user::rw-,group::r--,other::r--\n"));

    // 5. Parse back and ensure 100% roundtrip fidelity
    let parsed_records = parse_pax_records_from_bytes(&serialized_bytes);
    assert_eq!(parsed_records.len(), 4);

    let roundtrip_xattrs = extract_xattrs_from_pax(&parsed_records);
    assert_eq!(roundtrip_xattrs.len(), 4);
    assert_eq!(roundtrip_xattrs[0], xattrs[0]);
    assert_eq!(roundtrip_xattrs[1], xattrs[1]);
    assert_eq!(roundtrip_xattrs[2], TarXattr::new(XATTR_LINUX_SELINUX, "system_u:object_r:var_log_t:s0".as_bytes()));
    assert_eq!(roundtrip_xattrs[3], TarXattr::new(XATTR_LINUX_POSIX_ACL_ACCESS, "user::rw-,group::r--,other::r--".as_bytes()));
}

#[test]
fn test_macos_finderinfo_and_user_tags_binary_fidelity() {
    // macOS FinderInfo is strictly 32 bytes of raw binary data (type, creator, flags, location, stationery)
    let finder_info_bytes: [u8; 32] = [
        0x54, 0x45, 0x58, 0x54, // Type: 'TEXT'
        0x74, 0x74, 0x7a, 0x70, // Creator: 'ttzp'
        0x00, 0x0E,             // Finder flags (color label orange)
        0x00, 0x00,             // Location v
        0x00, 0x00,             // Location h
        0x00, 0x00,             // Fldr
        0xFF, 0xFE, 0x00, 0x01, // Extended finder flags
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    let user_tags_payload = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><array><string>Important\n6</string><string>Red\n6</string></array></plist>";
    let quarantine_str = "0083;66d0c2e3;Safari;98A4B83F-18AE-4A42-BAFE-643532C2A1E7";

    let xattrs = vec![
        TarXattr::new(XATTR_MACOS_FINDER_INFO, finder_info_bytes.to_vec()),
        TarXattr::new(XATTR_MACOS_USER_TAGS, user_tags_payload.to_vec()),
        TarXattr::from_str_val(XATTR_MACOS_QUARANTINE, quarantine_str),
        TarXattr::new(XATTR_MACOS_RESOURCE_FORK, vec![0x00, 0x00, 0x01, 0x00, 0xFF, 0xEE]),
    ];

    // Classification predicates
    assert!(xattrs[0].is_finder_info());
    assert!(xattrs[0].is_macos_attribute());
    assert!(!xattrs[0].is_linux_attribute());

    assert!(xattrs[1].is_user_tags());
    assert!(xattrs[1].is_macos_attribute());

    assert!(xattrs[2].is_quarantine());
    assert_eq!(xattrs[2].as_str(), Some(quarantine_str));

    // Serialize to PAX records
    let pax_bytes = format_xattr_pax_records(&xattrs);
    assert!(!pax_bytes.is_empty());

    // Extract directly from raw bytes (ensuring non-UTF8 binary FinderInfo bytes are untouched)
    let extracted = extract_xattrs_from_pax_bytes(&pax_bytes);
    assert_eq!(extracted.len(), 4);

    assert_eq!(extracted[0].name(), XATTR_MACOS_FINDER_INFO);
    assert_eq!(extracted[0].value(), &finder_info_bytes[..]);

    assert_eq!(extracted[1].name(), XATTR_MACOS_USER_TAGS);
    assert_eq!(extracted[1].value(), &user_tags_payload[..]);

    assert_eq!(extracted[2].name(), XATTR_MACOS_QUARANTINE);
    assert_eq!(extracted[2].as_str(), Some(quarantine_str));

    assert_eq!(extracted[3].name(), XATTR_MACOS_RESOURCE_FORK);
    assert_eq!(extracted[3].value(), &[0x00, 0x00, 0x01, 0x00, 0xFF, 0xEE]);
}

#[test]
fn test_edge_cases_empty_long_and_special_attributes() {
    // 1. Empty attribute value
    let empty_xattr = TarXattr::new("user.empty", Vec::<u8>::new());
    let serialized_empty = format_xattr_pax_records(std::slice::from_ref(&empty_xattr));
    let extracted_empty = extract_xattrs_from_pax_bytes(&serialized_empty);
    assert_eq!(extracted_empty.len(), 1);
    assert_eq!(extracted_empty[0].name(), "user.empty");
    assert!(extracted_empty[0].value().is_empty());

    // 2. Large attribute value (64 KB)
    let large_data = vec![0xABu8; 64 * 1024];
    let large_xattr = TarXattr::new("user.large_blob", large_data.clone());
    let serialized_large = format_xattr_pax_records(&[large_xattr]);
    let extracted_large = extract_xattrs_from_pax_bytes(&serialized_large);
    assert_eq!(extracted_large.len(), 1);
    assert_eq!(extracted_large[0].name(), "user.large_blob");
    assert_eq!(extracted_large[0].value().len(), 64 * 1024);
    assert_eq!(extracted_large[0].value(), &large_data[..]);

    // 3. Special characters & Unicode in attribute name and value
    let special_xattr = TarXattr::new(
        "user.special:key-with_dots.and_dashes#symbols",
        "key1=value1;key2=value2 with spaces, tabs\t, unicode 🚀, and symbols @#$%^&*()".as_bytes().to_vec(),
    );
    let serialized_special = format_xattr_pax_records(&[special_xattr]);
    let extracted_special = extract_xattrs_from_pax_bytes(&serialized_special);
    assert_eq!(extracted_special.len(), 1);
    assert_eq!(extracted_special[0].name(), "user.special:key-with_dots.and_dashes#symbols");
    assert_eq!(
        extracted_special[0].as_str(),
        Some("key1=value1;key2=value2 with spaces, tabs\t, unicode 🚀, and symbols @#$%^&*()")
    );

    // 4. Malformed PAX data resilience
    let malformed_inputs: Vec<&[u8]> = vec![
        b"",
        b"\0\0\0\0\0\0\0\0",
        b"10 invalid\n",
        b"9999 truncated data",
        b"20 SCHILY.xattr.no_equal_sign\n",
        b"5 =val\n",
    ];

    for bad in malformed_inputs {
        let res = extract_xattrs_from_pax_bytes(bad);
        // Must never panic, safely returning empty or valid subset
        assert!(res.len() <= 1);
    }
}

#[test]
fn test_filesystem_native_xattr_read_write_roundtrip() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("test_target_file.dat");

    // Create file
    {
        let mut file = File::create(&file_path).expect("create test file");
        file.write_all(b"TTZip extended attributes payload content").expect("write test content");
    }

    // Define test attributes
    #[cfg(target_os = "macos")]
    let test_xattrs = vec![
        TarXattr::from_str_val("com.apple.quarantine", "0081;66d0c2e3;TTZipTest;12345678-ABCD-EF01-2345-6789ABCDEF01"),
        TarXattr::new("com.apple.FinderInfo", vec![0x41; 32]),
    ];

    #[cfg(all(unix, not(target_os = "macos")))]
    let test_xattrs = vec![
        TarXattr::from_str_val("user.ttzip_test_key", "ttzip_test_value_12345"),
    ];

    #[cfg(not(unix))]
    let test_xattrs: Vec<TarXattr> = vec![];

    if !test_xattrs.is_empty() {
        // 1. Apply xattrs to file
        let applied = apply_xattrs_to_file(&file_path, &test_xattrs).expect("apply xattrs must succeed");
        assert_eq!(applied, test_xattrs.len());

        // 2. Read specific attribute back
        for expected in &test_xattrs {
            let read_val = read_xattr_from_file(&file_path, expected.name())
                .expect("read xattr must not error")
                .expect("attribute must exist");
            assert_eq!(&read_val, expected.value());
        }

        // 3. Read non-existent attribute
        let non_existent = read_xattr_from_file(&file_path, "user.non_existent_key_9999")
            .expect("read non existent xattr");
        assert_eq!(non_existent, None);

        // 4. Read all xattrs from file
        let all_read = read_all_xattrs_from_file(&file_path).expect("read all xattrs");
        for expected in &test_xattrs {
            let found = all_read.iter().find(|x| x.name() == expected.name());
            assert!(found.is_some(), "Expected attribute {} to be present in read_all", expected.name());
            assert_eq!(found.unwrap().value(), expected.value());
        }
    }
}
