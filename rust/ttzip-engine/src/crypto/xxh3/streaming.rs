// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Constant-memory streaming XXH3 64-bit and 128-bit hashers.

use super::{
    derive_custom_secret, xxh3_128_with_seed, xxh3_64_with_seed, xxh3_accumulate_stripe,
    xxh3_merge_acc, xxh3_scramble_acc, ACC_NB, DEFAULT_SECRET, INITIAL_ACC, MIDSIZE_MAX,
    PRIME64_1, STRIPE_LEN,
};

/// Streaming XXH3 64-bit hasher with constant 256-byte stack buffer.
#[derive(Clone)]
pub struct Xxh3_64 {
    seed: u64,
    total_len: u64,
    buffered_len: usize,
    buffer: [u8; 256],
    acc: [u64; ACC_NB],
    nb_stripes_in_block: usize,
    custom_secret: [u8; 192],
    use_custom_secret: bool,
}

impl Default for Xxh3_64 {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Xxh3_64 {
    #[inline]
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> Self {
        let mut custom_secret = [0u8; 192];
        let use_custom_secret = seed != 0;
        if use_custom_secret {
            derive_custom_secret(&mut custom_secret, &DEFAULT_SECRET, seed);
        }
        Self {
            seed,
            total_len: 0,
            buffered_len: 0,
            buffer: [0u8; 256],
            acc: INITIAL_ACC,
            nb_stripes_in_block: 0,
            custom_secret,
            use_custom_secret,
        }
    }

    #[inline(always)]
    fn consume_stripe_64(&mut self) {
        let sec = if self.use_custom_secret {
            &self.custom_secret
        } else {
            &DEFAULT_SECRET
        };
        let s = self.nb_stripes_in_block;
        let sec_off = s * 8;
        xxh3_accumulate_stripe(&mut self.acc, &self.buffer[..STRIPE_LEN], &sec[sec_off..sec_off + STRIPE_LEN]);
        self.nb_stripes_in_block += 1;
        if self.nb_stripes_in_block == 16 {
            xxh3_scramble_acc(&mut self.acc, &sec[sec.len() - STRIPE_LEN..]);
            self.nb_stripes_in_block = 0;
        }
        self.buffer.copy_within(STRIPE_LEN..self.buffered_len, 0);
        self.buffered_len -= STRIPE_LEN;
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;

        if self.total_len <= MIDSIZE_MAX as u64 {
            self.buffer[self.buffered_len..self.buffered_len + data.len()].copy_from_slice(data);
            self.buffered_len += data.len();
            return;
        }

        while !data.is_empty() {
            let space = 256 - self.buffered_len;
            let take = space.min(data.len());
            self.buffer[self.buffered_len..self.buffered_len + take].copy_from_slice(&data[..take]);
            self.buffered_len += take;
            data = &data[take..];

            while self.buffered_len > 128 {
                self.consume_stripe_64();
            }
        }
    }

    pub fn finalize(mut self) -> u64 {
        if self.total_len <= MIDSIZE_MAX as u64 {
            return xxh3_64_with_seed(&self.buffer[..self.buffered_len], self.seed);
        }

        let sec = if self.use_custom_secret {
            &self.custom_secret
        } else {
            &DEFAULT_SECRET
        };

        if self.buffered_len > STRIPE_LEN {
            let s = self.nb_stripes_in_block;
            let sec_off = s * 8;
            xxh3_accumulate_stripe(&mut self.acc, &self.buffer[..STRIPE_LEN], &sec[sec_off..sec_off + STRIPE_LEN]);
            self.nb_stripes_in_block += 1;
            if self.nb_stripes_in_block == 16 {
                xxh3_scramble_acc(&mut self.acc, &sec[sec.len() - STRIPE_LEN..]);
                self.nb_stripes_in_block = 0;
            }
        }

        let last_stripe = &self.buffer[self.buffered_len - STRIPE_LEN..self.buffered_len];
        xxh3_accumulate_stripe(&mut self.acc, last_stripe, &sec[sec.len() - STRIPE_LEN - 7..sec.len() - 7]);
        xxh3_merge_acc(&self.acc, &sec[11..], self.total_len.wrapping_mul(PRIME64_1))
    }
}

/// Streaming XXH3 128-bit hasher with constant 256-byte stack buffer.
#[derive(Clone)]
pub struct Xxh3_128 {
    seed: u64,
    total_len: u64,
    buffered_len: usize,
    buffer: [u8; 256],
    acc_low: [u64; ACC_NB],
    acc_high: [u64; ACC_NB],
    nb_stripes_in_block: usize,
    custom_secret_low: [u8; 192],
    custom_secret_high: [u8; 192],
    use_custom_secret: bool,
}

impl Default for Xxh3_128 {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Xxh3_128 {
    #[inline]
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    pub fn with_seed(seed: u64) -> Self {
        let mut custom_secret_low = [0u8; 192];
        let use_custom_secret = seed != 0;
        if use_custom_secret {
            derive_custom_secret(&mut custom_secret_low, &DEFAULT_SECRET, seed);
        }
        let mut custom_secret_high = [0u8; 192];
        derive_custom_secret(&mut custom_secret_high, &DEFAULT_SECRET, seed ^ 0xFFFFFFFFFFFFFFFF);

        Self {
            seed,
            total_len: 0,
            buffered_len: 0,
            buffer: [0u8; 256],
            acc_low: INITIAL_ACC,
            acc_high: INITIAL_ACC,
            nb_stripes_in_block: 0,
            custom_secret_low,
            custom_secret_high,
            use_custom_secret,
        }
    }

