// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Streaming cryptographic hasher state machine exposed via Mozilla UniFFI.
//!
//! Provides thread-safe, heap-minimal streaming wrappers for hardware-accelerated
//! BLAKE3 and XXH3-64/128 hashing engines.

use std::sync::Arc;

pub mod blake3 {
    pub use crate::crypto::blake3::Hasher;
}

pub mod xxhash_rust {
    pub mod xxh3 {
        use crate::crypto::xxh3::{Xxh3_128, Xxh3_64};

        /// Composite streaming XXH3 state machine maintaining concurrent 64-bit and 128-bit state.
        #[derive(Clone, Default)]
        pub struct Xxh3 {
            hasher_64: Xxh3_64,
            hasher_128: Xxh3_128,
        }

        impl Xxh3 {
            /// Creates a new unseeded XXH3 state machine.
            #[inline]
            pub fn new() -> Self {
                Self {
                    hasher_64: Xxh3_64::new(),
                    hasher_128: Xxh3_128::new(),
                }
            }

            /// Creates a new XXH3 state machine initialized with a 64-bit seed.
            #[inline]
            pub fn with_seed(seed: u64) -> Self {
                Self {
                    hasher_64: Xxh3_64::with_seed(seed),
                    hasher_128: Xxh3_128::with_seed(seed),
                }
            }

            /// Ingests input bytes into both 64-bit and 128-bit accumulators.
            #[inline]
            pub fn update(&mut self, input: &[u8]) {
                self.hasher_64.update(input);
                self.hasher_128.update(input);
            }

            /// Computes the current 64-bit digest without invalidating the running state.
            #[inline]
            pub fn digest(&self) -> u64 {
                self.hasher_64.clone().finalize()
            }

            /// Computes the current 128-bit digest as a little-endian unsigned 128-bit integer.
            #[inline]
            pub fn digest128(&self) -> u128 {
                let bytes = self.hasher_128.clone().finalize_bytes();
                u128::from_le_bytes(bytes)
            }

            /// Computes the current 128-bit digest formatted as a 32-character hex string.
            #[inline]
            pub fn digest128_hex(&self) -> String {
                let bytes = self.hasher_128.clone().finalize_bytes();
                super::super::bytes_to_hex(&bytes)
            }
        }
    }
}

/// Converts a byte slice into a lowercase hexadecimal string without external dependencies.
#[inline]
pub(crate) fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    s
}

/// Thread-safe streaming BLAKE3 hasher object exposed to Swift and cross-platform SDKs.
#[derive(uniffi::Object)]
pub struct UniFFIBlake3Hasher {
    inner: parking_lot::Mutex<blake3::Hasher>,
}

#[uniffi::export]
impl UniFFIBlake3Hasher {
    /// Creates a new streaming BLAKE3 hasher instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: parking_lot::Mutex::new(blake3::Hasher::new()),
        })
    }

    /// Appends incoming byte slice into the running BLAKE3 tree stack state.
    pub fn update(&self, data: &[u8]) {
        self.inner.lock().update(data);
    }

    /// Finalizes the hash tree and returns a 64-character lowercase hex digest string.
    pub fn finalize_hex(&self) -> String {
        let digest = self.inner.lock().finalize();
        bytes_to_hex(&digest)
    }
}

/// Thread-safe streaming XXH3 hasher object exposed to Swift and cross-platform SDKs.
#[derive(uniffi::Object)]
pub struct UniFFIXxh3Hasher {
    inner: parking_lot::Mutex<xxhash_rust::xxh3::Xxh3>,
}

#[uniffi::export]
impl UniFFIXxh3Hasher {
    /// Creates a new streaming XXH3 hasher instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: parking_lot::Mutex::new(xxhash_rust::xxh3::Xxh3::new()),
        })
    }

    /// Appends incoming byte slice into the running XXH3 accumulators.
    pub fn update(&self, data: &[u8]) {
        self.inner.lock().update(data);
    }

    /// Computes and returns the 64-bit XXH3 digest.
    pub fn digest64(&self) -> u64 {
        self.inner.lock().digest()
    }

    /// Computes and returns the 128-bit XXH3 digest formatted as a 32-character hex string.
    pub fn digest128_hex(&self) -> String {
        self.inner.lock().digest128_hex()
    }
}
