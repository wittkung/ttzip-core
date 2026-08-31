// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for nanosecond TTZipEntry metadata, EntryFields bitmask,
//! sparse extent coalescing, and LinkResolver dual-strategy topologies.

use std::time::{Duration, UNIX_EPOCH};
use tempfile::NamedTempFile;
use ttzip_engine::archive::unified::{
    clean_sparse_extents, coalesce_sparse_extents, EntryFields, LinkAction, LinkResolver,
    LinkResolverStrategy, SparseExtent, TTZipEntry, TTZipFileType, TTZipTimestamp,
};

#[test]
fn test_nanosecond_timestamp_precision_and_normalization() {
    // 1. Basic normalized timestamp
    let ts1 = TTZipTimestamp::new(1700000000, 123456789);
    assert_eq!(ts1.sec, 1700000000);
    assert_eq!(ts1.nsec, 123456789);
    assert_eq!(ts1.epoch_secs(), 1700000000);
    assert_eq!(ts1.as_total_nanos(), 1700000000123456789_i128);

    // 2. Nanosecond overflow normalization
    let ts_overflow = TTZipTimestamp::new(100, 2_500_000_000);
    assert_eq!(ts_overflow.sec, 102);
    assert_eq!(ts_overflow.nsec, 500_000_000);

    // 3. Signed nanoseconds conversion
    let ts_from_nanos = TTZipTimestamp::from_nanos_signed(1500000000);
    assert_eq!(ts_from_nanos.sec, 1);
    assert_eq!(ts_from_nanos.nsec, 500_000_000);

    // 4. Milliseconds conversion
    let ts_from_ms = TTZipTimestamp::from_epoch_millis(12345);
    assert_eq!(ts_from_ms.sec, 12);
    assert_eq!(ts_from_ms.nsec, 345_000_000);
    assert_eq!(ts_from_ms.epoch_millis(), 12345);

    // 5. SystemTime roundtrip after epoch
    let st = UNIX_EPOCH + Duration::new(1672531199, 987654321);
    let ts_st = TTZipTimestamp::from_system_time(st);
    assert_eq!(ts_st.sec, 1672531199);
    assert_eq!(ts_st.nsec, 987654321);
    let st_back = ts_st.to_system_time().expect("to_system_time valid");
    assert_eq!(st, st_back);
}

#[test]
fn test_negative_timestamp_before_1970() {
    // 1. Negative total nanoseconds: -500ms before epoch (1969-12-31 23:59:59.500)
    let ts_neg1 = TTZipTimestamp::from_nanos_signed(-500_000_000);
    assert_eq!(ts_neg1.sec, -1);
    assert_eq!(ts_neg1.nsec, 500_000_000);
    assert_eq!(ts_neg1.as_total_nanos(), -500_000_000);

    // 2. Negative -1.5s before epoch (-1500_000_000 ns)
    let ts_neg2 = TTZipTimestamp::from_nanos_signed(-1_500_000_000);
    assert_eq!(ts_neg2.sec, -2);
    assert_eq!(ts_neg2.nsec, 500_000_000);
    assert_eq!(ts_neg2.as_total_nanos(), -1_500_000_000);

    // 3. Milliseconds before epoch
    let ts_neg_ms = TTZipTimestamp::from_epoch_millis(-1500);
    assert_eq!(ts_neg_ms.sec, -2);
    assert_eq!(ts_neg_ms.nsec, 500_000_000);
    assert_eq!(ts_neg_ms.epoch_millis(), -1500);

    // 4. SystemTime conversion before epoch
    let st_before = UNIX_EPOCH - Duration::new(10, 250_000_000);
    let ts_before = TTZipTimestamp::from_system_time(st_before);
    assert_eq!(ts_before.sec, -11);
    assert_eq!(ts_before.nsec, 750_000_000);
    let st_restored = ts_before.to_system_time().expect("restore before epoch");
    assert_eq!(st_before, st_restored);
}

