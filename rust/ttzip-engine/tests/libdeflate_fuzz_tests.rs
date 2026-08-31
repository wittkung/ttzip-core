// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed Deflate Fault-Injection Fuzzing Harness & Jitter Streaming Suite.
//!
//! Implements a 16-dimensional fault injection test matrix, micro-step jitter streaming
//! perturbation suite (1..7 bytes), and 500+ iteration automated mutation fuzzing loop
//! aligned with RFC 1950 (Zlib), RFC 1951 (Deflate), RFC 1952 (Gzip), and `research_119`:
//! 1. Bad Magic Number Corruption (Gzip bad `0x1F, 0x8B`, Zlib bad `CMF/FLG`).
//! 2. Illegal Block Type `11` (BTYPE = 3) Injections.
//! 3. Corrupted Uncompressed `NLEN != !LEN` Length Headers & Payload Overruns.
//! 4. Truncated Dynamic Block Headers (HLIT / HDIST / HCLEN cutoffs).
//! 5. Truncated Payload Bodies (Mid-stream and pre-EOB cutoffs).
//! 6. Out-of-Bounds Precode Codespace Overflow (Kraft inequality violations).
//! 7. Out-of-Bounds Litlen / Offset Huffman Code Lengths.
//! 8. Out-of-Bounds Offset Backward Match Distance ($D > dst\_pos$).
//! 9. Zero Offset Match Distance Injection ($D = 0$).
//! 10. Malformed Adler-32 Checksum Corruption (RFC 1950 footer tampering).
//! 11. Malformed CRC-32 Checksum Corruption (RFC 1952 footer tampering).
//! 12. Malformed Gzip ISIZE Length Mismatch Injections.
//! 13. Premature EOF Bitstream Underflow & Overread Defense.
//! 14. Random Single-Bit Flip Fuzzing (500+ Iterations).
//! 15. Random Multi-Byte Erasure & Chunk Splice Attacks (500+ Iterations).
//! 16. Multi-Seed High-Entropy Pseudo-Stream Injection (1,000+ Random Streams).

use std::io::{Cursor, Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::codecs::libdeflate::{
    deflate_decompress, gzip_compress, gzip_decompress, libdeflate_deflate_compress,
    zlib_compress, zlib_decompress, ContainerFormat, FastBitWriterVec, LibdeflateDecompressor,
    LibdeflateReader, LibdeflateWriter, DEFAULT_COMPRESS_CHUNK_SIZE, GZIP_CM_DEFLATE,
    GZIP_FRESERVED, GZIP_ID1, GZIP_ID2, GZIP_MIN_OVERHEAD, ZLIB_MIN_OVERHEAD,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Deterministic Knuth Multiplicative PRNG (FUZ_rand)

/// Deterministic Knuth multiplicative hash PRNG for reproducible fuzzing seeds.
#[derive(Debug, Clone)]
pub struct FuzRand {
    seed: u32,
}

impl FuzRand {
    /// Constructs a new PRNG with the specified 32-bit seed.
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Generates the next pseudo-random 32-bit integer.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(2_654_435_761)
            .wrapping_add(2_246_822_519);
        self.seed >> 13
    }

    /// Generates a pseudo-random integer in the closed interval `[min, max]`.
    #[inline]
    pub fn rand_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u32() % span)
    }

    /// Generates a pseudo-random `usize` in half-open interval `[0, bound)`.
    #[inline]
    pub fn rand_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u32() as usize) % bound
        }
    }

    /// Generates a pseudo-random byte.
    #[inline]
    pub fn rand_u8(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }

    /// Generates a pseudo-random payload buffer with target compressibility.
    pub fn gen_buffer(&mut self, size: usize, compressibility_pct: u32) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        if compressibility_pct >= 100 {
            return buf;
        }
        let alphabet_len = match compressibility_pct {
            0..=10 => 256,
            11..=30 => 128,
            31..=60 => 32,
            61..=80 => 8,
            81..=95 => 3,
            _ => 1,
        };
        for b in buf.iter_mut() {
            *b = (self.next_u32() % alphabet_len) as u8;
        }
        buf
    }
}

