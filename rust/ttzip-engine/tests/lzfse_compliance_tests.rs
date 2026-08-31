// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple LZFSE / LZVN Official Specification Compliance & Industrial Corpora Test Suite.
//!
//! Evaluates:
//! 1. Official corpora matrix: Calgary prose, Canterbury structs, Silesia binary,
//!    APFS directory metadata, high-redundancy structured logs, and incompressible white noise.
//! 2. 100% Bit-Exact Roundtrip fidelity across all LZFSE block variants (Raw `bvx-`,
//!    LZVN `bvxn`, V1 `bvx1`, V2 `bvx2`, Stream `bvx$`).
//! 3. Bidirectional Oracle Differential Testing: Pure Safe Rust output is 100% decodable by
//!    Apple reference C (`lzfse_decode_buffer`), and Apple reference C output is 100%
//!    decodable by Pure Safe Rust (`lzfse_decompress_stream` / `LzfseReader`).
//! 4. 12-Tier Boundary size matrix: 0B, 1B, 2B, 15B, 16B, 256B, 4095B, 4096B, 65535B,
//!    65536B, 262144B (256KB block threshold), and 1MB (multi-block spanning).
//! 5. Defensive 0-Panic Invariants against truncated, malformed, and out-of-bounds streams.

use std::io::{Cursor, Read, Write};
use ttzip_engine::codecs::lzfse::block::{parse_block_header, BvxMagic, LzfseFreqTables};
use ttzip_engine::codecs::lzfse::encoder::{lzfse_encode_block, LzfseMatchTable};
use ttzip_engine::codecs::lzfse::lzvn_decoder::{
    lzvn_decompress, lzvn_decompress_pure_rust, lzvn_validate,
};
use ttzip_engine::codecs::lzfse::lzvn_encoder::{lzvn_compress, lzvn_compress_bound};
use ttzip_engine::codecs::lzfse::reader::{
    lzfse_decompress_stream, lzfse_validate, LzfseReader,
};
use ttzip_engine::codecs::lzfse::writer::{lzfse_compress_stream, LzfseWriter};


// MARK: - Apple Reference C External Bindings

extern "C" {
    fn lzfse_encode_scratch_size() -> libc::size_t;
    fn lzfse_encode_buffer(
        dst_buffer: *mut u8,
        dst_size: libc::size_t,
        src_buffer: *const u8,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;

    fn lzfse_decode_scratch_size() -> libc::size_t;
    fn lzfse_decode_buffer(
        dst_buffer: *mut u8,
        dst_size: libc::size_t,
        src_buffer: *const u8,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;
}

fn apple_c_lzfse_encode(src: &[u8]) -> Result<Vec<u8>, String> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = src.len().saturating_add(4096).max(256);
    let mut dst = vec![0u8; bound];
    let scratch_size = unsafe { lzfse_encode_scratch_size() };
    let mut scratch = vec![0u8; scratch_size.max(2 * 1024 * 1024)];
    let written = unsafe {
        lzfse_encode_buffer(
            dst.as_mut_ptr(),
            dst.len(),
            src.as_ptr(),
            src.len(),
            scratch.as_mut_ptr() as *mut libc::c_void,
        )
    };
    if written == 0 {
        Err("Apple C lzfse_encode_buffer returned 0".to_string())
    } else {
        dst.truncate(written);
        Ok(dst)
    }
}

fn apple_c_lzfse_decode(src: &[u8], expected_len: usize) -> Result<Vec<u8>, String> {
    if src.is_empty() || expected_len == 0 {
        return Ok(Vec::new());
    }
    let mut dst = vec![0u8; expected_len.max(1)];
    let scratch_size = unsafe { lzfse_decode_scratch_size() };
    let mut scratch = vec![0u8; scratch_size.max(2 * 1024 * 1024)];
    let written = unsafe {
        lzfse_decode_buffer(
            dst.as_mut_ptr(),
            dst.len(),
            src.as_ptr(),
            src.len(),
            scratch.as_mut_ptr() as *mut libc::c_void,
        )
    };
    if written != expected_len {
        Err(format!(
            "Apple C decode returned {written} bytes, expected {expected_len}"
        ))
    } else {
        Ok(dst)
    }
}

// MARK: - Standard Corpora Synthesizers

fn generate_calgary_prose(target_size: usize) -> Vec<u8> {
    let passage = b"The Calgary Corpus is a collection of text and binary data files \
suitable for comparing data compression programs. Natural language text follows Zipf's law \
with power-law frequency distributions for vocabulary, digrams, and whitespace patterns. \
Philosophical treatises and historical records exhibit distinct entropy profiles.\n";
    let mut out = Vec::with_capacity(target_size + passage.len());
    while out.len() < target_size {
        out.extend_from_slice(passage);
    }
    out.truncate(target_size);
    out
}

fn generate_canterbury_struct(target_size: usize) -> Vec<u8> {
    let code = b"struct CanterburyNode {\n    uint32_t record_id;\n    char tag[16];\n    \
double metric_a, metric_b;\n    struct CanterburyNode *next, *prev;\n};\n\
int process_stream(const struct CanterburyNode *node) {\n    if (!node) return -1;\n    \
return (int)(node->record_id ^ (uint32_t)node->metric_a);\n}\n";
    let mut out = Vec::with_capacity(target_size + code.len());
    while out.len() < target_size {
        out.extend_from_slice(code);
    }
    out.truncate(target_size);
    out
}

fn generate_silesia_binary(target_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_size);
    let mut state: u32 = 0x1F2E3D4C;
    while out.len() < target_size {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let op = if (state & 7) == 0 {
            0xD503201F // ARM64 NOP
        } else if (state & 7) == 1 {
            0xD65F03C0 // ARM64 RET
        } else {
            state
        };
        out.extend_from_slice(&op.to_le_bytes());
    }
    out.truncate(target_size);
    out
}

