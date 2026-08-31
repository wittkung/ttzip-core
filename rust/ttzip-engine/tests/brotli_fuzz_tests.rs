// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed Brotli Fault-Injection Fuzzing Harness & Jitter Streaming Suite.
//!
//! Implements a 16-dimensional fault injection test matrix and dual-path
//! stream jitter perturbation suite aligned with RFC 7932, RFC 9841, and
//! Google Brotli canonical `decode_fuzzer.c`:
//! 1. Truncated Stream Header (1..10 bits & sub-byte cuts).
//! 2. Corrupted WBITS Prefix & Unauthorized Large Window.
//! 3. Kraft Inequality Huffman Space Violations (Over/Under-subscribed).
//! 4. Simple Prefix Code Duplicate Symbol Injections.
//! 5. Exuberant Nibbles & Exuberant Metadata Bytes.
//! 6. Non-Zero Padding Bit Injections.
//! 7. Out-of-Bounds Backward Distance & Ring Buffer Underflow.
//! 8. Illegal Static Transform Index & Buffer Overflow.
//! 9. Malformed Multi-Byte UTF-8 in Word Transforms.
//! 10. Metadata Block Reserved Bit Violations.
//! 11. Pathological All-0xFF and All-0x00 Bomb Sequences.
//! 12. 1% Pseudorandom Mutation Fuzz Stress (1,000 Iterations).
//! 13. Slow Jitter Streaming Push (Tail Byte Low 3-bit Driven).
//! 14. Asymmetric Chunking & Micro-Buffer Streaming.
//! 15. Single-Symbol Degenerate Trees & Deep 15-Bit Cascades.
//! 16. Multi-Seed Random Fuzz Matrix Stress.

use std::io::{Cursor, Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::codecs::brotli::{
    brotli_compress, brotli_compress_bound, brotli_compress_to_vec, brotli_decompress,
    shift_utf8, to_uppercase_utf8, transform_dictionary_word,
    BrotliBitReader, BrotliCompressorWriter, BrotliConfig, BrotliDecoderRingBuffer,
    BrotliDecompressorReader, BrotliError, BrotliWindow, HuffmanTable, MetaBlockHeader,
    BROTLI_LARGE_MAX_WINDOW_BITS, BROTLI_MIN_WINDOW_BITS,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Deterministic Knuth Multiplicative PRNG (FUZ_rand)

/// Deterministic Knuth multiplicative hash PRNG from canonical `decode_fuzzer.c`.
///
/// Formula: `state = (state * 2654435761U) + 2246822519U; return state >> 13;`
#[derive(Debug, Clone)]
pub struct FuzRand {
    seed: u32,
}

impl FuzRand {
    /// Constructs a new `FuzRand` initialized with `seed`.
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Computes the next 32-bit pseudo-random value matching `FUZ_rand()`.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(2_654_435_761)
            .wrapping_add(2_246_822_519);
        self.seed >> 13
    }

    /// Returns a pseudo-random integer in the closed interval `[min, max]`.
    #[inline]
    pub fn rand_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u32() % span)
    }

    /// Returns a pseudo-random `usize` in half-open interval `[0, bound)`.
    #[inline]
    pub fn rand_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u32() as usize) % bound
        }
    }

    /// Returns a pseudo-random byte `u8`.
    #[inline]
    pub fn rand_u8(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }

    /// Generates a pseudo-random payload with target compressibility.
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

/// Helper function to pack LSB-first bit sequences into a byte vector.
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
    out
}

// MARK: - Dimension 1: Truncated Stream Header & Sub-Byte Sweeps

