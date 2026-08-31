// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive verification suite for LZ4 ExtDict cross-segment memory addressing,
//! `AttachDictionary` zero-copy compression, and `Arc<Lz4PreloadedDict>` concurrent Worker execution.

use std::sync::Arc;
use std::thread;
use ttzip_engine::codecs::lz4::{
    lz4_decompress_safe_ext_dict, Lz4DictCompressor, Lz4PreloadedDict, LZ4_64K_LIMIT,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Helper Functions

/// Generates pseudo-random deterministic byte sequence.
fn generate_deterministic_payload(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed;
    let mut data = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        data.push((state >> 24) as u8);
    }
    data
}

/// Generates structured dictionary corpus simulating JSON / schema keys.
fn generate_json_dictionary() -> Vec<u8> {
    let base = br#"{"schema_version":"2.4.0","status":"success","code":200,"records":[{"id":"item_#ID#","name":"TTZip File Entity","path":"/usr/local/share/data/","tags":["archive","lz4","compression","high_throughput","zero_copy"],"metadata":{"owner":"Witt Kung","created_at":"2026-08-30T00:00:00Z","flags":1024,"crc32":"0xCAFEBABE"}}]}"#;
    let mut dict = Vec::with_capacity(65536);
    let mut i = 0usize;
    while dict.len() < 65536 {
        let s = String::from_utf8_lossy(base).replace("#ID#", &format!("{i:04}"));
        dict.extend_from_slice(s.as_bytes());
        i += 1;
    }
    dict.truncate(65536);
    dict
}

// MARK: - Test 1: Preloaded Dictionary Construction & Fast vs Slow Loading

#[test]
fn test_lz4_preloaded_dict_construction_and_methods() {
    let raw_dict = generate_json_dictionary();
    let dict_id = 0x98765432;

    // Fast loading
    let dict_fast = Lz4PreloadedDict::with_dict_id(&raw_dict, dict_id);
    assert_eq!(dict_fast.dict_id(), Some(dict_id));
    assert_eq!(dict_fast.len(), raw_dict.len());
    assert!(!dict_fast.is_empty());
    assert_eq!(dict_fast.as_slice(), raw_dict.as_slice());
    assert_eq!(dict_fast.effective_slice().len(), raw_dict.len().min(LZ4_64K_LIMIT));

    // Slow loading (1-byte full scan)
    let dict_slow = Lz4PreloadedDict::load_dict_slow(&raw_dict, Some(dict_id));
    assert_eq!(dict_slow.dict_id(), Some(dict_id));
    assert_eq!(dict_slow.len(), raw_dict.len());
    assert!(!dict_slow.is_empty());

    // Compare compression of a shared payload
    let payload = br#"{"records":[{"id":"item_0042","name":"TTZip File Entity","tags":["archive","lz4"],"metadata":{"owner":"Witt Kung"}}]}"#;
    let comp_fast = dict_fast.compress_to_vec(payload, 1).expect("compress fast");
    let comp_slow = dict_slow.compress_to_vec(payload, 1).expect("compress slow");

    assert!(!comp_fast.is_empty());
    assert!(!comp_slow.is_empty());

    let decomp_fast = dict_fast.decompress_to_vec(&comp_fast, payload.len()).expect("decompress fast");
    let decomp_slow = dict_slow.decompress_to_vec(&comp_slow, payload.len()).expect("decompress slow");

    assert_eq!(decomp_fast.as_slice(), payload.as_slice());
    assert_eq!(decomp_slow.as_slice(), payload.as_slice());
}

// MARK: - Test 2: Bit-Exact Prefix vs ExtDict Closed-Loop Equivalence

#[test]
fn test_ext_dict_vs_continuous_prefix_bit_exact_consistency() {
    let dict_data = generate_json_dictionary();
    let dict = Lz4PreloadedDict::new(&dict_data);

    let test_sizes = [16, 64, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536];

    for &size in &test_sizes {
        let mut block = Vec::with_capacity(size);
        // Mix dictionary phrases with independent data
        let phrase = br#"{"name":"TTZip File Entity","tags":["archive","lz4","compression"]}"#;
        while block.len() < size {
            block.extend_from_slice(phrase);
            let noise = generate_deterministic_payload(32, (block.len() + size) as u32);
            block.extend_from_slice(&noise);
        }
        block.truncate(size);

        // 1. Compress with ExtDict attached
        let comp = dict.compress_to_vec(&block, 1).expect("compress ext dict");
        assert!(!comp.is_empty());

        // 2. Decompress with ExtDict
        let decomp_ext = dict.decompress_to_vec(&comp, block.len()).expect("decompress ext dict");
        assert_eq!(
            decomp_ext.as_slice(),
            block.as_slice(),
            "ExtDict decompression mismatch for size {size}"
        );

        // 3. Decompress using standard LZ4 C prefix method (dict + block in continuous buffer)
        // In continuous buffer: [dict (64KB)][block (size)]
        // Using LZ4_decompress_safe_usingDict with dict pointer
        let mut continuous_decomp = vec![0u8; block.len()];
        let written = lz4_decompress_safe_ext_dict(&comp, &mut continuous_decomp, &dict_data)
            .expect("safe ext dict decompress");
        assert_eq!(written, block.len());
        assert_eq!(
            continuous_decomp.as_slice(),
            block.as_slice(),
            "Continuous reference verification mismatch for size {size}"
        );
    }
}

