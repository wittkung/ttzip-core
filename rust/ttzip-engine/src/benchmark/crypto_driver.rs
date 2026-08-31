// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified Cryptographic and Multimodal Checksum Benchmark Driver Suite.
//!
//! Provides a standardized `CryptoBenchmarkDriver` trait and implementations covering:
//! 1. Adler-32 (ARMv8 NEON UDOT & scalar deferred modulo)
//! 2. CRC-32 (12-Way PMULL / 8-Way small packet hw fold)
//! 3. CRC-64 (ECMA-182 precomputed lookup table)
//! 4. XXH3-64 (Vectorized 64-byte stripe accumulation)
//! 5. XXH3-128 (128-bit wide high-speed hash)
//! 6. BLAKE3 (SIMD tree hash & 256-bit output)
//! 7. WinZip AES-256 (PBKDF2-HMAC-SHA1 + CTR + HMAC-SHA1-10)
//! 8. 7z AES-256 (ARMv8 SHA256H KDF + CBC)
//! 9. ZipCrypto (PKZIP 3-key stream cipher)
//! 10. TTZip Vault AES-256-GCM (NIST SP 800-38D + zeroize)
//! 11. TTZip Vault ChaCha20-Poly1305 (RFC 8439 AEAD + zeroize)

use crate::crypto::{
    adler32_fast, aes256_cbc_decrypt, aes256_cbc_encrypt, blake3, chacha20_poly1305_decrypt,
    chacha20_poly1305_encrypt, crc32_fast, crc64_fast, derive_7z_key_arm64,
    vault::{aes256_gcm_decrypt, aes256_gcm_encrypt},
    winzip_aes256_decrypt_and_verify, winzip_aes256_encrypt_and_tag, xxh3_128_bytes, xxh3_64,
    ZipCryptoKeys,
};
use crate::types::TTZipStatus;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Cryptographic primitive classification category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CryptoCategory {
    Checksum,
    NonCryptographicHash,
    CryptographicHash,
    StreamCipher,
    AuthenticatedCipher,
}

impl CryptoCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Checksum => "Checksum",
            Self::NonCryptographicHash => "Non-Crypto Hash",
            Self::CryptographicHash => "Crypto Hash",
            Self::StreamCipher => "Stream Cipher",
            Self::AuthenticatedCipher => "AEAD / Encrypted",
        }
    }
}

/// Unified benchmark driver trait for hashing and cryptographic primitives.
pub trait CryptoBenchmarkDriver: Send + Sync {
    /// Canonical identifier for the cryptographic algorithm.
    fn algorithm_id(&self) -> &'static str;

    /// Functional category (Checksum, Hash, Stream Cipher, AEAD).
    fn category(&self) -> CryptoCategory;

    /// Descriptive human-readable display name.
    fn display_name(&self) -> String {
        self.algorithm_id().to_string()
    }

    /// Whether this primitive provides encryption/decryption (true) or one-way digest (false).
    fn is_encryption(&self) -> bool {
        matches!(
            self.category(),
            CryptoCategory::StreamCipher | CryptoCategory::AuthenticatedCipher
        )
    }

    /// Executes single-pass in-memory digest computation or encryption of `src`.
    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus>;

    /// Executes verification of digest or decryption of `processed`, comparing with `orig`.
    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus>;
}

// ============================================================================
// 1. Checksum & Hash Drivers
// ============================================================================

/// Adler-32 Checksum Benchmark Driver.
pub struct Adler32BenchmarkDriver;

impl CryptoBenchmarkDriver for Adler32BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Adler-32"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::Checksum
    }

    fn display_name(&self) -> String {
        "Adler-32 (ARMv8 NEON UDOT)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let val = adler32_fast(1, src);
        Ok(val.to_be_bytes().to_vec())
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() != 4 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let expected = adler32_fast(1, orig).to_be_bytes();
        Ok(processed == expected)
    }
}