#[test]
fn test_brotli_dim01_truncated_stream_header_and_sub_byte_interception() {
    for bits in 1..=10 {
        let pattern = vec![(0b1010_1010u32, bits)];
        let stream = pack_lsb_bits(&pattern);
        let mut br = BrotliBitReader::new(&stream);
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            BrotliWindow::parse_window_bits(&mut br, true)
        }));
        assert!(unwind_res.is_ok(), "Header truncated at {bits} bits panicked!");
        let res = unwind_res.unwrap();
        if bits < 1 {
            assert_eq!(res, Err(BrotliError::UnexpectedEof));
        }
    }

    let payload = b"TTZip Brotli stream truncation sweep across all sub-byte offsets 2026.";
    let mut comp = vec![0u8; brotli_compress_bound(payload.len())];
    let c_len = brotli_compress(payload, &mut comp, 6, 22).expect("compress");
    let valid_stream = &comp[..c_len];

    let mut dst = vec![0u8; payload.len() + 128];
    for trunc_len in 0..c_len {
        let slice = &valid_stream[..trunc_len];
        let unwind_res = catch_unwind(AssertUnwindSafe(|| brotli_decompress(slice, &mut dst)));
        assert!(unwind_res.is_ok(), "Truncation at offset {trunc_len}/{c_len} panicked!");
        let res = unwind_res.unwrap();
        if trunc_len == 0 {
            assert_eq!(res, Ok(0));
        } else {
            assert!(res.is_err(), "Truncated slice at {trunc_len} must fail");
            assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
        }
    }
}

// MARK: - Dimension 2: Corrupted WBITS Prefix & Unauthorized Large Window

#[test]
fn test_brotli_dim02_corrupted_wbits_and_unauthorized_large_window() {
    let large_win_pattern = [(1, 1), (0, 3), (1, 3), (0, 1), (26, 6)];
    let stream = pack_lsb_bits(&large_win_pattern);
    let mut br = BrotliBitReader::new(&stream);
    let err = BrotliWindow::parse_window_bits(&mut br, false)
        .expect_err("Unauthorized large window must be rejected");
    assert_eq!(err, BrotliError::InvalidWindowBits(1));

    let corrupted_8th_bit = [(1, 1), (0, 3), (1, 3), (1, 1), (26, 6)];
    let stream_err8 = pack_lsb_bits(&corrupted_8th_bit);
    let mut br_err8 = BrotliBitReader::new(&stream_err8);
    let err8 = BrotliWindow::parse_window_bits(&mut br_err8, true)
        .expect_err("Non-zero 8th bit must be rejected");
    assert_eq!(err8, BrotliError::InvalidWindowBits(0));

    let invalid_exponents: &[u8] = &[0, 1, 9, 31, 32, 255];
    for &exp in invalid_exponents {
        assert_eq!(
            BrotliWindow::new(exp, true),
            Err(BrotliError::InvalidWindowBits(exp))
        );
    }

    for exp in BROTLI_MIN_WINDOW_BITS..=BROTLI_LARGE_MAX_WINDOW_BITS {
        let win = BrotliWindow::new(exp, true).expect("valid window exponent");
        assert_eq!(win.window_bits, exp);
        assert_eq!(win.max_distance, (1usize << exp) - 16);
    }
}

// MARK: - Dimension 3: Kraft Inequality Huffman Space Violations

#[test]
fn test_brotli_dim03_kraft_inequality_huffman_space_violations() {
    let oversubscribed = [1u8, 1, 2];
    let err_over =
        HuffmanTable::build(&oversubscribed, 3).expect_err("Over-subscribed tree must fail");
    assert_eq!(err_over, BrotliError::HuffmanSpaceViolation);

    let undersubscribed = [2u8, 2];
    let err_under =
        HuffmanTable::build(&undersubscribed, 2).expect_err("Under-subscribed tree must fail");
    assert_eq!(err_under, BrotliError::HuffmanSpaceViolation);

    let empty_tree = [0u8, 0, 0];
    let err_empty =
        HuffmanTable::build(&empty_tree, 3).expect_err("Empty Huffman tree must fail");
    assert_eq!(err_empty, BrotliError::HuffmanSpaceViolation);

    let length_exceeded = [16u8, 1];
    let err_len =
        HuffmanTable::build(&length_exceeded, 2).expect_err("Code length > 15 must fail");
    assert_eq!(err_len, BrotliError::HuffmanSpaceViolation);

    let single_sym = [1u8];
    let table_single = HuffmanTable::build(&single_sym, 1).expect("single symbol is valid");
    assert_eq!(table_single.total_entries(), 256);
}

// MARK: - Dimension 4: Simple Prefix Code Duplicate Symbol Injections

