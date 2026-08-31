// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Full-Operator Single-Byte Micro-Buffer Cataclysm Stress Test Suite.
//!
//! Applies extreme 1-byte streaming constraints (`in_chunk_size = 1, out_chunk_size = 1`)
//! across 7z core codec operators: Varint, LZMA, BCJ2, PPMd, AES-256-CBC, and Composite.

use ttzip_engine::codecs::branch::bcj2::encode_bcj2;
use ttzip_engine::codecs::branch::bcj2_stream::{
    Bcj2ArbitratorStatus, Bcj2StreamArbitrator, Bcj2StreamId,
};
use ttzip_engine::codecs::lzma::alone::LzmaAloneDecoder;
use ttzip_engine::codecs::lzma::{RangeDecoder, RangeEncoder, PROB_INIT_VAL};
use ttzip_engine::codecs::ppmd::{ppmd_compress_to_vec, PpmdModel, PpmdRangeDecoder};
use ttzip_engine::codecs::ppmd_suballoc::{
    PpmdContext, PpmdState, PpmdVariant, SubAllocBumpArena, PPMD_UNIT_SIZE,
};
use ttzip_engine::crypto::aes256::{aes256_cbc_decrypt, aes256_cbc_encrypt};
use ttzip_engine::sevenz::varint::{
    decode_7z_varint, encode_7z_varint, encode_7z_varint_vec, varint_size_7z, VarintError,
    MAX_VARINT_LEN_7Z,
};

// MARK: - Test 1: 7z Varint Single-Byte Feeding Torture

#[test]
fn test_sevenz_varint_single_byte_feeding_torture() {
    let test_values: &[u64] = &[
        0, 1, 127, 128, 255, 256, 16383, 16384, 0x1F_FFFF, 0x20_0000, 0x0F_FFFF_FFFF,
        0x10_0000_0000, 0x0000_0007_FFFF_FFFF, 0x0000_0008_0000_0000, 0x0000_03FF_FFFF_FFFF,
        0x0000_0400_0000_0000, 0x0001_FFFF_FFFF_FFFF, 0x0002_0000_0000_0000,
        0x00FF_FFFF_FFFF_FFFF, 0x0100_0000_0000_0000, 0x7FFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000, u64::MAX,
    ];

    for &val in test_values {
        let expected_size = varint_size_7z(val);
        let mut encoded = [0u8; MAX_VARINT_LEN_7Z];
        let written = encode_7z_varint(val, &mut encoded);
        assert_eq!(written, expected_size);

        let mut rolling_buf = Vec::with_capacity(MAX_VARINT_LEN_7Z);
        for (byte_idx, &b) in encoded[..written].iter().enumerate() {
            rolling_buf.push(b);
            let current_len = rolling_buf.len();

            if current_len < expected_size {
                let res = decode_7z_varint(&rolling_buf);
                assert!(
                    matches!(res, Err(VarintError::UnexpectedEof { needed, available })
                        if needed == expected_size && available == current_len),
                    "Expected UnexpectedEof at byte index {byte_idx} for value {val}"
                );
            } else {
                let (decoded_val, consumed) =
                    decode_7z_varint(&rolling_buf).expect("Decode must succeed on full length");
                assert_eq!(decoded_val, val);
                assert_eq!(consumed, expected_size);
            }
        }
    }

    // Interleaved stream of 200 concatenated varints decoded 1 byte at a time
    let mut rng_state: u64 = 0xA395_4B89_C128_6D17;
    let mut expected_sequence = Vec::with_capacity(200);
    let mut byte_stream = Vec::with_capacity(1800);

    for _ in 0..200 {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let val = rng_state;
        expected_sequence.push(val);
        encode_7z_varint_vec(val, &mut byte_stream);
    }

    let mut decoded_sequence = Vec::with_capacity(200);
    let mut staging = Vec::with_capacity(MAX_VARINT_LEN_7Z);

    for &b in &byte_stream {
        staging.push(b);
        if let Ok((val, consumed)) = decode_7z_varint(&staging) {
            decoded_sequence.push(val);
            staging.drain(..consumed);
        }
    }

    assert!(staging.is_empty());
    assert_eq!(decoded_sequence, expected_sequence);
}

// MARK: - Test 2: LZMA Range Coder & Alone Single-Byte Torture

