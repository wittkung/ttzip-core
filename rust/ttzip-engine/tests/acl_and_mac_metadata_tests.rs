// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for POSIX.1e / NFSv4 ACL state transfers and macOS AppleDouble metadata.

use std::str::FromStr;
use ttzip_engine::archive::mac_metadata::*;
use ttzip_engine::security::acl::*;

#[test]
fn test_posix1e_acl_text_parsing_and_formatting() {
    let text = "user::rwx\nuser:alice:r-x\ngroup::r-x\ngroup:developers:rw-\nmask::rwx\nother::r--\n";
    let acl = Acl::parse_posix1e(text).expect("POSIX ACL should parse successfully");

    assert_eq!(acl.acl_type, AclType::Posix1e);
    assert_eq!(acl.entries.len(), 6);

    assert_eq!(acl.entries[0].tag, AclTag::UserObj);
    assert_eq!(acl.entries[0].permissions.to_posix_string(), "rwx");
    assert!(!acl.entries[0].is_default);

    assert_eq!(acl.entries[1].tag, AclTag::User("alice".to_string()));
    assert_eq!(acl.entries[1].permissions.to_posix_string(), "r-x");

    assert_eq!(acl.entries[2].tag, AclTag::GroupObj);
    assert_eq!(acl.entries[2].permissions.to_posix_string(), "r-x");

    assert_eq!(acl.entries[3].tag, AclTag::Group("developers".to_string()));
    assert_eq!(acl.entries[3].permissions.to_posix_string(), "rw-");

    assert_eq!(acl.entries[4].tag, AclTag::Mask);
    assert_eq!(acl.entries[4].permissions.to_posix_string(), "rwx");

    assert_eq!(acl.entries[5].tag, AclTag::Other);
    assert_eq!(acl.entries[5].permissions.to_posix_string(), "r--");

    let formatted = acl.to_text();
    assert_eq!(formatted, text);
}

#[test]
fn test_posix1e_default_acl_parsing() {
    let text = "default:user::rwx\ndefault:user:bob:r-x\ndefault:group::r-x\ndefault:mask::rwx\ndefault:other::---\n";
    let acl = Acl::parse_posix1e(text).expect("Default POSIX ACL should parse");

    assert_eq!(acl.entries.len(), 5);
    for entry in &acl.entries {
        assert!(entry.is_default);
        assert!(entry.inheritance.contains(AclInheritance::FILE_INHERIT));
        assert!(entry.inheritance.contains(AclInheritance::DIRECTORY_INHERIT));
    }

    assert_eq!(acl.entries[1].tag, AclTag::User("bob".to_string()));
    assert_eq!(acl.entries[1].permissions.to_posix_string(), "r-x");
    assert_eq!(acl.entries[4].permissions.to_posix_string(), "---");

    let formatted = acl.to_text();
    assert_eq!(formatted, text);
}

#[test]
fn test_nfsv4_acl_text_parsing_and_formatting() {
    let text = "owner@:rwxp--aARWcCos:------:allow\nuser:alice:r-x---a-R-c---:fd----:allow\ngroup@:r-x---a-R-c---:------:allow\neveryone@:r-x---a-R-c---:------:allow\n";
    let acl = Acl::parse_nfs4(text).expect("NFSv4 ACL should parse successfully");

    assert_eq!(acl.acl_type, AclType::Nfs4);
    assert_eq!(acl.entries.len(), 4);

    assert_eq!(acl.entries[0].tag, AclTag::UserObj);
    assert_eq!(acl.entries[0].ace_type, AceType::Allow);
    assert_eq!(acl.entries[0].inheritance, AclInheritance::NONE);

    assert_eq!(acl.entries[1].tag, AclTag::User("alice".to_string()));
    assert!(acl.entries[1].inheritance.contains(AclInheritance::FILE_INHERIT));
    assert!(acl.entries[1].inheritance.contains(AclInheritance::DIRECTORY_INHERIT));
    assert!(acl.entries[1].is_default);

    assert_eq!(acl.entries[2].tag, AclTag::GroupObj);
    assert_eq!(acl.entries[3].tag, AclTag::Everyone);

    let formatted = acl.to_text();
    assert_eq!(formatted, text);
}