/// Generates structured repetitive text for compression test fixtures.
fn generate_structured_text(repetitions: usize) -> Vec<u8> {
    let paragraph = b"TTZip Safe Libdeflate DEFLATE / Zlib / Gzip High-Performance Engine 2026. \
Pure Safe Rust implementation with branchless 64-bit bitbuffer and SIMD Wild Copy replication.\n";
    let mut out = Vec::with_capacity(paragraph.len() * repetitions);
    for _ in 0..repetitions {
        out.extend_from_slice(paragraph);
    }
    out
}

// MARK: - Target 1: Bad Magic Number Corruption

#[test]
fn test_libdeflate_target01_bad_magic_corruption() {
    // 1. Gzip Magic and Header Corruption
    let invalid_gzip_headers: Vec<Vec<u8>> = vec![
        vec![0x00, GZIP_ID2, GZIP_CM_DEFLATE, 0, 0, 0, 0, 0, 0, 255],
        vec![GZIP_ID1, 0x00, GZIP_CM_DEFLATE, 0, 0, 0, 0, 0, 0, 255],
        vec![0x1E, 0x8B, GZIP_CM_DEFLATE, 0, 0, 0, 0, 0, 0, 255],
        vec![0xFF, 0xFF, GZIP_CM_DEFLATE, 0, 0, 0, 0, 0, 0, 255],
        vec![GZIP_ID1, GZIP_ID2, 9, 0, 0, 0, 0, 0, 0, 255], // Invalid CM != 8
        vec![GZIP_ID1, GZIP_ID2, GZIP_CM_DEFLATE, GZIP_FRESERVED, 0, 0, 0, 0, 0, 255],
        vec![GZIP_ID1, GZIP_ID2], // Truncated header
    ];

    let mut dst = vec![0u8; 256];
    for hdr in &invalid_gzip_headers {
        let unwind_res = catch_unwind(AssertUnwindSafe(|| gzip_decompress(hdr, &mut dst)));
        assert!(unwind_res.is_ok(), "gzip_decompress panicked on bad header");
        assert!(unwind_res.unwrap().is_err());

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(hdr), ContainerFormat::Gzip).expect("init reader");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on bad gzip header");
    }

    // 2. Zlib CMF/FLG Alignment and Header Corruption
    let invalid_zlib_headers: Vec<Vec<u8>> = vec![
        vec![0x78, 0x00, 0, 0, 0, 0], // FCHECK violation ((0x7800) % 31 != 0)
        vec![0x00, 0x00, 0, 0, 0, 0], // All zeros
        vec![0xFF, 0xFF, 0, 0, 0, 0], // All ones
        vec![0x79, 0x94, 0, 0, 0, 0], // Invalid CM = 9
        vec![0x88, 0x98, 0, 0, 0, 0], // Invalid CINFO = 8 (> 7)
        vec![0x78, 0xBB, 0, 0, 0, 0], // FDICT bit set
        vec![0x78],                   // Truncated header (< 2 bytes)
    ];

    for hdr in &invalid_zlib_headers {
        let unwind_res = catch_unwind(AssertUnwindSafe(|| zlib_decompress(hdr, &mut dst)));
        assert!(unwind_res.is_ok(), "zlib_decompress panicked on bad header");
        assert!(unwind_res.unwrap().is_err());

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(hdr), ContainerFormat::Zlib).expect("init reader");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on bad zlib header");
    }
}

// MARK: - Target 2: Illegal Block Type `11` (BTYPE = 3) Injections

