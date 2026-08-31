// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for RFC 7932 2-Level Canonical Huffman DTable and zero-branch lookup engine.

use std::time::Instant;
use ttzip_engine::codecs::brotli::{
    BrotliBitReader, BrotliError, HuffmanCode, HuffmanTable, BROTLI_HUFFMAN_MAX_CODE_LENGTH,
    HUFFMAN_TABLE_BITS,
};

/// Helper to pack LSB-first bit sequences into a byte vector.
fn pack_lsb_bits(chunks: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u64;
    let mut count = 0u32;
    for &(val, len) in chunks {
        let mask = if len >= 32 {
            0xFFFF_FFFF
        } else {
            (1u32 << len) - 1
        };
        acc |= ((val & mask) as u64) << count;
        count += len;
        while count >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            count -= 8;
        }
    }
    if count > 0 {
        out.push((acc & 0xFF) as u8);
    }
    // Pad with extra bytes for safe peek_bits(15)
    out.extend_from_slice(&[0u8; 8]);
    out
}

#[test]
fn test_huffman_code_struct_layout_and_constants() {
    assert_eq!(BROTLI_HUFFMAN_MAX_CODE_LENGTH, 15);
    assert_eq!(HUFFMAN_TABLE_BITS, 8);

    let code = HuffmanCode::new(5, 42);
    assert_eq!(code.bits, 5);
    assert_eq!(code.value, 42);

    let default_code = HuffmanCode::default();
    assert_eq!(default_code.bits, 0);
    assert_eq!(default_code.value, 0);
}

#[test]
fn test_huffman_simple_1_symbol_fidelity() {
    let symbols = [42u16];
    let table = HuffmanTable::build_simple(&symbols, &[0]).expect("simple 1 symbol build");
    assert_eq!(table.total_entries(), 256);

    // Single-symbol tree requires 0 bits to decode.
    let data = [0xAA, 0xBB];
    let mut br = BrotliBitReader::new(&data);
    let unconsumed_before = br.unconsumed_bits();
    let decoded = table.decode_symbol(&mut br).expect("decode symbol");
    assert_eq!(decoded, 42);
    // Exactly 0 bits consumed from accumulator
    assert_eq!(br.unconsumed_bits(), unconsumed_before);
    assert_eq!(br.unload(), 0);
}

#[test]
fn test_huffman_simple_2_symbols_fidelity() {
    let symbols = [100u16, 200u16];
    let table = HuffmanTable::build_simple(&symbols, &[1, 1]).expect("simple 2 symbols build");
    assert_eq!(table.total_entries(), 256);

    // Symbol 100 has bit 0; Symbol 200 has bit 1.
    // Sequence: [100, 200, 100, 100, 200] -> bits: 0, 1, 0, 0, 1 -> 0b10010 = 0x12
    let sequence = [
        (0u32, 1u32),
        (1u32, 1u32),
        (0u32, 1u32),
        (0u32, 1u32),
        (1u32, 1u32),
    ];
    let stream = pack_lsb_bits(&sequence);
    let mut br = BrotliBitReader::new(&stream);

    assert_eq!(table.decode_symbol(&mut br).expect("s0"), 100);
    assert_eq!(table.decode_symbol(&mut br).expect("s1"), 200);
    assert_eq!(table.decode_symbol(&mut br).expect("s2"), 100);
    assert_eq!(table.decode_symbol(&mut br).expect("s3"), 100);
    assert_eq!(table.decode_symbol(&mut br).expect("s4"), 200);
}