#[test]
fn test_posix1e_to_nfsv4_and_roundtrip_conversion() {
    let posix_text = "user::rwx\nuser:alice:r-x\ngroup::r-x\nmask::r-x\nother::r--\n";
    let posix_acl = Acl::parse_posix1e(posix_text).unwrap();

    let nfs_acl = posix1e_to_nfs4(&posix_acl);
    assert_eq!(nfs_acl.acl_type, AclType::Nfs4);
    assert_eq!(nfs_acl.entries.len(), 5);

    // Verify NFSv4 entries
    assert_eq!(nfs_acl.entries[0].tag, AclTag::UserObj);
    assert!(nfs_acl.entries[0].permissions.contains(AclPermissions::READ_DATA));
    assert!(nfs_acl.entries[0].permissions.contains(AclPermissions::WRITE_DATA));
    assert!(nfs_acl.entries[0].permissions.contains(AclPermissions::EXECUTE));

    assert_eq!(nfs_acl.entries[1].tag, AclTag::User("alice".to_string()));
    assert!(nfs_acl.entries[1].permissions.contains(AclPermissions::READ_DATA));
    assert!(!nfs_acl.entries[1].permissions.contains(AclPermissions::WRITE_DATA));
    assert!(nfs_acl.entries[1].permissions.contains(AclPermissions::EXECUTE));

    assert_eq!(nfs_acl.entries[2].tag, AclTag::GroupObj);
    assert_eq!(nfs_acl.entries[3].tag, AclTag::Mask);
    assert_eq!(nfs_acl.entries[4].tag, AclTag::Everyone);

    // Roundtrip back to POSIX.1e
    let roundtrip_posix = nfs4_to_posix1e(&nfs_acl);
    assert_eq!(roundtrip_posix.acl_type, AclType::Posix1e);
    assert_eq!(roundtrip_posix.entries.len(), 5);

    assert_eq!(roundtrip_posix.entries[0].tag, AclTag::UserObj);
    assert_eq!(roundtrip_posix.entries[0].permissions.to_posix_string(), "rwx");

    assert_eq!(roundtrip_posix.entries[1].tag, AclTag::User("alice".to_string()));
    assert_eq!(roundtrip_posix.entries[1].permissions.to_posix_string(), "r-x");

    assert_eq!(roundtrip_posix.entries[2].tag, AclTag::GroupObj);
    assert_eq!(roundtrip_posix.entries[2].permissions.to_posix_string(), "r-x");

    assert_eq!(roundtrip_posix.entries[3].tag, AclTag::Mask);
    assert_eq!(roundtrip_posix.entries[3].permissions.to_posix_string(), "r-x");

    assert_eq!(roundtrip_posix.entries[4].tag, AclTag::Other);
    assert_eq!(roundtrip_posix.entries[4].permissions.to_posix_string(), "r--");
}

#[test]
fn test_acl_malformed_text_parsing_boundary_rejection() {
    assert!(Acl::parse_posix1e("invalid_no_colons").is_err());
    assert!(Acl::parse_posix1e("user::invalid_perm_chars!").is_err());
    assert!(Acl::parse_posix1e("unknown_tag::rwx").is_err());
    assert!(Acl::parse_posix1e("user:alice").is_err());

    assert!(Acl::parse_nfs4("unknown_tag@:rwx:------:allow").is_err());
    assert!(Acl::parse_nfs4("owner@:rwx:invalid_flags!:allow").is_err());
    assert!(Acl::parse_nfs4("owner@:rwx:------:invalid_type").is_err());
    assert!(Acl::parse_nfs4("user:bob:rwx").is_err());
}

#[test]
fn test_acl_from_str_auto_detection() {
    let posix_str = "user::rwx\ngroup::r-x\nother::r--\n";
    let acl_p = Acl::from_str(posix_str).expect("Should detect POSIX format");
    assert_eq!(acl_p.acl_type, AclType::Posix1e);

    let nfs_str = "owner@:rwxp--aARWcCos:------:allow\neveryone@:r-x---a-R-c---:------:allow\n";
    let acl_n = Acl::from_str(nfs_str).expect("Should detect NFSv4 format");
    assert_eq!(acl_n.acl_type, AclType::Nfs4);
}

#[test]
fn test_finder_info_32_bytes_manipulation() {
    let mut info = FinderInfo::new();
    assert_eq!(info.raw(), &[0u8; 32]);
    assert_eq!(info.file_type(), [0, 0, 0, 0]);
    assert_eq!(info.file_creator(), [0, 0, 0, 0]);
    assert!(!info.is_invisible());
    assert!(!info.has_bundle());
    assert!(!info.has_custom_icon());

    info.set_file_type(*b"TEXT");
    info.set_file_creator(*b"ttzp");
    assert_eq!(&info.file_type(), b"TEXT");
    assert_eq!(&info.file_creator(), b"ttzp");

    info.set_invisible(true);
    assert!(info.is_invisible());

    info.set_custom_icon(true);
    assert!(info.has_custom_icon());

    info.set_bundle(true);
    assert!(info.has_bundle());

    info.set_location(120, 340);
    assert_eq!(info.location(), (120, 340));

    info.set_extended_flags(0x0080);
    assert_eq!(info.extended_flags(), 0x0080);

    // Verify 32-byte layout consistency
    let raw = *info.raw();
    let reloaded = FinderInfo::from_raw(raw);
    assert_eq!(reloaded.file_type(), *b"TEXT");
    assert_eq!(reloaded.file_creator(), *b"ttzp");
    assert!(reloaded.is_invisible());
    assert!(reloaded.has_custom_icon());
    assert!(reloaded.has_bundle());
    assert_eq!(reloaded.location(), (120, 340));
    assert_eq!(reloaded.extended_flags(), 0x0080);
}