fn generate_apfs_metadata(target_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_size);
    let mut inode: u64 = 1000000;
    while out.len() < target_size {
        let entry_header = b"\x01\x00\x04\x00INOD";
        out.extend_from_slice(entry_header);
        out.extend_from_slice(&inode.to_le_bytes());
        out.extend_from_slice(b"com.apple.metadata:kMDItemWhereFroms\x00");
        out.extend_from_slice(&1693400000u64.to_le_bytes()); // timestamp
        out.extend_from_slice(&0o100644u32.to_le_bytes()); // mode
        inode += 1;
    }
    out.truncate(target_size);
    out
}

fn generate_redundant_logs(target_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_size);
    let mut req_id = 1000;
    while out.len() < target_size {
        let line = format!(
            "{{\"timestamp\":\"2026-08-30T12:00:00.000Z\",\"level\":\"INFO\",\
\"service\":\"ttzip-gateway\",\"req_id\":{},\"status\":200,\"path\":\"/api/v1/archive/inspect\",\
\"duration_ms\":1.42}}\n",
            req_id
        );
        out.extend_from_slice(line.as_bytes());
        req_id += 1;
    }
    out.truncate(target_size);
    out
}

fn generate_incompressible_noise(target_size: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut out = vec![0u8; target_size];
    for b in out.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state & 0xFF) as u8;
    }
    out
}

// MARK: - Block Container Synthesis Helpers

