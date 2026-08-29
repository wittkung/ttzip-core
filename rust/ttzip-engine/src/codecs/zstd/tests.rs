// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration tests for Zstandard Advanced Streaming,
//! LDM (Long Distance Matching), Pre-trained Dictionaries, and FSE/Huff0 entropy micro-kernels.

use super::*;
use std::io::{Cursor, Read, Write};

#[test]
fn test_zstd_basic_roundtrip() {
    let input = b"TTZip High-performance ZSTD compression engine test string in Safe Rust.";
    let mut compressed = vec![0u8; zstd_compress_bound(input.len())];
    let comp_len = zstd_compress(input, &mut compressed, 3).expect("zstd compression failed");
    assert!(comp_len > 0);

    let detected_size = zstd_get_decompressed_size(&compressed[..comp_len]);
    assert_eq!(detected_size, Some(input.len() as u64));

    let mut decompressed = vec![0u8; input.len()];
    let decomp_len = zstd_decompress(&compressed[..comp_len], &mut decompressed)
        .expect("zstd decompression failed");
    assert_eq!(decomp_len, input.len());
    assert_eq!(&decompressed[..decomp_len], input);
}

#[test]
fn test_zstd_advanced_multithread_ldm() {
    let pattern = b"Long repetitive block data designed for Zstandard Long Distance Matching (LDM) verification. ";
    let mut input = Vec::new();
    for _ in 0..1000 {
        input.extend_from_slice(pattern);
    }

    let config = ZstdConfig {
        level: 6,
        nb_workers: 2,
        job_size_mb: 1,
        overlap_log: 2,
        window_log: 20,
        enable_ldm: true,
        enable_checksum: true,
        ldm_hash_log: 0,
        ldm_min_match: 0,
        ldm_bucket_size_log: 0,
        ldm_hash_rate_log: 0,
    };

    let mut compressed = vec![0u8; zstd_compress_bound(input.len())];
    let comp_len = zstd_compress_advanced(&input, &mut compressed, &config)
        .expect("zstd advanced compression failed");
    assert!(comp_len > 0);
    assert!(comp_len < input.len() / 5);

    let mut decompressed = vec![0u8; input.len()];
    let decomp_len = zstd_decompress(&compressed[..comp_len], &mut decompressed)
        .expect("zstd decompression failed");
    assert_eq!(decomp_len, input.len());
    assert_eq!(&decompressed, &input);
}

#[test]
fn test_zstd_ldm_distant_block_deduplication() {
    // Construct a payload where two identical 32KB blocks are separated by 2MB of pseudorandom data.
    let block_size = 32 * 1024;
    let dist_gap = 2 * 1024 * 1024;

    let marker_block: Vec<u8> = (0..block_size).map(|i| ((i * 37 + 13) % 256) as u8).collect();
    let gap_filler: Vec<u8> = (0..dist_gap).map(|i| ((i * 101 + 79) % 256) as u8).collect();

    let mut payload = Vec::with_capacity(block_size * 2 + dist_gap);
    payload.extend_from_slice(&marker_block);
    payload.extend_from_slice(&gap_filler);
    payload.extend_from_slice(&marker_block);

    // 1. Compress with LDM enabled (window 64MB, window_log = 26)
    let ldm_config = ZstdConfig::ldm(9, 26).with_ldm_tuning(18, 32, 3, 2);
    let mut ldm_comp = vec![0u8; zstd_compress_bound(payload.len())];
    let ldm_comp_len = zstd_compress_advanced(&payload, &mut ldm_comp, &ldm_config)
        .expect("ldm compress failed");

    // 2. Verify lossless decompression
    let mut decomp = vec![0u8; payload.len()];
    let mut dctx = ZstdDCtx::new().expect("create dctx");
    dctx.set_max_window_log(28).expect("set window log");
    let decomp_len = dctx.decompress(&ldm_comp[..ldm_comp_len], &mut decomp)
        .expect("ldm decompress failed");
    assert_eq!(decomp_len, payload.len());
    assert_eq!(decomp, payload);
}