/// CRC-32 (IEEE 802.3) Checksum Benchmark Driver.
pub struct Crc32BenchmarkDriver;

impl CryptoBenchmarkDriver for Crc32BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "CRC-32"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::Checksum
    }

    fn display_name(&self) -> String {
        "CRC-32 (12-Way PMULL)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let val = crc32_fast(0, src);
        Ok(val.to_be_bytes().to_vec())
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() != 4 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let expected = crc32_fast(0, orig).to_be_bytes();
        Ok(processed == expected)
    }
}

/// CRC-64 (ECMA-182) Checksum Benchmark Driver.
pub struct Crc64BenchmarkDriver;

impl CryptoBenchmarkDriver for Crc64BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "CRC-64-ECMA"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::Checksum
    }

    fn display_name(&self) -> String {
        "CRC-64 (ECMA-182)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let val = crc64_fast(src);
        Ok(val.to_be_bytes().to_vec())
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() != 8 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let expected = crc64_fast(orig).to_be_bytes();
        Ok(processed == expected)
    }
}

/// XXH3 64-bit Non-Cryptographic Hash Benchmark Driver.
pub struct Xxh3_64BenchmarkDriver;

impl CryptoBenchmarkDriver for Xxh3_64BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "XXH3-64"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::NonCryptographicHash
    }

    fn display_name(&self) -> String {
        "XXH3-64 (Vectorized Stripe)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let val = xxh3_64(src);
        Ok(val.to_be_bytes().to_vec())
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() != 8 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let expected = xxh3_64(orig).to_be_bytes();
        Ok(processed == expected)
    }
}

/// XXH3 128-bit Hash Benchmark Driver.
pub struct Xxh3_128BenchmarkDriver;

impl CryptoBenchmarkDriver for Xxh3_128BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "XXH3-128"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::NonCryptographicHash
    }

    fn display_name(&self) -> String {
        "XXH3-128 (Dual-Lane Vector)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let bytes = xxh3_128_bytes(src);
        Ok(bytes.to_vec())
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() != 16 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let expected = xxh3_128_bytes(orig);
        Ok(processed == expected)
    }
}

/// BLAKE3 Cryptographic Tree Hash Benchmark Driver.
pub struct Blake3BenchmarkDriver;

impl CryptoBenchmarkDriver for Blake3BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "BLAKE3"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::CryptographicHash
    }

    fn display_name(&self) -> String {
        "BLAKE3 (SIMD Tree Hash)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let digest = blake3(src);
        Ok(digest.to_vec())
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() != 32 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let expected = blake3(orig);
        Ok(processed == expected)
    }
}

// ============================================================================
// 2. Encryption & AEAD Drivers
// ============================================================================

const BENCH_PASSWORD: &str = "TTZipBenchmarkPass2026!";
const BENCH_SALT_16: [u8; 16] = [0x5A; 16];
const BENCH_KEY_32: [u8; 32] = [0x42; 32];
const BENCH_IV_16: [u8; 16] = [0x24; 16];
const BENCH_NONCE_12: [u8; 12] = [0x19; 12];
const BENCH_AAD: &[u8] = b"TTZipVaultBenchmarkAADHeader";

/// WinZip AES-256 (PBKDF2-HMAC-SHA1 + AES-CTR + HMAC-SHA1-10) Benchmark Driver.
pub struct WinZipAes256BenchmarkDriver;

impl CryptoBenchmarkDriver for WinZipAes256BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "WinZip-AES256"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::AuthenticatedCipher
    }

    fn display_name(&self) -> String {
        "WinZip AES-256 (PBKDF2 + CTR)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let mut out = Vec::with_capacity(src.len() + 28);
        winzip_aes256_encrypt_and_tag(BENCH_PASSWORD, &BENCH_SALT_16, src, &mut out)?;
        Ok(out)
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        let mut decrypted = vec![0u8; orig.len()];
        let len = winzip_aes256_decrypt_and_verify(BENCH_PASSWORD, processed, &mut decrypted)?;
        Ok(len == orig.len() && decrypted == orig)
    }
}

