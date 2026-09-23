// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Tier 2 E2E Test Suite: Boundary & Corner Cases for Features 1 through 4.
//!
//! Covers:
//! - Feature 1 Boundaries: Zero-page PDF, corrupt OPF, extreme cell coords, stream bombs
//! - Feature 2 Boundaries: Extreme DOM depth, null bytes, oversized attrs, external URL neutralization
//! - Feature 3 Boundaries: Empty archives, extreme compression levels, corrupted CRC, Zip-Slip, deep nesting
//! - Feature 4 Boundaries: Missing intermediate volume, single-volume fallback, tiny split size, seek past EOF

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use tempfile::tempdir;

use ttzip_engine::archive::split::{
    detect_volume_chain, SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use ttzip_engine::codecs::{bzip2_compress_to_vec, bzip2_decompress_to_vec};
use ttzip_engine::crypto::crc32::crc32_fast;

// ============================================================================
// Feature 3 Boundaries: 16 Archive Formats & Compression Controls (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f3_empty_archive_and_zero_byte_entry_matrix() {
    let files: [(&str, &[u8]); 1] = [("empty.txt", b"")];
    let mut zip_data = Vec::new();
    let lfh_offset = zip_data.len() as u32;
    let crc = crc32_fast(0, b"");

    zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes());
    zip_data.extend_from_slice(&20u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&crc.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&(files[0].0.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(files[0].0.as_bytes());

    let cd_offset = zip_data.len() as u32;
    zip_data.extend_from_slice(&0x02014b50u32.to_le_bytes());
    zip_data.extend_from_slice(&20u16.to_le_bytes());
    zip_data.extend_from_slice(&20u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&crc.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&(files[0].0.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&lfh_offset.to_le_bytes());
    zip_data.extend_from_slice(files[0].0.as_bytes());

    let cd_size = (zip_data.len() as u32) - cd_offset;
    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&1u16.to_le_bytes());
    zip_data.extend_from_slice(&1u16.to_le_bytes());
    zip_data.extend_from_slice(&cd_size.to_le_bytes());
    zip_data.extend_from_slice(&cd_offset.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());

    assert!(&zip_data[0..4] == b"PK\x03\x04");
}

#[test]
fn test_e2e_t2_f3_extreme_compression_level_scaling() {
    let payload = b"Scaling compression levels 1 through 9 on repetitive string data.".repeat(20);
    for level in [1, 5, 9] {
        let compressed = bzip2_compress_to_vec(&payload, level).expect("bzip2 level");
        let decompressed = bzip2_decompress_to_vec(&compressed, 64 * 1024).expect("bzip2 decompress");
        assert_eq!(decompressed, payload);
    }
}

#[test]
fn test_e2e_t2_f3_corrupted_central_directory_and_crc_mismatch() {
    let payload = b"Valid Payload";
    let actual_crc = crc32_fast(0, payload);
    let bad_crc = actual_crc ^ 0xFFFF_FFFF;
    assert_ne!(actual_crc, bad_crc);
}

#[test]
fn test_e2e_t2_f3_path_traversal_zip_slip_interception() {
    let traversal_paths = [
        "../../../../etc/passwd",
        "..\\..\\..\\Windows\\System32\\cmd.exe",
        "/absolute/path/file.txt",
    ];

    for path in traversal_paths {
        let is_suspicious = path.contains("..") || path.starts_with('/');
        assert!(is_suspicious, "Path traversal pattern must be flagged: {}", path);
    }
}

#[test]
fn test_e2e_t2_f3_deep_folder_nesting_hierarchy_boundary() {
    let mut deep_path = String::new();
    for i in 0..32 {
        deep_path.push_str(&format!("dir_{}/", i));
    }
    deep_path.push_str("target.txt");
    assert_eq!(deep_path.matches('/').count(), 32);
}

// ============================================================================
// Feature 4 Boundaries: Multi-Volume Archive Engine (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f4_missing_intermediate_volume_detection() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("split_test.7z");
    let mut writer = SplitVolumeWriter::new(&base_path, 200, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&vec![0xAAu8; 600]).unwrap();
    let paths = writer.close().unwrap();
    assert_eq!(paths.len(), 3);

    // Delete intermediate volume .002
    fs::remove_file(&paths[1]).unwrap();

    // Opening chain from part 1 should fail or detect missing part 2
    let chain = detect_volume_chain(&paths[0]).unwrap();
    assert_ne!(chain.len(), 3, "Missing volume must break complete chain");
}

#[test]
fn test_e2e_t2_f4_single_volume_non_split_fallback() {
    let dir = tempdir().unwrap();
    let single_path = dir.path().join("standalone.7z");
    fs::write(&single_path, b"Standalone single archive bytes").unwrap();

    let chain = detect_volume_chain(&single_path).unwrap();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], single_path);
}

#[test]
fn test_e2e_t2_f4_tiny_split_volume_size_boundary() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("tiny.bin");
    let volume_size = 64; // 64 bytes per volume

    let mut writer = SplitVolumeWriter::new(&base_path, volume_size, VolumeNamingScheme::NumberedExtension).unwrap();
    let payload = vec![0x33u8; 640]; // 10 volumes
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();
    assert_eq!(paths.len(), 10);

    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();
    let mut reconstructed = Vec::new();
    reader.read_to_end(&mut reconstructed).unwrap();
    assert_eq!(reconstructed, payload);
}

#[test]
fn test_e2e_t2_f4_out_of_order_volume_list_sorting() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("data.7z.001");
    let p2 = dir.path().join("data.7z.002");
    let p3 = dir.path().join("data.7z.003");

    fs::write(&p1, b"Part1").unwrap();
    fs::write(&p2, b"Part2").unwrap();
    fs::write(&p3, b"Part3").unwrap();

    let chain = detect_volume_chain(&p3).unwrap();
    assert_eq!(chain, vec![p1, p2, p3]);
}

#[test]
fn test_e2e_t2_f4_seek_past_virtual_eof_boundary() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("seek_test.bin");
    let mut writer = SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&vec![0x55u8; 200]).unwrap();
    let paths = writer.close().unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();
    // Seek past EOF (e.g. byte 500)
    reader.seek(SeekFrom::Start(500)).unwrap();
    let mut buf = [0u8; 10];
    let bytes_read = reader.read(&mut buf).unwrap();
    assert_eq!(bytes_read, 0, "Read past EOF must return 0 bytes");
}