fn synthesize_v1_block_stream(src: &[u8]) -> Option<Vec<u8>> {
    let mut table = LzfseMatchTable::new();
    let mut v2_bytes = Vec::new();
    if lzfse_encode_block(src, &mut table, &mut v2_bytes).is_err() {
        return None;
    }

    if v2_bytes.len() < 32 {
        return None;
    }
    let magic = u32::from_le_bytes(v2_bytes[0..4].try_into().unwrap());
    if magic != BvxMagic::CompressedV2.as_u32() {
        return None;
    }

    let (header, v2_hdr_len) = parse_block_header(&v2_bytes).ok()?;
    let freq_tables = header.freq_tables.unwrap_or_else(LzfseFreqTables::default);
    let symbols = freq_tables.to_symbols();

    let mut v1_stream = Vec::with_capacity(770 + v2_bytes.len());
    v1_stream.extend_from_slice(&BvxMagic::CompressedV1.as_bytes());
    v1_stream.extend_from_slice(&header.n_raw_bytes.to_le_bytes());
    v1_stream.extend_from_slice(&header.n_payload_bytes.to_le_bytes());
    v1_stream.extend_from_slice(&header.n_literals.to_le_bytes());
    v1_stream.extend_from_slice(&header.n_matches.to_le_bytes());
    v1_stream.extend_from_slice(&header.n_literal_payload_bytes.to_le_bytes());
    v1_stream.extend_from_slice(&header.n_lmd_payload_bytes.to_le_bytes());
    v1_stream.extend_from_slice(&header.literal_bits.to_le_bytes());
    for s in header.literal_state {
        v1_stream.extend_from_slice(&s.to_le_bytes());
    }
    v1_stream.extend_from_slice(&header.lmd_bits.to_le_bytes());
    v1_stream.extend_from_slice(&header.l_state.to_le_bytes());
    v1_stream.extend_from_slice(&header.m_state.to_le_bytes());
    v1_stream.extend_from_slice(&header.d_state.to_le_bytes());
    for &sym in &symbols {
        v1_stream.extend_from_slice(&sym.to_le_bytes());
    }
    v1_stream.extend_from_slice(&v2_bytes[v2_hdr_len..]);
    v1_stream.extend_from_slice(&BvxMagic::EndOfStream.as_bytes());
    Some(v1_stream)
}

fn synthesize_raw_block_stream(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + src.len() + 4);
    out.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
    out.extend_from_slice(&(src.len() as u32).to_le_bytes());
    out.extend_from_slice(src);
    out.extend_from_slice(&BvxMagic::EndOfStream.as_bytes());
    out
}

fn synthesize_lzvn_block_stream(src: &[u8]) -> Result<Vec<u8>, String> {
    let bound = lzvn_compress_bound(src.len());
    let mut lzvn_payload = vec![0u8; bound];
    let written = lzvn_compress(src, &mut lzvn_payload)
        .map_err(|e| format!("LZVN compression failed: {e:?}"))?;
    lzvn_payload.truncate(written);

    let mut out = Vec::with_capacity(12 + written + 4);
    out.extend_from_slice(&BvxMagic::CompressedLZVN.as_bytes());
    out.extend_from_slice(&(src.len() as u32).to_le_bytes());
    out.extend_from_slice(&(written as u32).to_le_bytes());
    out.extend_from_slice(&lzvn_payload);
    out.extend_from_slice(&BvxMagic::EndOfStream.as_bytes());
    Ok(out)
}

// MARK: - 1. Standard Corpora Full Format Matrix Tests

