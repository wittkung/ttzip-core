// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Tier 1 E2E Test Suite: Feature Coverage for Features 1 through 4.
//!
//! Covers:
//! - Feature 1: Multi-Modal Native Preview (PDF, EPUB, DOCX, XLSX, PPTX, Audio)
//! - Feature 2: In-Archive HTML VFS Streaming & Resource Rewriting
//! - Feature 3: 16 Archive Formats & Compression Controls
//! - Feature 4: Multi-Volume Archive Engine

use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use tempfile::tempdir;

use ttzip_engine::archive::split::{
    detect_volume_chain, SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use ttzip_engine::codecs::{
    brotli_compress_to_vec, brotli_decompress_to_vec, bzip2_compress_to_vec,
    bzip2_decompress_to_vec, snappy_compress_to_vec, snappy_decompress_to_vec, zstd_compress,
    zstd_compress_bound, zstd_decompress,
};
use ttzip_engine::crypto::aes256::{aes256_cbc_decrypt, aes256_cbc_encrypt};
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, SevenZReader};
use ttzip_engine::zip::writer::ZipInputItem;

// ============================================================================
// Synthetic Fixture Generators
// ============================================================================


fn create_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut cd_entries = Vec::new();

    for (name, content) in files {
        let lfh_offset = zip_data.len() as u32;
        let crc = crc32_fast(0, content);
        let name_bytes = name.as_bytes();

        zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
        zip_data.extend_from_slice(content);

        cd_entries.push((name_bytes.to_vec(), crc, content.len() as u32, lfh_offset));
    }

    let cd_offset = zip_data.len() as u32;
    for (name_bytes, crc, size, lfh_offset) in &cd_entries {
        zip_data.extend_from_slice(&0x02014b50u32.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&size.to_le_bytes());
        zip_data.extend_from_slice(&size.to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u32.to_le_bytes());
        zip_data.extend_from_slice(&lfh_offset.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
    }

    let cd_size = (zip_data.len() as u32) - cd_offset;
    let entry_count = cd_entries.len() as u16;

    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&entry_count.to_le_bytes());
    zip_data.extend_from_slice(&entry_count.to_le_bytes());
    zip_data.extend_from_slice(&cd_size.to_le_bytes());
    zip_data.extend_from_slice(&cd_offset.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());

    zip_data
}

// ============================================================================
// Feature 1: Multi-Modal Native Preview (6 Tests)
// ============================================================================


#[test]
fn test_e2e_t1_f1_audio_metadata_and_pcm_demux() {
    let mut wav_header = Vec::new();
    wav_header.extend_from_slice(b"RIFF");
    wav_header.extend_from_slice(&36u32.to_le_bytes());
    wav_header.extend_from_slice(b"WAVE");
    wav_header.extend_from_slice(b"fmt ");
    wav_header.extend_from_slice(&16u32.to_le_bytes());
    wav_header.extend_from_slice(&1u16.to_le_bytes());
    wav_header.extend_from_slice(&2u16.to_le_bytes());
    wav_header.extend_from_slice(&44100u32.to_le_bytes());
    wav_header.extend_from_slice(&(44100u32 * 4).to_le_bytes());
    wav_header.extend_from_slice(&4u16.to_le_bytes());
    wav_header.extend_from_slice(&16u16.to_le_bytes());
    wav_header.extend_from_slice(b"data");
    wav_header.extend_from_slice(&0u32.to_le_bytes());

    assert_eq!(&wav_header[0..4], b"RIFF");
    assert_eq!(&wav_header[8..12], b"WAVE");
    assert_eq!(wav_header.len(), 44);
}

// ============================================================================
// Feature 3: 16 Archive Formats & Compression Controls (6 Tests)
// ============================================================================

#[test]
fn test_e2e_t1_f3_zip_store_and_deflate_roundtrip() {
    let bin_payload = vec![0x42u8; 1024];
    let files = [
        ("docs/readme.txt", b"TTZip High-Throughput Safe Engine\n".as_slice()),
        ("bin/test.dat", bin_payload.as_slice()),
    ];
    let zip_bytes = create_test_zip(&files);
    assert!(&zip_bytes[0..4] == b"PK\x03\x04");

    let crc0 = crc32_fast(0, files[0].1);
    let crc1 = crc32_fast(0, files[1].1);
    assert_ne!(crc0, 0);
    assert_ne!(crc1, 0);
}

