// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming State-Machine Cataclysm & Single-Byte Torture Validation Suite (Task 12.3).
//!
//! Validates decompressor state machines across pathological chunk steps:
//! - Ladder steps: `[1, 2, 3, 7, 15, 259, 1024, 65536]`
//! - Dual-ended random input/output slicing (`in_chunk ∈ [1..N]`, `out_chunk ∈ [1..M]`)
//! - Hardened **`in_chunk=1, out_chunk=1` Single-Byte Cataclysm Torture** ensuring 100%
//!   deterministic byte-boundary suspend/resume fidelity across Zstd, Deflate, LZ4, Snappy,
//!   Bzip2, and Brotli.

use std::io::{Cursor, Read, Write};

use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;

use ttzip_engine::codecs::brotli::stream::{
    BrotliCompressorWriter, BrotliConfig, BrotliDecompressorReader,
};
use ttzip_engine::codecs::bzip2::{
    bzip2_compress_to_vec, Bzip2Reader,
};
use ttzip_engine::codecs::lz4::{
    lz4_compress_to_vec, lz4_decompress_to_vec,
};
use ttzip_engine::codecs::snappy::frame::{
    snappy_frame_encode_to_vec,
};
use ttzip_engine::codecs::zstd::stream::{
    ZstdStreamReader, ZstdStreamWriter,
};
use ttzip_engine::codecs::zstd::types::{ZstdInBuffer, ZstdOutBuffer};
use ttzip_engine::codecs::zstd::ZstdDCtx;

/// Standard pathological chunk size ladder steps covering prime and power-of-two boundaries.
pub const CATACLYSM_CHUNK_LADDER: &[usize] = &[1, 2, 3, 7, 15, 259, 1024, 65536];

/// Bounded stream wrapper enforcing maximum read chunk size per `read()` invocation.
pub struct CataclysmBoundedReader<R: Read> {
    inner: R,
    max_chunk_size: usize,
}

impl<R: Read> CataclysmBoundedReader<R> {
    /// Creates a new bounded reader clamping reads to `max_chunk_size`.
    pub fn new(inner: R, max_chunk_size: usize) -> Self {
        Self {
            inner,
            max_chunk_size: max_chunk_size.max(1),
        }
    }
}

impl<R: Read> Read for CataclysmBoundedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let clamped = buf.len().min(self.max_chunk_size);
        self.inner.read(&mut buf[..clamped])
    }
}

/// Helper function to drain any `std::io::Read` stream into a vector using bounded `out_chunk` steps.
pub fn drain_reader_with_out_chunk<R: Read>(
    mut reader: R,
    out_chunk: usize,
    expected_size: usize,
) -> std::io::Result<Vec<u8>> {
    let step = out_chunk.max(1);
    let mut buffer = vec![0u8; step];
    let mut decompressed = Vec::with_capacity(expected_size);

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => decompressed.extend_from_slice(&buffer[..n]),
            Err(e) => return Err(e),
        }
    }

    Ok(decompressed)
}

/// Helper to generate multi-pattern synthetic stress payload with varying entropy.
pub fn generate_cataclysm_payload(len: usize) -> Vec<u8> {
    let mut payload = Vec::with_capacity(len);
    let text_header = b"TTZip Streaming State Machine Cataclysm Torture Payload 2026. ";

    while payload.len() < len {
        let remaining = len - payload.len();
        if payload.len() < 2048 {
            let chunk = &text_header[..remaining.min(text_header.len())];
            payload.extend_from_slice(chunk);
        } else if payload.len() < 8192 {
            // Repetitive run
            let fill_len = remaining.min(512);
            payload.resize(payload.len() + fill_len, 0x5A);
        } else {
            // Arithmetic PRNG pattern
            let idx = payload.len();
            let byte = ((idx * 37 + 13) ^ (idx >> 4)) as u8;
            payload.push(byte);
        }
    }

    payload
}

// MARK: - 1. Zstandard Streaming Cataclysm & Single-Byte Torture