#[test]
fn test_entry_fields_bitflags_isolation() {
    let mut entry = TTZipEntry::new();

    // Default: no fields explicitly set
    assert!(!entry.is_field_set(EntryFields::SIZE));
    assert!(!entry.is_field_set(EntryFields::PATHNAME));
    assert_eq!(entry.size, 0);

    // Explicitly setting size to 0 must be tracked as SET
    entry.set_size(0);
    assert!(entry.is_field_set(EntryFields::SIZE));
    assert_eq!(entry.size, 0);

    // Setting timestamps
    entry.set_mtime(TTZipTimestamp::new(1000, 0));
    assert!(entry.is_field_set(EntryFields::MTIME));
    assert!(entry.fields.intersects(EntryFields::TIMESTAMPS));

    // Bitwise operators
    let f1 = EntryFields::UID | EntryFields::GID;
    assert_eq!(f1, EntryFields::UID_GID);
    assert!(f1.contains(EntryFields::UID));
    assert!(f1.contains(EntryFields::GID));
    assert!(!f1.contains(EntryFields::PATHNAME));

    let f2 = f1 - EntryFields::UID;
    assert_eq!(f2, EntryFields::GID);

    // Unset field resets value and clears bitmask
    entry.unset_field(EntryFields::SIZE);
    assert!(!entry.is_field_set(EntryFields::SIZE));
    assert_eq!(entry.size, 0);
}

#[test]
fn test_sparse_extent_coalescing_and_cleaning() {
    // 1. Overlapping and adjacent extents coalescing
    let mut extents = vec![
        SparseExtent::new(1000, 500), // [1000, 1500)
        SparseExtent::new(0, 500),    // [0, 500)
        SparseExtent::new(500, 500),  // [500, 1000) -> merges with [0, 500) & [1000, 1500) into [0, 1500)
        SparseExtent::new(2000, 100), // [2000, 2100)
        SparseExtent::new(2050, 200), // [2050, 2250) -> merges with [2000, 2100) into [2000, 2250)
        SparseExtent::new(3000, 0),   // Empty block -> filtered out
    ];

    coalesce_sparse_extents(&mut extents);
    assert_eq!(
        extents,
        vec![
            SparseExtent::new(0, 1500),
            SparseExtent::new(2000, 250),
        ]
    );

    // 2. Truly sparse file cleaning
    let is_sparse = clean_sparse_extents(&mut extents, 5000);
    assert!(is_sparse);
    assert_eq!(extents.len(), 2);

    // 3. Dense (non-sparse) file degradation cleaning
    let mut dense_extents = vec![
        SparseExtent::new(0, 2500),
        SparseExtent::new(2500, 2500),
    ];
    let is_dense_sparse = clean_sparse_extents(&mut dense_extents, 5000);
    assert!(!is_dense_sparse);
    assert!(dense_extents.is_empty());
}

#[test]
fn test_link_resolver_tar_strategy() {
    let mut resolver = LinkResolver::new(LinkResolverStrategy::Tar);
    assert_eq!(resolver.strategy(), LinkResolverStrategy::Tar);

    let dev = 100;
    let ino = 42001;
    let nlink = 3;
    let original_size = 65536;

    // Node 1: First occurrence -> Full payload data
    let mut entry1 = TTZipEntry::new_file("/tar/data.bin", original_size);
    entry1.set_dev(dev);
    entry1.set_ino(ino);
    entry1.set_nlink(nlink);

    let action1 = resolver.resolve(&mut entry1);
    assert_eq!(action1, LinkAction::TarFirstEntry { write_data: true });
    assert_eq!(entry1.size, original_size);
    assert_eq!(entry1.file_type, TTZipFileType::RegularFile);
    assert_eq!(resolver.pending_inodes_count(), 1);

    // Node 2: Second occurrence -> Hardlink pointer with size 0
    let mut entry2 = TTZipEntry::new_file("/tar/link1.bin", original_size);
    entry2.set_dev(dev);
    entry2.set_ino(ino);
    entry2.set_nlink(nlink);

    let action2 = resolver.resolve(&mut entry2);
    assert_eq!(
        action2,
        LinkAction::TarHardlink {
            target: "/tar/data.bin".to_string(),
            write_data: false
        }
    );
    assert_eq!(entry2.size, 0);
    assert_eq!(entry2.file_type, TTZipFileType::Hardlink);
    assert_eq!(entry2.hardlink_target.as_deref(), Some("/tar/data.bin"));
    assert_eq!(resolver.pending_inodes_count(), 1);

    // Node 3: Final occurrence -> Hardlink pointer with size 0, memory released
    let mut entry3 = TTZipEntry::new_file("/tar/link2.bin", original_size);
    entry3.set_dev(dev);
    entry3.set_ino(ino);
    entry3.set_nlink(nlink);

    let action3 = resolver.resolve(&mut entry3);
    assert_eq!(
        action3,
        LinkAction::TarHardlink {
            target: "/tar/data.bin".to_string(),
            write_data: false
        }
    );
    assert_eq!(entry3.size, 0);
    assert_eq!(entry3.file_type, TTZipFileType::Hardlink);
    assert_eq!(entry3.hardlink_target.as_deref(), Some("/tar/data.bin"));

    // Memory safely freed on final link
    assert_eq!(resolver.pending_inodes_count(), 0);
}

