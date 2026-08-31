// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for TAR 512-byte sector geometry mapping,
//! strongly typed entry flags, octal ASCII codecs, and GNU Base-256 big-endian binary codecs.

use std::mem::{align_of, size_of};
use ttzip_engine::tar::{
    base256_from, base256_into, numeric_extended_from, numeric_extended_into, octal_from,
    octal_into, GnuExtSparseHeader, GnuHeader, GnuSparseHeader, OldHeader, TarEntryType, TarHeader,
    UstarHeader, BLOCK_SIZE, LEN_CHKSUM, LEN_DEVMAJOR, LEN_DEVMINOR, LEN_GID, LEN_GNAME,
    LEN_LINKNAME, LEN_MAGIC, LEN_MODE, LEN_MTIME, LEN_NAME, LEN_PREFIX, LEN_SIZE, LEN_TYPEFLAG,
    LEN_UID, LEN_UNAME, LEN_VERSION, MAGIC_GNU, MAGIC_USTAR, OFFSET_CHKSUM, OFFSET_DEVMAJOR,
    OFFSET_DEVMINOR, OFFSET_GID, OFFSET_GNAME, OFFSET_LINKNAME, OFFSET_MAGIC, OFFSET_MODE,
    OFFSET_MTIME, OFFSET_NAME, OFFSET_PREFIX, OFFSET_SIZE, OFFSET_TYPEFLAG, OFFSET_UID,
    OFFSET_UNAME, OFFSET_VERSION, VERSION_GNU, VERSION_USTAR,
};

#[test]
fn test_tar_sector_geometry_and_struct_sizes() {
    // 1. Sector block size and hardware alignment assertions
    assert_eq!(BLOCK_SIZE, 512);
    assert_eq!(size_of::<TarHeader>(), 512);
    assert_eq!(align_of::<TarHeader>(), 512);

    // 2. Standard TAR fixed-length struct size assertions
    assert_eq!(size_of::<OldHeader>(), 512);
    assert_eq!(size_of::<UstarHeader>(), 512);
    assert_eq!(size_of::<GnuSparseHeader>(), 24);
    assert_eq!(size_of::<GnuHeader>(), 512);
    assert_eq!(size_of::<GnuExtSparseHeader>(), 512);

    // 3. Field offsets in a standard 512-byte block
    assert_eq!(OFFSET_NAME, 0);
    assert_eq!(OFFSET_MODE, 100);
    assert_eq!(OFFSET_UID, 108);
    assert_eq!(OFFSET_GID, 116);
    assert_eq!(OFFSET_SIZE, 124);
    assert_eq!(OFFSET_MTIME, 136);
    assert_eq!(OFFSET_CHKSUM, 148);
    assert_eq!(OFFSET_TYPEFLAG, 156);
    assert_eq!(OFFSET_LINKNAME, 157);
    assert_eq!(OFFSET_MAGIC, 257);
    assert_eq!(OFFSET_VERSION, 263);
    assert_eq!(OFFSET_UNAME, 265);
    assert_eq!(OFFSET_GNAME, 297);
    assert_eq!(OFFSET_DEVMAJOR, 329);
    assert_eq!(OFFSET_DEVMINOR, 337);
    assert_eq!(OFFSET_PREFIX, 345);

    // 4. Field lengths assertions
    assert_eq!(LEN_NAME, 100);
    assert_eq!(LEN_MODE, 8);
    assert_eq!(LEN_UID, 8);
    assert_eq!(LEN_GID, 8);
    assert_eq!(LEN_SIZE, 12);
    assert_eq!(LEN_MTIME, 12);
    assert_eq!(LEN_CHKSUM, 8);
    assert_eq!(LEN_TYPEFLAG, 1);
    assert_eq!(LEN_LINKNAME, 100);
    assert_eq!(LEN_MAGIC, 6);
    assert_eq!(LEN_VERSION, 2);
    assert_eq!(LEN_UNAME, 32);
    assert_eq!(LEN_GNAME, 32);
    assert_eq!(LEN_DEVMAJOR, 8);
    assert_eq!(LEN_DEVMINOR, 8);
    assert_eq!(LEN_PREFIX, 155);

    // 5. GNU Ext Sparse block math verification: 21 * 24 + 1 + 7 == 512
    assert_eq!(21 * size_of::<GnuSparseHeader>() + 1 + 7, 512);
}

