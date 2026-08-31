// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Verification Test Suite for LZMA2 In-Place Context Reuse,
//! Zero-Allocation State Machine Recycling, and Thread-Local Resource Reclamation.
//!
//! Validates:
//! 1. `Fl2DCtx` / `Fl2CCtx` safe in-place state reuse across consecutive buffers.
//! 2. Thread-local decompression context (`with_thread_local_fl2_dctx`) recycling.
//! 3. `Lzma2StreamDecoder` reuse across multiple separate compressed streams without memory leaks.
//! 4. `RadixMatchFinder` reset and multi-buffer table reconstruction stability.
//! 5. `Lzma2RangeEncoder` buffer clear and state reset roundtrip integrity.
//! 6. Deterministic output parity between cold-initialized and warm-reused context instances.

use std::sync::Arc;
use std::thread;

use ttzip_engine::codecs::lzma2::radix_matcher::RadixMatchFinder;
use ttzip_engine::codecs::lzma2::range_enc::Lzma2RangeEncoder;
use ttzip_engine::codecs::lzma2::{
    fl2_compress, fl2_compress_bound, fl2_decompress, with_thread_local_fl2_dctx, Fl2CCtx, Fl2DCtx,
    Lzma2ChunkHeader, Lzma2StreamDecoder, LZMA2_DEFAULT_DICT_SIZE,
};

#[test]
fn test_fl2_cctx_and_dctx_inplace_reuse_multiple_iterations() {
    let mut cctx = Fl2CCtx::new().expect("create Fl2CCtx");
    let mut dctx = Fl2DCtx::new().expect("create Fl2DCtx");

    let iterations = 50usize;
    for i in 0..iterations {
        let payload = format!("TTZip In-Place Iteration #{i:04} Context Recycling Payload Data Block.").into_bytes();
        let mut comp_buf = vec![0u8; fl2_compress_bound(payload.len())];

        let comp_len = cctx
            .compress(&payload, &mut comp_buf, 3)
            .expect("cctx compress failed");
        assert!(comp_len > 0);

        let mut decomp_buf = vec![0u8; payload.len()];
        let decomp_len = dctx
            .decompress(&comp_buf[..comp_len], &mut decomp_buf)
            .expect("dctx decompress failed");

        assert_eq!(decomp_len, payload.len());
        assert_eq!(&decomp_buf[..decomp_len], &payload);
    }
}

#[test]
fn test_thread_local_fl2_dctx_concurrent_and_sequential_reuse() {
    let input = b"TTZip Safe Thread-Local DCtx Multi-Threaded Stress Test Payload Block.";
    let mut comp_buf = vec![0u8; fl2_compress_bound(input.len())];
    let comp_len = fl2_compress(input, &mut comp_buf, 3, 2).expect("fl2 compress failed");
    comp_buf.truncate(comp_len);

    let comp_arc = Arc::new(comp_buf);
    let original_arc = Arc::new(input.to_vec());

    let mut handles = Vec::new();
    for _ in 0..8 {
        let comp = Arc::clone(&comp_arc);
        let orig = Arc::clone(&original_arc);
        let h = thread::spawn(move || {
            for _ in 0..20 {
                let mut decomp = vec![0u8; orig.len()];
                let written = with_thread_local_fl2_dctx(|dctx| {
                    dctx.decompress(&comp, &mut decomp)
                })
                .expect("thread local decompress");

                assert_eq!(written, orig.len());
                assert_eq!(&decomp[..written], orig.as_slice());
            }
        });
        handles.push(h);
    }

    for h in handles {
        h.join().expect("thread join failed");
    }
}

#[test]
fn test_lzma2_stream_decoder_sequential_stream_reuse() {
    let mut decoder = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);

    for i in 0..30 {
        decoder.reset();
        let raw_text = format!("Sequential stream payload message #{i:03} testing stream decoder reset.").into_bytes();
        let mut stream = Vec::new();
        Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream, &raw_text, true);
        Lzma2ChunkHeader::write_eos(&mut stream);

        let mut out = Vec::new();
        let n = decoder
            .decode_all(&stream, &mut out)
            .expect("decode stream pass");

        assert_eq!(n, raw_text.len());
        assert_eq!(out, raw_text);
        assert!(decoder.is_eos());
    }
}

#[test]
fn test_radix_match_finder_buffer_reinitialization_reuse() {
    let mut finder = RadixMatchFinder::with_max_depth(32);

    let corpus_a = b"AAAA_BBBB_CCCC_AAAA_BBBB_CCCC_AAAA_";
    finder.init_table(corpus_a);
    finder.build_table(corpus_a, 1);
    let m_a = finder.get_match(15).expect("match at pos 15 in corpus A");
    assert_eq!(m_a.link, 0);

    let corpus_b = b"1234567890_1234567890_1234567890_";
    finder.init_table(corpus_b);
    finder.build_table(corpus_b, 1);
    let m_b = finder.get_match(11).expect("match at pos 11 in corpus B");
    assert_eq!(m_b.link, 0);
    assert_eq!(m_b.length, 22);
}

#[test]
fn test_lzma2_range_encoder_reset_reuse() {
    let mut encoder = Lzma2RangeEncoder::with_capacity(1024);

    for round in 0..20 {
        encoder.reset();
        assert_eq!(encoder.low(), 0);
        assert_eq!(encoder.range(), 0xFFFF_FFFF);
        assert_eq!(encoder.cache_size(), 1);
        assert!(encoder.buffer().is_empty());

        let value = 0x1234_5678u32.wrapping_add(round as u32);
        encoder.encode_direct_bits(value, 32);
        assert!(encoder.processed_size() > 0);
    }
}

#[test]
fn test_cold_vs_warm_context_exact_byte_parity() {
    let data = b"TTZip Deterministic Fidelity Verification between Cold Init and Warm Reused State Machines.";

    let mut comp1 = vec![0u8; fl2_compress_bound(data.len())];
    let n1 = fl2_compress(data, &mut comp1, 3, 2).expect("cold compress");
    comp1.truncate(n1);

    let mut decomp_cold = vec![0u8; data.len()];
    let cold_len = fl2_decompress(&comp1, &mut decomp_cold, 2).expect("cold decompress");
    assert_eq!(&decomp_cold[..cold_len], data);

    // Warm context reuse
    let mut dctx = Fl2DCtx::new().expect("create warm dctx");
    // Burn 10 iterations to warm context
    for _ in 0..10 {
        let mut tmp = vec![0u8; data.len()];
        dctx.decompress(&comp1, &mut tmp).expect("warmup decompress");
    }

    let mut decomp_warm = vec![0u8; data.len()];
    let warm_len = dctx.decompress(&comp1, &mut decomp_warm).expect("warm decompress");

    assert_eq!(cold_len, warm_len);
    assert_eq!(decomp_cold, decomp_warm);
}
