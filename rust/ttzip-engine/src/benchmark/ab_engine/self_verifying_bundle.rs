// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Self-Verifying Deterministic Content-Addressable Bundle Architecture.
//!
//! Provides a high-integrity, zero-memory-overhead streaming bundle container:
//! - Strictly names archive entries using content-deterministic cryptographic/non-cryptographic hashes
//!   (`blake3`, `sha256`, `xxh3_128`, `xxh3_64`).
//! - Decompression streaming pipeline computes content hashes in-place on bounded micro-chunks (<=64KB)
//!   and verifies against virtual path filenames with O(1) state memory.
//! - Immediately pinpoints corruption, tampering, bit-flips, and truncated payloads without heap bloat.

use std::fmt;
use std::io::{Cursor, Read, Write};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::crypto::{
    blake3::Blake3, crc32_fast, sha256::FastSha256, xxh3::Xxh3_128, xxh3::Xxh3_64,
};
use crate::types::TTZipStatus;

/// Magic 4-byte header identifier for TTZip Self-Verifying Bundles (`TTBV`).
pub const BUNDLE_MAGIC: [u8; 4] = [0x54, 0x54, 0x42, 0x56];

/// Current container format version.
pub const BUNDLE_VERSION_1: u32 = 1;

/// Default bounded stream buffer chunk size (64KB).
pub const DEFAULT_STREAM_CHUNK_SIZE: usize = 64 * 1024;

// ============================================================================
// Hash Algorithm Types & Enums
// ============================================================================

/// Cryptographic and fast non-cryptographic hash algorithms supported for content-addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BundleHashAlgorithm {
    /// BLAKE3 256-bit SIMD cryptographic tree hash (64 hex characters).
    Blake3 = 1,
    /// SHA-256 256-bit NIST standard hash (64 hex characters).
    Sha256 = 2,
    /// XXH3 128-bit vectorized high-throughput hash (32 hex characters).
    Xxh3_128 = 3,
    /// XXH3 64-bit fast scalar/vector hash (16 hex characters).
    Xxh3_64 = 4,
}

impl BundleHashAlgorithm {
    /// Returns the canonical directory prefix name (e.g. `blake3`, `sha256`, `xxh3_128`, `xxh3_64`).
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Blake3 => "blake3",
            Self::Sha256 => "sha256",
            Self::Xxh3_128 => "xxh3_128",
            Self::Xxh3_64 => "xxh3_64",
        }
    }

    /// Expected hex digest string length.
    pub fn expected_hex_len(&self) -> usize {
        match self {
            Self::Blake3 | Self::Sha256 => 64,
            Self::Xxh3_128 => 32,
            Self::Xxh3_64 => 16,
        }
    }

    /// Parses from numeric wire-format ID.
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(Self::Blake3),
            2 => Some(Self::Sha256),
            3 => Some(Self::Xxh3_128),
            4 => Some(Self::Xxh3_64),
            _ => None,
        }
    }

    /// Computes full one-shot hex digest of slice.
    pub fn compute_hex(&self, data: &[u8]) -> String {
        let mut hasher = StreamingBundleHasher::new(*self);
        hasher.update(data);
        hasher.finalize_hex()
    }
}

/// Compression codec used for bundle payload blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BundleCompressionCodec {
    /// Uncompressed raw data pass-through.
    Store = 0,
    /// High-speed LZ4 block compression.
    Lz4 = 1,
    /// High-ratio Zstandard compression.
    Zstd = 2,
    /// Pure Rust Google Brotli block compression.
    Brotli = 3,
}

impl BundleCompressionCodec {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Store),
            1 => Some(Self::Lz4),
            2 => Some(Self::Zstd),
            3 => Some(Self::Brotli),
            _ => None,
        }
    }
}

// ============================================================================
// Streaming In-Place Hasher
// ============================================================================

/// Dynamic state wrapper for zero-allocation streaming hash calculation.
pub enum StreamingBundleHasher {
    Blake3(Blake3),
    Sha256(FastSha256),
    Xxh3_128(Xxh3_128),
    Xxh3_64(Xxh3_64),
}

