// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! WinZip AES-128/192/256 Authenticated Encryption & AE-1/AE-2 State Machine.
//!
//! Compliant with WinZip AES Encryption Specification (AE-1 and AE-2 formats),
//! PBKDF2-HMAC-SHA1 1000-round key derivation, Little-Endian 128-bit AES-CTR streaming,
//! and HMAC-SHA1-80 truncated 10-byte constant-time authentication.

use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::sha1::{pbkdf2_sha1, sha1, FastSha1};
use crate::crypto::crc32::crc32_fast;
use crate::types::TTZipStatus;

/// Size of the WinZip AES Password Verification Value (PVV) in bytes.
pub const WINZIP_AES_PVV_LEN: usize = 2;

/// Size of the truncated HMAC-SHA1 authentication tag in bytes (80 bits).
pub const WINZIP_AES_AUTH_TAG_LEN: usize = 10;

/// Number of PBKDF2 iterations defined by the WinZip AES specification.
pub const WINZIP_AES_PBKDF2_ROUNDS: u32 = 1000;

/// Compares two byte slices in constant time to prevent side-channel timing leaks.
#[inline]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

/// WinZip AES key strength and encryption level.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WinZipAesKeyStrength {
    /// 128-bit AES encryption: 16-byte key, 8-byte salt.
    Aes128 = 1,
    /// 192-bit AES encryption: 24-byte key, 12-byte salt.
    Aes192 = 2,
    /// 256-bit AES encryption: 32-byte key, 16-byte salt.
    Aes256 = 3,
}

impl WinZipAesKeyStrength {
    /// Returns the encryption and authentication key length in bytes (16, 24, or 32).
    #[inline]
    pub const fn key_len(self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes192 => 24,
            Self::Aes256 => 32,
        }
    }

    /// Returns the salt length in bytes (8, 12, or 16).
    #[inline]
    pub const fn salt_len(self) -> usize {
        match self {
            Self::Aes128 => 8,
            Self::Aes192 => 12,
            Self::Aes256 => 16,
        }
    }

    /// Returns total key material derived via PBKDF2: `2 * key_len + 2` bytes.
    #[inline]
    pub const fn total_derived_len(self) -> usize {
        2 * self.key_len() + WINZIP_AES_PVV_LEN
    }

    /// Returns the WinZip extra field strength code (1, 2, or 3).
    #[inline]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Parses strength from the extra field mode byte (1 = 128-bit, 2 = 192-bit, 3 = 256-bit).
    #[inline]
    pub fn from_code(code: u8) -> Result<Self, TTZipStatus> {
        match code {
            1 => Ok(Self::Aes128),
            2 => Ok(Self::Aes192),
            3 => Ok(Self::Aes256),
            _ => Err(TTZipStatus::ErrUnsupportedFeature),
        }
    }
}

impl Zeroize for WinZipAesKeyStrength {
    #[inline]
    fn zeroize(&mut self) {}
}

/// WinZip AES specification version defining CRC behavior (AE-1 vs AE-2).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
#[repr(u16)]
pub enum WinZipAesVersion {
    /// AE-1: Standard CRC32 stored in header and verified against uncompressed stream.
    AE1 = 0x0001,
    /// AE-2: CRC32 field in header forced to 0; CRC verification suppressed; HMAC is sole oracle.
    #[default]
    AE2 = 0x0002,
}

impl WinZipAesVersion {
    /// Returns the 2-byte vendor version code (0x0001 or 0x0002).
    #[inline]
    pub const fn code(self) -> u16 {
        self as u16
    }

    /// Parses version code from extra field.
    #[inline]
    pub fn from_code(code: u16) -> Result<Self, TTZipStatus> {
        match code {
            0x0001 => Ok(Self::AE1),
            0x0002 => Ok(Self::AE2),
            _ => Err(TTZipStatus::ErrUnsupportedFeature),
        }
    }

    /// Returns `true` if this version suppresses CRC-32 (forces CRC = 0).
    #[inline]
    pub const fn suppresses_crc(self) -> bool {
        matches!(self, Self::AE2)
    }
}

/// Secure container for derived WinZip AES key material with automatic memory zeroization.
#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct WinZipAesDerivedKeys {
    #[zeroize(skip)]
    pub strength: WinZipAesKeyStrength,
    pub enc_key: [u8; 32],
    pub auth_key: [u8; 32],
    pub pwd_verify_2b: [u8; WINZIP_AES_PVV_LEN],
}

