// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! HMAC-SHA1 and PBKDF2-HMAC-SHA1 routines (RFC 2104 / RFC 2898 / RFC 6070).

use super::{sha1, FastSha1};
use ttzip_types::TTZipStatus;
use zeroize::Zeroize;

fn init_hmac_sha1_contexts(key: &[u8]) -> (FastSha1, FastSha1) {
    let mut k_pad = [0u8; 64];
    if key.len() > 64 {
        let digest = sha1(key);
        k_pad[..20].copy_from_slice(&digest);
    } else {
        k_pad[..key.len()].copy_from_slice(key);
    }

    let mut k_ipad = [0x36u8; 64];
    let mut k_opad = [0x5cu8; 64];
    for i in 0..64 {
        k_ipad[i] ^= k_pad[i];
        k_opad[i] ^= k_pad[i];
    }

    let mut base_inner = FastSha1::new();
    base_inner.update(&k_ipad);

    let mut base_outer = FastSha1::new();
    base_outer.update(&k_opad);

    k_pad.zeroize();
    k_ipad.zeroize();
    k_opad.zeroize();

    (base_inner, base_outer)
}

/// Computes standard 20-byte HMAC-SHA1.
pub fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let (mut inner, mut outer) = init_hmac_sha1_contexts(key);
    inner.update(data);
    let inner_hash = inner.finalize();
    outer.update(&inner_hash);
    outer.finalize()
}

/// Computes truncated 10-byte HMAC-SHA1 tag for WinZip AES.
#[inline]
pub fn hmac_sha1_10(key: &[u8], data: &[u8]) -> [u8; 10] {
    let full = hmac_sha1(key, data);
    let mut out = [0u8; 10];
    out.copy_from_slice(&full[..10]);
    out
}

/// Derives key material using PBKDF2-HMAC-SHA1.
pub fn pbkdf2_sha1(
    password: &[u8],
    salt: &[u8],
    rounds: u32,
    out_key: &mut [u8],
) -> Result<(), TTZipStatus> {
    if (password.is_empty() && salt.is_empty()) || rounds == 0 || out_key.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let (base_inner, base_outer) = init_hmac_sha1_contexts(password);
    let key_len = out_key.len();
    let blocks_needed = key_len.div_ceil(20);

    let mut u_digest = [0u8; 20];
    let mut t_digest = [0u8; 20];

    for block_idx in 1..=blocks_needed as u32 {
        let be_block = block_idx.to_be_bytes();

        let mut inner = base_inner.clone();
        inner.update(salt);
        inner.update(&be_block);
        let inner_hash = inner.finalize();

        let mut outer = base_outer.clone();
        outer.update(&inner_hash);
        u_digest = outer.finalize();
        t_digest.copy_from_slice(&u_digest);

        for _ in 1..rounds {
            let mut inner = base_inner.clone();
            inner.update(&u_digest);
            let inner_hash = inner.finalize();

            let mut outer = base_outer.clone();
            outer.update(&inner_hash);
            u_digest = outer.finalize();

            for k in 0..20 {
                t_digest[k] ^= u_digest[k];
            }
        }

        let offset = (block_idx as usize - 1) * 20;
        let copy_len = (offset + 20).min(key_len) - offset;
        out_key[offset..offset + copy_len].copy_from_slice(&t_digest[..copy_len]);
    }

    u_digest.zeroize();
    t_digest.zeroize();

    Ok(())
}
