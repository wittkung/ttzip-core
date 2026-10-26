// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Reference kernels and synthetic generators for AArch64 benchmark suite.

use ttzip_crypto::aes256::Aes256Context;
use ttzip_crypto::vault::aes256_encrypt_block;

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

/// Single-block serial CTR mode encryption baseline.
pub fn gcm_ctr_crypt_serial(ctx: &Aes256Context, iv: &[u8; 12], src: &[u8], dst: &mut [u8]) {
    let mut ctr_base = [0u8; 16];
    ctr_base[..12].copy_from_slice(iv);
    let total_blocks = src.len().div_ceil(16);
    for block_idx in 0..total_blocks {
        let offset = block_idx * 16;
        let counter_val = (block_idx as u32).wrapping_add(2);
        let mut c = ctr_base;
        c[12..16].copy_from_slice(&counter_val.to_be_bytes());
        let mut keystream = [0u8; 16];
        aes256_encrypt_block(ctx, &c, &mut keystream);
        let chunk_len = (src.len() - offset).min(16);
        for k in 0..chunk_len {
            dst[offset + k] = src[offset + k] ^ keystream[k];
        }
    }
}

/// Single-stream serial hardware CRC-32C loop (3-cycle RAW dependency stall).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "crc")]
pub unsafe fn crc32c_serial_hw(mut crc: u32, data: &[u8]) -> u32 {
    let mut p = data.as_ptr();
    let mut len = data.len();
    while len >= 8 {
        crc = __crc32cd(crc, (p as *const u64).read_unaligned());
        p = p.add(8);
        len -= 8;
    }
    while len > 0 {
        crc = __crc32cb(crc, *p);
        p = p.add(1);
        len -= 1;
    }
    crc
}

/// AES-256 CTR cipher with scalar cross-domain moves (GPR -> memory -> SIMD load).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_ctr_cross_domain_move(
    ctx: &Aes256Context,
    initial_counter: u64,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) {
    let mut rk = [vdupq_n_u8(0); 15];
    for (r, key) in rk.iter_mut().zip(ctx.round_keys_enc.iter()) {
        *r = vld1q_u8(key.as_ptr());
    }

    let num_blocks = len.div_ceil(16);
    let mut i = 0;
    let mut counter = initial_counter;

    while i + 8 <= num_blocks {
        let mut c0 = [0u8; 16]; c0[..8].copy_from_slice(&(counter).to_le_bytes());
        let mut c1 = [0u8; 16]; c1[..8].copy_from_slice(&(counter + 1).to_le_bytes());
        let mut c2 = [0u8; 16]; c2[..8].copy_from_slice(&(counter + 2).to_le_bytes());
        let mut c3 = [0u8; 16]; c3[..8].copy_from_slice(&(counter + 3).to_le_bytes());
        let mut c4 = [0u8; 16]; c4[..8].copy_from_slice(&(counter + 4).to_le_bytes());
        let mut c5 = [0u8; 16]; c5[..8].copy_from_slice(&(counter + 5).to_le_bytes());
        let mut c6 = [0u8; 16]; c6[..8].copy_from_slice(&(counter + 6).to_le_bytes());
        let mut c7 = [0u8; 16]; c7[..8].copy_from_slice(&(counter + 7).to_le_bytes());

        let mut b0 = vld1q_u8(c0.as_ptr());
        let mut b1 = vld1q_u8(c1.as_ptr());
        let mut b2 = vld1q_u8(c2.as_ptr());
        let mut b3 = vld1q_u8(c3.as_ptr());
        let mut b4 = vld1q_u8(c4.as_ptr());
        let mut b5 = vld1q_u8(c5.as_ptr());
        let mut b6 = vld1q_u8(c6.as_ptr());
        let mut b7 = vld1q_u8(c7.as_ptr());

        counter += 8;

        for &round_key in rk.iter().take(13) {
            b0 = vaesmcq_u8(vaeseq_u8(b0, round_key));
            b1 = vaesmcq_u8(vaeseq_u8(b1, round_key));
            b2 = vaesmcq_u8(vaeseq_u8(b2, round_key));
            b3 = vaesmcq_u8(vaeseq_u8(b3, round_key));
            b4 = vaesmcq_u8(vaeseq_u8(b4, round_key));
            b5 = vaesmcq_u8(vaeseq_u8(b5, round_key));
            b6 = vaesmcq_u8(vaeseq_u8(b6, round_key));
            b7 = vaesmcq_u8(vaeseq_u8(b7, round_key));
        }

        b0 = veorq_u8(vaeseq_u8(b0, rk[13]), rk[14]);
        b1 = veorq_u8(vaeseq_u8(b1, rk[13]), rk[14]);
        b2 = veorq_u8(vaeseq_u8(b2, rk[13]), rk[14]);
        b3 = veorq_u8(vaeseq_u8(b3, rk[13]), rk[14]);
        b4 = veorq_u8(vaeseq_u8(b4, rk[13]), rk[14]);
        b5 = veorq_u8(vaeseq_u8(b5, rk[13]), rk[14]);
        b6 = veorq_u8(vaeseq_u8(b6, rk[13]), rk[14]);
        b7 = veorq_u8(vaeseq_u8(b7, rk[13]), rk[14]);

        let block_offset = i * 16;
        let s0 = vld1q_u8(src.add(block_offset));
        let s1 = vld1q_u8(src.add(block_offset + 16));
        let s2 = vld1q_u8(src.add(block_offset + 32));
        let s3 = vld1q_u8(src.add(block_offset + 48));
        let s4 = vld1q_u8(src.add(block_offset + 64));
        let s5 = vld1q_u8(src.add(block_offset + 80));
        let s6 = vld1q_u8(src.add(block_offset + 96));
        let s7 = vld1q_u8(src.add(block_offset + 112));

        vst1q_u8(dst.add(block_offset), veorq_u8(s0, b0));
        vst1q_u8(dst.add(block_offset + 16), veorq_u8(s1, b1));
        vst1q_u8(dst.add(block_offset + 32), veorq_u8(s2, b2));
        vst1q_u8(dst.add(block_offset + 48), veorq_u8(s3, b3));
        vst1q_u8(dst.add(block_offset + 64), veorq_u8(s4, b4));
        vst1q_u8(dst.add(block_offset + 80), veorq_u8(s5, b5));
        vst1q_u8(dst.add(block_offset + 96), veorq_u8(s6, b6));
        vst1q_u8(dst.add(block_offset + 112), veorq_u8(s7, b7));

        i += 8;
    }
}