impl WinZipAesDerivedKeys {
    /// Returns the active encryption key slice corresponding to key strength.
    #[inline]
    pub fn enc_key_slice(&self) -> &[u8] {
        &self.enc_key[..self.strength.key_len()]
    }

    /// Returns the active authentication key slice corresponding to key strength.
    #[inline]
    pub fn auth_key_slice(&self) -> &[u8] {
        &self.auth_key[..self.strength.key_len()]
    }
}

/// PBKDF2-HMAC-SHA1 Key Derivation Function for WinZip AES.
pub struct WinZipAesKdf;

impl WinZipAesKdf {
    /// Derives encryption key, auth key, and 2-byte verification value.
    pub fn derive(
        strength: WinZipAesKeyStrength,
        password: &[u8],
        salt: &[u8],
    ) -> Result<WinZipAesDerivedKeys, TTZipStatus> {
        if salt.len() != strength.salt_len() {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let total_len = strength.total_derived_len();
        let mut key_buf = [0u8; 66];
        pbkdf2_sha1(password, salt, WINZIP_AES_PBKDF2_ROUNDS, &mut key_buf[..total_len])?;

        let klen = strength.key_len();
        let mut derived = WinZipAesDerivedKeys {
            strength,
            enc_key: [0u8; 32],
            auth_key: [0u8; 32],
            pwd_verify_2b: [0u8; WINZIP_AES_PVV_LEN],
        };

        derived.enc_key[..klen].copy_from_slice(&key_buf[..klen]);
        derived.auth_key[..klen].copy_from_slice(&key_buf[klen..2 * klen]);
        derived.pwd_verify_2b
            .copy_from_slice(&key_buf[2 * klen..2 * klen + WINZIP_AES_PVV_LEN]);

        key_buf.zeroize();
        Ok(derived)
    }
}

/// Internal AES block cipher instance holding key-expanded schedule.
enum AesBlockEngine {
    Aes128(aes::Aes128),
    Aes192(aes::Aes192),
    Aes256(aes::Aes256),
}

impl AesBlockEngine {
    fn new(strength: WinZipAesKeyStrength, key: &[u8]) -> Result<Self, TTZipStatus> {
        match strength {
            WinZipAesKeyStrength::Aes128 => {
                if key.len() < 16 {
                    return Err(TTZipStatus::ErrInvalidParam);
                }
                let arr = GenericArray::from_slice(&key[..16]);
                Ok(Self::Aes128(aes::Aes128::new(arr)))
            }
            WinZipAesKeyStrength::Aes192 => {
                if key.len() < 24 {
                    return Err(TTZipStatus::ErrInvalidParam);
                }
                let arr = GenericArray::from_slice(&key[..24]);
                Ok(Self::Aes192(aes::Aes192::new(arr)))
            }
            WinZipAesKeyStrength::Aes256 => {
                if key.len() < 32 {
                    return Err(TTZipStatus::ErrInvalidParam);
                }
                let arr = GenericArray::from_slice(&key[..32]);
                Ok(Self::Aes256(aes::Aes256::new(arr)))
            }
        }
    }

    #[inline]
    fn encrypt_block(&self, block: &mut [u8; 16]) {
        let arr = GenericArray::from_mut_slice(block);
        match self {
            Self::Aes128(cipher) => cipher.encrypt_block(arr),
            Self::Aes192(cipher) => cipher.encrypt_block(arr),
            Self::Aes256(cipher) => cipher.encrypt_block(arr),
        }
    }
}

/// Dedicated 128-bit Little-Endian counter stream cipher for WinZip AES.
///
/// Increments counter from 1 (or custom offset) in little-endian byte order:
/// `[1, 0, ..., 0]`, `[2, 0, ..., 0]`, etc.
pub struct WinZipAesCtr {
    engine: AesBlockEngine,
    counter: u128,
    keystream: [u8; 16],
    keystream_pos: usize,
}

impl WinZipAesCtr {
    /// Creates a new stream cipher instance starting with counter = 1.
    pub fn new(strength: WinZipAesKeyStrength, key: &[u8]) -> Result<Self, TTZipStatus> {
        Self::with_counter(strength, key, 1)
    }