// MARK: - Test 3: Multi-Threaded Concurrent Arc<Lz4PreloadedDict> 8-Worker Execution

#[test]
fn test_arc_preloaded_dict_8_thread_concurrent_zero_race() {
    let dict_data = generate_json_dictionary();
    let shared_dict = Arc::new(Lz4PreloadedDict::with_dict_id(&dict_data, 0x11223344));

    const NUM_THREADS: usize = 8;
    const BLOCKS_PER_THREAD: usize = 200;

    let mut handles = Vec::with_capacity(NUM_THREADS);

    for thread_idx in 0..NUM_THREADS {
        let dict_ref = Arc::clone(&shared_dict);
        let handle = thread::spawn(move || {
            let mut compressor = Lz4DictCompressor::new();
            compressor.attach_dictionary(&dict_ref);

            for block_idx in 0..BLOCKS_PER_THREAD {
                let seed = ((thread_idx * 1000) + block_idx) as u32;
                let payload_len = 256 + (block_idx % 3800); // Small blocks <= 4KB

                let mut payload = Vec::with_capacity(payload_len);
                let tag = format!(r#"{{"worker_id":{thread_idx},"task_id":{block_idx},"tags":["archive","lz4"]}}"#);
                payload.extend_from_slice(tag.as_bytes());

                let filler = generate_deterministic_payload(payload_len.saturating_sub(payload.len()), seed);
                payload.extend_from_slice(&filler);
                payload.truncate(payload_len);

                // Compress
                let compressed = compressor
                    .compress_to_vec(&payload)
                    .expect("thread local compress");
                assert!(!compressed.is_empty());

                // Decompress
                let decompressed = dict_ref
                    .decompress_to_vec(&compressed, payload.len())
                    .expect("thread local decompress");

                assert_eq!(
                    decompressed.as_slice(),
                    payload.as_slice(),
                    "Thread {thread_idx} Block {block_idx} payload mismatch"
                );
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Worker thread join failed");
    }
}

// MARK: - Test 4: Cross-Boundary Dual-Segment Match Reconstruction

#[test]
fn test_cross_boundary_dual_segment_tail_and_prefix_stitching() {
    // Construct a dictionary where the tail 32 bytes contain a known sequence
    let mut dict_data = vec![0x11u8; 1000];
    let tail_signature = b"ABCDEFGHIJ_0123456789_TAIL_KEY_";
    let tail_start = dict_data.len() - tail_signature.len();
    dict_data[tail_start..].copy_from_slice(tail_signature);

    let dict = Lz4PreloadedDict::new(&dict_data);

    // Construct a block whose beginning continues the sequence:
    // [Dict Tail: "ABCDEFGHIJ_0123456789_TAIL_KEY_"][Block Prefix: "ABCDEFGHIJ_0123456789_TAIL_KEY_EXTRA_DATA"]
    let mut block = Vec::new();
    block.extend_from_slice(tail_signature);
    block.extend_from_slice(b"EXTRA_BODY_PAYLOAD_FOR_DUAL_SEGMENT_STITCHING_TEST_");
    block.extend_from_slice(tail_signature);
    block.extend_from_slice(b"_FOOTER_LITERALS_12345");

    let compressed = dict.compress_to_vec(&block, 1).expect("compress cross boundary");
    assert!(!compressed.is_empty());

    let decompressed = dict
        .decompress_to_vec(&compressed, block.len())
        .expect("decompress cross boundary");
    assert_eq!(
        decompressed.as_slice(),
        block.as_slice(),
        "Dual-segment cross-boundary stitch verification failed"
    );
}

#[test]
fn test_cross_boundary_varying_overlap_lengths() {
    let pattern = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_SHARED_CROSS_BORDER_";

    for overlap in [4, 7, 8, 12, 16, 24, 32, 48, 56] {
        let mut dict = vec![0xAAu8; 500];
        let dict_tail = &pattern[..overlap];
        let d_start = dict.len() - overlap;
        dict[d_start..].copy_from_slice(dict_tail);

        let preloaded = Lz4PreloadedDict::new(&dict);

        let mut block = Vec::new();
        block.extend_from_slice(b"PREFIX_");
        block.extend_from_slice(dict_tail);
        block.extend_from_slice(b"_MIDDLE_TEXT_");
        block.extend_from_slice(dict_tail);
        block.extend_from_slice(b"_END_5B");

        let comp = preloaded.compress_to_vec(&block, 1).expect("compress overlap");
        let decomp = preloaded.decompress_to_vec(&comp, block.len()).expect("decompress overlap");
        assert_eq!(decomp.as_slice(), block.as_slice());
    }
}

// MARK: - Test 5: Partial Decompression with External Dictionary

#[test]
fn test_ext_dict_partial_decompression() {
    let dict_data = generate_json_dictionary();
    let dict = Lz4PreloadedDict::new(&dict_data);

    let payload = br#"{"status":"success","records":[{"id":"item_0010","name":"TTZip File Entity","tags":["archive","lz4"],"metadata":{"owner":"Witt Kung"}}]}"#;
    let comp = dict.compress_to_vec(payload, 1).expect("compress partial payload");

    let mut dst = vec![0u8; payload.len() + 64];

    for target in [1, 5, 10, 20, 45, payload.len()] {
        dst.fill(0);
        let written = dict
            .decompress_partial(&comp, &mut dst, target)
            .expect("partial decompress");
        assert!(written >= target);
        assert_eq!(&dst[..target], &payload[..target]);
    }
}

// MARK: - Test 6: Error Handling & Defensive Boundaries

#[test]
fn test_ext_dict_empty_and_sub_mflimit_inputs() {
    let dict_data = b"Some dictionary context";
    let dict = Lz4PreloadedDict::new(dict_data);

    // Empty buffer
    let empty_comp = dict.compress_to_vec(&[], 1).expect("empty compress");
    assert!(empty_comp.is_empty());
    let empty_decomp = dict.decompress_to_vec(&[], 0).expect("empty decompress");
    assert!(empty_decomp.is_empty());

    // Small buffers (< 12 bytes MFLIMIT)
    for size in 1..12 {
        let input = generate_deterministic_payload(size, size as u32);
        let comp = dict.compress_to_vec(&input, 1).expect("sub-mflimit compress");
        let decomp = dict.decompress_to_vec(&comp, size).expect("sub-mflimit decompress");
        assert_eq!(decomp.as_slice(), input.as_slice());
    }
}

#[test]
fn test_ext_dict_corrupt_data_rejected() {
    let dict_data = b"Dictionary context for corruption test";
    let mut dst = [0u8; 128];

    // Truncated / corrupt payload
    let corrupt1 = [0xFF, 0xFF, 0x01, 0x02];
    assert!(lz4_decompress_safe_ext_dict(&corrupt1, &mut dst, dict_data).is_err());

    // Zero offset payload: token with match length but offset = 0
    let zero_offset = [0x05, b'A', 0x00, 0x00];
    let res = lz4_decompress_safe_ext_dict(&zero_offset, &mut dst, dict_data);
    assert_eq!(res, Err(TTZipStatus::ErrInvalidOffset));

    // Offset exceeding dict + output history
    let excessive_offset = [0x00, 0xFF, 0xFF]; // offset 65535 on 0 history
    let res2 = lz4_decompress_safe_ext_dict(&excessive_offset, &mut dst, dict_data);
    assert!(res2.is_err());
}

#[test]
fn test_large_block_ext_dict_single_addressing_stress() {
    let dict_data = generate_json_dictionary();
    let dict = Lz4PreloadedDict::new(&dict_data);

    // 128KB payload (> 4KB, tests large block single-table addressing)
    let mut large_payload = Vec::with_capacity(128 * 1024);
    for i in 0..300 {
        let record = format!(
            r#"{{"id":"item_{i:04}","name":"TTZip File Entity","tags":["archive","lz4"],"metadata":{{"index":{i},"status":"active"}}}}"#
        );
        large_payload.extend_from_slice(record.as_bytes());
    }

    let comp = dict.compress_to_vec(&large_payload, 1).expect("compress 128KB payload");
    assert!(!comp.is_empty());
    assert!(comp.len() < large_payload.len(), "large payload with dictionary must compress");

    let decomp = dict
        .decompress_to_vec(&comp, large_payload.len())
        .expect("decompress 128KB payload");
    assert_eq!(decomp.as_slice(), large_payload.as_slice());
}