#[test]
fn test_libdeflate_target02_illegal_block_type_11_injection() {
    let mut dst = vec![0u8; 128];

    // RFC 1951: BTYPE = 3 (0b11) is illegal / reserved
    let invalid_btypes = [
        vec![0x06, 0x00, 0x00, 0x00], // BFINAL=0, BTYPE=3
        vec![0x07, 0x00, 0x00, 0x00], // BFINAL=1, BTYPE=3
        vec![0x06],                   // 1 byte BTYPE=3
        vec![0x07],                   // 1 byte BFINAL=1, BTYPE=3
    ];

    for stream in &invalid_btypes {
        let unwind_res = catch_unwind(AssertUnwindSafe(|| deflate_decompress(stream, &mut dst)));
        assert!(unwind_res.is_ok(), "deflate_decompress panicked on BTYPE=3");
        assert_eq!(unwind_res.unwrap(), Err(TTZipStatus::ErrCorruptHeader));

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(stream), ContainerFormat::Raw).expect("init reader");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on BTYPE=3");
    }
}

// MARK: - Target 3: Corrupted Uncompressed `NLEN != !LEN` Length Headers

#[test]
fn test_libdeflate_target03_corrupted_uncompressed_nlen_mismatch() {
    let payload = b"Safe TTZip Libdeflate Uncompressed Block Fuzz Test";
    let len = payload.len() as u16;

    // 1. NLEN == LEN (Fails NLEN == !LEN invariant)
    let mut stream_nlen_eq = Vec::new();
    stream_nlen_eq.push(0x01); // BFINAL=1, BTYPE=00
    stream_nlen_eq.extend_from_slice(&len.to_le_bytes());
    stream_nlen_eq.extend_from_slice(&len.to_le_bytes()); // Corrupted NLEN
    stream_nlen_eq.extend_from_slice(payload);

    let mut dst = vec![0u8; 128];
    assert_eq!(
        deflate_decompress(&stream_nlen_eq, &mut dst),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 2. Single-bit flipped NLEN
    let mut stream_bitflip_nlen = Vec::new();
    stream_bitflip_nlen.push(0x01);
    stream_bitflip_nlen.extend_from_slice(&len.to_le_bytes());
    stream_bitflip_nlen.extend_from_slice(&(!len ^ 1).to_le_bytes());
    stream_bitflip_nlen.extend_from_slice(payload);

    assert_eq!(
        deflate_decompress(&stream_bitflip_nlen, &mut dst),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 3. LEN exceeds available payload bytes
    let mut stream_overrun = Vec::new();
    stream_overrun.push(0x01);
    stream_overrun.extend_from_slice(&1000u16.to_le_bytes());
    stream_overrun.extend_from_slice(&(!1000u16).to_le_bytes());
    stream_overrun.extend_from_slice(b"Short");

    let mut dst_large = vec![0u8; 2048];
    assert!(deflate_decompress(&stream_overrun, &mut dst_large).is_err());
}

// MARK: - Target 4: Truncated Dynamic Block Headers

#[test]
fn test_libdeflate_target04_truncated_dynamic_block_header() {
    let mut dst = vec![0u8; 128];

    // Dynamic block BFINAL=1, BTYPE=10 (0b101 = 0x05)
    let cutoffs: Vec<Vec<u8>> = vec![
        vec![0x05],                         // Cut immediately after block type
        vec![0x05, 0x00],                   // Cut mid HLIT/HDIST
        vec![0x05, 0x00, 0x00],             // Cut before HCLEN completes
        vec![0x05, 0x00, 0x00, 0x00],       // Cut before precode lengths
        vec![0x05, 0xFF, 0xFF, 0xFF, 0x0F], // Valid HLIT/HDIST/HCLEN but missing precode table
    ];

    for stream in &cutoffs {
        let unwind_res = catch_unwind(AssertUnwindSafe(|| deflate_decompress(stream, &mut dst)));
        assert!(unwind_res.is_ok(), "deflate_decompress panicked on truncated dynamic header");
        assert_eq!(unwind_res.unwrap(), Err(TTZipStatus::ErrCorruptHeader));
    }
}

// MARK: - Target 5: Truncated Payload Bodies

#[test]
fn test_libdeflate_target05_truncated_payload_bodies() {
    let payload = generate_structured_text(4);
    let valid_stream = libdeflate_deflate_compress(&payload, 6).expect("deflate_compress");

    let mut dst = vec![0u8; payload.len() + 64];

    // Truncate at every 10% slice and critical boundaries
    for pct in 1..100 {
        let cut = (valid_stream.len() * pct) / 100;
        if cut == 0 || cut >= valid_stream.len() {
            continue;
        }
        let truncated = &valid_stream[..cut];

        let unwind_res = catch_unwind(AssertUnwindSafe(|| deflate_decompress(truncated, &mut dst)));
        assert!(unwind_res.is_ok(), "deflate_decompress panicked on cut at {cut} bytes");
        assert!(unwind_res.unwrap().is_err());

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(truncated), ContainerFormat::Raw).expect("init");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on cut at {cut} bytes");
    }
}

// MARK: - Target 6: Out-of-Bounds Precode Codespace Overflow (Kraft Violation)

#[test]
fn test_libdeflate_target06_out_of_bounds_precode_codespace_overflow() {
    let mut writer = FastBitWriterVec::new();
    // BFINAL=1, BTYPE=10 (Dynamic Huffman) -> 0b101 (3 bits)
    writer.add_bits(5, 3);
    // HLIT = 0 (5 bits -> 257 symbols)
    writer.add_bits(0, 5);
    // HDIST = 0 (5 bits -> 1 symbol)
    writer.add_bits(0, 5);
    // HCLEN = 1 (4 bits -> 5 precode code lengths)
    writer.add_bits(1, 4);

    // Write 5 precode lengths, each length = 1 (sum of 2^-1 * 5 = 2.5 > 1.0, Codespace Overflow)
    for _ in 0..5 {
        writer.add_bits(1, 3);
    }
    let stream = writer.finish();

    let mut dst = vec![0u8; 128];
    let res = deflate_decompress(&stream, &mut dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Precode codespace overflow must be rejected with ErrCorruptHeader"
    );
}

// MARK: - Target 7: Out-of-Bounds Litlen / Offset Huffman Code Lengths

#[test]
fn test_libdeflate_target07_out_of_bounds_litlen_huffman_codelengths() {
    // 1. Repeat code 16 at symbol index 0 (No preceding symbol to repeat)
    let mut writer = FastBitWriterVec::new();
    writer.add_bits(5, 3); // BFINAL=1, BTYPE=10
    writer.add_bits(0, 5); // HLIT=0 (257)
    writer.add_bits(0, 5); // HDIST=0 (1)
    writer.add_bits(0, 4); // HCLEN=0 (4 precode lengths)

    // Precode permutation: 16, 17, 18, 0
    // Set precode len for sym 16 = 1 (code 0b0, len 1)
    writer.add_bits(1, 3); // sym 16 len = 1
    writer.add_bits(0, 3); // sym 17 len = 0
    writer.add_bits(0, 3); // sym 18 len = 0
    writer.add_bits(0, 3); // sym 0 len = 0

    // Now emit symbol 16 (1 bit: 0) + 2 extra repeat bits at the very start
    writer.add_bits(0, 1); // sym 16
    writer.add_bits(0, 2); // repeat 3 times
    let stream = writer.finish();

    let mut dst = vec![0u8; 128];
    let res = deflate_decompress(&stream, &mut dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Repeat code 16 at index 0 must be rejected"
    );
}

// MARK: - Target 8: Out-of-Bounds Offset Backward Match Distance ($D > dst_pos$)

#[test]
fn test_libdeflate_target08_out_of_bounds_offset_backward_distance() {
    // Construct a Static Huffman block emitting 'A', then matching with D = 49 (> dst_pos = 1)
    let mut writer = FastBitWriterVec::new();
    // BFINAL=1, BTYPE=01 (Static Huffman) -> 3 bits: 0b011 = 3
    writer.add_bits(3, 3);

    // Literal 'A' (65): 8 bits, code 0x71 -> reversed: 0b10001110
    let code_a = (0x30u32 + 65).reverse_bits() >> (32 - 8);
    writer.add_bits(code_a as u64, 8);

    // Length symbol 257 (Length 3): 7 bits, code 1 -> reversed: 0b1000000
    let code_len3 = (1u32).reverse_bits() >> (32 - 7);
    writer.add_bits(code_len3 as u64, 7);

    // Distance symbol 10 (base 49): 5 bits, code 10 (0b01010) -> reversed: 0b01010
    let code_dist10 = (10u32).reverse_bits() >> (32 - 5);
    writer.add_bits(code_dist10 as u64, 5);
    // Distance symbol 10 has 4 extra bits (0..15). Emit 0. Total distance = 49.
    writer.add_bits(0, 4);

    let stream = writer.finish();

    let mut dst = vec![0u8; 128];
    let res = deflate_decompress(&stream, &mut dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Backward distance D > dst_pos must be rejected with ErrCorruptHeader"
    );
}

// MARK: - Target 9: Zero Offset Match Distance Injection ($D = 0$)

#[test]
fn test_libdeflate_target09_zero_offset_distance_injection() {
    // 1. Match emitted at dst_pos = 0 (Any distance >= 0 violates D <= dst_pos)
    let mut writer = FastBitWriterVec::new();
    // BFINAL=1, BTYPE=01 (Static Huffman)
    writer.add_bits(3, 3);
    // Length symbol 257 (Length 3): 7 bits (0b1000000)
    let code_len3 = (1u32).reverse_bits() >> (32 - 7);
    writer.add_bits(code_len3 as u64, 7);
    // Distance symbol 0: 5 bits (0b00000)
    writer.add_bits(0, 5);
    let stream = writer.finish();

    let mut dst = vec![0u8; 64];
    let res = deflate_decompress(&stream, &mut dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Match distance at dst_pos = 0 must be rejected with ErrCorruptHeader"
    );

    // 2. Direct LibdeflateDecompressor state machine invocation on zero-distance match
    let mut decompressor = LibdeflateDecompressor::new();
    let unwind_res = catch_unwind(AssertUnwindSafe(|| decompressor.decompress(&stream, &mut dst)));
    assert!(unwind_res.is_ok(), "Decompressor panicked on zero/invalid distance");
    assert_eq!(unwind_res.unwrap(), Err(TTZipStatus::ErrCorruptHeader));
}

// MARK: - Target 10: Malformed Adler-32 Checksum Corruption

#[test]
fn test_libdeflate_target10_malformed_adler32_corruption() {
    let payload = generate_structured_text(5);
    let valid_zlib = zlib_compress(&payload, 6).expect("zlib compress");
    assert!(valid_zlib.len() >= ZLIB_MIN_OVERHEAD);

    let mut dst = vec![0u8; payload.len() + 64];

    // Flip bits in the 4-byte Adler-32 footer
    for footer_byte_idx in (valid_zlib.len() - 4)..valid_zlib.len() {
        let mut corrupted = valid_zlib.clone();
        corrupted[footer_byte_idx] ^= 0xFF;

        let res = zlib_decompress(&corrupted, &mut dst);
        assert_eq!(
            res,
            Err(TTZipStatus::ErrCorruptHeader),
            "Adler-32 checksum tampering at offset {footer_byte_idx} must return ErrCorruptHeader"
        );

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(&corrupted), ContainerFormat::Zlib).expect("init");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on corrupted Adler-32");
    }
}