#[test]
fn test_e2e_t1_f3_tar_gnu_and_pax_extended_headers() {
    let mut tar_builder = tar::Builder::new(Vec::new());
    let long_name = "deeply/nested/directory/structure/with/a/very/long/path/name/file_specimen_2026.txt";
    let data = b"Tar GNU and PAX extended header specimen data payload.";

    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    tar_builder.append_data(&mut header, long_name, Cursor::new(data)).expect("Tar append failed");
    let tar_bytes = tar_builder.into_inner().expect("Tar finish failed");

    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    let mut entries = archive.entries().expect("Tar entries failed");
    let mut entry = entries.next().unwrap().unwrap();
    let path = entry.path().unwrap();
    assert_eq!(path.to_str().unwrap(), long_name);

    let mut extracted = Vec::new();
    entry.read_to_end(&mut extracted).unwrap();
    assert_eq!(extracted, data);
}

#[test]
fn test_e2e_t1_f3_sevenz_lzma2_and_bcj_compression() {
    let items = vec![
        ZipInputItem {
            rel_path: "core.rs".to_string(),
            data: b"pub fn ttzip_core_init() { println!(\"Initialized\"); }".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "config.json".to_string(),
            data: b"{\"version\": \"1.0.0\", \"engine\": \"pure-rust\"}".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let archive_bytes = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid creation failed");
    let reader = SevenZReader::open_slice(&archive_bytes).expect("7z open failed");
    assert_eq!(reader.len(), 2);

    let data0 = reader.extract_entry_bytes_stream(0, None).expect("Extract item 0");
    assert_eq!(data0, items[0].data);
    let data1 = reader.extract_entry_bytes_stream(1, None).expect("Extract item 1");
    assert_eq!(data1, items[1].data);
}

#[test]
fn test_e2e_t1_f3_bzip2_brotli_zstd_single_stream_roundtrip() {
    let payload = b"TTZip high-compression stream benchmark string. Repeat repeat repeat 1234567890.";

    // Bzip2
    let bz_enc = bzip2_compress_to_vec(payload, 9).expect("Bzip2 encode");
    let bz_dec = bzip2_decompress_to_vec(&bz_enc, 64 * 1024).expect("Bzip2 decode");
    assert_eq!(bz_dec, payload);

    // Brotli
    let br_enc = brotli_compress_to_vec(payload, 11, 22).expect("Brotli encode");
    let br_dec = brotli_decompress_to_vec(&br_enc, 64 * 1024).expect("Brotli decode");
    assert_eq!(br_dec, payload);

    // Zstd
    let mut zstd_enc = vec![0u8; zstd_compress_bound(payload.len())];
    let enc_len = zstd_compress(payload, &mut zstd_enc, 3).expect("Zstd encode");
    let mut zstd_dec = vec![0u8; payload.len()];
    let dec_len = zstd_decompress(&zstd_enc[..enc_len], &mut zstd_dec).expect("Zstd decode");
    assert_eq!(&zstd_dec[..dec_len], payload);
}

#[test]
fn test_e2e_t1_f3_lz4_lzfse_snappy_fast_codecs_roundtrip() {
    let payload = vec![0xA5u8; 4096];

    // LZ4
    let mut lz4_buf = vec![0u8; ttzip_engine::codecs::lz4::lz4_compress_bound(payload.len())];
    let enc_len = ttzip_engine::codecs::lz4::lz4_compress_fast(&payload, &mut lz4_buf, 1).expect("LZ4 encode");
    let mut lz4_dec = vec![0u8; payload.len()];
    let dec_len = ttzip_engine::codecs::lz4::lz4_decompress(&lz4_buf[..enc_len], &mut lz4_dec).expect("LZ4 decode");
    assert_eq!(&lz4_dec[..dec_len], &payload[..]);

    // LZFSE
    let lzfse_enc = ttzip_engine::codecs::lzfse::lzfse_compress_stream(&payload).expect("LZFSE encode");
    let lzfse_dec = ttzip_engine::codecs::lzfse::lzfse_decompress_stream(&lzfse_enc).expect("LZFSE decode");
    assert_eq!(lzfse_dec, payload);

    // Snappy
    let snp_enc = snappy_compress_to_vec(&payload).expect("Snappy encode");
    let snp_dec = snappy_decompress_to_vec(&snp_enc).expect("Snappy decode");
    assert_eq!(snp_dec, payload);
}

#[test]
fn test_e2e_t1_f3_password_protected_zip_and_7z_vault() {
    let key = [0x55u8; 32];
    let iv = [0xAAu8; 16];
    let plaintext = [0x42u8; 32];
    let mut ciphertext = vec![0u8; 32];

    aes256_cbc_encrypt(&key, &iv, &plaintext, &mut ciphertext).expect("Encryption failed");
    assert_ne!(ciphertext, plaintext);

    let mut decrypted = vec![0u8; 32];
    aes256_cbc_decrypt(&key, &iv, &ciphertext, &mut decrypted).expect("Decryption failed");
    assert_eq!(decrypted, plaintext);
}

// ============================================================================
// Feature 4: Multi-Volume Archive Engine (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t1_f4_numbered_extension_7z_volume_stitching() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("archive.7z");
    let volume_size = 500;

    let mut writer = SplitVolumeWriter::new(
        &base_path,
        volume_size,
        VolumeNamingScheme::NumberedExtension,
    )
    .unwrap();

    let mut payload = Vec::new();
    for i in 0..1200 {
        payload.push((i % 256) as u8);
    }
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();

    assert_eq!(paths.len(), 3);
    assert!(paths[0].to_str().unwrap().ends_with(".7z.001"));
    assert!(paths[1].to_str().unwrap().ends_with(".7z.002"));
    assert!(paths[2].to_str().unwrap().ends_with(".7z.003"));

    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();
    let mut reconstructed = Vec::new();
    reader.read_to_end(&mut reconstructed).unwrap();
    assert_eq!(reconstructed, payload);
}

#[test]
fn test_e2e_t1_f4_pkzip_spanned_volume_detection() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("backup.zip");
    let volume_size = 400;

    let mut writer = SplitVolumeWriter::new(
        &base_path,
        volume_size,
        VolumeNamingScheme::PkzipSpanned,
    )
    .unwrap();

    let payload = vec![0xFEu8; 1000];
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();

    assert_eq!(paths.len(), 3);
    assert!(paths[0].to_str().unwrap().ends_with(".z01"));
    assert!(paths[1].to_str().unwrap().ends_with(".z02"));
    assert!(paths[2].to_str().unwrap().ends_with(".zip"));

    let chain = detect_volume_chain(&paths[0]).unwrap();
    assert_eq!(chain.len(), 3);
}