#[test]
fn test_lzma_range_coder_primitives_torture() {
    let mut encoder = RangeEncoder::new();
    let mut bit_probs = [PROB_INIT_VAL; 16];
    let mut tree_probs = [PROB_INIT_VAL; 32];
    let mut rev_tree_probs = [PROB_INIT_VAL; 32];
    let mut lit_probs = [PROB_INIT_VAL; 0x300];

    let mut truth_bits = Vec::with_capacity(100);
    let mut truth_tree_syms = Vec::with_capacity(50);
    let mut truth_rev_syms = Vec::with_capacity(50);
    let mut truth_direct = Vec::with_capacity(50);
    let mut truth_lits = Vec::with_capacity(50);
    let mut truth_matched = Vec::with_capacity(50);
    let mut encoded_stream = Vec::new();
    let mut rng_state: u64 = 0x517C_C1B7_2722_0A95;

    let mut next_rnd = |bound: u32| -> u32 {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((rng_state >> 32) as u32) % bound
    };

    for i in 0..100 {
        let prob_idx = i % 16;
        let bit = next_rnd(2);
        truth_bits.push((prob_idx, bit));
        encoder.encode_bit(&mut bit_probs[prob_idx], bit, &mut encoded_stream);
    }
    for _ in 0..50 {
        let sym = next_rnd(16);
        truth_tree_syms.push(sym);
        encoder.encode_bit_tree(&mut tree_probs, sym, 4, &mut encoded_stream);
    }
    for _ in 0..50 {
        let sym = next_rnd(16);
        truth_rev_syms.push(sym);
        encoder.encode_reverse_bit_tree(&mut rev_tree_probs, sym, 4, &mut encoded_stream);
    }
    for _ in 0..50 {
        let val = next_rnd(256);
        truth_direct.push(val);
        encoder.encode_direct_bits(val, 8, &mut encoded_stream);
    }
    for _ in 0..50 {
        let b = next_rnd(256) as u8;
        truth_lits.push(b);
        encoder.encode_literal_byte(&mut lit_probs, b, &mut encoded_stream);
    }
    for _ in 0..50 {
        let b = next_rnd(256) as u8;
        let match_b = next_rnd(256) as u8;
        truth_matched.push((b, match_b));
        encoder.encode_matched_byte(&mut lit_probs, b, match_b, &mut encoded_stream);
    }
    encoder.finish(&mut encoded_stream);

    let mut decoder = RangeDecoder::new(&encoded_stream).expect("RangeDecoder init must succeed");
    let mut dec_bit_probs = [PROB_INIT_VAL; 16];
    let mut dec_tree_probs = [PROB_INIT_VAL; 32];
    let mut dec_rev_tree_probs = [PROB_INIT_VAL; 32];
    let mut dec_lit_probs = [PROB_INIT_VAL; 0x300];

    for &(prob_idx, expected_bit) in &truth_bits {
        assert_eq!(decoder.decode_bit(&mut dec_bit_probs[prob_idx]).unwrap(), expected_bit);
    }
    for &expected_sym in &truth_tree_syms {
        assert_eq!(decoder.decode_bit_tree(&mut dec_tree_probs, 4).unwrap(), expected_sym);
    }
    for &expected_sym in &truth_rev_syms {
        assert_eq!(decoder.decode_reverse_bit_tree(&mut dec_rev_tree_probs, 4).unwrap(), expected_sym);
    }
    for &expected_val in &truth_direct {
        assert_eq!(decoder.decode_direct_bits(8).unwrap(), expected_val);
    }
    for &expected_b in &truth_lits {
        assert_eq!(decoder.decode_literal_byte(&mut dec_lit_probs).unwrap(), expected_b);
    }
    for &(expected_b, match_b) in &truth_matched {
        assert_eq!(decoder.decode_matched_byte(&mut dec_lit_probs, match_b).unwrap(), expected_b);
    }
}

#[repr(C)]
struct LzmaOptionsLzma {
    dict_size: u32,
    preset_dict: *const u8,
    preset_dict_size: u32,
    lc: u32,
    lp: u32,
    pb: u32,
    mode: libc::c_int,
    nice_len: u32,
    mf: libc::c_int,
    depth: u32,
    reserved_int1: u32,
    reserved_int2: u32,
    reserved_int3: u32,
    reserved_int4: u32,
    reserved_ptr1: *mut libc::c_void,
    reserved_ptr2: *mut libc::c_void,
}

