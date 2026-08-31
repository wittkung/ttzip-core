// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and verification test suite for XZ BCJ Hardware Instruction Filters.
//!
//! Verifies:
//! 1. Bijective Roundtrip Identity across all 4 architectures (x86, ARM, ARM64, RISC-V):
//!    $\forall B \in \Sigma^*, \text{decode}(\text{encode}(B)) \equiv B$.
//! 2. Continuous sliding-window chunk boundary streaming continuity for x86 5-byte state machine.
//! 3. ARM64 BL (+/-128 MiB) and ADRP (+/-512 MiB with $PC \gg 12$ page base) address remapping fidelity.
//! 4. RISC-V JAL ($rd=x1/x5$) big-endian immediate normalization and AUIPC+inst2 bijective pairs & fake decode.
//! 5. Adversarial pseudo-random data streams, pathological boundary offsets, and extreme buffer slices.

use ttzip_engine::xz::bcj::{
    BcjArm, BcjArm64, BcjRiscv, BcjStreamFilter, BcjX86, BranchFilter, FILTER_ID_ARM,
    FILTER_ID_ARM64, FILTER_ID_RISCV, FILTER_ID_X86,
};

/// Helper PRNG (xorshift64) to generate deterministic, reproducible pseudo-random byte buffers.
struct TestRng {
    state: u64,
}

impl TestRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x8542_5892_a824_1947 } else { seed },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x as u32
    }

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(4) {
            let val = self.next_u32().to_le_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&val[..len]);
        }
    }
}

#[test]
fn test_filter_id_and_architectural_properties() {
    let x86 = BcjX86::new();
    assert_eq!(x86.filter_id(), FILTER_ID_X86);
    assert_eq!(x86.alignment(), 1);
    assert_eq!(x86.unfiltered_max(), 5);

    let arm = BcjArm::new();
    assert_eq!(arm.filter_id(), FILTER_ID_ARM);
    assert_eq!(arm.alignment(), 4);
    assert_eq!(arm.unfiltered_max(), 4);

    let arm64 = BcjArm64::new();
    assert_eq!(arm64.filter_id(), FILTER_ID_ARM64);
    assert_eq!(arm64.alignment(), 4);
    assert_eq!(arm64.unfiltered_max(), 4);

    let riscv = BcjRiscv::new();
    assert_eq!(riscv.filter_id(), FILTER_ID_RISCV);
    assert_eq!(riscv.alignment(), 2);
    assert_eq!(riscv.unfiltered_max(), 8);
}

#[test]
fn test_x86_bcj_roundtrip_synthetic_and_adversarial() {
    let seeds = [0x1234_5678, 0x9ABC_DEF0, 0xCAFE_BABE, 0xDEAD_BEEF];
    let sizes = [0, 1, 4, 5, 6, 16, 64, 255, 1024, 8192];

    for &seed in &seeds {
        let mut rng = TestRng::new(seed);
        for &size in &sizes {
            let mut original = vec![0u8; size];
            rng.fill_bytes(&mut original);

            // Inject known x86 CALL and JMP opcodes
            for i in 0..size.saturating_sub(5) {
                if i % 7 == 0 {
                    original[i] = 0xE8;
                    original[i + 4] = if i % 2 == 0 { 0x00 } else { 0xFF };
                } else if i % 11 == 0 {
                    original[i] = 0xE9;
                    original[i + 4] = if i % 3 == 0 { 0x00 } else { 0xFF };
                }
            }

            let mut encoded = original.clone();
            let mut filter_enc = BcjX86::new();
            let processed_enc = filter_enc.encode(&mut encoded, 0x1000);

            let mut decoded = encoded.clone();
            let mut filter_dec = BcjX86::new();
            let processed_dec = filter_dec.decode(&mut decoded, 0x1000);

            assert_eq!(processed_enc, processed_dec);
            assert_eq!(
                original, decoded,
                "x86 BCJ roundtrip identity failed for seed {:#X}, size {}",
                seed, size
            );
        }
    }
}