// MARK: - Target 11: Malformed CRC-32 Checksum Corruption

#[test]
fn test_libdeflate_target11_malformed_crc32_corruption() {
    let payload = generate_structured_text(5);
    let valid_gzip = gzip_compress(&payload, 6).expect("gzip compress");
    assert!(valid_gzip.len() >= GZIP_MIN_OVERHEAD);

    let mut dst = vec![0u8; payload.len() + 64];

    // Flip bits in the 4-byte CRC-32 footer (bytes [len - 8 .. len - 4])
    for footer_byte_idx in (valid_gzip.len() - 8)..(valid_gzip.len() - 4) {
        let mut corrupted = valid_gzip.clone();
        corrupted[footer_byte_idx] ^= 0xFF;

        let res = gzip_decompress(&corrupted, &mut dst);
        assert_eq!(
            res,
            Err(TTZipStatus::ErrCorruptHeader),
            "CRC-32 checksum tampering at offset {footer_byte_idx} must return ErrCorruptHeader"
        );

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(&corrupted), ContainerFormat::Gzip).expect("init");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on corrupted CRC-32");
    }
}

// MARK: - Target 12: Malformed Gzip ISIZE Length Mismatch Injections

#[test]
fn test_libdeflate_target12_malformed_gzip_isize_mismatch() {
    let payload = generate_structured_text(5);
    let valid_gzip = gzip_compress(&payload, 6).expect("gzip compress");
    assert!(valid_gzip.len() >= GZIP_MIN_OVERHEAD);

    let mut dst = vec![0u8; payload.len() + 64];

    // Modify the last 4 bytes (ISIZE) to mismatch decompressed size
    for footer_byte_idx in (valid_gzip.len() - 4)..valid_gzip.len() {
        let mut corrupted = valid_gzip.clone();
        corrupted[footer_byte_idx] ^= 0x55;

        let res = gzip_decompress(&corrupted, &mut dst);
        assert_eq!(
            res,
            Err(TTZipStatus::ErrCorruptHeader),
            "ISIZE tampering at offset {footer_byte_idx} must return ErrCorruptHeader"
        );

        let unwind_reader = catch_unwind(AssertUnwindSafe(|| {
            let mut reader =
                LibdeflateReader::new(Cursor::new(&corrupted), ContainerFormat::Gzip).expect("init");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));
        assert!(unwind_reader.is_ok(), "LibdeflateReader panicked on corrupted ISIZE");
    }
}

// MARK: - Target 13: Premature EOF Bitstream Underflow & Overread Defense

#[test]
fn test_libdeflate_target13_premature_eof_bitstream_underflow() {
    let payload = generate_structured_text(2);
    let valid_deflate = libdeflate_deflate_compress(&payload, 6).expect("compress");

    let mut dst = vec![0u8; payload.len() + 64];

    // Test cutting before final bits can be refilled
    for cut in 1..valid_deflate.len().min(16) {
        let truncated = &valid_deflate[..cut];
        let res = deflate_decompress(truncated, &mut dst);
        assert_eq!(
            res,
            Err(TTZipStatus::ErrCorruptHeader),
            "Premature EOF at cut {cut} must return ErrCorruptHeader"
        );
    }
}

// MARK: - Target 14: Random Single-Bit Flip Fuzzing (500+ Iterations)

#[test]
fn test_libdeflate_target14_random_single_bit_flip_fuzzing() {
    let payload = generate_structured_text(6);
    let raw_deflate = libdeflate_deflate_compress(&payload, 6).expect("compress");
    let zlib_stream = zlib_compress(&payload, 6).expect("zlib");
    let gzip_stream = gzip_compress(&payload, 6).expect("gzip");

    let mut rng = FuzRand::new(0x2026_0831);
    let mut dst = vec![0u8; payload.len() + 512];

    for iter in 0..500 {
        // 1. Raw Deflate bit flip
        let mut mut_raw = raw_deflate.clone();
        let idx = rng.rand_usize(mut_raw.len());
        mut_raw[idx] ^= 1 << (rng.next_u32() & 7);
        let unwind_raw = catch_unwind(AssertUnwindSafe(|| {
            let _ = deflate_decompress(&mut_raw, &mut dst);
        }));
        assert!(unwind_raw.is_ok(), "deflate_decompress panicked on bitflip iter {iter}");

        // 2. Zlib bit flip
        let mut mut_zlib = zlib_stream.clone();
        let idx = rng.rand_usize(mut_zlib.len());
        mut_zlib[idx] ^= 1 << (rng.next_u32() & 7);
        let unwind_zlib = catch_unwind(AssertUnwindSafe(|| {
            let _ = zlib_decompress(&mut_zlib, &mut dst);
        }));
        assert!(unwind_zlib.is_ok(), "zlib_decompress panicked on bitflip iter {iter}");

        // 3. Gzip bit flip
        let mut mut_gzip = gzip_stream.clone();
        let idx = rng.rand_usize(mut_gzip.len());
        mut_gzip[idx] ^= 1 << (rng.next_u32() & 7);
        let unwind_gzip = catch_unwind(AssertUnwindSafe(|| {
            let _ = gzip_decompress(&mut_gzip, &mut dst);
        }));
        assert!(unwind_gzip.is_ok(), "gzip_decompress panicked on bitflip iter {iter}");
    }
}

// MARK: - Target 15: Random Multi-Byte Erasure & Chunk Splice Attacks (500+ Iterations)

#[test]
fn test_libdeflate_target15_random_multi_byte_erasure() {
    let payload = generate_structured_text(8);
    let gzip_stream = gzip_compress(&payload, 6).expect("gzip compress");
    let mut rng = FuzRand::new(0x1337_DEEF);
    let mut dst = vec![0u8; payload.len() + 512];

    for iter in 0..500 {
        let mut mutated = gzip_stream.clone();
        let erase_len = rng.rand_range(1, 32) as usize;
        let erase_offset = rng.rand_usize(mutated.len().saturating_sub(erase_len));

        if rng.next_u32().is_multiple_of(2) {
            for b in &mut mutated[erase_offset..erase_offset + erase_len] {
                *b = 0;
            }
        } else {
            mutated.drain(erase_offset..erase_offset + erase_len);
        }

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let _ = gzip_decompress(&mutated, &mut dst);
            let mut reader =
                LibdeflateReader::new(Cursor::new(&mutated), ContainerFormat::Gzip).expect("init");
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));

        assert!(
            unwind_res.is_ok(),
            "Multi-byte erasure panicked on iter {iter} (offset {erase_offset}, len {erase_len})"
        );
    }
}