    /// Creates a new stream cipher instance with a custom starting counter.
    pub fn with_counter(
        strength: WinZipAesKeyStrength,
        key: &[u8],
        initial_counter: u128,
    ) -> Result<Self, TTZipStatus> {
        let engine = AesBlockEngine::new(strength, key)?;
        Ok(Self {
            engine,
            counter: initial_counter,
            keystream: [0u8; 16],
            keystream_pos: 16,
        })
    }

    /// Returns the current 128-bit counter state.
    #[inline]
    pub fn counter(&self) -> u128 {
        self.counter
    }

    /// Applies AES-CTR keystream in-place to the target buffer.
    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        let mut offset = 0;
        let total = data.len();

        if self.keystream_pos < 16 && offset < total {
            let avail = 16 - self.keystream_pos;
            let take = avail.min(total - offset);
            for i in 0..take {
                data[offset + i] ^= self.keystream[self.keystream_pos + i];
            }
            self.keystream_pos += take;
            offset += take;
        }

        while offset + 16 <= total {
            let mut block = self.counter.to_le_bytes();
            self.counter = self.counter.wrapping_add(1);
            self.engine.encrypt_block(&mut block);

            for i in 0..16 {
                data[offset + i] ^= block[i];
            }
            offset += 16;
        }

        if offset < total {
            let mut block = self.counter.to_le_bytes();
            self.counter = self.counter.wrapping_add(1);
            self.engine.encrypt_block(&mut block);

            let rem = total - offset;
            for i in 0..rem {
                data[offset + i] ^= block[i];
            }
            self.keystream = block;
            self.keystream_pos = rem;
        }
    }

    /// Encrypts or decrypts bytes from `src` into `dst`.
    pub fn process_slice(&mut self, src: &[u8], dst: &mut [u8]) -> Result<(), TTZipStatus> {
        if dst.len() < src.len() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        dst[..src.len()].copy_from_slice(src);
        self.apply_keystream(&mut dst[..src.len()]);
        Ok(())
    }
}

impl Drop for WinZipAesCtr {
    fn drop(&mut self) {
        self.keystream.zeroize();
    }
}

/// Streaming HMAC-SHA1 authentication tag generator for WinZip AES (10-byte truncated).
pub struct WinZipAesHmac {
    k_opad: [u8; 64],
    inner: FastSha1,
}

impl WinZipAesHmac {
    /// Initializes HMAC-SHA1 with the specified authentication key.
    pub fn new(auth_key: &[u8]) -> Self {
        let mut k_pad = [0u8; 64];
        if auth_key.len() > 64 {
            let digest = sha1(auth_key);
            k_pad[..20].copy_from_slice(&digest);
        } else {
            k_pad[..auth_key.len()].copy_from_slice(auth_key);
        }

        let mut k_ipad = [0x36u8; 64];
        let mut k_opad = [0x5cu8; 64];
        for i in 0..64 {
            k_ipad[i] ^= k_pad[i];
            k_opad[i] ^= k_pad[i];
        }

        let mut inner = FastSha1::new();
        inner.update(&k_ipad);

        k_pad.zeroize();
        k_ipad.zeroize();

        Self { k_opad, inner }
    }

    /// Feeds ciphertext bytes into the HMAC calculation.
    #[inline]
    pub fn update(&mut self, ciphertext: &[u8]) {
        self.inner.update(ciphertext);
    }

    /// Finalizes and produces the 10-byte truncated HMAC-SHA1-80 authentication tag.
    pub fn finalize(mut self) -> [u8; WINZIP_AES_AUTH_TAG_LEN] {
        let inner_hash = self.inner.clone().finalize();
        let mut outer = FastSha1::new();
        outer.update(&self.k_opad);
        outer.update(&inner_hash);
        let full = outer.finalize();
        self.k_opad.zeroize();

        let mut tag = [0u8; WINZIP_AES_AUTH_TAG_LEN];
        tag.copy_from_slice(&full[..WINZIP_AES_AUTH_TAG_LEN]);
        tag
    }

    /// Verifies authentication tag in constant time.
    #[inline]
    pub fn verify_tag(
        stored: &[u8; WINZIP_AES_AUTH_TAG_LEN],
        computed: &[u8; WINZIP_AES_AUTH_TAG_LEN],
    ) -> bool {
        constant_time_eq(stored, computed)
    }
}

impl Drop for WinZipAesHmac {
    fn drop(&mut self) {
        self.k_opad.zeroize();
    }
}

