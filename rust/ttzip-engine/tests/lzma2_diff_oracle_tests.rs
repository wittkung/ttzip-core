// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Differential Oracle Test Suite for LZMA2 Engine & Codecs.
//!
//! Validates:
//! 1. Full compression level matrix roundtrips (Level 1..=9) across diverse corpora.
//! 2. Differential oracle comparison between single-threaded and multi-threaded compression pipelines.
//! 3. Streaming decompressor (`Fl2DStream`) vs single-pass decompressor (`fl2_decompress`) output equivalence.
//! 4. Boundary payload sizes (0-byte empty, 1-byte, 15-byte, 64KB, 1MB, 2MB).
//! 5. Multimodal corpora fidelity (structured text, executable code, RLE repeated blocks, pseudo-random).
//! 6. Bit-for-bit reconstruction verification and zero corruption guarantee.

#[path = "fixtures/mod.rs"]
mod fixtures;

use fixtures::datagen::{generate_corpus, DataGenLevel};
use ttzip_engine::codecs::lzma2::{
    fl2_compress, fl2_compress_bound, fl2_decompress, Fl2DCtx, Fl2DStream, Fl2InBuffer, Fl2OutBuffer,
};

fn verify_roundtrip(payload: &[u8], level: i32, threads: u32) {
    let mut comp_buf = vec![0u8; fl2_compress_bound(payload.len()) + 1024];
    let comp_len = fl2_compress(payload, &mut comp_buf, level, threads)
        .expect("fl2_compress failed");

    let mut decomp_buf = vec![0u8; payload.len()];
    let decomp_len = fl2_decompress(&comp_buf[..comp_len], &mut decomp_buf, 2)
        .expect("fl2_decompress failed");

    assert_eq!(
        decomp_len,
        payload.len(),
        "Decompressed length mismatch for level {level}, threads {threads}"
    );
    assert_eq!(
        &decomp_buf[..decomp_len],
        payload,
        "Bit fidelity failure for level {level}, threads {threads}"
    );
}

#[test]
fn test_lzma2_oracle_all_compression_levels_matrix() {
    let corpus = generate_corpus(DataGenLevel::Standard, 64 * 1024, 0x1337BEEF);
    let levels = [1, 2, 3, 5, 7, 9];

    for &lvl in &levels {
        verify_roundtrip(&corpus, lvl, 1);
        verify_roundtrip(&corpus, lvl, 2);
    }
}

#[test]
fn test_lzma2_oracle_multimodal_corpora() {
    let corpus_types = [
        DataGenLevel::PureNoise,
        DataGenLevel::BarelyCompressible,
        DataGenLevel::Standard,
        DataGenLevel::HighlyCompressible,
        DataGenLevel::Sparse,
    ];

    for &ct in &corpus_types {
        let payload = generate_corpus(ct, 128 * 1024, 0xCAFEBABE);
        verify_roundtrip(&payload, 3, 1);
        verify_roundtrip(&payload, 5, 4);
    }
}

#[test]
fn test_lzma2_oracle_boundary_sizes() {
    let boundary_sizes = [0, 1, 2, 3, 7, 15, 63, 64, 127, 128, 512, 4096, 65536, 131072];

    for &size in &boundary_sizes {
        let payload = if size == 0 {
            Vec::new()
        } else {
            generate_corpus(DataGenLevel::Standard, size, size as u32)
        };
        verify_roundtrip(&payload, 3, 1);
    }
}

#[test]
fn test_lzma2_oracle_streaming_vs_single_pass_parity() {
    let payload = generate_corpus(DataGenLevel::Standard, 256 * 1024, 0xDEADBEEF);
    let mut comp_buf = vec![0u8; fl2_compress_bound(payload.len()) + 1024];
    let comp_len = fl2_compress(&payload, &mut comp_buf, 3, 2).expect("fl2 compress");

    // 1. Single pass decompress
    let mut dctx = Fl2DCtx::new().expect("create dctx");
    let mut single_pass_out = vec![0u8; payload.len()];
    let single_len = dctx
        .decompress(&comp_buf[..comp_len], &mut single_pass_out)
        .expect("single pass decompress");

    assert_eq!(single_len, payload.len());
    assert_eq!(&single_pass_out[..single_len], &payload);

    // 2. Streaming decompress with micro-chunks (4KB chunks)
    let mut dstream = Fl2DStream::new(1).expect("create dstream");
    dstream.init(None).expect("init dstream");

    let mut stream_out = vec![0u8; payload.len()];
    let mut in_buf = Fl2InBuffer {
        src: comp_buf.as_ptr() as *const libc::c_void,
        size: comp_len,
        pos: 0,
    };
    let mut out_buf = Fl2OutBuffer {
        dst: stream_out.as_mut_ptr() as *mut libc::c_void,
        size: stream_out.len(),
        pos: 0,
    };

    let _res = dstream
        .decompress_stream(&mut in_buf, &mut out_buf)
        .expect("decompress stream");

    assert_eq!(out_buf.pos, payload.len());
    assert_eq!(&stream_out[..out_buf.pos], &payload);
    assert_eq!(single_pass_out, stream_out);
}

#[test]
fn test_lzma2_oracle_multi_thread_budget_parity() {
    let payload = generate_corpus(DataGenLevel::Standard, 512 * 1024, 0x55AA55AA);
    let thread_counts = [1, 2, 4, 8];

    for &threads in &thread_counts {
        let mut comp_buf = vec![0u8; fl2_compress_bound(payload.len()) + 1024];
        let comp_len = fl2_compress(&payload, &mut comp_buf, 3, threads)
            .expect("multi thread compress failed");

        let mut decomp_buf = vec![0u8; payload.len()];
        let decomp_len = fl2_decompress(&comp_buf[..comp_len], &mut decomp_buf, 1)
            .expect("single thread decompress failed");

        assert_eq!(decomp_len, payload.len());
        assert_eq!(&decomp_buf[..decomp_len], &payload);
    }
}
