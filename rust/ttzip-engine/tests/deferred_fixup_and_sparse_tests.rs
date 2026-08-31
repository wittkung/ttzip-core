// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for DepthFirstDirFixup reverse directory attribute restoration
//! and APFS native sparse file hole-punching state machines.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;
use ttzip_engine::archive::unified::entry::fields::EntryFields;
use ttzip_engine::archive::unified::entry::timestamp::TTZipTimestamp;
use ttzip_engine::archive::unified::entry::TTZipEntry;
use ttzip_engine::fs::deferred_fixup::{DepthFirstDirFixup, DirFixupItem};
use ttzip_engine::fs::sparse::{is_zero_block, SparseFileWriter};

#[test]
fn test_nested_readonly_directory_structure_fixup() {
    let tmp = tempdir().expect("tempdir");
    let base = tmp.path();

    let dir_l1 = base.join("readonly_l1");
    let dir_l2 = dir_l1.join("readonly_l2");
    let dir_l3 = dir_l2.join("readonly_l3");

    let file_l1 = dir_l1.join("file_l1.txt");
    let file_l2 = dir_l2.join("file_l2.txt");
    let file_l3 = dir_l3.join("file_l3.txt");

    let mut fixup = DepthFirstDirFixup::new();

    // 1. Create directory hierarchy with temporary permissive permissions (0o700)
    fixup
        .create_dir_all_secure(&dir_l3, Some(0o555), None, None)
        .expect("create dir l3");
    fixup.register_dir(&dir_l2, Some(0o555), None, None);
    fixup.register_dir(&dir_l1, Some(0o555), None, None);

    // Verify all directories are initially writable during extraction stage
    let meta_l1_init = fs::metadata(&dir_l1).expect("meta l1 init");
    let meta_l2_init = fs::metadata(&dir_l2).expect("meta l2 init");
    let meta_l3_init = fs::metadata(&dir_l3).expect("meta l3 init");
    assert_eq!(meta_l1_init.permissions().mode() & 0o777, 0o700);
    assert_eq!(meta_l2_init.permissions().mode() & 0o777, 0o700);
    assert_eq!(meta_l3_init.permissions().mode() & 0o777, 0o700);

    // 2. Write child files inside nested directories
    fs::write(&file_l1, b"Payload Level 1").expect("write file l1");
    fs::write(&file_l2, b"Payload Level 2").expect("write file l2");
    fs::write(&file_l3, b"Payload Level 3").expect("write file l3");

    // 3. Execute deferred fixup in descending depth order (L3 -> L2 -> L1)
    fixup.apply_all(true).expect("apply fixup");

    // 4. Verify all files are intact and directory permissions are now read-only (0o555)
    assert_eq!(fs::read(&file_l1).expect("read l1"), b"Payload Level 1");
    assert_eq!(fs::read(&file_l2).expect("read l2"), b"Payload Level 2");
    assert_eq!(fs::read(&file_l3).expect("read l3"), b"Payload Level 3");

    let meta_l1_final = fs::metadata(&dir_l1).expect("meta l1 final");
    let meta_l2_final = fs::metadata(&dir_l2).expect("meta l2 final");
    let meta_l3_final = fs::metadata(&dir_l3).expect("meta l3 final");

    assert_eq!(meta_l1_final.permissions().mode() & 0o777, 0o555);
    assert_eq!(meta_l2_final.permissions().mode() & 0o777, 0o555);
    assert_eq!(meta_l3_final.permissions().mode() & 0o777, 0o555);
}

#[test]
fn test_parent_directory_nanosecond_mtime_invariance() {
    let tmp = tempdir().expect("tempdir");
    let base = tmp.path();

    let parent_dir = base.join("timed_parent");
    let child_dir = parent_dir.join("timed_child");
    let child_file = child_dir.join("child_payload.bin");

    let parent_target_mtime = TTZipTimestamp::new(1680000000, 123456789);
    let child_target_mtime = TTZipTimestamp::new(1690000000, 987654321);

    let mut fixup = DepthFirstDirFixup::new();

    fixup
        .create_dir_all_secure(&child_dir, Some(0o755), Some(child_target_mtime), None)
        .expect("create child dir");
    fixup.register_dir(&parent_dir, Some(0o755), Some(parent_target_mtime), None);

    // Create child file and modify directory contents
    fs::write(&child_file, b"High precision timestamp invariant payload").expect("write file");

    // Execute reverse depth-first fixup (child_dir then parent_dir)
    fixup.apply_all(true).expect("apply fixup");

    // Read back high precision timestamps using libc stat / symlink_metadata
    let parent_stat = get_nanosecond_mtime(&parent_dir);
    let child_stat = get_nanosecond_mtime(&child_dir);

    assert_eq!(parent_stat.sec, parent_target_mtime.sec);
    assert_eq!(parent_stat.nsec, parent_target_mtime.nsec);

    assert_eq!(child_stat.sec, child_target_mtime.sec);
    assert_eq!(child_stat.nsec, child_target_mtime.nsec);
}