#[repr(C)]
struct RawLzmaStream {
    next_in: *const u8,
    avail_in: libc::size_t,
    total_in: u64,
    next_out: *mut u8,
    avail_out: libc::size_t,
    total_out: u64,
    allocator: *const libc::c_void,
    internal: *mut libc::c_void,
    reserved_ptr1: *mut libc::c_void,
    reserved_ptr2: *mut libc::c_void,
    reserved_ptr3: *mut libc::c_void,
    reserved_ptr4: *mut libc::c_void,
    reserved_seek: u64,
    reserved_int1: u64,
    reserved_int2: libc::size_t,
    reserved_int3: libc::size_t,
    reserved_enum1: libc::c_int,
    reserved_enum2: libc::c_int,
}

impl Default for RawLzmaStream {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

extern "C" {
    fn lzma_lzma_preset(options: *mut LzmaOptionsLzma, preset: u32) -> bool;
    fn lzma_alone_encoder(strm: *mut RawLzmaStream, options: *const LzmaOptionsLzma) -> libc::c_int;
    fn lzma_code(strm: *mut RawLzmaStream, action: libc::c_int) -> libc::c_int;
    fn lzma_end(strm: *mut RawLzmaStream);
}

#[test]
fn test_lzma_alone_single_byte_streaming_torture() {
    let payload = b"TTZip 7z Cataclysm Torture: LZMA Alone Single-Byte In / Out Micro-Buffer Stress Test. \
        The quick brown fox jumps over the lazy dog. Repeating patterns 1234567890 1234567890 \
        ABCDEFGH IJKLMNOP QRSTUV WXYZ 0987654321! Random filler with entropy and compressibility.";

    let mut opt = std::mem::MaybeUninit::<LzmaOptionsLzma>::uninit();
    assert!(!unsafe { lzma_lzma_preset(opt.as_mut_ptr(), 6) });
    let opt = unsafe { opt.assume_init() };

    let mut strm = RawLzmaStream::default();
    assert_eq!(unsafe { lzma_alone_encoder(&mut strm, &opt) }, 0);

    let mut comp = vec![0u8; 4096];
    strm.next_in = payload.as_ptr();
    strm.avail_in = payload.len();
    strm.next_out = comp.as_mut_ptr();
    strm.avail_out = comp.len();

    assert_eq!(unsafe { lzma_code(&mut strm, 3) }, 1);
    let total_comp = strm.total_out as usize;
    unsafe { lzma_end(&mut strm) };
    comp.truncate(total_comp);

    let mut decoder = LzmaAloneDecoder::new().expect("LzmaAloneDecoder init");
    let mut reconstructed = Vec::with_capacity(payload.len());
    let mut in_pos = 0usize;
    let mut suspend_count = 0usize;

    while in_pos < comp.len() || reconstructed.len() < payload.len() {
        let in_chunk = if in_pos < comp.len() { &comp[in_pos..in_pos + 1] } else { &[] };
        let mut out_byte = [0u8; 1];
        let is_finish = in_pos >= comp.len();

        let (consumed, produced, is_end) = decoder
            .decompress_chunk(in_chunk, &mut out_byte, is_finish)
            .expect("decompress_chunk single-byte step");

        in_pos += consumed;
        if produced > 0 {
            reconstructed.push(out_byte[0]);
        } else {
            suspend_count += 1;
        }

        if is_end && reconstructed.len() == payload.len() {
            break;
        }
        if in_chunk.is_empty() && produced == 0 && consumed == 0 {
            break;
        }
    }

    assert!(suspend_count > 10, "Must exercise partial suspend cycles");
    assert_eq!(reconstructed.as_slice(), payload, "LZMA Alone Single-Byte Torture mismatch");
}

// MARK: - Test 3: BCJ2 4-Stream Single-Byte Demand-Driven Arbitration

fn generate_synthetic_x86_stream(len: usize, base_ip: u64) -> Vec<u8> {
    let mut code = Vec::with_capacity(len);
    let mut pc = base_ip;
    let mut step = 0usize;

    while code.len() < len {
        step += 1;
        match step % 6 {
            0 => {
                let p = [0x55, 0x48, 0x89, 0xE5, 0x48, 0x83, 0xEC, 0x20];
                code.extend_from_slice(&p);
                pc = pc.wrapping_add(p.len() as u64);
            }
            1 => {
                let target = base_ip.wrapping_add((step * 512) as u64);
                let rel = target.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE8);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            2 => {
                let instrs = [0x48, 0x8B, 0x05, 0x10, 0x00, 0x00, 0x00, 0x90];
                code.extend_from_slice(&instrs);
                pc = pc.wrapping_add(instrs.len() as u64);
            }
            3 => {
                let target = base_ip.wrapping_add(0x80);
                let rel = target.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE9);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            4 => {
                let instrs = [0x0F, 0x1F, 0x44, 0x00, 0x00];
                code.extend_from_slice(&instrs);
                pc = pc.wrapping_add(instrs.len() as u64);
            }
            _ => {
                let e = [0xC9, 0xC3];
                code.extend_from_slice(&e);
                pc = pc.wrapping_add(e.len() as u64);
            }
        }
    }
    code.truncate(len);
    code
}

#[test]
fn test_bcj2_4stream_single_byte_demand_driven_arbitration_torture() {
    let original_bytecode = generate_synthetic_x86_stream(8192, 0x1000_0000);
    let streams = encode_bcj2(&original_bytecode, 0x1000_0000);

    assert!(!streams.main.is_empty());
    assert!(!streams.call.is_empty());
    assert!(!streams.jump.is_empty());
    assert!(!streams.rc.is_empty());

    let mut main_cursor = 0usize;
    let mut call_cursor = 0usize;
    let mut jump_cursor = 0usize;
    let mut rc_cursor = 0usize;

    let mut arbitrator = Bcj2StreamArbitrator::new(0x1000_0000);
    let mut reconstructed = Vec::with_capacity(original_bytecode.len());
    let mut suspensions_input = 0usize;
    let mut suspensions_output = 0usize;

    loop {
        let mut main_chunk: &[u8] = if main_cursor < streams.main.len() {
            &streams.main[main_cursor..main_cursor + 1]
        } else {
            &[]
        };
        let mut call_chunk: &[u8] = if call_cursor < streams.call.len() {
            &streams.call[call_cursor..call_cursor + 1]
        } else {
            &[]
        };
        let mut jump_chunk: &[u8] = if jump_cursor < streams.jump.len() {
            &streams.jump[jump_cursor..jump_cursor + 1]
        } else {
            &[]
        };
        let mut rc_chunk: &[u8] = if rc_cursor < streams.rc.len() {
            &streams.rc[rc_cursor..rc_cursor + 1]
        } else {
            &[]
        };

        let mut out_byte = [0u8; 1];
        let mut out_slice: &mut [u8] = &mut out_byte[..];
        let main_is_eof = main_cursor >= streams.main.len();

        let initial_main_len = main_chunk.len();
        let initial_call_len = call_chunk.len();
        let initial_jump_len = jump_chunk.len();
        let initial_rc_len = rc_chunk.len();

        let status = arbitrator
            .process(
                &mut main_chunk,
                &mut call_chunk,
                &mut jump_chunk,
                &mut rc_chunk,
                &mut out_slice,
                main_is_eof,
            )
            .expect("arbitrator process must not fail");

        main_cursor += initial_main_len - main_chunk.len();
        call_cursor += initial_call_len - call_chunk.len();
        jump_cursor += initial_jump_len - jump_chunk.len();
        rc_cursor += initial_rc_len - rc_chunk.len();

        if out_slice.is_empty() {
            reconstructed.push(out_byte[0]);
        }

        match status {
            Bcj2ArbitratorStatus::NeedsMoreInput(stream_id) => {
                suspensions_input += 1;
                match stream_id {
                    Bcj2StreamId::StreamMain => assert!(main_cursor <= streams.main.len()),
                    Bcj2StreamId::StreamCall => assert!(call_cursor <= streams.call.len()),
                    Bcj2StreamId::StreamJump => assert!(jump_cursor <= streams.jump.len()),
                    Bcj2StreamId::StreamRc => assert!(rc_cursor <= streams.rc.len()),
                }
            }
            Bcj2ArbitratorStatus::NeedsMoreOutput => {
                suspensions_output += 1;
            }
            Bcj2ArbitratorStatus::Finished => {
                break;
            }
        }
    }

    assert!(suspensions_input > 1000, "Input suspensions actual: {suspensions_input}");
    assert!(suspensions_output > 100, "Output suspensions actual: {suspensions_output}");
    assert_eq!(reconstructed, original_bytecode, "BCJ2 4-Stream output is NOT Bit-Exact!");
}

// MARK: - Test 4: PPMd SubAlloc & Range Decoder Single-Byte Torture

#[test]
fn test_ppmd_suballoc_single_byte_streaming_torture() {
    let arena = SubAllocBumpArena::new(2 * 1024 * 1024, PpmdVariant::Ppmd7).expect("arena init");
    assert!(arena.size >= 2 * 1024 * 1024);
    assert_eq!(std::mem::size_of::<PpmdContext>(), 12);
    assert_eq!(std::mem::size_of::<PpmdState>(), 6);
    assert_eq!(PPMD_UNIT_SIZE, 12);
    assert!(arena.hi_unit > arena.lo_unit);

    let test_data = b"TTZip PPMd Model H Statistical Single-Byte Micro-Buffer Cataclysm Torture Payload! \
        Repeating sequences: 0123456789 0123456789 aabbccddeeff gghhiijjkkll.";

    let compressed = ppmd_compress_to_vec(test_data, 6, 2 * 1024 * 1024).expect("ppmd compress");
    assert!(!compressed.is_empty());

    let mut rc = PpmdRangeDecoder::new(&compressed).expect("PpmdRangeDecoder init");
    let mut model = PpmdModel::new(6, 2 * 1024 * 1024).expect("PpmdModel init");
    let mut decompressed = Vec::with_capacity(test_data.len());

    for _ in 0..test_data.len() {
        let sym = model.decode_symbol(&mut rc).expect("decode symbol");
        decompressed.push(sym);
    }

    assert_eq!(decompressed.as_slice(), test_data, "PPMd Single-Byte mismatch!");
}

// MARK: - Test 5: AES-256-CBC Single-Byte Streaming State Machine Torture

struct StreamingAes256CbcDecryptor {
    key: [u8; 32],
    iv: [u8; 16],
    stage_in: [u8; 16],
    stage_in_len: usize,
    stage_out: [u8; 16],
    stage_out_pos: usize,
    stage_out_len: usize,
}

impl StreamingAes256CbcDecryptor {
    fn new(key: &[u8; 32], iv: &[u8; 16]) -> Self {
        Self {
            key: *key,
            iv: *iv,
            stage_in: [0; 16],
            stage_in_len: 0,
            stage_out: [0; 16],
            stage_out_pos: 0,
            stage_out_len: 0,
        }
    }