#[test]
fn test_x86_bcj_streaming_chunk_boundary_continuity() {
    let mut rng = TestRng::new(0xABCD_1234);
    let total_size = 4096;
    let mut original = vec![0u8; total_size];
    rng.fill_bytes(&mut original);

    // Sprinkle real x86 CALL / JMP instructions
    for i in (0..total_size - 5).step_by(13) {
        original[i] = if i % 2 == 0 { 0xE8 } else { 0xE9 };
        original[i + 4] = 0x00;
    }

    // 1. One-shot reference processing
    let mut one_shot_stream = BcjStreamFilter::new(BcjX86::new(), 0x2000);
    let reference_encoded = one_shot_stream.process_all(&original, true);

    let mut one_shot_decode = BcjStreamFilter::new(BcjX86::new(), 0x2000);
    let reference_decoded = one_shot_decode.process_all(&reference_encoded, false);
    assert_eq!(original, reference_decoded);

    // 2. Stream chunking with various arbitrary and pathological chunk sizes
    let chunk_sizes = [1, 2, 3, 4, 5, 7, 16, 64, 127, 256, 513];
    for &chunk_sz in &chunk_sizes {
        let mut stream_enc = BcjStreamFilter::new(BcjX86::new(), 0x2000);
        let mut stream_encoded = Vec::new();

        for chunk in original.chunks(chunk_sz) {
            stream_encoded.extend_from_slice(&stream_enc.process_chunk(chunk, true));
        }
        stream_encoded.extend_from_slice(&stream_enc.finish());

        assert_eq!(
            reference_encoded, stream_encoded,
            "x86 stream chunking failed for chunk_size {}",
            chunk_sz
        );

        // Decode with different chunk sizing
        let mut stream_dec = BcjStreamFilter::new(BcjX86::new(), 0x2000);
        let mut stream_decoded = Vec::new();
        let dec_chunk_sz = if chunk_sz == 1 { 7 } else { 3 };

        for chunk in stream_encoded.chunks(dec_chunk_sz) {
            stream_decoded.extend_from_slice(&stream_dec.process_chunk(chunk, false));
        }
        stream_decoded.extend_from_slice(&stream_dec.finish());

        assert_eq!(
            original, stream_decoded,
            "x86 stream decode reconstruction failed for chunk_size {}",
            chunk_sz
        );
    }
}

#[test]
fn test_arm_bcj_roundtrip_and_pipeline_compensation() {
    let mut arm = BcjArm::new();

    // Construct ARM 32-bit BL instruction:
    // Opcode = 0xEB, Offset = 0x001234
    // Raw little-endian bytes: [0x34, 0x12, 0x00, 0xEB]
    let mut buffer = vec![0x34, 0x12, 0x00, 0xEB, 0x00, 0x00, 0x00, 0x00];
    let start_pos = 0x0000_1000u32;

    let original = buffer.clone();
    arm.encode(&mut buffer, start_pos);

    // Verify transformation occurred
    assert_ne!(buffer, original);
    assert_eq!(buffer[3], 0xEB);

    // Verify mathematical pipeline compensation:
    // src = 0x001234 << 2 = 0x0048D0
    // dest = start_pos(0x1000) + i(0) + 8 + src(0x48D0) = 0x58D8
    // encoded word = 0x58D8 >> 2 = 0x1636
    let expected_dest_word = (start_pos + 8 + (0x001234 << 2)) >> 2;
    let actual_dest_word = (buffer[0] as u32) | ((buffer[1] as u32) << 8) | ((buffer[2] as u32) << 16);
    assert_eq!(actual_dest_word, expected_dest_word);

    // Decode and verify roundtrip
    arm.decode(&mut buffer, start_pos);
    assert_eq!(buffer, original);

    // Adversarial pseudo-random sweep
    let mut rng = TestRng::new(0x7777_8888);
    let mut rand_buf = vec![0u8; 1024];
    rng.fill_bytes(&mut rand_buf);
    for i in (0..1024).step_by(8) {
        rand_buf[i + 3] = 0xEB;
    }
    let orig_rand = rand_buf.clone();
    arm.encode(&mut rand_buf, 0x4000);
    arm.decode(&mut rand_buf, 0x4000);
    assert_eq!(rand_buf, orig_rand);
}