#[test]
fn test_tar_entry_type_mappings_and_predicates() {
    let cases = [
        (0u8, TarEntryType::Regular),
        (b'0', TarEntryType::Regular),
        (b'1', TarEntryType::Link),
        (b'2', TarEntryType::Symlink),
        (b'3', TarEntryType::Char),
        (b'4', TarEntryType::Block),
        (b'5', TarEntryType::Directory),
        (b'6', TarEntryType::Fifo),
        (b'7', TarEntryType::Contiguous),
        (b'L', TarEntryType::GNULongName),
        (b'K', TarEntryType::GNULongLink),
        (b'S', TarEntryType::GNUSparse),
        (b'x', TarEntryType::XHeader),
        (b'g', TarEntryType::XGlobalHeader),
        (b'X', TarEntryType::SolarisExt),
        (0xFF, TarEntryType::Other(0xFF)),
    ];

    for (raw_byte, expected_type) in cases {
        let parsed = TarEntryType::from_byte(raw_byte);
        assert_eq!(parsed, expected_type);
        if raw_byte != 0 {
            assert_eq!(parsed.as_byte(), raw_byte);
        } else {
            assert_eq!(parsed.as_byte(), b'0');
        }
    }

    // Test predicate methods
    assert!(TarEntryType::Regular.is_regular());
    assert!(TarEntryType::Contiguous.is_regular());
    assert!(!TarEntryType::Directory.is_regular());

    assert!(TarEntryType::Directory.is_directory());
    assert!(!TarEntryType::Regular.is_directory());

    assert!(TarEntryType::Symlink.is_symlink());
    assert!(TarEntryType::Link.is_hardlink());

    assert!(TarEntryType::XHeader.is_pax_header());
    assert!(TarEntryType::XGlobalHeader.is_pax_header());
    assert!(!TarEntryType::Regular.is_pax_header());

    assert!(TarEntryType::GNULongName.is_gnu_long_meta());
    assert!(TarEntryType::GNULongLink.is_gnu_long_meta());
    assert!(!TarEntryType::GNUSparse.is_gnu_long_meta());

    assert!(TarEntryType::GNUSparse.is_sparse());
}

#[test]
fn test_standard_octal_roundtrip_fidelity() {
    let test_values = [
        0u64,
        1,
        7,
        8,
        0o644,
        0o755,
        0o1777,
        0o777777,
        0o77777777777, // 8 GiB - 1 (11 octal digits)
    ];

    let mut buf = [0u8; 12];
    for &val in &test_values {
        octal_into(&mut buf, val);
        let parsed = octal_from(&buf);
        assert_eq!(parsed, Some(val), "Failed for value: {:#o}", val);
    }

    // Test 8-byte buffer (e.g. mode/uid/gid)
    let mut mode_buf = [0u8; 8];
    octal_into(&mut mode_buf, 0o755);
    assert_eq!(octal_from(&mode_buf), Some(0o755));

    // Test trimming whitespace & leading/trailing nulls
    assert_eq!(octal_from(b"  0755 \0"), Some(0o755));
    assert_eq!(octal_from(b"\0\0 0644\0 "), Some(0o644));
    assert_eq!(octal_from(b""), Some(0));
    assert_eq!(octal_from(b"\0\0\0\0"), Some(0));
    assert_eq!(octal_from(b"   "), Some(0));
}