    fn step(&mut self, in_byte: Option<u8>, out_byte: &mut Option<u8>) {
        *out_byte = None;
        if self.stage_out_pos < self.stage_out_len {
            *out_byte = Some(self.stage_out[self.stage_out_pos]);
            self.stage_out_pos += 1;
            return;
        }

        if let Some(b) = in_byte {
            self.stage_in[self.stage_in_len] = b;
            self.stage_in_len += 1;

            if self.stage_in_len == 16 {
                let mut decrypted_block = [0u8; 16];
                aes256_cbc_decrypt(&self.key, &self.iv, &self.stage_in, &mut decrypted_block)
                    .expect("Block decryption failed");

                self.iv.copy_from_slice(&self.stage_in);
                self.stage_out = decrypted_block;
                self.stage_out_pos = 1;
                self.stage_out_len = 16;
                self.stage_in_len = 0;
                *out_byte = Some(self.stage_out[0]);
            }
        }
    }
}

#[test]
fn test_aes256_cbc_single_byte_streaming_torture() {
    let key: [u8; 32] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32,
        0x10, 0x55, 0xAA, 0x55, 0xAA, 0x33, 0xCC, 0x33, 0xCC, 0x0F, 0xF0, 0x0F, 0xF0, 0x12, 0x34,
        0x56, 0x78,
    ];
    let iv: [u8; 16] = [
        0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23,
        0x01,
    ];