#[test]
fn test_link_resolver_cpio_strategy() {
    let mut resolver = LinkResolver::new(LinkResolverStrategy::NewCpio);
    assert_eq!(resolver.strategy(), LinkResolverStrategy::NewCpio);

    let dev = 200;
    let ino = 84002;
    let nlink = 3;
    let original_size = 131072;

    // Node 1: First occurrence -> Metadata only, size 0
    let mut entry1 = TTZipEntry::new_file("/cpio/file1.dat", original_size);
    entry1.set_dev(dev);
    entry1.set_ino(ino);
    entry1.set_nlink(nlink);

    let action1 = resolver.resolve(&mut entry1);
    assert_eq!(action1, LinkAction::CpioMetadataOnly { write_data: false });
    assert_eq!(entry1.size, 0);
    assert_eq!(resolver.pending_inodes_count(), 1);

    // Node 2: Second occurrence -> Metadata only, size 0
    let mut entry2 = TTZipEntry::new_file("/cpio/file2.dat", original_size);
    entry2.set_dev(dev);
    entry2.set_ino(ino);
    entry2.set_nlink(nlink);

    let action2 = resolver.resolve(&mut entry2);
    assert_eq!(action2, LinkAction::CpioMetadataOnly { write_data: false });
    assert_eq!(entry2.size, 0);
    assert_eq!(resolver.pending_inodes_count(), 1);

    // Node 3: Final occurrence -> Final node with physical data and full size
    let mut entry3 = TTZipEntry::new_file("/cpio/file3.dat", 0);
    entry3.set_dev(dev);
    entry3.set_ino(ino);
    entry3.set_nlink(nlink);

    let action3 = resolver.resolve(&mut entry3);
    assert_eq!(action3, LinkAction::CpioFinalData { write_data: true });
    assert_eq!(entry3.size, original_size);
    assert_eq!(resolver.pending_inodes_count(), 0);
}

#[test]
fn test_mbs_wcs_utf8_synchronization_and_metadata() {
    let mut entry = TTZipEntry::new();
    entry.set_pathname("test_档案_archive.tar");

    assert_eq!(entry.pathname, "test_档案_archive.tar");
    assert_eq!(entry.pathname_mbs.as_deref(), Some("test_档案_archive.tar".as_bytes()));
    assert!(entry.pathname_wcs.is_some());

    // Setting via MBS
    let raw_bytes = b"another_path.txt";
    entry.set_pathname_mbs(raw_bytes);
    assert_eq!(entry.pathname, "another_path.txt");
    assert_eq!(entry.pathname_mbs.as_deref(), Some(raw_bytes.as_slice()));

    // Setting via WCS
    let wcs_chars: Vec<u32> = "wide_string.dat".chars().map(|c| c as u32).collect();
    entry.set_pathname_wcs(&wcs_chars);
    assert_eq!(entry.pathname, "wide_string.dat");
    assert_eq!(entry.pathname_wcs.as_deref(), Some(wcs_chars.as_slice()));

    // Extended attributes and ACLs
    entry.add_xattr("user.checksum", vec![0xDE, 0xAD, 0xBE, 0xEF]);
    entry.add_acl("user:1000:rwx");
    assert_eq!(entry.xattrs.get("user.checksum"), Some(&vec![0xDE, 0xAD, 0xBE, 0xEF]));
    assert_eq!(entry.acls, vec!["user:1000:rwx"]);
}

#[test]
fn test_lazy_stat_adapter_from_tempfile() {
    let mut tmp = NamedTempFile::new().expect("create tempfile");
    use std::io::Write;
    tmp.write_all(b"Hello TTZip Metadata Engine!").expect("write bytes");
    tmp.flush().expect("flush bytes");

    let entry = TTZipEntry::from_path(tmp.path()).expect("from_path succeeded");
    assert_eq!(entry.size, 28);
    assert!(entry.file_type.is_file());
    assert!(entry.is_field_set(EntryFields::SIZE));
    assert!(entry.is_field_set(EntryFields::PATHNAME));

    #[cfg(unix)]
    {
        assert!(entry.is_field_set(EntryFields::PERMISSIONS));
        assert!(entry.is_field_set(EntryFields::INO));
        assert!(entry.is_field_set(EntryFields::DEV));
        assert!(entry.is_field_set(EntryFields::MTIME));
        assert!(entry.ino > 0);
    }
}