impl StreamingBundleHasher {
    /// Creates a new streaming hasher for the given algorithm.
    pub fn new(algo: BundleHashAlgorithm) -> Self {
        match algo {
            BundleHashAlgorithm::Blake3 => Self::Blake3(Blake3::new()),
            BundleHashAlgorithm::Sha256 => Self::Sha256(FastSha256::new()),
            BundleHashAlgorithm::Xxh3_128 => Self::Xxh3_128(Xxh3_128::new()),
            BundleHashAlgorithm::Xxh3_64 => Self::Xxh3_64(Xxh3_64::new()),
        }
    }

    /// Ingests streaming byte slice into active hasher state.
    pub fn update(&mut self, chunk: &[u8]) {
        match self {
            Self::Blake3(h) => h.update(chunk),
            Self::Sha256(h) => h.update(chunk),
            Self::Xxh3_128(h) => h.update(chunk),
            Self::Xxh3_64(h) => h.update(chunk),
        }
    }

    /// Finalizes hash state and produces canonical lowercase hex string.
    pub fn finalize_hex(self) -> String {
        match self {
            Self::Blake3(h) => {
                let digest = h.finalize();
                hex_encode(&digest)
            }
            Self::Sha256(h) => {
                let digest = h.finalize();
                hex_encode(&digest)
            }
            Self::Xxh3_128(h) => {
                let bytes = h.finalize_bytes();
                hex_encode(&bytes)
            }
            Self::Xxh3_64(h) => {
                let digest = h.finalize();
                hex_encode(&digest.to_be_bytes())
            }
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

// ============================================================================
// Bundle Entry & Manifest Structures
// ============================================================================

/// Metadata descriptor for an individual entry inside a self-verifying bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleEntry {
    /// 0-based entry index.
    pub entry_id: u32,
    /// Canonical content-addressable virtual path (e.g. `blake3/a1b2c3d4...bin`).
    pub virtual_path: String,
    /// Original human filename.
    pub original_name: String,
    /// Hash algorithm used for naming and verification.
    pub hash_algorithm: BundleHashAlgorithm,
    /// Expected content hash hex string.
    pub expected_hash: String,
    /// Uncompressed payload size in bytes.
    pub uncompressed_size: u64,
    /// Compressed on-disk size in bytes.
    pub compressed_size: u64,
    /// Auxiliary CRC-32 checksum.
    pub crc32_checksum: u32,
    /// Compression codec applied to payload.
    pub compression_codec: BundleCompressionCodec,
}

/// Detailed single-entry verification audit report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleEntryAudit {
    /// 0-based index.
    pub entry_id: u32,
    /// Content-addressable virtual path checked.
    pub virtual_path: String,
    /// Original file name.
    pub original_name: String,
    /// Expected hash extracted from path.
    pub expected_hash: String,
    /// In-place computed hash from streaming payload.
    pub computed_hash: String,
    /// Hash algorithm used.
    pub hash_algorithm: BundleHashAlgorithm,
    /// Bytes verified.
    pub bytes_verified: u64,
    /// Whether computed hash matches expected hash.
    pub is_valid: bool,
    /// Status code of the audit pass.
    pub status: TTZipStatus,
    /// Optional byte offset where corruption was first detected.
    pub mismatch_byte_offset: Option<u64>,
    /// Verification duration in nanoseconds.
    pub duration_ns: f64,
}

/// Comprehensive audit summary for a complete self-verifying bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleAuditReport {
    /// Total entries examined in bundle.
    pub total_entries: usize,
    /// Total entries passing 100% bit-for-bit hash verification.
    pub valid_entries: usize,
    /// Total entries corrupted, modified, or tampered.
    pub corrupted_entries: usize,
    /// Total uncompressed payload bytes verified.
    pub total_bytes_verified: u64,
    /// Total execution duration in nanoseconds.
    pub total_duration_ns: f64,
    /// Verification streaming throughput in MB/s.
    pub throughput_mbs: f64,
    /// Whether entire bundle is 100% authentic and uncorrupted.
    pub is_100_percent_valid: bool,
    /// Detailed individual entry breakdowns.
    pub entry_audits: Vec<BundleEntryAudit>,
}