    let plaintext = b"TTZip AES-256-CBC 1-Byte Micro-Buffer Cataclysm Torture Test: Multi-block validation! \
        Zero state drift, strict 16-byte chaining block integrity across 128 bytes total.";
    let mut aligned = plaintext.to_vec();
    while !aligned.len().is_multiple_of(16) {
        aligned.push(0x20);
    }

    let mut ciphertext = vec![0u8; aligned.len()];
    aes256_cbc_encrypt(&key, &iv, &aligned, &mut ciphertext).expect("Encrypt must succeed");

    let mut decryptor = StreamingAes256CbcDecryptor::new(&key, &iv);
    let mut decrypted = Vec::with_capacity(aligned.len());
    let mut in_pos = 0usize;

    while in_pos < ciphertext.len() || decrypted.len() < aligned.len() {
        let in_byte = if in_pos < ciphertext.len() {
            let b = ciphertext[in_pos];
            in_pos += 1;
            Some(b)
        } else {
            None
        };

        let mut out_byte = None;
        decryptor.step(in_byte, &mut out_byte);
        if let Some(b) = out_byte {
            decrypted.push(b);
        }

        while decryptor.stage_out_pos < decryptor.stage_out_len {
            let mut extra_out = None;
            decryptor.step(None, &mut extra_out);
            if let Some(b) = extra_out {
                decrypted.push(b);
            }
        }
    }