// MARK: - Target 16: Multi-Seed High-Entropy Pseudo-Stream Injection (1,000+ Random Streams)

#[test]
fn test_libdeflate_target16_random_high_entropy_pseudostream_injection() {
    let seeds = [
        0u32, 1, 42, 1337, 0x2026_0831, 0xDEAD_BEEF, 0xCAFE_BABE, 0x8000_0000, 0xFFFF_FFFF,
        0x1234_5678,
    ];
    let mut dst = vec![0u8; 4096];

    for &seed in &seeds {
        let mut rng = FuzRand::new(seed);
        for _ in 0..100 {
            let len = rng.rand_range(1, 2048) as usize;
            let noise = rng.gen_buffer(len, 0);

            let unwind_res = catch_unwind(AssertUnwindSafe(|| {
                let _ = deflate_decompress(&noise, &mut dst);
                let _ = zlib_decompress(&noise, &mut dst);
                let _ = gzip_decompress(&noise, &mut dst);

                for format in [ContainerFormat::Raw, ContainerFormat::Zlib, ContainerFormat::Gzip] {
                    if let Ok(mut reader) = LibdeflateReader::new(Cursor::new(&noise), format) {
                        let mut out = Vec::new();
                        let _ = reader.read_to_end(&mut out);
                    }
                }
            }));

            assert!(
                unwind_res.is_ok(),
                "High-entropy pseudo-stream panicked on seed {seed:#010X}"
            );
        }
    }
}