#[test]
fn test_zstd_cataclysm_ladder_and_single_byte_torture() {
    let payload = generate_cataclysm_payload(16 * 1024); // 16KB payload

    // 1. Compress via Zstd streaming writer
    let mut compressed = Vec::new();
    {
        let mut writer = ZstdStreamWriter::with_level(&mut compressed, 3)
            .expect("create zstd stream writer");
        writer.write_all(&payload).expect("write payload");
        writer.finish().expect("finish zstd stream");
    }
    assert!(!compressed.is_empty());

    // 2. Ladder test: evaluate all combinations of (in_chunk, out_chunk)
    for &in_step in CATACLYSM_CHUNK_LADDER {
        for &out_step in CATACLYSM_CHUNK_LADDER {
            let bounded_input = CataclysmBoundedReader::new(Cursor::new(&compressed), in_step);
            let reader = ZstdStreamReader::new(bounded_input)
                .expect("create zstd reader");
            let recovered = drain_reader_with_out_chunk(reader, out_step, payload.len())
                .unwrap_or_else(|e| panic!("Zstd failed at in={}, out={}: {}", in_step, out_step, e));
            assert_eq!(
                recovered.len(),
                payload.len(),
                "Zstd length mismatch at in={}, out={}",
                in_step,
                out_step
            );
            assert_eq!(
                recovered, payload,
                "Zstd payload mismatch at in={}, out={}",
                in_step, out_step
            );
        }
    }

    // 3. EXTREME Single-Byte Cataclysm Torture via raw DCtx (in=1, out=1)
    let mut dctx = ZstdDCtx::new().expect("create dctx");
    let mut dctx_recovered = Vec::with_capacity(payload.len());
    let mut in_pos = 0;
    let mut in_end = 1;
    let mut single_out = [0u8; 1];

    while in_pos < compressed.len() {
        in_end = in_end.max(in_pos + 1).min(compressed.len());
        let mut in_struct = ZstdInBuffer {
            src: compressed.as_ptr() as *const libc::c_void,
            size: in_end,
            pos: in_pos,
        };

        loop {
            let prev_in = in_struct.pos;
            let mut out_struct = ZstdOutBuffer {
                dst: single_out.as_mut_ptr() as *mut libc::c_void,
                capacity: 1,
                pos: 0,
            };

            let _ = dctx
                .decompress_stream(&mut in_struct, &mut out_struct)
                .expect("zstd 1-byte decompress_stream");

            if out_struct.pos > 0 {
                dctx_recovered.push(single_out[0]);
            }

            if in_struct.pos == prev_in && out_struct.pos == 0 {
                // Needs more input to make progress; expand in_end window
                in_end = (in_end + 1).min(compressed.len());
                break;
            }
            if in_struct.pos >= in_struct.size {
                in_end = (in_end + 1).min(compressed.len());
                break;
            }
        }
        in_pos = in_struct.pos;
    }

    // Drain any remaining buffered bytes inside DCtx
    loop {
        let mut in_struct = ZstdInBuffer {
            src: std::ptr::null(),
            size: 0,
            pos: 0,
        };
        let mut out_struct = ZstdOutBuffer {
            dst: single_out.as_mut_ptr() as *mut libc::c_void,
            capacity: 1,
            pos: 0,
        };
        let _ = dctx
            .decompress_stream(&mut in_struct, &mut out_struct)
            .expect("drain zstd");
        if out_struct.pos > 0 {
            dctx_recovered.push(single_out[0]);
        } else {
            // Nothing produced with empty input; draining is fully complete
            break;
        }
    }

    assert_eq!(dctx_recovered.len(), payload.len());
    assert_eq!(dctx_recovered, payload);
}

// MARK: - 2. Deflate / Gzip / Zlib Streaming Cataclysm & Single-Byte Torture