    assert_eq!(decrypted, aligned, "AES-256-CBC Single-Byte mismatch!");
}

// MARK: - Test 6: Composite Multi-Operator Streaming Pipeline

#[test]
fn test_composite_7z_single_byte_cataclysm_pipeline() {
    let original_code = generate_synthetic_x86_stream(1024, 0x4000_0000);
    let bcj2_streams = encode_bcj2(&original_code, 0x4000_0000);

    let key: [u8; 32] = [0x42; 32];
    let iv: [u8; 16] = [0x24; 16];

    let mut main_padded = bcj2_streams.main.clone();
    let original_main_len = main_padded.len();
    while !main_padded.len().is_multiple_of(16) {
        main_padded.push(0);
    }

    let mut main_encrypted = vec![0u8; main_padded.len()];
    aes256_cbc_encrypt(&key, &iv, &main_padded, &mut main_encrypted).expect("Encrypt main");

    let mut decryptor = StreamingAes256CbcDecryptor::new(&key, &iv);
    let mut main_decrypted_padded = Vec::with_capacity(main_padded.len());
    let mut in_pos = 0usize;

    while in_pos < main_encrypted.len() || main_decrypted_padded.len() < main_padded.len() {
        let in_byte = if in_pos < main_encrypted.len() {
            let b = main_encrypted[in_pos];
            in_pos += 1;
            Some(b)
        } else {
            None
        };

        let mut out_byte = None;
        decryptor.step(in_byte, &mut out_byte);
        if let Some(b) = out_byte {
            main_decrypted_padded.push(b);
        }
        while decryptor.stage_out_pos < decryptor.stage_out_len {
            let mut extra = None;
            decryptor.step(None, &mut extra);
            if let Some(b) = extra {
                main_decrypted_padded.push(b);
            }
        }
    }

    let mut main_decrypted = main_decrypted_padded;
    main_decrypted.truncate(original_main_len);
    assert_eq!(main_decrypted, bcj2_streams.main);

    let mut arbitrator = Bcj2StreamArbitrator::new(0x4000_0000);
    let mut final_reconstructed = Vec::with_capacity(original_code.len());

    let mut main_c = 0usize;
    let mut call_c = 0usize;
    let mut jump_c = 0usize;
    let mut rc_c = 0usize;

    loop {
        let mut main_slice = if main_c < main_decrypted.len() {
            &main_decrypted[main_c..main_c + 1]
        } else {
            &[]
        };
        let mut call_slice = if call_c < bcj2_streams.call.len() {
            &bcj2_streams.call[call_c..call_c + 1]
        } else {
            &[]
        };
        let mut jump_slice = if jump_c < bcj2_streams.jump.len() {
            &bcj2_streams.jump[jump_c..jump_c + 1]
        } else {
            &[]
        };
        let mut rc_slice = if rc_c < bcj2_streams.rc.len() {
            &bcj2_streams.rc[rc_c..rc_c + 1]
        } else {
            &[]
        };

        let mut out_byte = [0u8; 1];
        let mut out_slice = &mut out_byte[..];
        let is_eof = main_c >= main_decrypted.len();

        let orig_main_l = main_slice.len();
        let orig_call_l = call_slice.len();
        let orig_jump_l = jump_slice.len();
        let orig_rc_l = rc_slice.len();

        let status = arbitrator
            .process(
                &mut main_slice,
                &mut call_slice,
                &mut jump_slice,
                &mut rc_slice,
                &mut out_slice,
                is_eof,
            )
            .expect("Arbitrator processing in composite pipeline");

        main_c += orig_main_l - main_slice.len();
        call_c += orig_call_l - call_slice.len();
        jump_c += orig_jump_l - jump_slice.len();
        rc_c += orig_rc_l - rc_slice.len();

        if out_slice.is_empty() {
            final_reconstructed.push(out_byte[0]);
        }

        if status == Bcj2ArbitratorStatus::Finished {
            break;
        }
    }

    assert_eq!(final_reconstructed, original_code, "Composite Pipeline mismatch!");
}