#[test]
fn test_lzfse_compliance_all_corpora_roundtrip() {
    let corpora: [(&str, Vec<u8>); 6] = [
        ("Calgary Prose", generate_calgary_prose(64 * 1024)),
        ("Canterbury Struct", generate_canterbury_struct(64 * 1024)),
        ("Silesia Binary", generate_silesia_binary(64 * 1024)),
        ("APFS Metadata", generate_apfs_metadata(64 * 1024)),
        ("Redundant Logs", generate_redundant_logs(64 * 1024)),
        ("Incompressible", generate_incompressible_noise(64 * 1024, 0x8899AABBCCDDEEFF)),
    ];

    for (name, data) in &corpora {
        // 1. LZFSE Multi-Block Streaming roundtrip
        let compressed_stream = lzfse_compress_stream(data)
            .unwrap_or_else(|e| panic!("lzfse_compress_stream failed for {name}: {e:?}"));
        assert!(lzfse_validate(&compressed_stream), "Validation failed for {name}");



        let decompressed_stream = lzfse_decompress_stream(&compressed_stream)
            .unwrap_or_else(|e| panic!("lzfse_decompress_stream failed for {name}: {e:?}"));
        assert_eq!(&decompressed_stream[..], &data[..], "Stream fidelity mismatch on {name}");

        // 2. Pure Safe Rust LZVN roundtrip
        let lzvn_bound = lzvn_compress_bound(data.len());
        let mut lzvn_buf = vec![0u8; lzvn_bound];
        let lzvn_written = lzvn_compress(data, &mut lzvn_buf)
            .unwrap_or_else(|e| panic!("lzvn_compress failed for {name}: {e:?}"));
        assert!(lzvn_written > 0, "LZVN emitted 0 bytes for {name}");
        assert!(lzvn_validate(&lzvn_buf[..lzvn_written]), "LZVN validate failed for {name}");

        let mut lzvn_decomp = vec![0u8; data.len()];
        let lzvn_dec_written = lzvn_decompress(&lzvn_buf[..lzvn_written], &mut lzvn_decomp)
            .unwrap_or_else(|e| panic!("lzvn_decompress failed for {name}: {e:?}"));
        assert_eq!(lzvn_dec_written, data.len());
        assert_eq!(&lzvn_decomp[..], &data[..], "LZVN fidelity mismatch on {name}");

        // 3. Raw Block (bvx-) roundtrip
        let raw_stream = synthesize_raw_block_stream(data);
        assert!(lzfse_validate(&raw_stream), "Raw stream validate failed for {name}");
        let raw_decomp = lzfse_decompress_stream(&raw_stream)
            .unwrap_or_else(|e| panic!("Raw decompress failed for {name}: {e:?}"));
        assert_eq!(&raw_decomp[..], &data[..], "Raw block fidelity mismatch on {name}");

        // 4. LZVN Block (bvxn) in LZFSE container roundtrip
        let lzvn_stream = synthesize_lzvn_block_stream(data)
            .unwrap_or_else(|e| panic!("Synthesize LZVN stream failed for {name}: {e}"));
        assert!(lzfse_validate(&lzvn_stream), "LZVN stream validate failed for {name}");
        let lzvn_stream_decomp = lzfse_decompress_stream(&lzvn_stream)
            .unwrap_or_else(|e| panic!("LZVN stream decompress failed for {name}: {e:?}"));
        assert_eq!(&lzvn_stream_decomp[..], &data[..], "LZVN container fidelity mismatch on {name}");

        // 5. V1 Block (bvx1) uncompressed frequency table roundtrip
        if let Some(v1_stream) = synthesize_v1_block_stream(data) {
            assert!(lzfse_validate(&v1_stream), "V1 stream validate failed for {name}");
            let v1_decomp = lzfse_decompress_stream(&v1_stream)
                .unwrap_or_else(|e| panic!("V1 stream decompress failed for {name}: {e:?}"));
            assert_eq!(&v1_decomp[..], &data[..], "V1 block fidelity mismatch on {name}");
        }

        // 6. Streaming Reader / Writer Pipe
        let mut pipe_compressed = Vec::new();
        {
            let mut writer = LzfseWriter::new(&mut pipe_compressed);
            for chunk in data.chunks(3571) {
                writer.write_all(chunk).expect("pipe write_all");
            }
            writer.finish().expect("pipe finish");
        }
        assert!(lzfse_validate(&pipe_compressed), "Pipe stream validate failed for {name}");
        let mut pipe_decomp = Vec::new();
        let mut reader = LzfseReader::new(Cursor::new(&pipe_compressed));
        reader.read_to_end(&mut pipe_decomp).expect("pipe read_to_end");
        assert_eq!(&pipe_decomp[..], &data[..], "Pipe streaming fidelity mismatch on {name}");
    }
}

// MARK: - 2. Dual-Oracle Bidirectional Differential Testing