#[test]
fn test_arm64_bcj_bl_and_adrp_fidelity() {
    let mut arm64 = BcjArm64::new();
    let now_pos = 0x0010_0000u32; // 1 MB PC base

    // 1. ARM64 BL instruction: opcode 0x94000000 with 26-bit immediate
    // BL +0x1000 (offset = 0x400 words)
    let bl_instr = 0x9400_0400u32;
    let mut buf = bl_instr.to_le_bytes().to_vec();
    let bl_orig = buf.clone();

    arm64.encode(&mut buf, now_pos);
    assert_ne!(buf, bl_orig);

    let encoded_instr = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    assert_eq!(encoded_instr >> 26, 0x25); // BL opcode preserved

    arm64.decode(&mut buf, now_pos);
    assert_eq!(buf, bl_orig);

    // 2. ARM64 ADRP instruction: opcode 0x90000000
    // Test in-range (+/- 512 MiB) ADRP:
    // immlo = 2 (bits 30..29), immhi = 0x100 (bits 23..5), Rd = x0 (bits 4..0)
    let adrp_in_range = 0x9000_0000u32 | (2 << 29) | (0x100 << 5);
    let mut adrp_buf = adrp_in_range.to_le_bytes().to_vec();
    let adrp_orig = adrp_buf.clone();

    arm64.encode(&mut adrp_buf, now_pos);
    assert_ne!(adrp_buf, adrp_orig);

    arm64.decode(&mut adrp_buf, now_pos);
    assert_eq!(adrp_buf, adrp_orig);

    // 3. Out-of-range ADRP (must be skipped without modification)
    // Create an ADRP immediate exceeding +/-512MB (src = 0x080000, +2GB)
    let adrp_out_of_range = 0x9000_0000u32 | (0x20000 << 5);
    let mut out_buf = adrp_out_of_range.to_le_bytes().to_vec();
    let out_orig = out_buf.clone();

    arm64.encode(&mut out_buf, now_pos);
    assert_eq!(
        out_buf, out_orig,
        "Out of range ADRP instruction should not be modified by BCJ ARM64"
    );

    // 4. Large randomized sequence roundtrip
    let mut rng = TestRng::new(0x4321_8765);
    let mut corpus = vec![0u8; 4096];
    rng.fill_bytes(&mut corpus);
    for i in (0..4096).step_by(16) {
        corpus[i..i + 4].copy_from_slice(&bl_instr.to_le_bytes());
        corpus[i + 8..i + 12].copy_from_slice(&adrp_in_range.to_le_bytes());
    }

    let orig_corpus = corpus.clone();
    arm64.encode(&mut corpus, 0x8000);
    arm64.decode(&mut corpus, 0x8000);
    assert_eq!(corpus, orig_corpus);
}