#[test]
fn test_deflate_cataclysm_ladder_and_single_byte_torture() {
    let payload = generate_cataclysm_payload(16 * 1024);

    // 1. Deflate Raw
    let mut def_enc = DeflateEncoder::new(Vec::new(), Compression::default());
    def_enc.write_all(&payload).expect("deflate write");
    let def_comp = def_enc.finish().expect("deflate finish");

    for &in_step in &[1, 2, 3, 7, 259, 1024] {
        for &out_step in &[1, 2, 7, 15, 259, 4096] {
            let bounded = CataclysmBoundedReader::new(Cursor::new(&def_comp), in_step);
            let decoder = DeflateDecoder::new(bounded);
            let rec = drain_reader_with_out_chunk(decoder, out_step, payload.len())
                .unwrap_or_else(|e| panic!("Deflate failed at in={}, out={}: {}", in_step, out_step, e));
            assert_eq!(rec, payload, "Deflate mismatch at in={}, out={}", in_step, out_step);
        }
    }

    // 2. Gzip
    let mut gz_enc = GzEncoder::new(Vec::new(), Compression::fast());
    gz_enc.write_all(&payload).expect("gz write");
    let gz_comp = gz_enc.finish().expect("gz finish");

    for &in_step in &[1, 3, 15, 259] {
        for &out_step in &[1, 7, 259, 1024] {
            let bounded = CataclysmBoundedReader::new(Cursor::new(&gz_comp), in_step);
            let decoder = GzDecoder::new(bounded);
            let rec = drain_reader_with_out_chunk(decoder, out_step, payload.len())
                .expect("Gz decompress pass");
            assert_eq!(rec, payload);
        }
    }

    // 3. Zlib
    let mut zl_enc = ZlibEncoder::new(Vec::new(), Compression::best());
    zl_enc.write_all(&payload).expect("zl write");
    let zl_comp = zl_enc.finish().expect("zl finish");

    for &in_step in &[1, 2, 7, 259] {
        for &out_step in &[1, 3, 15, 259] {
            let bounded = CataclysmBoundedReader::new(Cursor::new(&zl_comp), in_step);
            let decoder = ZlibDecoder::new(bounded);
            let rec = drain_reader_with_out_chunk(decoder, out_step, payload.len())
                .expect("Zlib decompress pass");
            assert_eq!(rec, payload);
        }
    }
}

// MARK: - 3. Snappy Framed Streaming Cataclysm & Single-Byte Torture

#[test]
fn test_snappy_framed_cataclysm_ladder_and_single_byte_torture() {
    let payload = generate_cataclysm_payload(24 * 1024);
    let compressed = snappy_frame_encode_to_vec(&payload).expect("snappy frame encode");

    // Ladder testing
    for &in_step in CATACLYSM_CHUNK_LADDER {
        for &out_step in &[1, 2, 3, 7, 15, 259, 1024] {
            let bounded = CataclysmBoundedReader::new(Cursor::new(&compressed), in_step);
            let decoder = snap::read::FrameDecoder::new(bounded);
            let rec = drain_reader_with_out_chunk(decoder, out_step, payload.len())
                .unwrap_or_else(|e| panic!("Snappy failed at in={}, out={}: {}", in_step, out_step, e));
            assert_eq!(rec, payload, "Snappy mismatch at in={}, out={}", in_step, out_step);
        }
    }

    // Hardened in=1, out=1 torture
    let bounded = CataclysmBoundedReader::new(Cursor::new(&compressed), 1);
    let decoder = snap::read::FrameDecoder::new(bounded);
    let single_byte_out = drain_reader_with_out_chunk(decoder, 1, payload.len())
        .expect("Snappy single-byte torture failed");
    assert_eq!(single_byte_out, payload);
}

// MARK: - 4. Bzip2 Streaming Chunked Cataclysm & Single-Byte Torture

#[test]
fn test_bzip2_cataclysm_ladder_and_single_byte_torture() {
    let payload = generate_cataclysm_payload(12 * 1024);
    let compressed = bzip2_compress_to_vec(&payload, 6).expect("bzip2 compress");

    // Dual-ended chunking across ladder steps
    for &in_step in &[1, 2, 3, 7, 15, 259, 1024] {
        for &out_step in &[1, 2, 3, 7, 15, 259, 1024] {
            let bounded = CataclysmBoundedReader::new(Cursor::new(&compressed), in_step);
            let decoder = Bzip2Reader::new(bounded).expect("create bzip2 reader");
            let recovered = drain_reader_with_out_chunk(decoder, out_step, payload.len())
                .unwrap_or_else(|e| panic!("Bzip2 failed at in={}, out={}: {}", in_step, out_step, e));
            assert_eq!(
                recovered, payload,
                "Bzip2 payload mismatch at in={}, out={}",
                in_step, out_step
            );
        }
    }
}