// MARK: - Micro-Step Jitter Streaming Push & Pull (1..7 Bytes)

#[test]
fn test_libdeflate_microstep_jitter_streaming_1_to_7_bytes() {
    let payloads: Vec<Vec<u8>> = vec![
        b"Small microstep jitter test payload.".to_vec(),
        generate_structured_text(5),
        generate_structured_text(25), // Crosses 64KB micro-buffer thresholds
    ];

    let formats = [
        ContainerFormat::Raw,
        ContainerFormat::Zlib,
        ContainerFormat::Gzip,
    ];

    for (case_idx, payload) in payloads.iter().enumerate() {
        for &format in &formats {
            for step in 1..=7 {
                // 1. Jitter Push into LibdeflateWriter
                let mut compressed = Vec::new();
                {
                    let mut writer = LibdeflateWriter::with_chunk_size(
                        &mut compressed,
                        format,
                        6,
                        DEFAULT_COMPRESS_CHUNK_SIZE,
                    )
                    .expect("init writer");
                    let mut cursor = 0;
                    while cursor < payload.len() {
                        let end = (cursor + step).min(payload.len());
                        writer
                            .write_all(&payload[cursor..end])
                            .expect("jitter write chunk");
                        cursor = end;
                    }
                    writer.finish().expect("finish jitter writer");
                }

                // 2. Jitter Pull from LibdeflateReader
                let mut reader = LibdeflateReader::new(Cursor::new(&compressed), format)
                    .expect("init jitter reader");
                let mut decompressed = Vec::new();
                let mut chunk = vec![0u8; step];

                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => decompressed.extend_from_slice(&chunk[..n]),
                        Err(e) => panic!(
                            "Jitter read failed for case {case_idx}, format {format:?}, step {step}: {e:?}"
                        ),
                    }
                }

                assert_eq!(
                    decompressed.as_slice(),
                    payload.as_slice(),
                    "Decompressed mismatch for case {case_idx}, format {format:?}, step {step}"
                );
            }
        }
    }
}