#[test]
fn test_huffman_simple_3_symbols_fidelity() {
    let symbols = [10u16, 20u16, 30u16];
    let table = HuffmanTable::build_simple(&symbols, &[1, 2, 2]).expect("simple 3 symbols build");

    // Symbol 10: bit 0 (len 1)
    // Symbol 20: bits 01 (len 2)
    // Symbol 30: bits 11 (len 2)
    let sequence = [
        (0b0u32, 1u32),  // 10
        (0b01u32, 2u32), // 20
        (0b11u32, 2u32), // 30
        (0b0u32, 1u32),  // 10
        (0b11u32, 2u32), // 30
    ];
    let stream = pack_lsb_bits(&sequence);
    let mut br = BrotliBitReader::new(&stream);

    assert_eq!(table.decode_symbol(&mut br).expect("s0"), 10);
    assert_eq!(table.decode_symbol(&mut br).expect("s1"), 20);
    assert_eq!(table.decode_symbol(&mut br).expect("s2"), 30);
    assert_eq!(table.decode_symbol(&mut br).expect("s3"), 10);
    assert_eq!(table.decode_symbol(&mut br).expect("s4"), 30);
}

#[test]
fn test_huffman_simple_4_symbols_type_a_fidelity() {
    // Type A: [2, 2, 2, 2]
    let symbols = [1u16, 2u16, 3u16, 4u16];
    let table =
        HuffmanTable::build_simple(&symbols, &[2, 2, 2, 2]).expect("simple 4 symbols type A");

    // LSB patterns for sorted symbols [1, 2, 3, 4]:
    // sym 1: 00 (0)
    // sym 2: 10 (2)
    // sym 3: 01 (1)
    // sym 4: 11 (3)
    let sequence = [
        (0b00u32, 2u32), // 1
        (0b10u32, 2u32), // 2
        (0b01u32, 2u32), // 3
        (0b11u32, 2u32), // 4
        (0b10u32, 2u32), // 2
    ];
    let stream = pack_lsb_bits(&sequence);
    let mut br = BrotliBitReader::new(&stream);

    assert_eq!(table.decode_symbol(&mut br).expect("s0"), 1);
    assert_eq!(table.decode_symbol(&mut br).expect("s1"), 2);
    assert_eq!(table.decode_symbol(&mut br).expect("s2"), 3);
    assert_eq!(table.decode_symbol(&mut br).expect("s3"), 4);
    assert_eq!(table.decode_symbol(&mut br).expect("s4"), 2);
}

#[test]
fn test_huffman_simple_4_symbols_type_b_fidelity() {
    // Type B: [1, 2, 3, 3]
    let symbols = [100u16, 200u16, 300u16, 400u16];
    let table =
        HuffmanTable::build_simple(&symbols, &[1, 2, 3, 3]).expect("simple 4 symbols type B");

    // sym 100: bit 0 (len 1)
    // sym 200: bits 01 (len 2)
    // sym 300: bits 011 (len 3, value 3)
    // sym 400: bits 111 (len 3, value 7)
    let sequence = [
        (0b0u32, 1u32),   // 100
        (0b01u32, 2u32),  // 200
        (0b011u32, 3u32), // 300
        (0b111u32, 3u32), // 400
        (0b0u32, 1u32),   // 100
    ];
    let stream = pack_lsb_bits(&sequence);
    let mut br = BrotliBitReader::new(&stream);

    assert_eq!(table.decode_symbol(&mut br).expect("s0"), 100);
    assert_eq!(table.decode_symbol(&mut br).expect("s1"), 200);
    assert_eq!(table.decode_symbol(&mut br).expect("s2"), 300);
    assert_eq!(table.decode_symbol(&mut br).expect("s3"), 400);
    assert_eq!(table.decode_symbol(&mut br).expect("s4"), 100);
}

#[test]
fn test_huffman_simple_duplicate_symbols_interception() {
    // 2 symbols duplicate
    let err2 = HuffmanTable::build_simple(&[5, 5], &[1, 1]).expect_err("must reject duplicate");
    assert_eq!(err2, BrotliError::DuplicateSymbol);

    // 3 symbols duplicate
    let err3 =
        HuffmanTable::build_simple(&[10, 20, 10], &[1, 2, 2]).expect_err("must reject duplicate");
    assert_eq!(err3, BrotliError::DuplicateSymbol);

    // 4 symbols duplicate
    let err4 = HuffmanTable::build_simple(&[1, 2, 3, 2], &[2, 2, 2, 2])
        .expect_err("must reject duplicate");
    assert_eq!(err4, BrotliError::DuplicateSymbol);
}