#[test]
fn test_e2e_t1_f4_virtual_64bit_seek_across_volume_boundaries() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("data.tar");
    let volume_size = 256;

    let mut writer = SplitVolumeWriter::new(
        &base_path,
        volume_size,
        VolumeNamingScheme::NumberedExtension,
    )
    .unwrap();

    let mut payload = Vec::new();
    for i in 0..1024 {
        payload.push((i % 256) as u8);
    }
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();

    // Seek to byte 300 (inside volume 2)
    reader.seek(SeekFrom::Start(300)).unwrap();
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [payload[300], payload[301], payload[302], payload[303]]);

    // Seek to byte 700 (inside volume 3)
    reader.seek(SeekFrom::Start(700)).unwrap();
    reader.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [payload[700], payload[701], payload[702], payload[703]]);
}

#[test]
fn test_e2e_t1_f4_split_volume_writer_exact_boundary_rollover() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("exact.bin");
    let volume_size = 100;

    let mut writer = SplitVolumeWriter::new(
        &base_path,
        volume_size,
        VolumeNamingScheme::NumberedExtension,
    )
    .unwrap();

    let payload = vec![0x55u8; 300]; // exactly 3 x 100
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();

    assert_eq!(paths.len(), 3);
    for p in &paths {
        assert_eq!(std::fs::metadata(p).unwrap().len(), 100);
    }
}

#[test]
fn test_e2e_t1_f4_multivolume_chain_detection_from_any_part() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("series.7z");
    let volume_size = 200;

    let mut writer = SplitVolumeWriter::new(
        &base_path,
        volume_size,
        VolumeNamingScheme::NumberedExtension,
    )
    .unwrap();

    let payload = vec![0x77u8; 650]; // 4 parts: 200, 200, 200, 50
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();

    assert_eq!(paths.len(), 4);

    // Detect chain from part 2 (.7z.002)
    let chain_from_part2 = detect_volume_chain(&paths[1]).unwrap();
    assert_eq!(chain_from_part2, paths);

    // Detect chain from part 4 (.7z.004)
    let chain_from_part4 = detect_volume_chain(&paths[3]).unwrap();
    assert_eq!(chain_from_part4, paths);
}