// MARK: - 500+ Round Automated Mutation Fuzzing Loop

#[test]
fn test_libdeflate_500_round_automated_mutation_fuzzing_loop() {
    let payload = generate_structured_text(8);
    let zlib_stream = zlib_compress(&payload, 6).expect("zlib compress");
    let mut rng = FuzRand::new(0x2026_0831);
    let mut dst = vec![0u8; payload.len() + 1024];

    for iter in 0..500 {
        let mut mutated = zlib_stream.clone();
        let mutation_kind = rng.rand_range(0, 3);

        match mutation_kind {
            0 => {
                // Random single byte replacement
                let idx = rng.rand_usize(mutated.len());
                mutated[idx] = rng.rand_u8();
            }
            1 => {
                // Random bit flip
                let idx = rng.rand_usize(mutated.len());
                mutated[idx] ^= 1 << (rng.next_u32() & 7);
            }
            2 => {
                // Random byte swap
                let idx1 = rng.rand_usize(mutated.len());
                let idx2 = rng.rand_usize(mutated.len());
                mutated.swap(idx1, idx2);
            }
            _ => {
                // Header byte corruption
                let idx = rng.rand_usize(mutated.len().min(16));
                mutated[idx] ^= 0xFF;
            }
        }

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let block_res = zlib_decompress(&mutated, &mut dst);
            if let Ok(written) = block_res {
                assert!(written <= dst.len());
            }

            if let Ok(mut reader) =
                LibdeflateReader::new(Cursor::new(&mutated), ContainerFormat::Zlib)
            {
                let mut out = Vec::new();
                let _ = reader.read_to_end(&mut out);
            }
        }));

        assert!(
            unwind_res.is_ok(),
            "Decompressor panicked on mutation fuzzing iteration {iter} (mutation_kind {mutation_kind})"
        );
    }
}