/// Adler-32 with immediate horizontal reduction per 64 bytes (8x ADDLV stalls per iteration).
#[cfg(target_arch = "aarch64")]
pub unsafe fn adler32_per_64b_immediate_reduction(adler: u32, mut p: *const u8, mut len: usize) -> u32 {
    let mut s1 = adler & 0xFFFF;
    let mut s2 = adler >> 16;

    while len >= 64 {
        let d0 = vld1q_u8(p);
        let d1 = vld1q_u8(p.add(16));
        let d2 = vld1q_u8(p.add(32));
        let d3 = vld1q_u8(p.add(48));

        let sum0 = vaddvq_u8(d0) as u32;
        let sum1 = vaddvq_u8(d1) as u32;
        let sum2 = vaddvq_u8(d2) as u32;
        let sum3 = vaddvq_u8(d3) as u32;

        s1 = s1.wrapping_add(sum0 + sum1 + sum2 + sum3);
        s2 = s2.wrapping_add(s1);

        p = p.add(64);
        len -= 64;
    }

    s1 %= 65521;
    s2 %= 65521;

    while len > 0 {
        s1 = s1.wrapping_add(*p as u32);
        s2 = s2.wrapping_add(s1);
        p = p.add(1);
        len -= 1;
    }

    s1 %= 65521;
    s2 %= 65521;

    (s2 << 16) | s1
}

/// Generates noisy comment buffer for EOCD scanner benchmark.
pub fn generate_noisy_eocd_buffer(window_size: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; window_size];
    let mut rng_state: u64 = 0x853c49e6748fea9b;
    for i in 0..window_size {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        buffer[i] = (rng_state >> 33) as u8;
    }

    for step in (100..window_size - 30).step_by(512) {
        buffer[step] = b'P';
        buffer[step + 1] = b'K';
        buffer[step + 2] = 0x01;
        buffer[step + 3] = 0x02;
    }

    let eocd_pos = window_size / 5;
    buffer[eocd_pos] = b'P';
    buffer[eocd_pos + 1] = b'K';
    buffer[eocd_pos + 2] = 0x05;
    buffer[eocd_pos + 3] = 0x06;

    buffer
}