#[test]
fn test_riscv_bcj_jal_and_auipc_bijective_fidelity() {
    let mut riscv = BcjRiscv::new();
    let now_pos = 0x0001_0000u32;

    // 1. RISC-V JAL with rd=x1 (ra):
    // Opcode = 0x6F | (1 << 7) = 0xEF
    // Low byte = 0xEF, b1 = 0x00 (rd=x1 bit 0 set, bits [3:0] = 0)
    // Padded to >= 8 bytes to satisfy RISC-V lookahead window
    let jal_ra = [0xEF, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut buf = jal_ra.to_vec();
    let jal_orig = buf.clone();

    riscv.encode(&mut buf, now_pos);
    assert_ne!(buf, jal_orig);
    riscv.decode(&mut buf, now_pos);
    assert_eq!(buf, jal_orig);

    // 2. RISC-V JAL with rd=x0 (zero) - should be SKIPPED
    let jal_zero = [0x6F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut buf_zero = jal_zero.to_vec();
    riscv.encode(&mut buf_zero, now_pos);
    assert_eq!(buf_zero, jal_zero.to_vec());

    // 3. RISC-V AUIPC + ADDI pair:
    // auipc a0 (x10), 0x1000
    // addi  a0, a0, 0x20
    let auipc_inst = 0x0000_1517u32; // auipc a0, 1 (rd=10 -> 0x500 in [11:7] -> 0x517)
    let addi_inst  = 0x0205_0513u32; // addi a0, a0, 32 (rs1=10 -> bits [19:15]=10)
    let mut pair_buf = Vec::new();
    pair_buf.extend_from_slice(&auipc_inst.to_le_bytes());
    pair_buf.extend_from_slice(&addi_inst.to_le_bytes());
    let pair_orig = pair_buf.clone();

    riscv.encode(&mut pair_buf, now_pos);
    assert_ne!(pair_buf, pair_orig);

    // Verify that special AUIPC transformed format was used:
    // rd becomes x2 (sp -> 2 << 7 = 0x100)
    let enc_auipc = u32::from_le_bytes([pair_buf[0], pair_buf[1], pair_buf[2], pair_buf[3]]);
    assert_eq!((enc_auipc >> 7) & 0x1F, 2);

    riscv.decode(&mut pair_buf, now_pos);
    assert_eq!(pair_buf, pair_orig);

    // 4. Bijective Pseudo-random Corpus with Fake Decode resilience
    let mut rng = TestRng::new(0x9988_7766);
    let mut corpus = vec![0u8; 8192];
    rng.fill_bytes(&mut corpus);

    // Inject valid pairs and synthetic collisions
    for i in (0..8192 - 16).step_by(24) {
        corpus[i..i + 8].copy_from_slice(&jal_ra);
        corpus[i + 8..i + 12].copy_from_slice(&auipc_inst.to_le_bytes());
        corpus[i + 12..i + 16].copy_from_slice(&addi_inst.to_le_bytes());
    }

    let orig_corpus = corpus.clone();
    let mut stream_enc = BcjStreamFilter::new(BcjRiscv::new(), 0x1000);
    let encoded_corpus = stream_enc.process_all(&corpus, true);

    let mut stream_dec = BcjStreamFilter::new(BcjRiscv::new(), 0x1000);
    let decoded_corpus = stream_dec.process_all(&encoded_corpus, false);

    assert_eq!(
        orig_corpus, decoded_corpus,
        "RISC-V BCJ full stream bijection failed"
    );
}

#[test]
fn test_all_architectures_exhaustive_boundary_sizes() {
    let mut rng = TestRng::new(0x55AA_55AA);
    let boundary_sizes = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 15, 16, 17, 31, 32, 33, 63, 64, 65, 1024,
    ];

    for &sz in &boundary_sizes {
        let mut data = vec![0u8; sz];
        rng.fill_bytes(&mut data);
        let orig = data.clone();

        // 1. x86
        let mut x86_stream = BcjStreamFilter::new(BcjX86::new(), 0);
        let enc = x86_stream.process_all(&data, true);
        let mut x86_dec = BcjStreamFilter::new(BcjX86::new(), 0);
        let dec = x86_dec.process_all(&enc, false);
        assert_eq!(orig, dec, "x86 failed on size {}", sz);

        // 2. ARM
        let mut arm_stream = BcjStreamFilter::new(BcjArm::new(), 0);
        let enc = arm_stream.process_all(&data, true);
        let mut arm_dec = BcjStreamFilter::new(BcjArm::new(), 0);
        let dec = arm_dec.process_all(&enc, false);
        assert_eq!(orig, dec, "ARM failed on size {}", sz);

        // 3. ARM64
        let mut arm64_stream = BcjStreamFilter::new(BcjArm64::new(), 0);
        let enc = arm64_stream.process_all(&data, true);
        let mut arm64_dec = BcjStreamFilter::new(BcjArm64::new(), 0);
        let dec = arm64_dec.process_all(&enc, false);
        assert_eq!(orig, dec, "ARM64 failed on size {}", sz);

        // 4. RISC-V
        let mut riscv_stream = BcjStreamFilter::new(BcjRiscv::new(), 0);
        let enc = riscv_stream.process_all(&data, true);
        let mut riscv_dec = BcjStreamFilter::new(BcjRiscv::new(), 0);
        let dec = riscv_dec.process_all(&enc, false);
        assert_eq!(orig, dec, "RISC-V failed on size {}", sz);
    }
}
