// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for Apple LZVN Bytecode Decoder & Wild Copy VM.
//!
//! Validates:
//! 1. All 16 Opcode types and their literal/match combinations.
//! 2. Distance reuse ($D_0 = D_{\text{prev}}$) across consecutive instructions.
//! 3. Overlapping match expansion (RLE $D=1$, 16-bit $D=2$, 24-bit $D=3$, 32-bit $D=4$, $D=5..7$, and $D \ge 8$ Wild Copy).
//! 4. 100% roundtrip decompression fidelity against C reference `lzvn_encode_buffer` and pure Rust encoder.
//! 5. Robust defensive error handling: distance underflow ($D=0 \lor D > \text{dst\_pos}$), truncated streams, invalid opcodes, and 0-panic fuzz resistance.

use ttzip_engine::codecs::lzfse::lzvn_decoder::{
    lzvn_decompress_pure_rust, lzvn_decompress_to_vec_pure_rust, lzvn_validate, LzvnDecoder,
    LzvnOpcodeKind, LZVN_OPCODE_TABLE,
};
use ttzip_engine::codecs::lzfse::{lzvn_compress, lzvn_compress_bound};
use ttzip_engine::types::TTZipStatus;

// MARK: - Test Helper: Build Synthetic LZVN Streams

/// Appends an 8-byte EOS marker (`0x06` followed by 7 zero bytes) to a byte stream.
fn append_eos(stream: &mut Vec<u8>) {
    stream.extend_from_slice(&[0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
}

// MARK: - Section 1: Opcode Coverage Tests (16 Opcode Variants)

#[test]
fn test_opcode_sml_l_small_literal() {
    // SmlL: 0xE0 + L (where L is 1..15)
    let mut stream = Vec::new();
    let literals = b"Hello, World!";
    let l = literals.len() as u8;
    stream.push(0xE0 | l);
    stream.extend_from_slice(literals);
    append_eos(&mut stream);

    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let (src_read, dst_written) = decoder.decode(&stream, &mut dst).expect("decode SmlL");

    assert_eq!(src_read, stream.len());
    assert_eq!(dst_written, literals.len());
    assert_eq!(&dst[..dst_written], literals);
    assert!(decoder.end_of_stream);
}

#[test]
fn test_opcode_lrg_l_large_literal() {
    // LrgL: 0xE0 followed by (L - 16) byte, then L literals (L in 16..271)
    let mut stream = Vec::new();
    let l_total = 40usize;
    let literal_bytes: Vec<u8> = (0..l_total).map(|i| (i as u8).wrapping_add(65)).collect();

    stream.push(0xE0);
    stream.push((l_total - 16) as u8);
    stream.extend_from_slice(&literal_bytes);
    append_eos(&mut stream);

    let mut dst = vec![0u8; 64];
    let mut decoder = LzvnDecoder::new();
    let (src_read, dst_written) = decoder.decode(&stream, &mut dst).expect("decode LrgL");

    assert_eq!(src_read, stream.len());
    assert_eq!(dst_written, l_total);
    assert_eq!(&dst[..dst_written], &literal_bytes);
}

#[test]
fn test_opcode_sml_d_zero_literal() {
    // SmlD: LLMMMDDD DDDDDDDD LITERAL
    // With L=0, M=6 (x=3 => M = 3 + 3 = 6), D=4
    // Byte 0: (L << 6) | ((M - 3) << 3) | (D >> 8) = (0 << 6) | (3 << 3) | 0 = 0x18
    // Byte 1: D & 0xFF = 4
    let mut stream = Vec::new();
    // Preload 4 literal bytes with SmlL
    stream.push(0xE4);
    stream.extend_from_slice(b"WXYZ");
    // Emit SmlD (L=0, M=6, D=4)
    stream.push(0x18);
    stream.push(0x04);
    append_eos(&mut stream);

    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode SmlD L=0");

    assert_eq!(dst_written, 10);
    assert_eq!(&dst[..dst_written], b"WXYZWXYZWX");
}

#[test]
fn test_opcode_sml_d_with_literals() {
    // SmlD with L=3, M=4 (M-3=1), D=5
    // Byte 0: (3 << 6) | (1 << 3) | 0 = 0xC8
    // Byte 1: 0x05
    // Literal: 3 bytes b"ABC"
    let mut stream = Vec::new();
    // Seed initial literal
    stream.push(0xE5);
    stream.extend_from_slice(b"12345");
    // SmlD
    stream.push(0xC8);
    stream.push(0x05);
    stream.extend_from_slice(b"ABC");
    append_eos(&mut stream);

    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode SmlD L=3");

    // Initial 5 + Literal 3 ("ABC") + Match 4 ("45AB" from distance 5 before match pos 8: pos 3..7)
    assert_eq!(dst_written, 5 + 3 + 4);
    assert_eq!(&dst[..dst_written], b"12345ABC45AB");
}

#[test]
fn test_opcode_med_d_zero_literal() {
    // MedD: 101LLMMM DDDDDDMM DDDDDDDD LITERAL
    // With L=0, M=8 (x = 5 => top 3 bits = 1, bottom 2 bits = 1 => (1 << 2) | 1 = 5, M = 5 + 3 = 8)
    // D = 12
    // opc: 0xA0 | (0 << 3) | 1 = 0xA1
    // opc23: (D << 2) | (x & 3) = (12 << 2) | 1 = 49 (0x0031 in little endian => [0x31, 0x00])
    let mut stream = Vec::new();
    // Seed 12 bytes
    stream.push(0xE0);
    stream.push(0); // LrgL 16 bytes
    let seed = b"0123456789AB0123";
    stream.extend_from_slice(seed);

    stream.push(0xA1);
    stream.push(0x31);
    stream.push(0x00);
    append_eos(&mut stream);

    let mut dst = vec![0u8; 64];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode MedD L=0");

    assert_eq!(dst_written, 16 + 8);
    // distance 12 from pos 16 points to pos 4 (seed[4..12] = b"456789AB")
    assert_eq!(&dst[16..24], b"456789AB");
}

#[test]
fn test_opcode_med_d_with_literals() {
    // MedD with L=2, M=10 (x=7 => top=1, bot=3 => (1<<2)|3 = 7, M=10), D=14
    // opc: 0xA0 | (2 << 3) | 1 = 0xB1
    // opc23: (14 << 2) | 3 = 59 (0x003B) => [0x3B, 0x00]
    let mut stream = Vec::new();
    stream.push(0xE0);
    stream.push(0);
    stream.extend_from_slice(b"ABCDEFGHIJKLMNOP");

    stream.push(0xB1);
    stream.push(0x3B);
    stream.push(0x00);
    stream.extend_from_slice(b"XY");
    append_eos(&mut stream);

    let mut dst = vec![0u8; 64];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode MedD L=2");

    assert_eq!(dst_written, 16 + 2 + 10);
    assert_eq!(&dst[16..18], b"XY");
    // dst_pos for match is 18. D=14 => reads from pos 4 (b"EFGHIJKLMN")
    assert_eq!(&dst[18..28], b"EFGHIJKLMN");
}

#[test]
fn test_opcode_lrg_d_zero_literal() {
    // LrgD: LLMMM111 DDDDDDDD DDDDDDDD LITERAL
    // L=0, M=5 (M-3=2 => MMM=2), low 3 bits = 7 (0b111)
    // opc: (0 << 6) | (2 << 3) | 7 = 0x17
    // D: 1000 (0x03E8 => [0xE8, 0x03])
    let mut stream = Vec::new();
    // Seed 1024 bytes of known pattern
    let seed: Vec<u8> = (0..1024).map(|i| (i & 0xFF) as u8).collect();
    let l_len = seed.len();
    // We can emit multiple chunks of LrgL or a loop
    let mut off = 0;
    while off < l_len {
        let chunk = (l_len - off).min(271);
        if chunk >= 16 {
            stream.push(0xE0);
            stream.push((chunk - 16) as u8);
        } else {
            stream.push(0xE0 | (chunk as u8));
        }
        stream.extend_from_slice(&seed[off..off + chunk]);
        off += chunk;
    }

    // LrgD: D=1000, M=5, L=0
    stream.push(0x17);
    stream.push(0xE8);
    stream.push(0x03);
    append_eos(&mut stream);

    let mut dst = vec![0u8; 2048];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode LrgD L=0");

    assert_eq!(dst_written, 1024 + 5);
    // At pos 1024, D=1000 reads from pos 24 (seed[24..29])
    assert_eq!(&dst[1024..1029], &seed[24..29]);
}

#[test]
fn test_opcode_lrg_d_with_literals() {
    // LrgD: L=3, M=4 (MMM=1), low=7 => (3 << 6) | (1 << 3) | 7 = 0xCF
    // D=500 ([0xF4, 0x01]), literal: b"123"
    let mut stream = Vec::new();
    let seed: Vec<u8> = (0..600).map(|i| (i % 251) as u8).collect();
    let mut off = 0;
    while off < seed.len() {
        let chunk = (seed.len() - off).min(271);
        if chunk >= 16 {
            stream.push(0xE0);
            stream.push((chunk - 16) as u8);
        } else {
            stream.push(0xE0 | (chunk as u8));
        }
        stream.extend_from_slice(&seed[off..off + chunk]);
        off += chunk;
    }

    stream.push(0xCF);
    stream.push(0xF4);
    stream.push(0x01);
    stream.extend_from_slice(b"123");
    append_eos(&mut stream);

    let mut dst = vec![0u8; 1024];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode LrgD L=3");

    assert_eq!(dst_written, 600 + 3 + 4);
    assert_eq!(&dst[600..603], b"123");
    // match starts at 603, D=500 => reads from pos 103 (seed[103..107])
    assert_eq!(&dst[603..607], &seed[103..107]);
}

#[test]
fn test_opcode_pre_d_zero_literal() {
    // PreD: LLMMM110 LITERAL (L=0, M=6 (MMM=3), low=6 => 0x1E is udef, wait! PreD with L=0, MMM=3: (0<<6)|(3<<3)|6 = 0x1E is Udef? No! Check opcode table: 0x46 is L=1, MMM=0. For L=0, low=6: 0x06 is EOS, 0x0E is NOP, 0x16 is NOP, 0x1E is UDEF, 0x46 is L=1 MMM=0 PreD!)
    // Let's test PreD with L=1 (opc = 0x46: L=1, M=3, PreD)
    let mut stream = Vec::new();
    // Seed initial literal and set d_prev via SmlD
    stream.push(0xE8);
    stream.extend_from_slice(b"ABCDEFGH");
    // SmlD (L=0, M=4, D=8) => (0<<6)|(1<<3)|0 = 0x08, D=8
    stream.push(0x08);
    stream.push(0x08);
    // Now d_prev = 8.
    // PreD with L=1, M=3: opc = (1<<6)|(0<<3)|6 = 0x46, followed by 1 literal byte b"Z"
    stream.push(0x46);
    stream.push(b'Z');
    append_eos(&mut stream);

    let mut dst = vec![0u8; 64];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode PreD");

    // Initial: 8 ("ABCDEFGH")
    // SmlD: +4 ("ABCD" from D=8) => pos 12
    // PreD: +1 lit ("Z" at pos 12) => pos 13, +3 match from D=8 (pos 13-8=5, seed[5..8] = "FGH") => "FGH"
    assert_eq!(dst_written, 8 + 4 + 1 + 3);
    assert_eq!(&dst[..dst_written], b"ABCDEFGHABCDZFGH");
}

#[test]
fn test_opcode_sml_m_and_lrg_m() {
    // SmlM: 0xF0 | M (M in 1..15)
    // LrgM: 0xF0, followed by (M - 16)
    let mut stream = Vec::new();
    // Seed 10 bytes and set d_prev=4 via SmlD
    stream.push(0xEA);
    stream.extend_from_slice(b"0123456789");
    // SmlD D=4, M=4, L=0 => opc=0x08, D=4
    stream.push(0x08);
    stream.push(0x04);
    // SmlM M=6 => opc = 0xF6
    stream.push(0xF6);
    // LrgM M=20 => opc = 0xF0, byte2 = 4 (20 - 16)
    stream.push(0xF0);
    stream.push(0x04);
    append_eos(&mut stream);

    let mut dst = vec![0u8; 128];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode SmlM & LrgM");

    // 10 + 4 + 6 + 20 = 40 bytes
    assert_eq!(dst_written, 40);
    assert_eq!(decoder.d_prev, 4);
}

#[test]
fn test_opcode_nop_0e_and_16() {
    let mut stream = Vec::new();
    // Insert NOPs before and after literals
    stream.push(0x0E); // NOP
    stream.push(0x16); // NOP
    stream.push(0xE5);
    stream.extend_from_slice(b"Hello");
    stream.push(0x0E); // NOP
    stream.push(0x16); // NOP
    append_eos(&mut stream);

    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode with NOPs");

    assert_eq!(dst_written, 5);
    assert_eq!(&dst[..5], b"Hello");
}

#[test]
fn test_opcode_table_completeness() {
    assert_eq!(LZVN_OPCODE_TABLE.len(), 256);
    assert_eq!(LZVN_OPCODE_TABLE[0x06], LzvnOpcodeKind::Eos);
    assert_eq!(LZVN_OPCODE_TABLE[0x0E], LzvnOpcodeKind::Nop);
    assert_eq!(LZVN_OPCODE_TABLE[0x16], LzvnOpcodeKind::Nop);
    assert_eq!(LZVN_OPCODE_TABLE[0xE0], LzvnOpcodeKind::LrgL);
    assert_eq!(LZVN_OPCODE_TABLE[0xE1], LzvnOpcodeKind::SmlL);
    assert_eq!(LZVN_OPCODE_TABLE[0xF0], LzvnOpcodeKind::LrgM);
    assert_eq!(LZVN_OPCODE_TABLE[0xF1], LzvnOpcodeKind::SmlM);
    assert_eq!(LZVN_OPCODE_TABLE[0xA0], LzvnOpcodeKind::MedD);
    assert_eq!(LZVN_OPCODE_TABLE[0xBF], LzvnOpcodeKind::MedD);
    assert_eq!(LZVN_OPCODE_TABLE[0x70], LzvnOpcodeKind::Udef);
    assert_eq!(LZVN_OPCODE_TABLE[0xD0], LzvnOpcodeKind::Udef);
}

// MARK: - Section 2: Distance Reuse Chain Verification

#[test]
fn test_d_prev_persistence_chain() {
    let mut stream = Vec::new();
    // Seed "12345678"
    stream.push(0xE8);
    stream.extend_from_slice(b"12345678");

    // 1. SmlD: establish D = 8, M = 4, L = 0 (opc = 0x08, D = 8)
    stream.push(0x08);
    stream.push(0x08);

    // 2. PreD: L = 1, M = 3, uses D = 8 (opc = 0x46, lit = 'A')
    stream.push(0x46);
    stream.push(b'A');

    // 3. SmlM: M = 4, uses D = 8 (opc = 0xF4)
    stream.push(0xF4);

    // 4. LrgM: M = 18, uses D = 8 (opc = 0xF0, 18 - 16 = 2)
    stream.push(0xF0);
    stream.push(0x02);

    append_eos(&mut stream);

    let mut dst = vec![0u8; 128];
    let mut decoder = LzvnDecoder::new();
    let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode d_prev chain");

    assert_eq!(decoder.d_prev, 8);
    assert_eq!(dst_written, 8 + 4 + 1 + 3 + 4 + 18);
}

// MARK: - Section 3: Overlapping Match Expansion (D = 1..7, M = 4..64)

#[test]
fn test_overlap_rle_splat_d1() {
    for m in 4..=64 {
        // Seed single byte 'K' and SmlD: D=1, M=3 (opc=0x00, D=1)
        let mut stream = vec![0xE1, b'K', 0x00, 0x01];

        let rem = m - 3;
        if rem > 0 {
            if rem <= 15 {
                stream.push(0xF0 | (rem as u8));
            } else {
                stream.push(0xF0);
                stream.push((rem - 16) as u8);
            }
        }
        append_eos(&mut stream);

        let mut dst = vec![0u8; 128];
        let mut decoder = LzvnDecoder::new();
        let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode D=1 RLE");

        assert_eq!(dst_written, 1 + m);
        let expected = vec![b'K'; 1 + m];
        assert_eq!(&dst[..dst_written], &expected[..]);
    }
}

#[test]
fn test_overlap_word16_repeat_d2() {
    for m in 4..=64 {
        let mut stream = Vec::new();
        stream.push(0xE2);
        stream.extend_from_slice(b"AB");

        // SmlD: D=2, M=3 (opc=0x00, D=2)
        stream.push(0x00);
        stream.push(0x02);

        let rem = m - 3;
        if rem > 0 {
            if rem <= 15 {
                stream.push(0xF0 | (rem as u8));
            } else {
                stream.push(0xF0);
                stream.push((rem - 16) as u8);
            }
        }
        append_eos(&mut stream);

        let mut dst = vec![0u8; 128];
        let mut decoder = LzvnDecoder::new();
        let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode D=2 repeat");

        assert_eq!(dst_written, 2 + m);
        for i in 0..dst_written {
            let expected_byte = if (i & 1) == 0 { b'A' } else { b'B' };
            assert_eq!(dst[i], expected_byte, "Mismatch at pos {i} for M={m}");
        }
    }
}

#[test]
fn test_overlap_triplet_repeat_d3() {
    for m in 4..=64 {
        let mut stream = Vec::new();
        stream.push(0xE3);
        stream.extend_from_slice(b"XYZ");

        // SmlD: D=3, M=3 (opc=0x00, D=3)
        stream.push(0x00);
        stream.push(0x03);

        let rem = m - 3;
        if rem > 0 {
            if rem <= 15 {
                stream.push(0xF0 | (rem as u8));
            } else {
                stream.push(0xF0);
                stream.push((rem - 16) as u8);
            }
        }
        append_eos(&mut stream);

        let mut dst = vec![0u8; 128];
        let mut decoder = LzvnDecoder::new();
        let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode D=3 repeat");

        assert_eq!(dst_written, 3 + m);
        let pattern = *b"XYZ";
        for i in 0..dst_written {
            assert_eq!(dst[i], pattern[i % 3], "Mismatch at pos {i} for M={m}");
        }
    }
}

#[test]
fn test_overlap_dword32_repeat_d4() {
    for m in 4..=64 {
        let mut stream = Vec::new();
        stream.push(0xE4);
        stream.extend_from_slice(b"1234");

        // SmlD: D=4, M=3 (opc=0x00, D=4)
        stream.push(0x00);
        stream.push(0x04);

        let rem = m - 3;
        if rem > 0 {
            if rem <= 15 {
                stream.push(0xF0 | (rem as u8));
            } else {
                stream.push(0xF0);
                stream.push((rem - 16) as u8);
            }
        }
        append_eos(&mut stream);

        let mut dst = vec![0u8; 128];
        let mut decoder = LzvnDecoder::new();
        let (_, dst_written) = decoder.decode(&stream, &mut dst).expect("decode D=4 repeat");

        assert_eq!(dst_written, 4 + m);
        let pattern = *b"1234";
        for i in 0..dst_written {
            assert_eq!(dst[i], pattern[i % 4], "Mismatch at pos {i} for M={m}");
        }
    }
}

#[test]
fn test_overlap_arbitrary_d5_d6_d7() {
    for d in 5..=7 {
        for m in 4..=64 {
            let mut stream = Vec::new();
            let seed: Vec<u8> = (0..d).map(|i| b'a' + (i as u8)).collect();
            stream.push(0xE0 | (d as u8));
            stream.extend_from_slice(&seed);

            // SmlD: D=d, M=3
            stream.push(0x00);
            stream.push(d as u8);

            let rem = m - 3;
            if rem > 0 {
                if rem <= 15 {
                    stream.push(0xF0 | (rem as u8));
                } else {
                    stream.push(0xF0);
                    stream.push((rem - 16) as u8);
                }
            }
            append_eos(&mut stream);

            let mut dst = vec![0u8; 128];
            let mut decoder = LzvnDecoder::new();
            let (_, dst_written) =
                decoder.decode(&stream, &mut dst).expect("decode D=5..7 repeat");

            assert_eq!(dst_written, d + m);
            for i in 0..dst_written {
                assert_eq!(dst[i], seed[i % d], "Mismatch at pos {i} for D={d}, M={m}");
            }
        }
    }
}

#[test]
fn test_wild_copy_fast_path_d8_plus() {
    // Test D >= 8 with various match lengths (both with and without destination tail padding)
    let mut stream = Vec::new();
    let seed = b"0123456789ABCDEF"; // 16 bytes
    stream.push(0xE0);
    stream.push(0); // LrgL 16 bytes
    stream.extend_from_slice(seed);

    // SmlD: D=16, M=10 (x=7 => opc=0x38, D=16)
    stream.push(0x38);
    stream.push(0x10);
    // SmlM: M=15
    stream.push(0xFF);
    // LrgM: M=64 (64 - 16 = 48 => 0x30)
    stream.push(0xF0);
    stream.push(0x30);
    append_eos(&mut stream);

    let total_len = 16 + 10 + 15 + 64; // 105 bytes

    // 1. With ample destination buffer (triggers 8-byte Wild Copy)
    let mut dst_ample = vec![0u8; total_len + 64];
    let mut dec1 = LzvnDecoder::new();
    let (_, w1) = dec1.decode(&stream, &mut dst_ample).expect("decode ample dst");
    assert_eq!(w1, total_len);

    // 2. With exact destination buffer (exercises safety fallback near buffer end)
    let mut dst_exact = vec![0u8; total_len];
    let mut dec2 = LzvnDecoder::new();
    let (_, w2) = dec2.decode(&stream, &mut dst_exact).expect("decode exact dst");
    assert_eq!(w2, total_len);

    assert_eq!(&dst_ample[..total_len], &dst_exact[..total_len]);
}

// MARK: - Section 4: 100% Roundtrip Fidelity with C Encoder

#[test]
fn test_c_lzvn_encode_roundtrip_text_corpora() {
    let test_texts = [
        "The quick brown fox jumps over the lazy dog.",
        "To be, or not to be, that is the question: Whether 'tis nobler in the mind to suffer...",
        "Apple Silicon M-Series processors deliver extreme memory bandwidth and energy efficiency.",
        "TTZip High-Performance Archiving and Compression Engine with 100% Pure Safe Rust Architecture.",
    ];

    for text in test_texts {
        let original = text.as_bytes();
        let bound = lzvn_compress_bound(original.len());
        let mut compressed = vec![0u8; bound];

        let compressed_size = lzvn_compress(original, &mut compressed).expect("C lzvn_compress");
        compressed.truncate(compressed_size);

        let decompressed =
            lzvn_decompress_to_vec_pure_rust(&compressed, original.len()).expect("pure rust decompress");

        assert_eq!(decompressed, original);
        assert!(lzvn_validate(&compressed));
    }
}

#[test]
fn test_c_lzvn_encode_roundtrip_binary_patterns() {
    let mut original = Vec::new();
    for i in 0..4096 {
        original.push((i ^ (i >> 3) ^ (i >> 7)) as u8);
    }

    let bound = lzvn_compress_bound(original.len());
    let mut compressed = vec![0u8; bound];
    let compressed_size = lzvn_compress(&original, &mut compressed).expect("lzvn_compress binary");
    compressed.truncate(compressed_size);

    let decompressed =
        lzvn_decompress_to_vec_pure_rust(&compressed, original.len()).expect("decompress binary");

    assert_eq!(decompressed, original);
}

#[test]
fn test_c_lzvn_encode_roundtrip_edge_buffer_sizes() {
    let sizes = [1, 2, 3, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 128, 255, 256, 512, 1024, 65536];

    for &size in &sizes {
        let original: Vec<u8> = (0..size).map(|i| ((i * 37) & 0xFF) as u8).collect();
        let bound = lzvn_compress_bound(original.len());
        let mut compressed = vec![0u8; bound];

        let compressed_size = lzvn_compress(&original, &mut compressed).expect("compress edge size");
        compressed.truncate(compressed_size);

        let decompressed =
            lzvn_decompress_to_vec_pure_rust(&compressed, original.len()).expect("decompress edge size");

        assert_eq!(decompressed, original, "Mismatch for buffer size {size}");
    }
}

// MARK: - Section 5: Robust Defensive Error Handling & 0-Panic Guarantees

#[test]
fn test_error_distance_zero_rejected() {
    // SmlD with D = 0: strictly invalid
    let mut stream = Vec::new();
    stream.push(0xE4);
    stream.extend_from_slice(b"TEST");
    stream.push(0x00); // SmlD L=0, M=3, D_high=0
    stream.push(0x00); // D_low=0 => D = 0
    append_eos(&mut stream);

    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_error_distance_underflow_rejected() {
    // SmlD with D = 10 when dst_pos is only 4
    let mut stream = Vec::new();
    stream.push(0xE4);
    stream.extend_from_slice(b"TEST");
    stream.push(0x00);
    stream.push(0x0A); // D = 10 > dst_pos (4)
    append_eos(&mut stream);

    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_error_truncated_opcode_stream() {
    // SmlD requires 2 bytes + literals, but input ends immediately
    let stream = vec![0x18]; // missing byte 2
    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_error_truncated_literal_payload() {
    // SmlL claims 5 literals, but only 2 exist before EOF
    let stream = vec![0xE5, b'A', b'B'];
    let mut dst = vec![0u8; 32];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_error_missing_eos_token() {
    let mut stream = Vec::new();
    stream.push(0xE4);
    stream.extend_from_slice(b"ABCD");
    // Omit EOS marker

    let mut dst = vec![0u8; 32];
    let res = lzvn_decompress_pure_rust(&stream, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_error_destination_capacity_exceeded() {
    let mut stream = Vec::new();
    stream.push(0xE8);
    stream.extend_from_slice(b"12345678");
    append_eos(&mut stream);

    // Destination only has 4 bytes capacity
    let mut dst = vec![0u8; 4];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrExtractionFailed));
}

#[test]
fn test_error_undefined_opcodes_rejected() {
    let invalid_opcodes = [0x1E, 0x26, 0x2E, 0x36, 0x3E, 0x70, 0x75, 0x7F, 0xD0, 0xD7, 0xDF];

    for &opc in &invalid_opcodes {
        let stream = vec![opc, 0x00, 0x00, 0x00];
        let mut dst = vec![0u8; 32];
        let mut decoder = LzvnDecoder::new();
        let res = decoder.decode(&stream, &mut dst);
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader), "Opcode {opc:#04X} should be rejected");
    }
}

#[test]
fn test_fuzz_random_garbage_zero_panic() {
    let mut lcg_state: u64 = 0xDEAD_BEEF_CAFE_BABE;

    for len in [1, 2, 3, 7, 8, 9, 16, 32, 64, 128, 256, 512] {
        for _ in 0..50 {
            let mut garbage = vec![0u8; len];
            for b in garbage.iter_mut() {
                lcg_state = lcg_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                *b = (lcg_state >> 32) as u8;
            }

            let mut dst = vec![0u8; len * 4 + 64];
            let mut decoder = LzvnDecoder::new();
            // Must return Ok or Err, absolutely never panic!
            let _ = decoder.decode(&garbage, &mut dst);
            let _ = lzvn_validate(&garbage);
        }
    }
}