#[test]
fn test_appledouble_header_encode_decode_roundtrip() {
    let header = AppleDoubleHeader {
        magic: APPLEDOUBLE_MAGIC,
        version: APPLEDOUBLE_VERSION_2,
        home_fs: *DEFAULT_HOME_FS,
        num_entries: 2,
        entries: vec![
            AppleDoubleEntryDescriptor {
                entry_id: ENTRY_FINDER_INFO,
                offset: 50,
                length: 32,
            },
            AppleDoubleEntryDescriptor {
                entry_id: ENTRY_RESOURCE_FORK,
                offset: 82,
                length: 128,
            },
        ],
    };

    let encoded = header.encode();
    assert_eq!(encoded.len(), 26 + 2 * 12);

    // Pad buffer to simulate full file with data
    let mut full_file = encoded.clone();
    full_file.resize(82 + 128, 0xAA);

    let (decoded, _) = (
        AppleDoubleHeader::decode(&full_file).expect("AppleDouble header decode failed"),
        full_file.len(),
    );

    assert_eq!(decoded.magic, APPLEDOUBLE_MAGIC);
    assert_eq!(decoded.version, APPLEDOUBLE_VERSION_2);
    assert_eq!(&decoded.home_fs, DEFAULT_HOME_FS);
    assert_eq!(decoded.num_entries, 2);
    assert_eq!(decoded.entries, header.entries);
}

#[test]
fn test_appledouble_file_full_pipeline_roundtrip() {
    let mut finder_info = FinderInfo::new();
    finder_info.set_file_type(*b"APPL");
    finder_info.set_file_creator(*b"ttzp");
    finder_info.set_location(42, 84);

    let resource_data = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let real_name = "test_document.txt";

    let file = AppleDoubleFile::new()
        .with_finder_info(finder_info)
        .with_resource_fork(resource_data.clone())
        .with_real_name(real_name);

    let encoded_bytes = file.encode();
    assert_eq!(encoded_bytes.len(), 26 + 3 * 12 + 32 + 8 + real_name.len());

    let decoded_file = AppleDoubleFile::decode(&encoded_bytes).expect("Decode should succeed");
    assert_eq!(decoded_file.finder_info, Some(finder_info));
    assert_eq!(decoded_file.resource_fork, Some(resource_data));
    assert_eq!(decoded_file.real_name, Some(real_name.to_string()));
}

#[test]
fn test_appledouble_malformed_and_truncated_inputs() {
    // 1. Buffer too short for header base
    let short_buf = vec![0u8; 10];
    assert!(matches!(
        AppleDoubleFile::decode(&short_buf),
        Err(MacMetadataError::BufferTooShort { .. })
    ));

    // 2. Invalid magic
    let mut bad_magic = vec![0u8; 50];
    bad_magic[0..4].copy_from_slice(&0x12345678u32.to_be_bytes());
    assert!(matches!(
        AppleDoubleFile::decode(&bad_magic),
        Err(MacMetadataError::InvalidMagic(_))
    ));

    // 3. Out-of-bounds entry descriptor
    let mut oob_file = AppleDoubleHeader {
        magic: APPLEDOUBLE_MAGIC,
        version: APPLEDOUBLE_VERSION_2,
        home_fs: *DEFAULT_HOME_FS,
        num_entries: 1,
        entries: vec![AppleDoubleEntryDescriptor {
            entry_id: ENTRY_FINDER_INFO,
            offset: 1000,
            length: 32,
        }],
    }
    .encode();
    oob_file.resize(60, 0);
    assert!(matches!(
        AppleDoubleFile::decode(&oob_file),
        Err(MacMetadataError::OffsetOutOfBounds { .. })
    ));
}

#[test]
fn test_extended_attributes_appledouble_bridge() {
    let mut xattrs = ExtendedAttributes::new();

    let mut info = FinderInfo::new();
    info.set_file_type(*b"PDF ");
    info.set_file_creator(*b"ttzp");
    xattrs.set_finder_info(&info);

    let rsrc_payload = vec![0xCA, 0xFE, 0xBA, 0xBE];
    xattrs.set_resource_fork(rsrc_payload.clone());

    xattrs.set_quarantine("0181;63b6510a;Safari;3D4B662C-5D34-4C2E-89DC-DF6A4B952B9F");

    // Check individual getters
    assert_eq!(xattrs.finder_info(), Some(info));
    assert_eq!(xattrs.resource_fork(), Some(rsrc_payload.as_slice()));
    assert_eq!(
        xattrs.quarantine(),
        Some("0181;63b6510a;Safari;3D4B662C-5D34-4C2E-89DC-DF6A4B952B9F")
    );

    // Convert to AppleDouble binary and reconstruct
    let ad_bytes = xattrs.to_appledouble().expect("Should produce AppleDouble bytes");
    let restored_xattrs = ExtendedAttributes::from_appledouble(&ad_bytes).expect("Should reconstruct");

    assert_eq!(restored_xattrs.finder_info(), Some(info));
    assert_eq!(restored_xattrs.resource_fork(), Some(rsrc_payload.as_slice()));
}