#[test]
fn test_brotli_dim04_simple_prefix_code_duplicate_symbol_injections() {
    let err2 = HuffmanTable::build_simple(&[7, 7], &[1, 1]).expect_err("duplicate 2 symbols");
    assert_eq!(err2, BrotliError::DuplicateSymbol);

    let err3 =
        HuffmanTable::build_simple(&[10, 20, 10], &[1, 2, 2]).expect_err("duplicate 3 symbols");
    assert_eq!(err3, BrotliError::DuplicateSymbol);

    let err4 = HuffmanTable::build_simple(&[1, 2, 3, 2], &[2, 2, 2, 2])
        .expect_err("duplicate 4 symbols");
    assert_eq!(err4, BrotliError::DuplicateSymbol);

    let err_zero = HuffmanTable::build_simple(&[], &[]).expect_err("0 symbols");
    assert!(matches!(err_zero, BrotliError::CorruptHeader(_)));

    let err_five =
        HuffmanTable::build_simple(&[1, 2, 3, 4, 5], &[1, 2, 3, 4, 5]).expect_err("5 symbols");
    assert!(matches!(err_five, BrotliError::CorruptHeader(_)));
}

// MARK: - Dimension 5: Exuberant Nibbles & Exuberant Metadata Bytes

#[test]
fn test_brotli_dim05_exuberant_nibbles_and_meta_bytes_rejection() {
    let stream_nibbles = pack_lsb_bits(&[
        (0, 1),
        (1, 2),
        (1, 4),
        (2, 4),
        (3, 4),
        (4, 4),
        (0, 4),
        (0, 1),
    ]);
    let mut br_nib = BrotliBitReader::new(&stream_nibbles);
    let err_nib =
        MetaBlockHeader::parse(&mut br_nib).expect_err("Exuberant 5-nibble length must fail");
    assert!(matches!(err_nib, BrotliError::CorruptHeader(_)));

    let stream_nibbles6 = pack_lsb_bits(&[
        (0, 1),
        (2, 2),
        (1, 4),
        (2, 4),
        (3, 4),
        (4, 4),
        (5, 4),
        (0, 4),
        (0, 1),
    ]);
    let mut br_nib6 = BrotliBitReader::new(&stream_nibbles6);
    let err_nib6 =
        MetaBlockHeader::parse(&mut br_nib6).expect_err("Exuberant 6-nibble length must fail");
    assert!(matches!(err_nib6, BrotliError::CorruptHeader(_)));

    let stream_meta = pack_lsb_bits(&[
        (0, 1),
        (3, 2),
        (0, 1),
        (2, 2),
        (0x55, 8),
        (0x00, 8),
        (0, 2),
    ]);
    let mut br_meta = BrotliBitReader::new(&stream_meta);
    let err_meta =
        MetaBlockHeader::parse(&mut br_meta).expect_err("Exuberant meta-byte must fail");
    assert!(matches!(err_meta, BrotliError::CorruptHeader(_)));
}

// MARK: - Dimension 6: Non-Zero Padding Bit Injections

#[test]
fn test_brotli_dim06_nonzero_padding_bit_injections() {
    let stream_pad = pack_lsb_bits(&[
        (0, 1),
        (0, 2),
        (1, 4),
        (0, 4),
        (0, 4),
        (0, 4),
        (1, 1),
        (0b1000, 4),
    ]);
    let mut br_pad = BrotliBitReader::new(&stream_pad);
    let err_pad =
        MetaBlockHeader::parse(&mut br_pad).expect_err("Non-zero padding bits must fail");
    assert_eq!(err_pad, BrotliError::InvalidPadding);

    let raw_jump_data = [0b0010_0101, 0x42];
    let mut br_jump = BrotliBitReader::new(&raw_jump_data);
    assert_eq!(br_jump.read_bits(3).expect("read 3"), 5);
    let err_jump = br_jump
        .jump_to_byte_boundary()
        .expect_err("jump with non-zero padding must fail");
    assert_eq!(err_jump, BrotliError::InvalidPadding);
}

// MARK: - Dimension 7: Out-of-Bounds Backward Distance & Ring Buffer Underflow