    #[inline(always)]
    fn consume_stripe_128(&mut self) {
        let sec_low = if self.use_custom_secret {
            &self.custom_secret_low
        } else {
            &DEFAULT_SECRET
        };
        let sec_high = &self.custom_secret_high;

        let s = self.nb_stripes_in_block;
        let sec_off = s * 8;
        let stripe = &self.buffer[..STRIPE_LEN];

        xxh3_accumulate_stripe(&mut self.acc_low, stripe, &sec_low[sec_off..sec_off + STRIPE_LEN]);
        xxh3_accumulate_stripe(&mut self.acc_high, stripe, &sec_high[sec_off..sec_off + STRIPE_LEN]);

        self.nb_stripes_in_block += 1;
        if self.nb_stripes_in_block == 16 {
            xxh3_scramble_acc(&mut self.acc_low, &sec_low[sec_low.len() - STRIPE_LEN..]);
            xxh3_scramble_acc(&mut self.acc_high, &sec_high[sec_high.len() - STRIPE_LEN..]);
            self.nb_stripes_in_block = 0;
        }

        self.buffer.copy_within(STRIPE_LEN..self.buffered_len, 0);
        self.buffered_len -= STRIPE_LEN;
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;

        if self.total_len <= MIDSIZE_MAX as u64 {
            self.buffer[self.buffered_len..self.buffered_len + data.len()].copy_from_slice(data);
            self.buffered_len += data.len();
            return;
        }

        while !data.is_empty() {
            let space = 256 - self.buffered_len;
            let take = space.min(data.len());
            self.buffer[self.buffered_len..self.buffered_len + take].copy_from_slice(&data[..take]);
            self.buffered_len += take;
            data = &data[take..];

            while self.buffered_len > 128 {
                self.consume_stripe_128();
            }
        }
    }

    pub fn finalize(mut self) -> (u64, u64) {
        if self.total_len <= MIDSIZE_MAX as u64 {
            return xxh3_128_with_seed(&self.buffer[..self.buffered_len], self.seed);
        }

        let sec_low = if self.use_custom_secret {
            &self.custom_secret_low
        } else {
            &DEFAULT_SECRET
        };
        let sec_high = &self.custom_secret_high;

        if self.buffered_len > STRIPE_LEN {
            let s = self.nb_stripes_in_block;
            let sec_off = s * 8;
            let stripe = &self.buffer[..STRIPE_LEN];
            xxh3_accumulate_stripe(&mut self.acc_low, stripe, &sec_low[sec_off..sec_off + STRIPE_LEN]);
            xxh3_accumulate_stripe(&mut self.acc_high, stripe, &sec_high[sec_off..sec_off + STRIPE_LEN]);
            self.nb_stripes_in_block += 1;
            if self.nb_stripes_in_block == 16 {
                xxh3_scramble_acc(&mut self.acc_low, &sec_low[sec_low.len() - STRIPE_LEN..]);
                xxh3_scramble_acc(&mut self.acc_high, &sec_high[sec_high.len() - STRIPE_LEN..]);
                self.nb_stripes_in_block = 0;
            }
        }

        let last_stripe = &self.buffer[self.buffered_len - STRIPE_LEN..self.buffered_len];
        xxh3_accumulate_stripe(&mut self.acc_low, last_stripe, &sec_low[sec_low.len() - STRIPE_LEN - 7..sec_low.len() - 7]);
        xxh3_accumulate_stripe(&mut self.acc_high, last_stripe, &sec_high[sec_high.len() - STRIPE_LEN - 7..sec_high.len() - 7]);

        let low = xxh3_merge_acc(&self.acc_low, &sec_low[11..], self.total_len.wrapping_mul(PRIME64_1));
        let high = xxh3_merge_acc(&self.acc_high, &sec_high[11..], self.total_len.wrapping_mul(PRIME64_1));

        (low, high)
    }

    pub fn finalize_bytes(self) -> [u8; 16] {
        let (low, high) = self.finalize();
        let mut out = [0u8; 16];
        out[..8].copy_from_slice(&low.to_le_bytes());
        out[8..].copy_from_slice(&high.to_le_bytes());
        out
    }
}