#[test]
fn test_zstd_dictionary_micro_files_gain() {
    // Generate small JSON micro-files (<500 bytes each)
    let mut samples = Vec::new();
    for i in 0..40 {
        let json = format!(
            r#"{{"user_id":{},"event":"login","timestamp":"2026-08-29T10:00:00Z","status":"active","roles":["reader","editor"],"session_key":"tok_{:08x}"}}"#,
            1000 + i,
            i * 9999
        );
        samples.push(json.into_bytes());
    }

    let sample_refs: Vec<&[u8]> = samples.iter().map(|s| s.as_slice()).collect();
    let dict = ZstdDictionary::train("user_sessions", &sample_refs, 4096, 3)
        .expect("train dictionary failed");

    assert_eq!(dict.name(), "user_sessions");
    assert!(dict.dict_id() > 0);

    // Test a new unseen micro-file
    let test_file = br#"{"user_id":9999,"event":"login","timestamp":"2026-08-29T10:00:00Z","status":"active","roles":["reader","editor"],"session_key":"tok_00abcdef"}"#;

    // 1. Without dictionary
    let mut no_dict_comp = vec![0u8; zstd_compress_bound(test_file.len())];
    let no_dict_len = zstd_compress(test_file, &mut no_dict_comp, 3).expect("no dict compress");

    // 2. With dictionary
    let mut with_dict_comp = vec![0u8; zstd_compress_bound(test_file.len())];
    let with_dict_len = dict.compress_small(test_file, &mut with_dict_comp).expect("dict compress");

    // Dict compression should be significantly smaller on micro-files
    assert!(
        with_dict_len < no_dict_len,
        "Dict compressed len {} should be smaller than no-dict len {}",
        with_dict_len,
        no_dict_len
    );

    // 3. Lossless decompression with dictionary
    let mut decomp = vec![0u8; test_file.len()];
    let decomp_len = dict.decompress_small(&with_dict_comp[..with_dict_len], &mut decomp)
        .expect("dict decompress");
    assert_eq!(decomp_len, test_file.len());
    assert_eq!(&decomp[..decomp_len], test_file);
}

#[test]
fn test_zstd_dictionary_manager_global_112kb() {
    let mgr = ZstdDictionaryManager::global();
    let dict_112kb = mgr.ensure_standard_112kb();
    assert_eq!(dict_112kb.name(), "ttzip_std_112kb");
    assert!(!dict_112kb.raw_bytes().is_empty());

    let payload = br#"{"status":"success","code":200,"data":{"id":"item_0042","name":"TTZip File 0042","path":"/usr/local/share/data/0042.json","tags":["zstd","compression","microkernel","archive"],"attributes":{"size":4096,"crc32":"0xABCD1234","compressed":true,"timestamp":"2026-08-29T00:00:00Z"}},"metadata":{"schema_version":"3.2.0","engine":"ttzip-rust"}}"#;

    let mut comp_buf = vec![0u8; zstd_compress_bound(payload.len())];
    let comp_len = mgr.compress_small_file("ttzip_std_112kb", payload, &mut comp_buf)
        .expect("compress small file via manager");
    assert!(comp_len > 0);

    let mut decomp_buf = vec![0u8; payload.len()];
    let decomp_len = dict_112kb.decompress_small(&comp_buf[..comp_len], &mut decomp_buf)
        .expect("decompress small file via dict");
    assert_eq!(decomp_len, payload.len());
    assert_eq!(&decomp_buf[..decomp_len], payload);
}

#[test]
fn test_fse_entropy_roundtrip() {
    let mut payload = Vec::new();
    // Skewed probability distribution
    for _ in 0..500 {
        payload.extend_from_slice(b"AAAAAABBBCCDE");
    }

    let mut comp = vec![0u8; fse_compress_bound(payload.len())];
    let comp_len = fse_compress(&payload, &mut comp).expect("fse_compress failed");
    assert!(comp_len > 0);
    assert!(comp_len < payload.len());

    let mut decomp = vec![0u8; payload.len()];
    let decomp_len = fse_decompress(&comp[..comp_len], &mut decomp).expect("fse_decompress failed");
    assert_eq!(decomp_len, payload.len());
    assert_eq!(decomp, payload);
}