/// High-throughput streaming WinZip AES encrypter with AE-1/AE-2 state machine.
pub struct WinZipAesEncrypter {
    version: WinZipAesVersion,
    strength: WinZipAesKeyStrength,
    pvv: [u8; WINZIP_AES_PVV_LEN],
    ctr: WinZipAesCtr,
    hmac: WinZipAesHmac,
    crc_acc: u32,
}

impl WinZipAesEncrypter {
    /// Initializes a new WinZip AES encrypter.
    pub fn new(
        version: WinZipAesVersion,
        strength: WinZipAesKeyStrength,
        password: &str,
        salt: &[u8],
    ) -> Result<Self, TTZipStatus> {
        let keys = WinZipAesKdf::derive(strength, password.as_bytes(), salt)?;
        let ctr = WinZipAesCtr::new(strength, keys.enc_key_slice())?;
        let hmac = WinZipAesHmac::new(keys.auth_key_slice());

        Ok(Self {
            version,
            strength,
            pvv: keys.pwd_verify_2b,
            ctr,
            hmac,
            crc_acc: 0,
        })
    }

    /// Returns the 2-byte Password Verification Value (PVV).
    #[inline]
    pub fn pvv(&self) -> [u8; WINZIP_AES_PVV_LEN] {
        self.pvv
    }

    /// Returns the configured key strength.
    #[inline]
    pub fn strength(&self) -> WinZipAesKeyStrength {
        self.strength
    }

    /// Returns the AE-1 or AE-2 specification version.
    #[inline]
    pub fn version(&self) -> WinZipAesVersion {
        self.version
    }

    /// Encrypts a chunk in-place, updating HMAC on ciphertext and CRC on plaintext (for AE-1).
    pub fn encrypt_chunk(&mut self, data: &mut [u8]) {
        if !self.version.suppresses_crc() {
            self.crc_acc = crc32_fast(self.crc_acc, data);
        }
        self.ctr.apply_keystream(data);
        self.hmac.update(data);
    }

    /// Encrypts from `plaintext` into `dst_ciphertext`.
    pub fn encrypt_slice(
        &mut self,
        plaintext: &[u8],
        dst_ciphertext: &mut [u8],
    ) -> Result<usize, TTZipStatus> {
        if dst_ciphertext.len() < plaintext.len() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        dst_ciphertext[..plaintext.len()].copy_from_slice(plaintext);
        self.encrypt_chunk(&mut dst_ciphertext[..plaintext.len()]);
        Ok(plaintext.len())
    }

    /// Finalizes encryption and returns the 10-byte authentication tag and CRC-32 (0 for AE-2).
    pub fn finalize(self) -> ([u8; WINZIP_AES_AUTH_TAG_LEN], u32) {
        let tag = self.hmac.finalize();
        let crc = if self.version.suppresses_crc() {
            0
        } else {
            self.crc_acc
        };
        (tag, crc)
    }
}

/// High-throughput streaming WinZip AES decrypter with AE-1/AE-2 state machine.
pub struct WinZipAesDecrypter {
    version: WinZipAesVersion,
    strength: WinZipAesKeyStrength,
    ctr: WinZipAesCtr,
    hmac: WinZipAesHmac,
    crc_acc: u32,
}

impl WinZipAesDecrypter {
    /// Initializes a new decrypter, immediately verifying PVV in constant time.
    ///
    /// If the password verification value fails to match, `ErrInvalidPassword`
    /// is returned immediately with zero ciphertext decryption CPU cost.
    pub fn new(
        version: WinZipAesVersion,
        strength: WinZipAesKeyStrength,
        password: &str,
        salt: &[u8],
        stored_pvv: [u8; WINZIP_AES_PVV_LEN],
    ) -> Result<Self, TTZipStatus> {
        let keys = WinZipAesKdf::derive(strength, password.as_bytes(), salt)?;

        if !constant_time_eq(&keys.pwd_verify_2b, &stored_pvv) {
            return Err(TTZipStatus::ErrInvalidPassword);
        }

        let ctr = WinZipAesCtr::new(strength, keys.enc_key_slice())?;
        let hmac = WinZipAesHmac::new(keys.auth_key_slice());

        Ok(Self {
            version,
            strength,
            ctr,
            hmac,
            crc_acc: 0,
        })
    }

    /// Returns the configured key strength.
    #[inline]
    pub fn strength(&self) -> WinZipAesKeyStrength {
        self.strength
    }

    /// Returns the AE-1 or AE-2 specification version.
    #[inline]
    pub fn version(&self) -> WinZipAesVersion {
        self.version
    }