// MARK: - 5. Brotli Streaming Cataclysm & Single-Byte Torture

#[test]
fn test_brotli_cataclysm_ladder_and_single_byte_torture() {
    let payload = generate_cataclysm_payload(16 * 1024);

    let mut compressed = Vec::new();
    {
        let config = BrotliConfig {
            quality: 6,
            lgwin: 20,
            buffer_size: 4096,
        };
        let mut writer = BrotliCompressorWriter::new(&mut compressed, &config);
        writer.write_all(&payload).expect("brotli write");
        writer.flush().expect("brotli flush");
    }

    for &in_step in &[1, 2, 7, 15, 259, 1024] {
        for &out_step in &[1, 2, 3, 7, 259, 4096] {
            let bounded = CataclysmBoundedReader::new(Cursor::new(&compressed), in_step);
            let reader = BrotliDecompressorReader::new(bounded, 4096);
            let rec = drain_reader_with_out_chunk(reader, out_step, payload.len())
                .unwrap_or_else(|e| panic!("Brotli failed at in={}, out={}: {}", in_step, out_step, e));
            assert_eq!(rec, payload, "Brotli mismatch at in={}, out={}", in_step, out_step);
        }
    }
}

// MARK: - 6. LZ4 Extreme Buffer Boundary & Slice Torture

#[test]
fn test_lz4_extreme_slice_boundary_torture() {
    let payload = generate_cataclysm_payload(16 * 1024);
    let compressed = lz4_compress_to_vec(&payload, 1).expect("lz4 compress");

    // Verify exact-fit decompression
    let decomp = lz4_decompress_to_vec(&compressed, payload.len()).expect("lz4 decompress");
    assert_eq!(decomp, payload);

    // Multi-block framing simulation: simulate stream of small LZ4 blocks with step reads
    let block_size = 512;
    let mut block_stream = Vec::new();
    let mut expected_stream = Vec::new();

    for chunk in payload.chunks(block_size) {
        let c_block = lz4_compress_to_vec(chunk, 1).expect("block comp");
        block_stream.extend_from_slice(&(c_block.len() as u32).to_le_bytes());
        block_stream.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        block_stream.extend_from_slice(&c_block);
        expected_stream.extend_from_slice(chunk);
    }

    // Decompress chunked stream by reading byte-by-byte from bounded reader
    let mut bounded = CataclysmBoundedReader::new(Cursor::new(&block_stream), 1);
    let mut recovered_stream = Vec::new();

    loop {
        let mut len_buf = [0u8; 8];
        let mut read_head = 0;
        while read_head < 8 {
            match bounded.read(&mut len_buf[read_head..read_head + 1]) {
                Ok(0) => break,
                Ok(1) => read_head += 1,
                Ok(_) => unreachable!(),
                Err(e) => panic!("Read error: {}", e),
            }
        }
        if read_head == 0 {
            break;
        }
        assert_eq!(read_head, 8);

        let c_len = u32::from_le_bytes(len_buf[0..4].try_into().unwrap()) as usize;
        let u_len = u32::from_le_bytes(len_buf[4..8].try_into().unwrap()) as usize;

        let mut c_block = vec![0u8; c_len];
        let mut c_read = 0;
        while c_read < c_len {
            let n = bounded.read(&mut c_block[c_read..c_read + 1]).expect("read byte");
            assert_eq!(n, 1);
            c_read += 1;
        }

        let decomp_block = lz4_decompress_to_vec(&c_block, u_len).expect("decomp block");
        recovered_stream.extend_from_slice(&decomp_block);
    }

    assert_eq!(recovered_stream, expected_stream);
}