#[test]
fn test_brotli_dim07_out_of_bounds_distance_and_ring_buffer_underflow() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("ring buffer 10");
    rb.copy_slice(b"TEST_SEED");

    let err0 = rb.copy_match(0, 5).expect_err("Distance 0 must fail");
    assert!(matches!(err0, BrotliError::CorruptHeader(_)));

    let err_underflow = rb
        .copy_match(10, 4)
        .expect_err("Distance > current position must fail");
    assert!(matches!(err_underflow, BrotliError::CorruptHeader(_)));

    let fill = vec![0x41u8; 1500];
    rb.copy_slice(&fill);
    let err_win = rb
        .copy_match(1025, 4)
        .expect_err("Distance > window size must fail");
    assert!(matches!(err_win, BrotliError::CorruptHeader(_)));
}

// MARK: - Dimension 8: Illegal Static Transform Index & Buffer Overflow

#[test]
fn test_brotli_dim08_illegal_transform_index_and_buffer_overflow() {
    let mut dst = [0u8; 64];
    let word = b"brotli";

    let invalid_indices = [121, 122, 255, 1000, usize::MAX];
    for &idx in &invalid_indices {
        let res = transform_dictionary_word(&mut dst, word, idx);
        assert_eq!(res, Err(BrotliError::InvalidTransformIndex(idx)));
    }

    let mut small_dst = [0u8; 3];
    let res_small = transform_dictionary_word(&mut small_dst, word, 1);
    assert_eq!(
        res_small,
        Err(BrotliError::BufferTooSmall {
            required: 7,
            available: 3,
        })
    );
}

// MARK: - Dimension 9: Malformed Multi-Byte UTF-8 in Word Transforms

#[test]
fn test_brotli_dim09_malformed_multibyte_utf8_in_word_transforms() {
    let mut empty: [u8; 0] = [];
    assert_eq!(to_uppercase_utf8(&mut empty), 0);
    assert_eq!(shift_utf8(&mut empty, 100), 0);

    let mut truncated_2 = [0xD0];
    assert_eq!(to_uppercase_utf8(&mut truncated_2), 1);
    assert_eq!(shift_utf8(&mut truncated_2, 5), 1);

    let mut truncated_3 = [0xE4, 0xBD];
    assert_eq!(to_uppercase_utf8(&mut truncated_3), 2);
    assert_eq!(shift_utf8(&mut truncated_3, 5), 2);

    let mut truncated_4 = [0xF0, 0x9F, 0x98];
    assert_eq!(shift_utf8(&mut truncated_4, 5), 3);

    let mut dst = [0u8; 32];
    let len = transform_dictionary_word(&mut dst, b"a", 23).expect("omit exceeds");
    assert_eq!(len, 0);
}

// MARK: - Dimension 10: Metadata Block Reserved Bit Violations

#[test]
fn test_brotli_dim10_metadata_block_reserved_bit_violations() {
    let stream = pack_lsb_bits(&[(0, 1), (3, 2), (1, 1)]);
    let mut br = BrotliBitReader::new(&stream);
    let err = MetaBlockHeader::parse(&mut br)
        .expect_err("Non-zero reserved bit in metadata block must fail");
    assert!(matches!(err, BrotliError::CorruptHeader(_)));
}

// MARK: - Dimension 11: Pathological All-0xFF and All-0x00 Bomb Sequences

#[test]
fn test_brotli_dim11_pathological_all_ff_and_all_zero_bomb_sequences() {
    let sizes = [1, 2, 4, 16, 64, 256, 1024, 65536];

    for &size in &sizes {
        let ff_stream = vec![0xFFu8; size];
        let mut dst_ff = vec![0u8; 1024];
        let unwind_ff =
            catch_unwind(AssertUnwindSafe(|| brotli_decompress(&ff_stream, &mut dst_ff)));
        assert!(unwind_ff.is_ok(), "All-0xFF payload of size {size} panicked!");
        let res_ff = unwind_ff.unwrap();
        assert!(res_ff.is_err(), "All-0xFF payload of size {size} must return error");

        let zero_stream = vec![0x00u8; size];
        let mut dst_zero = vec![0u8; 1024];
        let unwind_zero = catch_unwind(AssertUnwindSafe(|| {
            brotli_decompress(&zero_stream, &mut dst_zero)
        }));
        assert!(
            unwind_zero.is_ok(),
            "All-0x00 payload of size {size} panicked!"
        );
    }
}

