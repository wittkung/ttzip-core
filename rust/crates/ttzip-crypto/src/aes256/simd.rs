// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! ARM64 NEON Hardware Pipelined AES-256 Implementations.

#[cfg(target_arch = "aarch64")]
use super::Aes256Context;
#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;
#[cfg(target_arch = "aarch64")]
use zeroize::Zeroize;

/// Encrypts or decrypts bytes in CTR mode using ARM NEON AES vector instructions.
///
/// # Safety
/// Caller must ensure `src` has at least `len` readable bytes, `dst` has at least `len` writable bytes,
/// and CPU supports ARMv8 Cryptography Extensions (FEAT_AES).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_ctr_crypt_neon(
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

    // Initialize base counter vector [initial_counter (LE), 0]
    let zero = vdup_n_u64(0);
    let mut ctr_vec = vcombine_u64(vcreate_u64(initial_counter), zero);

    // Precompute constant lane increments in NEON vector register file
    let step1 = vcombine_u64(vcreate_u64(1), zero);
    let step2 = vcombine_u64(vcreate_u64(2), zero);
    let step3 = vcombine_u64(vcreate_u64(3), zero);
    let step4 = vcombine_u64(vcreate_u64(4), zero);
    let step5 = vcombine_u64(vcreate_u64(5), zero);
    let step6 = vcombine_u64(vcreate_u64(6), zero);
    let step7 = vcombine_u64(vcreate_u64(7), zero);
    let step8 = vcombine_u64(vcreate_u64(8), zero);

    while i + 8 <= num_blocks {
        // Derive 8 counter blocks directly in NEON vector registers via single-cycle vaddq_u64
        let mut b0 = vreinterpretq_u8_u64(ctr_vec);
        let mut b1 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step1));
        let mut b2 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step2));
        let mut b3 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step3));
        let mut b4 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step4));
        let mut b5 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step5));
        let mut b6 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step6));
        let mut b7 = vreinterpretq_u8_u64(vaddq_u64(ctr_vec, step7));

        // Advance base counter vector by 8 for subsequent iterations
        ctr_vec = vaddq_u64(ctr_vec, step8);

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
        let rem = if block_offset + 128 <= len {
            128
        } else {
            len - block_offset
        };

        if rem == 128 {
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
        } else {
            let mut ks = [0u8; 128];
            vst1q_u8(ks.as_mut_ptr(), b0);
            vst1q_u8(ks.as_mut_ptr().add(16), b1);
            vst1q_u8(ks.as_mut_ptr().add(32), b2);
            vst1q_u8(ks.as_mut_ptr().add(48), b3);
            vst1q_u8(ks.as_mut_ptr().add(64), b4);
            vst1q_u8(ks.as_mut_ptr().add(80), b5);
            vst1q_u8(ks.as_mut_ptr().add(96), b6);
            vst1q_u8(ks.as_mut_ptr().add(112), b7);
            for (k, &keystream_byte) in ks.iter().take(rem).enumerate() {
                *dst.add(block_offset + k) = *src.add(block_offset + k) ^ keystream_byte;
            }
            // Securely wipe sensitive keystream buffer from stack and issue compiler fence
            ks.zeroize();
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }

        i += 8;
    }

    while i < num_blocks {
        let mut b0 = vreinterpretq_u8_u64(ctr_vec);
        ctr_vec = vaddq_u64(ctr_vec, step1);

        for &round_key in rk.iter().take(13) {
            b0 = vaesmcq_u8(vaeseq_u8(b0, round_key));
        }
        b0 = veorq_u8(vaeseq_u8(b0, rk[13]), rk[14]);

        let block_offset = i * 16;
        let rem = if block_offset + 16 <= len {
            16
        } else {
            len - block_offset
        };

        if rem == 16 {
            let s0 = vld1q_u8(src.add(block_offset));
            vst1q_u8(dst.add(block_offset), veorq_u8(s0, b0));
        } else {
            let mut ks = [0u8; 16];
            vst1q_u8(ks.as_mut_ptr(), b0);
            for (k, &keystream_byte) in ks.iter().take(rem).enumerate() {
                *dst.add(block_offset + k) = *src.add(block_offset + k) ^ keystream_byte;
            }
            // Securely wipe sensitive keystream buffer from stack and issue compiler fence
            ks.zeroize();
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }

        i += 1;
    }
}