    /// Decrypts a chunk in-place, updating HMAC on ciphertext and CRC on plaintext (for AE-1).
    pub fn decrypt_chunk(&mut self, data: &mut [u8]) {
        self.hmac.update(data);
        self.ctr.apply_keystream(data);
        if !self.version.suppresses_crc() {
            self.crc_acc = crc32_fast(self.crc_acc, data);
        }
    }

    /// Decrypts from `ciphertext` into `dst_plaintext`.
    pub fn decrypt_slice(
        &mut self,
        ciphertext: &[u8],
        dst_plaintext: &mut [u8],
    ) -> Result<usize, TTZipStatus> {
        if dst_plaintext.len() < ciphertext.len() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        dst_plaintext[..ciphertext.len()].copy_from_slice(ciphertext);
        self.decrypt_chunk(&mut dst_plaintext[..ciphertext.len()]);
        Ok(ciphertext.len())
    }

    /// Finalizes decryption, validating the 10-byte authentication tag in constant time.
    ///
    /// In AE-1 mode, also validates CRC32 if `expected_crc` is provided.
    /// In AE-2 mode, CRC validation is suppressed and returns 0.
    pub fn finalize(
        self,
        stored_auth_tag: &[u8; WINZIP_AES_AUTH_TAG_LEN],
        expected_crc: Option<u32>,
    ) -> Result<u32, TTZipStatus> {
        let computed_tag = self.hmac.finalize();
        if !constant_time_eq(&computed_tag, stored_auth_tag) {
            return Err(TTZipStatus::ErrInvalidPassword);
        }

        if !self.version.suppresses_crc() {
            if let Some(exp) = expected_crc {
                if exp != 0 && self.crc_acc != exp {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            Ok(self.crc_acc)
        } else {
            Ok(0)
        }
    }
}

/// Encrypts plaintext into a complete WinZip AES payload container.
///
/// Container format: `Salt(8|12|16) | PVV(2) | Ciphertext(N) | AuthTag(10)`
pub fn winzip_aes_encrypt_payload(
    version: WinZipAesVersion,
    strength: WinZipAesKeyStrength,
    password: &str,
    salt: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, TTZipStatus> {
    let mut enc = WinZipAesEncrypter::new(version, strength, password, salt)?;
    let pvv = enc.pvv();

    let mut payload = Vec::with_capacity(salt.len() + WINZIP_AES_PVV_LEN + plaintext.len() + WINZIP_AES_AUTH_TAG_LEN);
    payload.extend_from_slice(salt);
    payload.extend_from_slice(&pvv);

    let start = payload.len();
    payload.resize(start + plaintext.len(), 0);
    enc.encrypt_slice(plaintext, &mut payload[start..])?;

    let (tag, _) = enc.finalize();
    payload.extend_from_slice(&tag);

    Ok(payload)
}

/// Decrypts a full WinZip AES payload container and returns the plaintext and CRC-32.
pub fn winzip_aes_decrypt_payload(
    version: WinZipAesVersion,
    strength: WinZipAesKeyStrength,
    password: &str,
    enc_payload: &[u8],
    expected_crc: Option<u32>,
) -> Result<(Vec<u8>, u32), TTZipStatus> {
    let salt_len = strength.salt_len();
    let min_len = salt_len + WINZIP_AES_PVV_LEN + WINZIP_AES_AUTH_TAG_LEN;
    if enc_payload.len() < min_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let salt = &enc_payload[..salt_len];
    let mut stored_pvv = [0u8; WINZIP_AES_PVV_LEN];
    stored_pvv.copy_from_slice(&enc_payload[salt_len..salt_len + WINZIP_AES_PVV_LEN]);

    let cipher_len = enc_payload.len() - min_len;
    let cipher_start = salt_len + WINZIP_AES_PVV_LEN;
    let ciphertext = &enc_payload[cipher_start..cipher_start + cipher_len];

    let mut stored_tag = [0u8; WINZIP_AES_AUTH_TAG_LEN];
    stored_tag.copy_from_slice(&enc_payload[cipher_start + cipher_len..]);

    let mut dec = WinZipAesDecrypter::new(version, strength, password, salt, stored_pvv)?;
    let mut plaintext = vec![0u8; cipher_len];
    dec.decrypt_slice(ciphertext, &mut plaintext)?;

    let crc = dec.finalize(&stored_tag, expected_crc)?;
    Ok((plaintext, crc))
}