// MARK: - Dimension 12: 1% Pseudorandom Mutation Fuzz Stress (1,000 Iterations)

#[test]
fn test_brotli_dim12_1pct_pseudorandom_mutation_fuzzing_stress_1000_cycles() {
    let mut rng = FuzRand::new(0x2026_0830);

    let text_payload = rng.gen_buffer(2048, 75);
    let rand_payload = rng.gen_buffer(1024, 15);

    let comp_text = brotli_compress_to_vec(&text_payload, 6, 22).expect("compress text");
    let comp_rand = brotli_compress_to_vec(&rand_payload, 9, 22).expect("compress rand");
    let baselines = [&comp_text, &comp_rand];

    let mut dst_buf = vec![0u8; 65536];

    for iteration in 0..1000 {
        let base = baselines[rng.rand_usize(baselines.len())];
        let mut mutated = base.clone();

        let mutation_type = rng.rand_range(0, 4);
        match mutation_type {
            0 => {
                let num_flips = ((mutated.len() as f64) * 0.01).ceil() as usize;
                for _ in 0..num_flips.max(1) {
                    let byte_idx = rng.rand_usize(mutated.len());
                    let bit_idx = rng.rand_range(0, 7);
                    mutated[byte_idx] ^= 1 << bit_idx;
                }
            }
            1 => {
                let num_overwrites = rng.rand_range(1, 5) as usize;
                for _ in 0..num_overwrites {
                    let idx = rng.rand_usize(mutated.len());
                    mutated[idx] = rng.rand_u8();
                }
            }
            2 => {
                let trunc_len = rng.rand_usize(mutated.len());
                mutated.truncate(trunc_len);
            }
            3 => {
                let run_len = rng.rand_range(1, 16) as usize;
                let insert_pos = rng.rand_usize(mutated.len());
                mutated.splice(insert_pos..insert_pos, std::iter::repeat_n(0xFF, run_len));
            }
            _ => {
                let zero_len = rng.rand_range(1, 8) as usize;
                let start_pos = rng.rand_usize(mutated.len());
                let end_pos = (start_pos + zero_len).min(mutated.len());
                for b in mutated[start_pos..end_pos].iter_mut() {
                    *b = 0;
                }
            }
        }

        let unwind_res =
            catch_unwind(AssertUnwindSafe(|| brotli_decompress(&mutated, &mut dst_buf)));
        assert!(
            unwind_res.is_ok(),
            "Decompressor panicked on iteration {iteration} (mutation_type {mutation_type})!"
        );
        let res = unwind_res.unwrap();
        if let Ok(written) = res {
            assert!(written <= dst_buf.len());
        } else {
            assert_eq!(res.unwrap_err(), TTZipStatus::ErrCorruptHeader);
        }
    }
}

// MARK: - Dimension 13: Slow Jitter Streaming Push (Tail Byte Low 3-bit Driven)

#[test]
fn test_brotli_dim13_slow_jitter_streaming_tail_bit_driven() {
    let payloads: [&[u8]; 3] = [
        b"TTZip Slow Jitter Streaming Push test based on canonical decode_fuzzer.c addend 1..7.",
        b"Short",
        &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC],
    ];

    for &payload in &payloads {
        let compressed = brotli_compress_to_vec(payload, 6, 22).expect("compress payload");
        assert!(!compressed.is_empty());

        let raw_addend = (compressed[compressed.len() - 1] & 7) as usize;
        let addends = if raw_addend == 0 {
            vec![1, 2, 3, 4, 5, 6, 7]
        } else {
            vec![raw_addend]
        };

        for &addend in &addends {
            let mut decompressed = Vec::new();
            let mut reader = Cursor::new(&compressed);
            let mut decompressor = BrotliDecompressorReader::new(&mut reader, 65536);

            let mut chunk = vec![0u8; addend];
            loop {
                match decompressor.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => decompressed.extend_from_slice(&chunk[..n]),
                    Err(e) => panic!("Jitter feed failed with addend {addend}: {e:?}"),
                }
            }

            assert_eq!(
                decompressed.as_slice(),
                payload,
                "Slow jitter output mismatch with addend {addend}"
            );
        }
    }
}