#[test]
fn test_sparse_100mb_virtual_file_allocation_and_integrity() {
    let tmp = tempdir().expect("tempdir");
    let sparse_path = tmp.path().join("sparse_100mb.bin");

    let total_size: u64 = 100 * 1024 * 1024; // 100 MB
    let mut writer = SparseFileWriter::create(&sparse_path)
        .expect("create sparse writer")
        .with_block_size(16384);

    writer.set_target_size(total_size);

    // 1. Data Extent 1 at 16MB .. 17MB (1MB non-zero pattern 0x77)
    let offset_1: u64 = 16 * 1024 * 1024;
    let data_1 = vec![0x77u8; 1024 * 1024];
    writer
        .write_extent(offset_1, &data_1)
        .expect("write extent 1");

    // 2. Data Extent 2 at 64MB .. 64MB + 64KB (64KB non-zero pattern 0x88)
    let offset_2: u64 = 64 * 1024 * 1024;
    let data_2 = vec![0x88u8; 64 * 1024];
    writer
        .write_extent(offset_2, &data_2)
        .expect("write extent 2");

    // 3. Finalize sparse file (closing trailing holes via ftruncate)
    let final_len = writer.finish().expect("finish sparse writer");
    assert_eq!(final_len, total_size);

    // Verify logical size on filesystem
    let meta = fs::metadata(&sparse_path).expect("metadata");
    assert_eq!(meta.len(), total_size);

    // Verify physical disk allocation (APFS Extent Hole Verification)
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::MetadataExt;
        let physical_bytes = meta.blocks() * 512;
        // 1MB + 64KB data + filesystem metadata is < 4MB on APFS
        assert!(
            physical_bytes < 5 * 1024 * 1024,
            "Physical bytes {} should be < 5MB on APFS (logical size is 100MB)",
            physical_bytes
        );
    }

    // Verify data integrity across sparse holes and non-zero extents
    let mut file = File::open(&sparse_path).expect("open sparse file");

    // Check leading hole (0 .. 16MB) is all zeros
    let mut leading_buf = vec![0u8; 65536];
    file.read_exact(&mut leading_buf).expect("read leading hole");
    assert!(is_zero_block(&leading_buf));

    // Check Data Extent 1 (16MB .. 17MB)
    file.seek(SeekFrom::Start(offset_1))
        .expect("seek to extent 1");
    let mut extent1_buf = vec![0u8; 1024 * 1024];
    file.read_exact(&mut extent1_buf).expect("read extent 1");
    assert_eq!(&extent1_buf[..], &data_1[..]);

    // Check Middle Hole (17MB .. 64MB)
    let mut middle_buf = vec![0u8; 65536];
    file.read_exact(&mut middle_buf).expect("read middle hole");
    assert!(is_zero_block(&middle_buf));

    // Check Data Extent 2 (64MB .. 64MB + 64KB)
    file.seek(SeekFrom::Start(offset_2))
        .expect("seek to extent 2");
    let mut extent2_buf = vec![0u8; 64 * 1024];
    file.read_exact(&mut extent2_buf).expect("read extent 2");
    assert_eq!(&extent2_buf[..], &data_2[..]);

    // Check Trailing Hole (64MB + 64KB .. 100MB)
    file.seek(SeekFrom::Start(total_size - 65536))
        .expect("seek to trailing hole");
    let mut trailing_buf = vec![0u8; 65536];
    file.read_exact(&mut trailing_buf).expect("read trailing hole");
    assert!(is_zero_block(&trailing_buf));
}

#[test]
fn test_sparse_writer_streaming_zero_block_detection() {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join("sparse_stream.bin");

    let mut writer = SparseFileWriter::create(&path)
        .expect("create writer")
        .with_block_size(16384);

    // Stream 64KB zeros, 32KB data, 64KB zeros in 4KB chunks
    let zero_chunk = vec![0u8; 4096];
    let data_chunk = vec![0xABu8; 4096];

    for _ in 0..16 {
        writer.write_all(&zero_chunk).expect("write zero chunk");
    }
    for _ in 0..8 {
        writer.write_all(&data_chunk).expect("write data chunk");
    }
    for _ in 0..16 {
        writer.write_all(&zero_chunk).expect("write zero chunk");
    }

    let final_size = writer.finish().expect("finish");
    assert_eq!(final_size, 163840);

    let read_back = fs::read(&path).expect("read back");
    assert_eq!(read_back.len(), 163840);
    assert!(is_zero_block(&read_back[0..65536]));
    assert_eq!(&read_back[65536..98304], &vec![0xABu8; 32768][..]);
    assert!(is_zero_block(&read_back[98304..163840]));
}

#[test]
fn test_dir_fixup_from_unified_entry_conversion() {
    let mut entry = TTZipEntry::new_dir("nested/folder/path");
    entry.mode = 0o040750;
    entry.mtime = Some(TTZipTimestamp::new(1710000000, 500000));
    entry.atime = Some(TTZipTimestamp::new(1710000010, 600000));
    entry.uid = 501;
    entry.gid = 20;
    entry.fields = EntryFields::PERMISSIONS
        | EntryFields::MTIME
        | EntryFields::ATIME
        | EntryFields::UID
        | EntryFields::GID;

    let dest = Path::new("/tmp/extract_dest");
    let item = DirFixupItem::from_entry(&entry, dest);

    assert_eq!(item.path, dest.join("nested/folder/path"));
    assert_eq!(item.mode, Some(0o040750));
    assert_eq!(item.mtime, Some(TTZipTimestamp::new(1710000000, 500000)));
    assert_eq!(item.atime, Some(TTZipTimestamp::new(1710000010, 600000)));
    assert_eq!(item.uid, Some(501));
    assert_eq!(item.depth(), 6); // / + tmp + extract_dest + nested + folder + path = 6 components
}

fn get_nanosecond_mtime(path: &Path) -> TTZipTimestamp {
    use std::ffi::CString;
    let c_path = CString::new(path.to_str().expect("valid utf-8")).expect("cstring");
    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        assert_eq!(libc::lstat(c_path.as_ptr(), &mut st), 0);
        TTZipTimestamp::new(st.st_mtime as i64, st.st_mtime_nsec as u32)
    }
}