#[test]
fn test_gnu_base256_roundtrip_fidelity_and_boundaries() {
    let test_values = [
        0u64,
        1u64,
        42u64,
        1024u64,
        65535u64,
        8 * 1024 * 1024 * 1024 - 1,   // 8 GiB - 1
        8 * 1024 * 1024 * 1024,       // 8 GiB boundary
        10 * 1024 * 1024 * 1024,      // 10 GiB
        100 * 1024 * 1024 * 1024,     // 100 GiB
        1024 * 1024 * 1024 * 1024,    // 1 TiB
        10 * 1024 * 1024 * 1024 * 1024, // 10 TiB
        u64::MAX - 1,
        u64::MAX,
    ];

    let mut buf = [0u8; 12];
    for &val in &test_values {
        // Test direct base256
        base256_into(&mut buf, val);
        assert_eq!(buf[0], 0x80, "Leading indicator must be 0x80 for val: {}", val);
        let parsed_b256 = base256_from(&buf);
        assert_eq!(parsed_b256, Some(val), "base256 roundtrip failed for: {}", val);

        // Test numeric_extended_into and numeric_extended_from
        numeric_extended_into(&mut buf, val);
        let parsed_num = numeric_extended_from(&buf);
        assert_eq!(parsed_num, val, "numeric_extended roundtrip failed for: {}", val);

        if val > 8 * 1024 * 1024 * 1024 - 1 {
            // Must have used Base-256 for > 8GB in 12-byte field
            assert_eq!(buf[0], 0x80);
        }
    }

    // Test large UID/GID (> 2097151 = 8^7 - 1) in 8-byte field
    let mut uid_buf = [0u8; 8];
    let large_uid = 100_000_000u64;
    numeric_extended_into(&mut uid_buf, large_uid);
    assert_eq!(uid_buf[0], 0x80);
    assert_eq!(numeric_extended_from(&uid_buf), large_uid);
}

#[test]
fn test_tar_header_construction_and_checksum_verification() {
    let mut header = TarHeader::new();
    assert!(header.is_zero_block());

    header.set_name("usr/local/bin/ttzip");
    header.set_mode(0o755);
    header.set_uid(1001);
    header.set_gid(1001);
    header.set_size(10 * 1024 * 1024 * 1024); // 10 GiB (Base-256)
    header.set_mtime(1700000000);
    header.set_entry_type(TarEntryType::Regular);
    header.set_uname("wittkung");
    header.set_gname("staff");
    header.set_ustar_magic();

    assert!(!header.is_zero_block());
    assert!(header.is_ustar());
    assert!(!header.is_gnu());
    assert_eq!(header.magic_bytes(), MAGIC_USTAR);
    assert_eq!(header.version_bytes(), VERSION_USTAR);
    assert_eq!(MAGIC_USTAR, b"ustar\0");
    assert_eq!(VERSION_USTAR, b"00");

    assert_eq!(header.name(), "usr/local/bin/ttzip");
    assert_eq!(header.mode(), 0o755);
    assert_eq!(header.uid(), 1001);
    assert_eq!(header.gid(), 1001);
    assert_eq!(header.size(), 10 * 1024 * 1024 * 1024);
    assert_eq!(header.mtime(), 1700000000);
    assert_eq!(header.entry_type(), TarEntryType::Regular);
    assert_eq!(header.uname(), "wittkung");
    assert_eq!(header.gname(), "staff");

    // Checksum update and verification
    header.update_checksum();
    assert!(header.verify_checksum());

    // Tamper with byte and verify rejection
    header.bytes[0] ^= 0xFF;
    assert!(!header.verify_checksum());
}

#[test]
fn test_gnu_header_specific_fields() {
    let mut header = TarHeader::new();
    header.set_gnu_magic();
    assert!(header.is_gnu());
    assert_eq!(header.magic_bytes(), MAGIC_GNU);
    assert_eq!(header.version_bytes(), VERSION_GNU);

    let gnu_view = header.as_gnu_header();
    assert_eq!(&gnu_view.magic, MAGIC_GNU);
    assert_eq!(&gnu_view.version, VERSION_GNU);
}

#[test]
fn test_malformed_and_illegal_data_safe_rejection_no_panic() {
    // 1. Non-octal ASCII digits
    assert_eq!(octal_from(b"01289ABC"), None);
    assert_eq!(octal_from(b"99999999"), None);
    assert_eq!(octal_from(b"\xFF\xFE\xFD"), None);

    // 2. numeric_extended_from safe fallback to 0
    assert_eq!(numeric_extended_from(b"invalid_non_octal"), 0);
    assert_eq!(numeric_extended_from(b""), 0);

    // 3. base256_from rejection when 0x80 bit not set
    assert_eq!(base256_from(b"012345678901"), None);
    assert_eq!(base256_from(b""), Some(0));

    // 4. from_slice with insufficient length
    assert!(TarHeader::from_slice(&[0u8; 511]).is_none());
    assert!(TarHeader::from_slice(&[0u8; 512]).is_some());
    assert!(TarHeader::from_slice(&[0u8; 1024]).is_some());
}