#[test]
fn test_huffman_15_bit_deep_tree_subtable_routing() {
    // 16 symbols with lengths: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15]
    // Kraft sum: 2^-1 + 2^-2 + ... + 2^-14 + 2 * 2^-15 = 1.0 (Kraft Space == 0)
    let code_lengths: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15];
    let table =
        HuffmanTable::build(&code_lengths, 16).expect("build 15-bit deep canonical Huffman tree");

    // Table must contain 2nd-level sub-tables (total_entries > 256)
    assert!(
        table.total_entries() > 256,
        "Deep tree must allocate secondary sub-tables, got {}",
        table.total_entries()
    );

    // Construct canonical prefix codes:
    // Sym 0 (len 1):  MSB code 0               -> LSB pattern: 0 (1 bit)
    // Sym 1 (len 2):  MSB code 10 (2)          -> LSB pattern: 01 (1, 2 bits)
    // Sym 2 (len 3):  MSB code 110 (6)         -> LSB pattern: 011 (3, 3 bits)
    // Sym 3 (len 4):  MSB code 1110 (14)       -> LSB pattern: 0111 (7, 4 bits)
    // Sym 4 (len 5):  MSB code 11110 (30)      -> LSB pattern: 01111 (15, 5 bits)
    // Sym 5 (len 6):  MSB code 111110 (62)     -> LSB pattern: 011111 (31, 6 bits)
    // Sym 6 (len 7):  MSB code 1111110 (126)   -> LSB pattern: 0111111 (63, 7 bits)
    // Sym 7 (len 8):  MSB code 11111110 (254)  -> LSB pattern: 01111111 (127, 8 bits)
    // Sym 8 (len 9):  MSB code 111111110 (510) -> LSB pattern: 011111111 (255, 9 bits)
    // Sym 9 (len 10): MSB code 1111111110      -> LSB pattern: 511 (10 bits)
    // Sym 10 (len 11):                         -> LSB pattern: 1023 (11 bits)
    // Sym 11 (len 12):                         -> LSB pattern: 2047 (12 bits)
    // Sym 12 (len 13):                         -> LSB pattern: 4095 (13 bits)
    // Sym 13 (len 14):                         -> LSB pattern: 8191 (14 bits)
    // Sym 14 (len 15): MSB code 111111111111110 -> LSB pattern: 16383 (15 bits)
    // Sym 15 (len 15): MSB code 111111111111111 -> LSB pattern: 32767 (15 bits)

    let mut test_symbols = Vec::new();
    for sym in 0..16 {
        let (pattern, len) = match sym {
            0..=13 => ((1u32 << sym) - 1, sym as u32 + 1),
            14 => ((1u32 << 14) - 1, 15),
            15 => ((1u32 << 15) - 1, 15),
            _ => unreachable!(),
        };
        test_symbols.push((pattern, len));
    }

    let stream = pack_lsb_bits(&test_symbols);
    let mut br = BrotliBitReader::new(&stream);

    for expected_sym in 0..16u16 {
        let decoded = table
            .decode_symbol(&mut br)
            .unwrap_or_else(|e| panic!("Failed to decode symbol {}: {:?}", expected_sym, e));
        assert_eq!(
            decoded, expected_sym,
            "Decoded symbol mismatch at index {}",
            expected_sym
        );
    }
}

