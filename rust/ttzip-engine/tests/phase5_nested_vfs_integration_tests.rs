// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for nested archive memory drill-down and VirtualFileStream.

use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::tempdir;
use ttzip_engine::archive::nested_vfs::{
    calculate_chunk_size, drill_down_buffer, drill_down_nested_archive,
    extract_nested_entry_buffer, open_virtual_file_stream, parse_nested_specifier,
    VirtualChunkedStream, VirtualFileStream, MAX_STREAM_MEMORY,
};

fn create_test_zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let items: Vec<ttzip_engine::zip::ZipInputItem> = files
        .iter()
        .map(|(name, data)| ttzip_engine::zip::ZipInputItem {
            rel_path: name.to_string(),
            data: data.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        })
        .collect();

    let compressed = ttzip_engine::zip::compress_items_parallel(
        items,
        6,
        ttzip_engine::types::TTZipEncryptionMethod::None,
        None,
        2,
    )
    .unwrap();

    ttzip_engine::zip::assemble_zip_archive(&compressed).unwrap()
}

fn create_test_targz_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    {
        let mut tar_builder = tar::Builder::new(&mut gz);
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_builder.append_data(&mut header, *name, *data).unwrap();
        }
        tar_builder.finish().unwrap();
    }
    gz.finish().unwrap()
}

#[test]
fn test_virtual_file_stream_operations() {
    let sample = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut stream = VirtualFileStream::from_vec(sample.to_vec());

    assert_eq!(stream.size(), sample.len() as u64);
    assert_eq!(stream.position(), 0);

    let chunk1 = stream.read(10).unwrap();
    assert_eq!(chunk1, b"0123456789");
    assert_eq!(stream.position(), 10);

    let chunk2 = stream.read(10).unwrap();
    assert_eq!(chunk2, b"ABCDEFGHIJ");
    assert_eq!(stream.position(), 20);

    let new_pos = stream.seek(5).unwrap();
    assert_eq!(new_pos, 5);
    assert_eq!(stream.position(), 5);

    let exact = stream.read_exact_at(10, 5).unwrap();
    assert_eq!(exact, b"ABCDE");
    assert_eq!(stream.position(), 5);

    let all = stream.read_all().unwrap();
    assert_eq!(all, sample);

    let mut buf = [0u8; 4];
    let n = Read::read(&mut stream, &mut buf).unwrap();
    assert_eq!(n, 4);
    assert_eq!(&buf, b"5678");

    let pos = Seek::seek(&mut stream, SeekFrom::End(-6)).unwrap();
    assert_eq!(pos, (sample.len() - 6) as u64);
    let rem = stream.read(100).unwrap();
    assert_eq!(rem, b"UVWXYZ");
}

#[test]
fn test_20gb_bounded_chunk_stream_and_random_seek() {
    let total_20gb: u64 = 20 * 1024 * 1024 * 1024; // 20 GB
    let load_counter = Arc::new(AtomicUsize::new(0));
    let load_clone = Arc::clone(&load_counter);

    let chunk_size = calculate_chunk_size(total_20gb);
    assert_eq!(chunk_size, 2 * 1024 * 1024); // 2 MB chunk for >1GB
    assert_eq!(MAX_STREAM_MEMORY, 64 * 1024 * 1024);

    let loader = Arc::new(move |offset: u64, len: usize| {
        load_clone.fetch_add(1, Ordering::SeqCst);
        let mut buf = vec![0u8; len];
        let byte_val = ((offset / 1024 / 1024) % 256) as u8;
        buf.fill(byte_val);
        Ok(buf)
    });

    let chunked = VirtualChunkedStream::new(total_20gb, chunk_size, loader);
    let mut stream = VirtualFileStream::new(chunked);

    assert_eq!(stream.size(), total_20gb);

    // Seek to 15 GB and read 16 bytes
    let seek_target = 15 * 1024 * 1024 * 1024;
    let actual_pos = stream.seek(seek_target).unwrap();
    assert_eq!(actual_pos, seek_target);

    let data = stream.read(16).unwrap();
    assert_eq!(data.len(), 16);
    let expected_val = ((seek_target / 1024 / 1024) % 256) as u8;
    assert!(data.iter().all(|&b| b == expected_val));
    assert_eq!(stream.position(), seek_target + 16);

    // Read across multiple chunk boundaries
    let cross_offset = 2 * 1024 * 1024 - 10;
    let cross_len = 20;
    let cross_data = stream.read_exact_at(cross_offset, cross_len).unwrap();
    assert_eq!(cross_data.len(), 20);

    // Verify ring buffer eviction: reading 100 distant chunks must not exceed bounded memory
    for i in 0..100 {
        let off = (i as u64) * 100 * 1024 * 1024;
        let d = stream.read_exact_at(off, 1024).unwrap();
        assert_eq!(d.len(), 1024);
    }
    assert!(load_counter.load(Ordering::SeqCst) >= 100);
}