#[test]
fn test_lzfse_bidirectional_differential_apple_c() {
    let corpora: [(&str, Vec<u8>); 6] = [
        ("Calgary Prose", generate_calgary_prose(48 * 1024)),
        ("Canterbury Struct", generate_canterbury_struct(48 * 1024)),
        ("Silesia Binary", generate_silesia_binary(48 * 1024)),
        ("APFS Metadata", generate_apfs_metadata(48 * 1024)),
        ("Redundant Logs", generate_redundant_logs(48 * 1024)),
        ("Incompressible", generate_incompressible_noise(48 * 1024, 0x1122334455667788)),
    ];

    for (name, data) in &corpora {
        // Oracle Direction 1: Pure Safe Rust Encode -> Apple Native C Decode
        let rust_compressed = lzfse_compress_stream(data)
            .unwrap_or_else(|e| panic!("Rust compress failed for {name}: {e:?}"));
        let c_decompressed = apple_c_lzfse_decode(&rust_compressed, data.len())
            .unwrap_or_else(|e| panic!("Apple C failed to decode Rust output for {name}: {e}"));
        assert_eq!(
            &c_decompressed[..],
            &data[..],
            "Apple C decode mismatch on Rust compressed output for {name}"
        );

        // Oracle Direction 2: Apple Native C Encode -> Pure Safe Rust Decode
        let c_compressed = apple_c_lzfse_encode(data)
            .unwrap_or_else(|e| panic!("Apple C compress failed for {name}: {e}"));
        assert!(
            lzfse_validate(&c_compressed),
            "Rust lzfse_validate failed on Apple C output for {name}"
        );
        let rust_decompressed = lzfse_decompress_stream(&c_compressed)
            .unwrap_or_else(|e| panic!("Rust decompress failed on Apple C output for {name}: {e:?}"));
        assert_eq!(
            &rust_decompressed[..],
            &data[..],
            "Rust decode mismatch on Apple C compressed output for {name}"
        );

        // Oracle Direction 3: Rust LZVN Container -> Apple Native C Decode
        let rust_lzvn_stream = synthesize_lzvn_block_stream(data)
            .unwrap_or_else(|e| panic!("Rust synthesize LZVN stream failed for {name}: {e}"));
        let c_lzvn_decomp = apple_c_lzfse_decode(&rust_lzvn_stream, data.len())
            .unwrap_or_else(|e| panic!("Apple C failed to decode Rust LZVN stream for {name}: {e}"));
        assert_eq!(
            &c_lzvn_decomp[..],
            &data[..],
            "Apple C decode mismatch on Rust LZVN stream for {name}"
        );
    }
}

// MARK: - 3. 12-Tier Boundary Size Matrix Tests