#[test]
fn test_huffman_kraft_inequality_violations() {
    // 1. Over-subscribed tree: sum of 2^-len = 0.5 + 0.5 + 0.25 = 1.25 > 1.0
    let oversubscribed = [1u8, 1, 2];
    let err1 =
        HuffmanTable::build(&oversubscribed, 3).expect_err("must reject oversubscribed tree");
    assert_eq!(err1, BrotliError::HuffmanSpaceViolation);

    // 2. Under-subscribed tree with >1 symbol: sum of 2^-len = 0.25 + 0.25 = 0.5 < 1.0
    let undersubscribed = [2u8, 2];
    let err2 =
        HuffmanTable::build(&undersubscribed, 2).expect_err("must reject undersubscribed tree");
    assert_eq!(err2, BrotliError::HuffmanSpaceViolation);

    // 3. Zero symbols present
    let empty_tree = [0u8, 0, 0];
    let err3 = HuffmanTable::build(&empty_tree, 3).expect_err("must reject empty tree");
    assert_eq!(err3, BrotliError::HuffmanSpaceViolation);

    // 4. Code length exceeds maximum allowable 15
    let out_of_range = [16u8, 1];
    let err4 = HuffmanTable::build(&out_of_range, 2).expect_err("must reject length > 15");
    assert_eq!(err4, BrotliError::HuffmanSpaceViolation);

    // 5. Valid single-symbol tree exception
    let single_sym = [1u8];
    let table_single = HuffmanTable::build(&single_sym, 1).expect("single symbol is valid");
    assert_eq!(table_single.total_entries(), 256);
}

#[test]
fn test_huffman_decoding_throughput_gate() {
    // Build a representative 8-symbol Huffman tree
    // Code lengths: [2, 2, 3, 3, 3, 4, 5, 5]
    // 2^-2 + 2^-2 + 3 * 2^-3 + 2^-4 + 2 * 2^-5 = 0.25 + 0.25 + 0.375 + 0.0625 + 0.0625 = 1.0 (Kraft == 0)
    let code_lengths = [2u8, 2, 3, 3, 3, 4, 5, 5];
    let table = HuffmanTable::build(&code_lengths, 8).expect("build table");

    // Canonical bit patterns for symbols 0..7:
    // Sym 0: len 2, MSB 00 -> LSB 00 (0)
    // Sym 1: len 2, MSB 01 -> LSB 10 (2)
    // Sym 2: len 3, MSB 100 -> LSB 001 (1)
    // Sym 3: len 3, MSB 101 -> LSB 101 (5)
    // Sym 4: len 3, MSB 110 -> LSB 011 (3)
    // Sym 5: len 4, MSB 1110 -> LSB 0111 (7)
    // Sym 6: len 5, MSB 11110 -> LSB 01111 (15)
    // Sym 7: len 5, MSB 11111 -> LSB 11111 (31)
    let sym_patterns: [(u32, u32); 8] = [
        (0b00, 2),
        (0b10, 2),
        (0b001, 3),
        (0b101, 3),
        (0b011, 3),
        (0b0111, 4),
        (0b01111, 5),
        (0b11111, 5),
    ];

    const NUM_SYMBOLS: usize = 1_000_000;
    let mut sequence = Vec::with_capacity(NUM_SYMBOLS);
    for i in 0..NUM_SYMBOLS {
        let sym_idx = i % 8;
        sequence.push(sym_patterns[sym_idx]);
    }

    let stream = pack_lsb_bits(&sequence);

    // Warmup
    let mut br_warmup = BrotliBitReader::new(&stream);
    for _ in 0..10_000 {
        let _ = table.decode_symbol(&mut br_warmup);
    }

    // Measure throughput over 1,000,000 symbols
    let mut br = BrotliBitReader::new(&stream);
    let start = Instant::now();
    let mut decoded_count = 0usize;

    for i in 0..NUM_SYMBOLS {
        let expected = (i % 8) as u16;
        let sym = table.decode_symbol(&mut br).expect("decode symbol in loop");
        assert_eq!(sym, expected);
        decoded_count += 1;
    }

    let elapsed = start.elapsed();
    let symbols_per_sec = (decoded_count as f64) / elapsed.as_secs_f64();

    println!(
        "Brotli Huffman DTable Throughput: {:.2} Million Symbols/sec ({:?})",
        symbols_per_sec / 1_000_000.0,
        elapsed
    );

    // Hard performance gate: must exceed 50 Million symbols/sec
    assert!(
        symbols_per_sec >= 50_000_000.0,
        "Huffman decode throughput gate violated: expected >= 50 Mops/s, got {:.2} Mops/s",
        symbols_per_sec / 1_000_000.0
    );
}