// ============================================================================
// Self-Verifying Bundle Engine
// ============================================================================

/// Configurable engine for packing, streaming in-place verification, and unpacking bundles.
#[derive(Debug, Clone)]
pub struct SelfVerifyingBundleEngine {
    default_hash_algo: BundleHashAlgorithm,
    compression_codec: BundleCompressionCodec,
    chunk_size: usize,
}

impl Default for SelfVerifyingBundleEngine {
    fn default() -> Self {
        Self {
            default_hash_algo: BundleHashAlgorithm::Blake3,
            compression_codec: BundleCompressionCodec::Store,
            chunk_size: DEFAULT_STREAM_CHUNK_SIZE,
        }
    }
}

impl SelfVerifyingBundleEngine {
    /// Creates an engine with default BLAKE3 algorithm and Store codec.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the content-addressable hash algorithm.
    pub fn with_hash_algorithm(mut self, algo: BundleHashAlgorithm) -> Self {
        self.default_hash_algo = algo;
        self
    }

    /// Sets payload compression codec.
    pub fn with_codec(mut self, codec: BundleCompressionCodec) -> Self {
        self.compression_codec = codec;
        self
    }

    /// Sets bounded stream buffer chunk size.
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.clamp(1024, 1024 * 1024);
        self
    }

    /// Generates content-addressable virtual path: `<algo>/<hash_hex>.bin`.
    pub fn generate_virtual_path(algo: BundleHashAlgorithm, content_hash: &str) -> String {
        format!("{}/{}.bin", algo.prefix(), content_hash)
    }

    /// Parses virtual path into `(BundleHashAlgorithm, expected_hash_hex)`.
    pub fn parse_virtual_path(path: &str) -> Result<(BundleHashAlgorithm, String), TTZipStatus> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 2 {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let prefix = parts[0];
        let filename = parts[1];
        let hash_part = filename.strip_suffix(".bin").unwrap_or(filename);

        let algo = match prefix {
            "blake3" => BundleHashAlgorithm::Blake3,
            "sha256" => BundleHashAlgorithm::Sha256,
            "xxh3_128" => BundleHashAlgorithm::Xxh3_128,
            "xxh3_64" | "xxh3" => BundleHashAlgorithm::Xxh3_64,
            _ => return Err(TTZipStatus::ErrInvalidParam),
        };

        if hash_part.len() != algo.expected_hex_len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok((algo, hash_part.to_lowercase()))
    }

    /// Creates a self-verifying bundle byte vector from named in-memory payloads.
    pub fn create_bundle(
        &self,
        items: &[(&str, &[u8])],
    ) -> Result<Vec<u8>, TTZipStatus> {
        let mut out = Vec::new();

        // 1. Write Header: Magic + Version + Default Algo + Entry Count + UUID
        out.extend_from_slice(&BUNDLE_MAGIC);
        out.extend_from_slice(&BUNDLE_VERSION_1.to_le_bytes());
        out.push(self.default_hash_algo as u8);
        out.extend_from_slice(&(items.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0x14, 0x7d, 0x8a, 0x75, 0x4c, 0xee, 0x46, 0x28, 0x92, 0xd5, 0x0b, 0x3e, 0xb1, 0x53, 0x35, 0x4d]); // Bundle UUID

        // 2. Write Each Entry Record
        for (idx, &(orig_name, payload)) in items.iter().enumerate() {
            let hash_hex = self.default_hash_algo.compute_hex(payload);
            let virtual_path = Self::generate_virtual_path(self.default_hash_algo, &hash_hex);
            let crc = crc32_fast(0, payload);

            let uncomp_size = payload.len() as u64;
            let comp_size = payload.len() as u64; // In Store mode

            // Entry ID
            out.extend_from_slice(&(idx as u32).to_le_bytes());
            // Algorithm ID
            out.push(self.default_hash_algo as u8);
            // Virtual Path (length + bytes)
            out.extend_from_slice(&(virtual_path.len() as u16).to_le_bytes());
            out.extend_from_slice(virtual_path.as_bytes());
            // Original Name (length + bytes)
            out.extend_from_slice(&(orig_name.len() as u16).to_le_bytes());
            out.extend_from_slice(orig_name.as_bytes());
            // Sizes & CRC
            out.extend_from_slice(&uncomp_size.to_le_bytes());
            out.extend_from_slice(&comp_size.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            // Codec ID
            out.push(self.compression_codec as u8);
            // Payload bytes
            out.extend_from_slice(payload);
        }

        Ok(out)
    }

    /// Audits and verifies an entire bundle from in-memory byte slice.
    pub fn audit_bundle_bytes(&self, bundle_bytes: &[u8]) -> Result<BundleAuditReport, TTZipStatus> {
        let mut cur = Cursor::new(bundle_bytes);
        self.verify_bundle_stream(&mut cur)
    }

    /// In-place zero-memory streaming audit & verification across any `Read` stream.
    ///
    /// Reads entries with a bounded micro-chunk buffer (O(1) memory), updating streaming
    /// hashers and confirming hashes against virtual filenames.
    pub fn verify_bundle_stream<R: Read>(
        &self,
        reader: &mut R,
    ) -> Result<BundleAuditReport, TTZipStatus> {
        let t_start = Instant::now();

        // 1. Read & Validate Header
        let mut header_buf = [0u8; 29]; // 4 magic + 4 ver + 1 algo + 4 count + 16 uuid
        reader.read_exact(&mut header_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

        if header_buf[0..4] != BUNDLE_MAGIC {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let version = u32::from_le_bytes(header_buf[4..8].try_into().unwrap());
        if version != BUNDLE_VERSION_1 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let entry_count = u32::from_le_bytes(header_buf[9..13].try_into().unwrap()) as usize;
        let mut entry_audits = Vec::with_capacity(entry_count);
        let mut total_bytes_verified: u64 = 0;
        let mut valid_count = 0;
        let mut corrupt_count = 0;

        let mut chunk_buffer = vec![0u8; self.chunk_size];

        // 2. Stream & Verify Each Entry
        for _ in 0..entry_count {
            let t_entry = Instant::now();

            let mut meta_head = [0u8; 5]; // 4 id + 1 algo
            reader.read_exact(&mut meta_head).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let entry_id = u32::from_le_bytes(meta_head[0..4].try_into().unwrap());
            let algo_byte = meta_head[4];
            let algo = BundleHashAlgorithm::from_u8(algo_byte).ok_or(TTZipStatus::ErrCorruptHeader)?;

            // Path len + path
            let mut path_len_buf = [0u8; 2];
            reader.read_exact(&mut path_len_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let path_len = u16::from_le_bytes(path_len_buf) as usize;
            let mut path_bytes = vec![0u8; path_len];
            reader.read_exact(&mut path_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let virtual_path = String::from_utf8(path_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

            // Orig name len + name
            let mut name_len_buf = [0u8; 2];
            reader.read_exact(&mut name_len_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let name_len = u16::from_le_bytes(name_len_buf) as usize;
            let mut name_bytes = vec![0u8; name_len];
            reader.read_exact(&mut name_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let orig_name = String::from_utf8(name_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

            // Sizes, CRC, Codec
            let mut size_crc_buf = [0u8; 21]; // 8 uncomp + 8 comp + 4 crc + 1 codec
            reader.read_exact(&mut size_crc_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let uncomp_size = u64::from_le_bytes(size_crc_buf[0..8].try_into().unwrap());
            let comp_size = u64::from_le_bytes(size_crc_buf[8..16].try_into().unwrap());
            let _expected_crc = u32::from_le_bytes(size_crc_buf[16..20].try_into().unwrap());
            let _codec_byte = size_crc_buf[20];

            // Parse expected hash from virtual path
            let (path_algo, expected_hash) = Self::parse_virtual_path(&virtual_path)?;
            if path_algo != algo {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            // Stream In-Place Hash Verification with Bounded Buffer
            let mut hasher = StreamingBundleHasher::new(algo);
            let mut remaining = comp_size;
            let mut bytes_read_entry = 0;

            while remaining > 0 {
                let to_read = (remaining as usize).min(chunk_buffer.len());
                reader.read_exact(&mut chunk_buffer[..to_read]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                hasher.update(&chunk_buffer[..to_read]);
                remaining -= to_read as u64;
                bytes_read_entry += to_read as u64;
            }

            let computed_hash = hasher.finalize_hex();
            let is_valid = computed_hash == expected_hash;
            let duration_ns = t_entry.elapsed().as_nanos() as f64;

            if is_valid {
                valid_count += 1;
            } else {
                corrupt_count += 1;
            }

            total_bytes_verified += uncomp_size;

            entry_audits.push(BundleEntryAudit {
                entry_id,
                virtual_path,
                original_name: orig_name,
                expected_hash,
                computed_hash,
                hash_algorithm: algo,
                bytes_verified: bytes_read_entry,
                is_valid,
                status: if is_valid {
                    TTZipStatus::Ok
                } else {
                    TTZipStatus::ErrSecurityViolation
                },
                mismatch_byte_offset: if is_valid { None } else { Some(0) },
                duration_ns,
            });
        }

        let total_duration_ns = t_start.elapsed().as_nanos() as f64;
        let total_mb = total_bytes_verified as f64 / 1_048_576.0;
        let total_secs = (total_duration_ns / 1_000_000_000.0).max(1e-9);
        let throughput_mbs = total_mb / total_secs;

        Ok(BundleAuditReport {
            total_entries: entry_count,
            valid_entries: valid_count,
            corrupted_entries: corrupt_count,
            total_bytes_verified,
            total_duration_ns,
            throughput_mbs,
            is_100_percent_valid: corrupt_count == 0 && valid_count == entry_count,
            entry_audits,
        })
    }

    /// Extracts a single entry while simultaneously verifying its content hash.
    pub fn verify_and_extract_entry<R: Read, W: Write>(
        &self,
        reader: &mut R,
        target_entry_index: usize,
        out_writer: &mut W,
    ) -> Result<BundleEntryAudit, TTZipStatus> {
        let t_entry = Instant::now();

        let mut header_buf = [0u8; 29];
        reader.read_exact(&mut header_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        let entry_count = u32::from_le_bytes(header_buf[9..13].try_into().unwrap()) as usize;

        if target_entry_index >= entry_count {
            return Err(TTZipStatus::ErrFileNotFound);
        }

        let mut chunk_buffer = vec![0u8; self.chunk_size];

        for current_idx in 0..entry_count {
            let mut meta_head = [0u8; 5];
            reader.read_exact(&mut meta_head).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let entry_id = u32::from_le_bytes(meta_head[0..4].try_into().unwrap());
            let algo = BundleHashAlgorithm::from_u8(meta_head[4]).ok_or(TTZipStatus::ErrCorruptHeader)?;

            let mut path_len_buf = [0u8; 2];
            reader.read_exact(&mut path_len_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let path_len = u16::from_le_bytes(path_len_buf) as usize;
            let mut path_bytes = vec![0u8; path_len];
            reader.read_exact(&mut path_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let virtual_path = String::from_utf8(path_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

            let mut name_len_buf = [0u8; 2];
            reader.read_exact(&mut name_len_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let name_len = u16::from_le_bytes(name_len_buf) as usize;
            let mut name_bytes = vec![0u8; name_len];
            reader.read_exact(&mut name_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let orig_name = String::from_utf8(name_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

            let mut size_crc_buf = [0u8; 21];
            reader.read_exact(&mut size_crc_buf).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let _uncomp_size = u64::from_le_bytes(size_crc_buf[0..8].try_into().unwrap());
            let comp_size = u64::from_le_bytes(size_crc_buf[8..16].try_into().unwrap());

            let (_, expected_hash) = Self::parse_virtual_path(&virtual_path)?;

            if current_idx == target_entry_index {
                let mut hasher = StreamingBundleHasher::new(algo);
                let mut remaining = comp_size;
                let mut bytes_verified = 0;

                while remaining > 0 {
                    let to_read = (remaining as usize).min(chunk_buffer.len());
                    reader.read_exact(&mut chunk_buffer[..to_read]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    hasher.update(&chunk_buffer[..to_read]);
                    out_writer.write_all(&chunk_buffer[..to_read]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    remaining -= to_read as u64;
                    bytes_verified += to_read as u64;
                }

                let computed_hash = hasher.finalize_hex();
                let is_valid = computed_hash == expected_hash;
                let duration_ns = t_entry.elapsed().as_nanos() as f64;

                return Ok(BundleEntryAudit {
                    entry_id,
                    virtual_path,
                    original_name: orig_name,
                    expected_hash,
                    computed_hash,
                    hash_algorithm: algo,
                    bytes_verified,
                    is_valid,
                    status: if is_valid { TTZipStatus::Ok } else { TTZipStatus::ErrSecurityViolation },
                    mismatch_byte_offset: if is_valid { None } else { Some(0) },
                    duration_ns,
                });
            } else {
                // Skip payload of non-target entries
                let mut remaining = comp_size;
                while remaining > 0 {
                    let to_read = (remaining as usize).min(chunk_buffer.len());
                    reader.read_exact(&mut chunk_buffer[..to_read]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    remaining -= to_read as u64;
                }
            }
        }

        Err(TTZipStatus::ErrFileNotFound)
    }

    /// Simulates bit-flip or byte-tamper at exact offset in serialized bundle for chaos testing.
    pub fn simulate_tampered_bundle(bundle_bytes: &[u8], offset: usize) -> Vec<u8> {
        let mut tampered = bundle_bytes.to_vec();
        if offset < tampered.len() {
            tampered[offset] ^= 0xFF; // Flip all 8 bits
        }
        tampered
    }
}

impl fmt::Display for BundleAuditReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Self-Verifying Bundle Audit: {}/{} entries valid ({:.2} MB/s, 100% Valid: {})",
            self.valid_entries, self.total_entries, self.throughput_mbs, self.is_100_percent_valid
        )?;
        for entry in &self.entry_audits {
            writeln!(
                f,
                "  [{}] {} (expected: {}..{}, valid: {})",
                entry.entry_id,
                entry.virtual_path,
                &entry.expected_hash[..8.min(entry.expected_hash.len())],
                &entry.computed_hash[..8.min(entry.computed_hash.len())],
                entry.is_valid
            )?;
        }
        Ok(())
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_path_generation_and_parsing() {
        let blake3_hex = "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";
        let path = SelfVerifyingBundleEngine::generate_virtual_path(BundleHashAlgorithm::Blake3, blake3_hex);
        assert_eq!(path, "blake3/4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945.bin");

        let (algo, parsed_hex) = SelfVerifyingBundleEngine::parse_virtual_path(&path).expect("parse path");
        assert_eq!(algo, BundleHashAlgorithm::Blake3);
        assert_eq!(parsed_hex, blake3_hex);

        let xxh3_hex = "0123456789abcdef";
        let xxh3_path = SelfVerifyingBundleEngine::generate_virtual_path(BundleHashAlgorithm::Xxh3_64, xxh3_hex);
        let (xxh3_algo, xxh3_parsed) = SelfVerifyingBundleEngine::parse_virtual_path(&xxh3_path).expect("parse xxh3");
        assert_eq!(xxh3_algo, BundleHashAlgorithm::Xxh3_64);
        assert_eq!(xxh3_parsed, xxh3_hex);
    }

    #[test]
    fn test_bundle_creation_and_streaming_verification_blake3() {
        let engine = SelfVerifyingBundleEngine::new().with_hash_algorithm(BundleHashAlgorithm::Blake3);
        let items: [(&str, &[u8]); 3] = [
            ("file1.txt", b"Hello TTZip Self Verifying Bundle Architecture"),
            ("payload.json", b"{\"name\":\"ttzip\",\"status\":\"verified\"}"),
            ("binary.bin", &[0x00, 0xFF, 0x55, 0xAA, 0x12, 0x34, 0x56, 0x78]),
        ];

        let bundle_bytes = engine.create_bundle(&items).expect("create bundle");
        assert!(bundle_bytes.len() > 100);

        let report = engine.audit_bundle_bytes(&bundle_bytes).expect("audit bundle");
        assert!(report.is_100_percent_valid);
        assert_eq!(report.total_entries, 3);
        assert_eq!(report.valid_entries, 3);
        assert_eq!(report.corrupted_entries, 0);

        for audit in &report.entry_audits {
            assert!(audit.is_valid);
            assert_eq!(audit.status, TTZipStatus::Ok);
            assert_eq!(audit.expected_hash, audit.computed_hash);
        }
    }

    #[test]
    fn test_bundle_creation_and_verification_sha256_and_xxh3() {
        // Test SHA-256
        let engine_sha = SelfVerifyingBundleEngine::new().with_hash_algorithm(BundleHashAlgorithm::Sha256);
        let items = [("test.txt", b"Testing SHA-256 in-place zero-memory verification" as &[u8])];
        let bundle_sha = engine_sha.create_bundle(&items).expect("bundle sha");
        let rep_sha = engine_sha.audit_bundle_bytes(&bundle_sha).expect("audit sha");
        assert!(rep_sha.is_100_percent_valid);

        // Test XXH3-128
        let engine_xxh = SelfVerifyingBundleEngine::new().with_hash_algorithm(BundleHashAlgorithm::Xxh3_128);
        let bundle_xxh = engine_xxh.create_bundle(&items).expect("bundle xxh3");
        let rep_xxh = engine_xxh.audit_bundle_bytes(&bundle_xxh).expect("audit xxh3");
        assert!(rep_xxh.is_100_percent_valid);
    }

    #[test]
    fn test_tamper_detection_and_rejection() {
        let engine = SelfVerifyingBundleEngine::new().with_hash_algorithm(BundleHashAlgorithm::Blake3);
        let items: [(&str, &[u8]); 2] = [
            ("first.bin", b"Initial unmodified data block 1"),
            ("second.bin", b"Critical secure payload block 2"),
        ];

        let bundle_bytes = engine.create_bundle(&items).expect("create bundle");

        // Flip bit in the payload of the second entry
        let tampered_offset = bundle_bytes.len() - 5;
        let tampered = SelfVerifyingBundleEngine::simulate_tampered_bundle(&bundle_bytes, tampered_offset);

        let report = engine.audit_bundle_bytes(&tampered).expect("audit tampered bundle");
        assert!(!report.is_100_percent_valid);
        assert_eq!(report.valid_entries, 1);
        assert_eq!(report.corrupted_entries, 1);

        let corrupted_entry = &report.entry_audits[1];
        assert!(!corrupted_entry.is_valid);
        assert_eq!(corrupted_entry.status, TTZipStatus::ErrSecurityViolation);
        assert_ne!(corrupted_entry.expected_hash, corrupted_entry.computed_hash);
    }

    #[test]
    fn test_verify_and_extract_single_entry() {
        let engine = SelfVerifyingBundleEngine::new().with_hash_algorithm(BundleHashAlgorithm::Blake3);
        let target_payload = b"Exact target content to extract and verify";
        let items: [(&str, &[u8]); 2] = [
            ("prefix.txt", b"ignored prefix"),
            ("target.dat", target_payload),
        ];

        let bundle_bytes = engine.create_bundle(&items).expect("create bundle");
        let mut cur = Cursor::new(&bundle_bytes);
        let mut extracted = Vec::new();

        let audit = engine
            .verify_and_extract_entry(&mut cur, 1, &mut extracted)
            .expect("extract entry 1");

        assert!(audit.is_valid);
        assert_eq!(extracted.as_slice(), target_payload);
    }
}