/// 7z AES-256 (SHA-256 KDF + AES-256-CBC) Benchmark Driver.
pub struct SevenZAes256BenchmarkDriver;

impl CryptoBenchmarkDriver for SevenZAes256BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "7z-AES256"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::AuthenticatedCipher
    }

    fn display_name(&self) -> String {
        "7z AES-256 (SHA256H KDF + CBC)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let key = derive_7z_key_arm64(BENCH_PASSWORD, &BENCH_SALT_16, 6);
        let pad_len = 16 - (src.len() % 16);
        let total_len = src.len() + pad_len;

        let mut padded = Vec::with_capacity(total_len);
        padded.extend_from_slice(src);
        padded.resize(total_len, pad_len as u8);

        let mut cipher = vec![0u8; total_len];
        aes256_cbc_encrypt(&key, &BENCH_IV_16, &padded, &mut cipher)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        Ok(cipher)
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        let key = derive_7z_key_arm64(BENCH_PASSWORD, &BENCH_SALT_16, 6);
        let mut decrypted = vec![0u8; processed.len()];
        aes256_cbc_decrypt(&key, &BENCH_IV_16, processed, &mut decrypted)
            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

        if decrypted.len() < orig.len() {
            return Ok(false);
        }
        Ok(&decrypted[..orig.len()] == orig)
    }
}

/// PKZIP Traditional ZipCrypto Stream Cipher Benchmark Driver.
pub struct ZipCryptoBenchmarkDriver;

impl CryptoBenchmarkDriver for ZipCryptoBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "ZipCrypto"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::StreamCipher
    }

    fn display_name(&self) -> String {
        "ZipCrypto (PKZIP Stream)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let mut keys = ZipCryptoKeys::from_password(BENCH_PASSWORD.as_bytes());
        let mut out = src.to_vec();
        keys.encrypt_slice(&mut out);
        Ok(out)
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        let mut keys = ZipCryptoKeys::from_password(BENCH_PASSWORD.as_bytes());
        let mut decrypted = processed.to_vec();
        keys.decrypt_slice(&mut decrypted);
        Ok(decrypted == orig)
    }
}

/// TTZip Vault AES-256-GCM AEAD Benchmark Driver.
pub struct VaultAesGcmBenchmarkDriver;

impl CryptoBenchmarkDriver for VaultAesGcmBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Vault-AES-GCM"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::AuthenticatedCipher
    }

    fn display_name(&self) -> String {
        "TTZip Vault (AES-256-GCM)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let mut cipher = vec![0u8; src.len()];
        let mut tag = [0u8; 16];
        aes256_gcm_encrypt(
            &BENCH_KEY_32,
            &BENCH_NONCE_12,
            src,
            BENCH_AAD,
            &mut cipher,
            &mut tag,
        )?;

        let mut payload = Vec::with_capacity(16 + cipher.len());
        payload.extend_from_slice(&tag);
        payload.extend_from_slice(&cipher);
        Ok(payload)
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() < 16 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&processed[..16]);
        let cipher = &processed[16..];

        let mut decrypted = vec![0u8; cipher.len()];
        aes256_gcm_decrypt(
            &BENCH_KEY_32,
            &BENCH_NONCE_12,
            cipher,
            BENCH_AAD,
            &tag,
            &mut decrypted,
        )?;
        Ok(decrypted == orig)
    }
}

/// TTZip Vault ChaCha20-Poly1305 AEAD Benchmark Driver.
pub struct VaultChaChaPolyBenchmarkDriver;