/// Decrypts bytes in CBC mode using ARM NEON AES vector instructions.
///
/// # Safety
/// Caller must ensure `src` has at least `len` readable bytes, `dst` has at least `len` writable bytes,
/// and CPU supports ARMv8 Cryptography Extensions (FEAT_AES).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_cbc_decrypt_neon(
    ctx: &Aes256Context,
    iv: &[u8; 16],
    src: *const u8,
    len: usize,
    dst: *mut u8,
) {
    let mut rk_dec = [vdupq_n_u8(0); 15];
    for (r, key) in rk_dec.iter_mut().zip(ctx.round_keys_dec.iter()) {
        *r = vld1q_u8(key.as_ptr());
    }

    let num_blocks = len / 16;
    let mut current_iv = vld1q_u8(iv.as_ptr());
    let mut i = 0;

    while i + 8 <= num_blocks {
        let offset = i * 16;
        let c0 = vld1q_u8(src.add(offset));
        let c1 = vld1q_u8(src.add(offset + 16));
        let c2 = vld1q_u8(src.add(offset + 32));
        let c3 = vld1q_u8(src.add(offset + 48));
        let c4 = vld1q_u8(src.add(offset + 64));
        let c5 = vld1q_u8(src.add(offset + 80));
        let c6 = vld1q_u8(src.add(offset + 96));
        let c7 = vld1q_u8(src.add(offset + 112));

        let mut b0 = c0;
        let mut b1 = c1;
        let mut b2 = c2;
        let mut b3 = c3;
        let mut b4 = c4;
        let mut b5 = c5;
        let mut b6 = c6;
        let mut b7 = c7;

        for &round_key in rk_dec.iter().take(13) {
            b0 = vaesimcq_u8(vaesdq_u8(b0, round_key));
            b1 = vaesimcq_u8(vaesdq_u8(b1, round_key));
            b2 = vaesimcq_u8(vaesdq_u8(b2, round_key));
            b3 = vaesimcq_u8(vaesdq_u8(b3, round_key));
            b4 = vaesimcq_u8(vaesdq_u8(b4, round_key));
            b5 = vaesimcq_u8(vaesdq_u8(b5, round_key));
            b6 = vaesimcq_u8(vaesdq_u8(b6, round_key));
            b7 = vaesimcq_u8(vaesdq_u8(b7, round_key));
        }

        b0 = veorq_u8(vaesdq_u8(b0, rk_dec[13]), rk_dec[14]);
        b1 = veorq_u8(vaesdq_u8(b1, rk_dec[13]), rk_dec[14]);
        b2 = veorq_u8(vaesdq_u8(b2, rk_dec[13]), rk_dec[14]);
        b3 = veorq_u8(vaesdq_u8(b3, rk_dec[13]), rk_dec[14]);
        b4 = veorq_u8(vaesdq_u8(b4, rk_dec[13]), rk_dec[14]);
        b5 = veorq_u8(vaesdq_u8(b5, rk_dec[13]), rk_dec[14]);
        b6 = veorq_u8(vaesdq_u8(b6, rk_dec[13]), rk_dec[14]);
        b7 = veorq_u8(vaesdq_u8(b7, rk_dec[13]), rk_dec[14]);

        let p0 = veorq_u8(b0, current_iv);
        let p1 = veorq_u8(b1, c0);
        let p2 = veorq_u8(b2, c1);
        let p3 = veorq_u8(b3, c2);
        let p4 = veorq_u8(b4, c3);
        let p5 = veorq_u8(b5, c4);
        let p6 = veorq_u8(b6, c5);
        let p7 = veorq_u8(b7, c6);

        current_iv = c7;

        vst1q_u8(dst.add(offset), p0);
        vst1q_u8(dst.add(offset + 16), p1);
        vst1q_u8(dst.add(offset + 32), p2);
        vst1q_u8(dst.add(offset + 48), p3);
        vst1q_u8(dst.add(offset + 64), p4);
        vst1q_u8(dst.add(offset + 80), p5);
        vst1q_u8(dst.add(offset + 96), p6);
        vst1q_u8(dst.add(offset + 112), p7);

        i += 8;
    }

    while i < num_blocks {
        let offset = i * 16;
        let c = vld1q_u8(src.add(offset));
        let mut b = c;
        for &round_key in rk_dec.iter().take(13) {
            b = vaesimcq_u8(vaesdq_u8(b, round_key));
        }
        b = veorq_u8(vaesdq_u8(b, rk_dec[13]), rk_dec[14]);
        let p = veorq_u8(b, current_iv);
        current_iv = c;
        vst1q_u8(dst.add(offset), p);
        i += 1;
    }
}

/// Encrypts bytes in CBC mode using ARM NEON AES vector instructions.
///
/// # Safety
/// Caller must ensure `src` has at least `len` readable bytes, `dst` has at least `len` writable bytes,
/// and CPU supports ARMv8 Cryptography Extensions (FEAT_AES).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
pub unsafe fn aes256_cbc_encrypt_neon(
    ctx: &Aes256Context,
    iv: &[u8; 16],
    src: *const u8,
    len: usize,
    dst: *mut u8,
) {
    let mut rk_enc = [vdupq_n_u8(0); 15];
    for (r, key) in rk_enc.iter_mut().zip(ctx.round_keys_enc.iter()) {
        *r = vld1q_u8(key.as_ptr());
    }

    let num_blocks = len / 16;
    let mut current_iv = vld1q_u8(iv.as_ptr());

    for i in 0..num_blocks {
        let offset = i * 16;
        let p = vld1q_u8(src.add(offset));
        let mut b = veorq_u8(p, current_iv);
        for &round_key in rk_enc.iter().take(13) {
            b = vaesmcq_u8(vaeseq_u8(b, round_key));
        }
        b = veorq_u8(vaeseq_u8(b, rk_enc[13]), rk_enc[14]);
        current_iv = b;
        vst1q_u8(dst.add(offset), b);
    }
}