#[test]
fn test_huff0_4x_and_1x_roundtrip() {
    let mut payload = Vec::new();
    for _ in 0..600 {
        payload.extend_from_slice(b"The quick brown fox jumps over the lazy dog. 1234567890! ");
    }

    // 1. Huff0 4X parallel
    let mut comp_4x = vec![0u8; huf0_compress_bound(payload.len())];
    let comp_4x_len = huf0_compress4x(&payload, &mut comp_4x).expect("huf0 4x compress failed");
    assert!(comp_4x_len > 0);
    assert!(comp_4x_len < payload.len());

    let mut decomp_4x = vec![0u8; payload.len()];
    let decomp_4x_len = huf0_decompress4x(&comp_4x[..comp_4x_len], &mut decomp_4x)
        .expect("huf0 4x decompress failed");
    assert_eq!(decomp_4x_len, payload.len());
    assert_eq!(decomp_4x, payload);

    // 2. Huff0 1X single stream
    let mut comp_1x = vec![0u8; huf0_compress_bound(payload.len())];
    let comp_1x_len = huf0_compress1x(&payload, &mut comp_1x).expect("huf0 1x compress failed");
    assert!(comp_1x_len > 0);
    assert!(comp_1x_len < payload.len());

    let mut decomp_1x = vec![0u8; payload.len()];
    let decomp_1x_len = huf0_decompress1x(&comp_1x[..comp_1x_len], &mut decomp_1x)
        .expect("huf0 1x decompress failed");
    assert_eq!(decomp_1x_len, payload.len());
    assert_eq!(decomp_1x, payload);
}



#[test]
fn test_zstd_stream_reader_writer_with_dict() {
    let dict_bytes = b"Common structured dictionary header for stream tests. 1234567890.".to_vec();
    let dict = ZstdDictionary::from_bytes("stream_dict", dict_bytes, 3)
        .expect("create dictionary");

    let payload = b"Structured payload stream using pre-digested dictionary for streaming IO.";
    let mut comp_out = Vec::new();

    {
        let mut writer = ZstdStreamWriter::with_dict(&mut comp_out, &dict)
            .expect("create writer with dict");
        writer.write_all(payload).expect("write failed");
        writer.finish().expect("finish failed");
    }

    assert!(!comp_out.is_empty());

    let mut reader = ZstdStreamReader::with_dict(Cursor::new(&comp_out), &dict)
        .expect("create reader with dict");
    let mut decomp = Vec::new();
    reader.read_to_end(&mut decomp).expect("read failed");
    assert_eq!(decomp, payload);
}

#[test]
fn test_zstd_corrupt_data() {
    let corrupt = [0x28, 0xb5, 0x2f, 0xfd, 0x00, 0x00, 0xff, 0xff];
    let mut out = [0u8; 128];
    let res = zstd_decompress(&corrupt, &mut out);
    assert!(res.is_err());
}

#[test]
fn test_zstd_stream_pipe_roundtrip() {
    let payload = vec![0xABu8; 1024 * 1024 * 5]; // 5MB payload (spans across 4MB pipe boundary)
    let mut reader = Cursor::new(&payload);
    let mut compressed_out = Vec::new();

    let config = ZstdConfig {
        level: 3,
        ..Default::default()
    };

    let (read_bytes, written_bytes) = zstd_compress_stream_pipe(&mut reader, &mut compressed_out, &config, None)
        .expect("compress pipe failed");
    assert_eq!(read_bytes, payload.len() as u64);
    assert!(written_bytes > 0);
    assert_eq!(written_bytes, compressed_out.len() as u64);

    let mut comp_reader = Cursor::new(&compressed_out);
    let mut decompressed_out = Vec::new();

    let (comp_read, decomp_written) = zstd_decompress_stream_pipe(&mut comp_reader, &mut decompressed_out, None)
        .expect("decompress pipe failed");
    assert_eq!(comp_read, compressed_out.len() as u64);
    assert_eq!(decomp_written, payload.len() as u64);
    assert_eq!(decompressed_out, payload);
}