impl CryptoBenchmarkDriver for VaultChaChaPolyBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Vault-ChaCha20-Poly1305"
    }

    fn category(&self) -> CryptoCategory {
        CryptoCategory::AuthenticatedCipher
    }

    fn display_name(&self) -> String {
        "TTZip Vault (ChaCha20-Poly1305)".to_string()
    }

    fn bench_process(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let mut cipher = vec![0u8; src.len()];
        let mut tag = [0u8; 16];
        chacha20_poly1305_encrypt(
            &BENCH_KEY_32,
            &BENCH_NONCE_12,
            src,
            BENCH_AAD,
            &mut cipher,
            &mut tag,
        )?;

        let mut payload = Vec::with_capacity(16 + cipher.len());
        payload.extend_from_slice(&tag);
        payload.extend_from_slice(&cipher);
        Ok(payload)
    }

    fn bench_verify_or_decrypt(&self, processed: &[u8], orig: &[u8]) -> Result<bool, TTZipStatus> {
        if processed.len() < 16 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&processed[..16]);
        let cipher = &processed[16..];

        let mut decrypted = vec![0u8; cipher.len()];
        chacha20_poly1305_decrypt(
            &BENCH_KEY_32,
            &BENCH_NONCE_12,
            cipher,
            BENCH_AAD,
            &tag,
            &mut decrypted,
        )?;
        Ok(decrypted == orig)
    }
}

// ============================================================================
// 3. Matrix Multi-Engine Benchmark Dispatcher & Result Types
// ============================================================================

/// Result of a single cryptographic benchmark point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CryptoBenchmarkPointResult {
    pub algorithm: String,
    pub category: String,
    pub display_name: String,
    pub input_size_bytes: usize,
    pub output_size_bytes: usize,
    pub process_throughput_mbs: f64,
    pub verify_throughput_mbs: f64,
    pub process_time_nanos: u64,
    pub verify_time_nanos: u64,
    pub is_verified: bool,
}

/// Composite report for the full cryptographic benchmark matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CryptoBenchmarkMatrixReport {
    pub total_engines_evaluated: usize,
    pub corpus_size_bytes: usize,
    pub peak_process_throughput_mbs: f64,
    pub peak_verify_throughput_mbs: f64,
    pub points: Vec<CryptoBenchmarkPointResult>,
    pub passed_gate: bool,
}

impl CryptoBenchmarkMatrixReport {
    /// Prints a structured ASCII report table to stdout.
    pub fn print_table(&self) {
        println!("==========================================================================================================================");
        println!("🔐 TTZip Unified Cryptography & Verification Matrix (Total Engines: {})", self.points.len());
        println!("==========================================================================================================================");
        println!("[Idx] Algorithm               | Category          | In Size   | Out Size  | Process Speed | Verify Speed  | Verification");
        println!("--------------------------------------------------------------------------------------------------------------------------");
        for (idx, pt) in self.points.iter().enumerate() {
            println!(
                "[{:>2}] {:<25} | {:<17} | {:>7} B | {:>7} B | {:>11.1} MB/s | {:>10.1} MB/s | {}",
                idx + 1,
                pt.display_name,
                pt.category,
                pt.input_size_bytes,
                pt.output_size_bytes,
                pt.process_throughput_mbs,
                pt.verify_throughput_mbs,
                if pt.is_verified { "✅ PASS" } else { "❌ FAIL" }
            );
        }
        println!("--------------------------------------------------------------------------------------------------------------------------");
        println!(
            "Summary: {} Engines | Peak Process: {:.1} MB/s | Peak Verify: {:.1} MB/s | Gate: {}",
            self.total_engines_evaluated,
            self.peak_process_throughput_mbs,
            self.peak_verify_throughput_mbs,
            if self.passed_gate { "✅ PASS" } else { "❌ FAIL" }
        );
        println!("==========================================================================================================================");
    }
}

/// Matrix Cryptographic Benchmark Runner.
pub struct MatrixCryptoDriver;