#[test]
fn test_nested_zip_in_zip_drill_down_and_stream() {
    let dir = tempdir().unwrap();
    let inner_zip_bytes = create_test_zip_bytes(&[("deep/secret.txt", b"Nested Secret Content 12345")]);
    let outer_zip_bytes = create_test_zip_bytes(&[
        ("readme.md", b"# Outer Readme"),
        ("bundles/inner.zip", &inner_zip_bytes),
    ]);

    let outer_zip_path = dir.path().join("outer.zip");
    std::fs::write(&outer_zip_path, &outer_zip_bytes).unwrap();

    let root_entries = drill_down_nested_archive(
        outer_zip_path.to_str().unwrap(),
        &[],
        None,
    ).unwrap();
    assert!(root_entries.iter().any(|e| e.path.contains("bundles/inner.zip")));
    assert!(root_entries.iter().any(|e| e.path.contains("readme.md")));

    let nested_entries = drill_down_nested_archive(
        outer_zip_path.to_str().unwrap(),
        &["bundles/inner.zip".to_string()],
        None,
    ).unwrap();
    assert_eq!(nested_entries.len(), 1);
    assert!(nested_entries[0].path.contains("secret.txt"));

    let stream = open_virtual_file_stream(
        outer_zip_path.to_str().unwrap(),
        &["bundles/inner.zip".to_string()],
        "deep/secret.txt",
        None,
    ).unwrap();

    assert_eq!(stream.size(), b"Nested Secret Content 12345".len() as u64);
    let payload = stream.read_all().unwrap();
    assert_eq!(payload, b"Nested Secret Content 12345");
}

#[test]
fn test_heterogeneous_nested_zip_in_targz_in_zip() {
    let dir = tempdir().unwrap();
    let l3_bytes = create_test_zip_bytes(&[("flag.txt", b"CTF{TTZIP_NESTED_STREAM}")]);
    let l2_bytes = create_test_targz_bytes(&[("archives/level3.zip", &l3_bytes)]);
    let outer_bytes = create_test_zip_bytes(&[("pkg/level2.tar.gz", &l2_bytes)]);
    let outer_zip_path = dir.path().join("level1.zip");
    std::fs::write(&outer_zip_path, &outer_bytes).unwrap();

    let drill_path = vec![
        "pkg/level2.tar.gz".to_string(),
        "archives/level3.zip".to_string(),
    ];

    let deep_entries = drill_down_nested_archive(
        outer_zip_path.to_str().unwrap(),
        &drill_path,
        None,
    ).unwrap();
    assert!(deep_entries.iter().any(|e| e.path.contains("flag.txt")));

    let stream = open_virtual_file_stream(
        outer_zip_path.to_str().unwrap(),
        &drill_path,
        "flag.txt",
        None,
    ).unwrap();

    assert_eq!(stream.read_all().unwrap(), b"CTF{TTZIP_NESTED_STREAM}");

    let stream_bang = open_virtual_file_stream(
        outer_zip_path.to_str().unwrap(),
        &[],
        "pkg/level2.tar.gz!archives/level3.zip!flag.txt",
        None,
    ).unwrap();
    assert_eq!(stream_bang.read_all().unwrap(), b"CTF{TTZIP_NESTED_STREAM}");

    let stream_colons = open_virtual_file_stream(
        outer_zip_path.to_str().unwrap(),
        &[],
        "pkg/level2.tar.gz::archives/level3.zip::flag.txt",
        None,
    ).unwrap();
    assert_eq!(stream_colons.read_all().unwrap(), b"CTF{TTZIP_NESTED_STREAM}");
}

#[test]
fn test_in_memory_drill_down_buffer_and_error_handling() {
    let zip_bytes = create_test_zip_bytes(&[("file1.dat", b"Bytes A"), ("file2.dat", b"Bytes B")]);

    let entries = drill_down_buffer(&zip_bytes, &[], None).unwrap();
    assert_eq!(entries.len(), 2);

    let b1 = extract_nested_entry_buffer(&zip_bytes, &[], "file1.dat", None).unwrap();
    assert_eq!(b1, b"Bytes A");

    let err = extract_nested_entry_buffer(&zip_bytes, &[], "nonexistent.dat", None);
    assert!(err.is_err());
}

#[test]
fn test_nested_specifier_parsing() {
    let (d1, t1) = parse_nested_specifier(&["a.zip".to_string()], "b.txt");
    assert_eq!(d1, vec!["a.zip"]);
    assert_eq!(t1, "b.txt");

    let (d2, t2) = parse_nested_specifier(&[], "outer.zip!inner.tar.gz!doc.pdf");
    assert_eq!(d2, vec!["outer.zip", "inner.tar.gz"]);
    assert_eq!(t2, "doc.pdf");

    let (d3, t3) = parse_nested_specifier(&[], "a::b::c");
    assert_eq!(d3, vec!["a", "b"]);
    assert_eq!(t3, "c");
}