// MARK: - Dimension 14: Asymmetric Chunking & Micro-Buffer Streaming

#[test]
fn test_brotli_dim14_asymmetric_chunking_and_micro_buffer_streaming() {
    let mut rng = FuzRand::new(0x1337_CAFE);
    let payload = rng.gen_buffer(1024, 60);

    let config = BrotliConfig {
        quality: 6,
        lgwin: 22,
        buffer_size: 4096,
    };
    let mut comp_sink = Vec::new();
    let mut comp_writer = BrotliCompressorWriter::new(&mut comp_sink, &config);

    let mut in_cursor = 0;
    while in_cursor < payload.len() {
        let in_step = rng.rand_range(1, 3) as usize;
        let end = (in_cursor + in_step).min(payload.len());
        comp_writer
            .write_all(&payload[in_cursor..end])
            .expect("write chunk");
        in_cursor = end;
    }
    comp_writer.flush().expect("flush writer");
    let finished_sink = comp_writer.finish().expect("finish writer");

    let mut reader = Cursor::new(finished_sink);
    let mut decompressor = BrotliDecompressorReader::new(&mut reader, 1024);
    let mut decompressed = Vec::new();

    loop {
        let out_step = rng.rand_range(1, 5) as usize;
        let mut out_chunk = vec![0u8; out_step];
        match decompressor.read(&mut out_chunk) {
            Ok(0) => break,
            Ok(n) => decompressed.extend_from_slice(chunk_ref(&out_chunk, n)),
            Err(e) => panic!("Asymmetric read failed: {e:?}"),
        }
    }

    assert_eq!(
        decompressed.as_slice(),
        payload.as_slice(),
        "Asymmetric micro-buffer decompression mismatch"
    );
}

#[inline]
fn chunk_ref(buf: &[u8], n: usize) -> &[u8] {
    &buf[..n]
}

// MARK: - Dimension 15: Single-Symbol Degenerate Trees & Deep 15-Bit Cascades

#[test]
fn test_brotli_dim15_single_symbol_and_deep_15bit_cascades_fuzz() {
    let single_sym = [99u16];
    let single_table =
        HuffmanTable::build_simple(&single_sym, &[0]).expect("single symbol table");
    let noise = [0x55, 0xAA, 0xFF, 0x00];
    let mut br_single = BrotliBitReader::new(&noise);
    for _ in 0..10 {
        let sym = single_table
            .decode_symbol(&mut br_single)
            .expect("decode single");
        assert_eq!(sym, 99);
    }

    let code_lengths: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15];
    let deep_table = HuffmanTable::build(&code_lengths, 16).expect("build 15-bit deep tree");
    assert!(deep_table.total_entries() > 256);

    let mut rng = FuzRand::new(0x0202_615B);
    let rand_noise = rng.gen_buffer(256, 0);
    let mut br_deep = BrotliBitReader::new(&rand_noise);
    for _ in 0..50 {
        if br_deep.unconsumed_bits() < 15 {
            break;
        }
        let _ = deep_table.decode_symbol(&mut br_deep);
    }
}

// MARK: - Dimension 16: Multi-Seed Random Fuzz Matrix Stress

#[test]
fn test_brotli_dim16_multi_seed_random_matrix_fuzz_stress() {
    let seeds = [
        0u32,
        1,
        42,
        1337,
        0x2026_0830,
        0xDEAD_BEEF,
        0xCAFE_BABE,
        0x8000_0000,
        0xFFFF_FFFF,
        0x1234_5678,
    ];
    let mut dst = [0u8; 4096];

    for &seed in &seeds {
        let mut rng = FuzRand::new(seed);
        for _ in 0..100 {
            let len = rng.rand_range(1, 512) as usize;
            let mut corrupt_block = vec![0u8; len];
            for b in corrupt_block.iter_mut() {
                *b = rng.rand_u8();
            }

            let unwind_res =
                catch_unwind(AssertUnwindSafe(|| brotli_decompress(&corrupt_block, &mut dst)));
            assert!(
                unwind_res.is_ok(),
                "Random block with seed {seed:#X} caused panic"
            );
        }
    }
}