impl MatrixCryptoDriver {
    /// Returns the full list of all 11 standardized cryptographic and checksum drivers.
    pub fn all_drivers() -> Vec<Box<dyn CryptoBenchmarkDriver>> {
        vec![
            Box::new(Adler32BenchmarkDriver),
            Box::new(Crc32BenchmarkDriver),
            Box::new(Crc64BenchmarkDriver),
            Box::new(Xxh3_64BenchmarkDriver),
            Box::new(Xxh3_128BenchmarkDriver),
            Box::new(Blake3BenchmarkDriver),
            Box::new(WinZipAes256BenchmarkDriver),
            Box::new(SevenZAes256BenchmarkDriver),
            Box::new(ZipCryptoBenchmarkDriver),
            Box::new(VaultAesGcmBenchmarkDriver),
            Box::new(VaultChaChaPolyBenchmarkDriver),
        ]
    }

    /// Finds a driver by canonical algorithm identifier.
    pub fn find_driver(id: &str) -> Option<Box<dyn CryptoBenchmarkDriver>> {
        Self::all_drivers()
            .into_iter()
            .find(|d| d.algorithm_id().eq_ignore_ascii_case(id))
    }

    /// Runs all cryptographic benchmark drivers against a given corpus buffer.
    pub fn run_benchmark(corpus: &[u8]) -> CryptoBenchmarkMatrixReport {
        let drivers = Self::all_drivers();
        let mut points = Vec::with_capacity(drivers.len());
        let mut peak_process = 0.0f64;
        let mut peak_verify = 0.0f64;
        let mut all_passed = true;

        for driver in &drivers {
            let corpus_len = corpus.len();

            // Measure processing (digest or encryption)
            let t0 = Instant::now();
            let processed = match driver.bench_process(corpus) {
                Ok(res) => res,
                Err(_) => {
                    all_passed = false;
                    continue;
                }
            };
            let process_time = t0.elapsed();
            let process_nanos = process_time.as_nanos().max(1) as u64;
            let process_mbs = (corpus_len as f64 / (1024.0 * 1024.0)) / process_time.as_secs_f64().max(1e-9);

            // Measure verification or decryption
            let t1 = Instant::now();
            let verified = driver.bench_verify_or_decrypt(&processed, corpus).unwrap_or_default();
            let verify_time = t1.elapsed();
            let verify_nanos = verify_time.as_nanos().max(1) as u64;
            let verify_mbs = (corpus_len as f64 / (1024.0 * 1024.0)) / verify_time.as_secs_f64().max(1e-9);

            if !verified {
                all_passed = false;
            }

            if process_mbs > peak_process {
                peak_process = process_mbs;
            }
            if verify_mbs > peak_verify {
                peak_verify = verify_mbs;
            }

            points.push(CryptoBenchmarkPointResult {
                algorithm: driver.algorithm_id().to_string(),
                category: driver.category().as_str().to_string(),
                display_name: driver.display_name(),
                input_size_bytes: corpus_len,
                output_size_bytes: processed.len(),
                process_throughput_mbs: process_mbs,
                verify_throughput_mbs: verify_mbs,
                process_time_nanos: process_nanos,
                verify_time_nanos: verify_nanos,
                is_verified: verified,
            });
        }

        CryptoBenchmarkMatrixReport {
            total_engines_evaluated: points.len(),
            corpus_size_bytes: corpus.len(),
            peak_process_throughput_mbs: peak_process,
            peak_verify_throughput_mbs: peak_verify,
            points,
            passed_gate: all_passed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_11_drivers_process_and_verify_roundtrip() {
        let corpus = b"High-speed cryptographic verification payload testing full matrix drivers 2026.";
        let report = MatrixCryptoDriver::run_benchmark(corpus);

        assert_eq!(report.total_engines_evaluated, 11);
        assert!(report.passed_gate, "All 11 cryptographic drivers must pass roundtrip verification");
        for pt in &report.points {
            assert!(pt.is_verified, "Engine {} failed verification", pt.algorithm);
        }
    }
}