#[test]
fn test_lzfse_boundary_sizes_matrix() {
    let boundary_sizes: [usize; 12] = [
        0,       // 0 bytes (Empty)
        1,       // 1 byte (Single byte)
        2,       // 2 bytes (Two bytes)
        15,      // 15 bytes (Sub-16 boundary)
        16,      // 16 bytes (16-byte boundary)
        256,     // 256 bytes (Small block boundary)
        4095,    // 4095 bytes (4KB - 1, LZVN threshold border)
        4096,    // 4096 bytes (4KB threshold boundary)
        65535,   // 65535 bytes (64KB - 1, LZVN 16-bit max distance)
        65536,   // 65536 bytes (64KB boundary)
        262144,  // 262144 bytes (256KB single-chunk block boundary)
        1048576, // 1048576 bytes (1MB multi-block spanning 4 chunks)
    ];

    for &size in &boundary_sizes {
        // Pattern 1: Structured / Repetitive
        let structured_data = generate_redundant_logs(size);
        assert_eq!(structured_data.len(), size);

        // Pattern 2: Incompressible Noise
        let noise_data = generate_incompressible_noise(size, 0xA1B2C3D4E5F60718 ^ (size as u64));
        assert_eq!(noise_data.len(), size);

        for (pattern_name, data) in [("Structured", &structured_data), ("Noise", &noise_data)] {
            // 1. Rust LZFSE Stream Roundtrip
            let comp_stream = lzfse_compress_stream(data)
                .unwrap_or_else(|e| panic!("Stream compress failed at size {size} ({pattern_name}): {e:?}"));
            if size > 0 {
                assert!(lzfse_validate(&comp_stream), "Validate failed at size {size}");
            }
            let decomp_stream = lzfse_decompress_stream(&comp_stream)
                .unwrap_or_else(|e| panic!("Stream decompress failed at size {size} ({pattern_name}): {e:?}"));
            assert_eq!(decomp_stream.len(), size, "Length mismatch at size {size}");
            assert_eq!(&decomp_stream[..], &data[..], "Content mismatch at size {size} ({pattern_name})");

            // 2. Rust LZVN Roundtrip
            let bound = lzvn_compress_bound(size);
            let mut lzvn_out = vec![0u8; bound];
            let written = lzvn_compress(data, &mut lzvn_out)
                .unwrap_or_else(|e| panic!("LZVN compress failed at size {size} ({pattern_name}): {e:?}"));
            if size == 0 {
                assert_eq!(written, 0);
            } else {
                assert!(
                    lzvn_validate(&lzvn_out[..written]),
                    "lzvn_validate failed at size {size} ({pattern_name})"
                );

            }
            let mut lzvn_dec = vec![0u8; size];
            let dec_written = lzvn_decompress(&lzvn_out[..written], &mut lzvn_dec)
                .unwrap_or_else(|e| panic!("LZVN decompress failed at size {size} ({pattern_name}): {e:?}"));
            assert_eq!(dec_written, size);
            assert_eq!(&lzvn_dec[..], &data[..]);

            // 3. Bidirectional Differential vs Apple Reference C (for size > 0)
            if size > 0 && size <= 262144 {
                // Rust -> Apple C
                let c_decoded = apple_c_lzfse_decode(&comp_stream, size)
                    .unwrap_or_else(|e| panic!("Apple C decode failed for size {size} ({pattern_name}): {e}"));
                assert_eq!(&c_decoded[..], &data[..], "Apple C decode mismatch at size {size}");

                // Apple C -> Rust
                let c_encoded = apple_c_lzfse_encode(data)
                    .unwrap_or_else(|e| panic!("Apple C encode failed for size {size} ({pattern_name}): {e}"));
                assert!(lzfse_validate(&c_encoded), "Rust validate failed on Apple C at size {size}");
                let rust_decoded = lzfse_decompress_stream(&c_encoded)
                    .unwrap_or_else(|e| panic!("Rust decode failed on Apple C at size {size}: {e:?}"));
                assert_eq!(&rust_decoded[..], &data[..], "Rust decode mismatch on Apple C at size {size}");
            }
        }
    }
}

// MARK: - 4. Defensive & Zero-Panic Invariants

#[test]
fn test_lzfse_defensive_zero_panic_invariants() {
    // 1. Truncated Streams
    let truncated_magic = [0x62, 0x76]; // "bv"
    assert!(!lzfse_validate(&truncated_magic));
    assert!(lzfse_decompress_stream(&truncated_magic).is_err());

    // 2. Corrupted Header Magic
    let bad_magic = [0xAA, 0xBB, 0xCC, 0xDD, 0x00, 0x00, 0x00, 0x00];
    assert!(!lzfse_validate(&bad_magic));
    assert!(lzfse_decompress_stream(&bad_magic).is_err());

    // 3. Truncated Payload Body
    let mut trunc_body = Vec::new();
    trunc_body.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
    trunc_body.extend_from_slice(&1000u32.to_le_bytes()); // claims 1000 bytes raw
    trunc_body.extend_from_slice(&[0x42; 10]); // only 10 bytes provided
    assert!(!lzfse_validate(&trunc_body));
    assert!(lzfse_decompress_stream(&trunc_body).is_err());

    // 4. LZVN Illegal Distance Underflow (D == 0)
    let bad_lzvn_zero_d = [0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut out_buf = [0u8; 64];
    assert!(lzvn_decompress(&bad_lzvn_zero_d, &mut out_buf).is_err());

    // 5. LZVN Premature EOF without EOS token
    let bad_lzvn_no_eos = [0xE5, b'H', b'e', b'l', b'l', b'o'];
    assert!(lzvn_decompress_pure_rust(&bad_lzvn_no_eos, &mut out_buf).is_err());
}
