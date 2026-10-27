// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Centralized, immutable, and thread-safe Codec Registry.
//!
//! Provides deterministic registration, cataloging, and dispatch for all built-in
//! compression/decompression algorithms without dynamic linking surprises or lazy loading failures.

use std::sync::LazyLock;
use ttzip_types::status::TTZipStatus;

/// Unique strongly-typed identifier for all supported compression codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodecId {
    Deflate,
    Zlib,
    Gzip,
    Zstd,
    Lz4,
    Lzfse,
    Brotli,
    Snappy,
    Bzip2,
    Copy,
}

impl CodecId {
    /// Returns the canonical name of the codec.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Deflate => "deflate",
            Self::Zlib => "zlib",
            Self::Gzip => "gzip",
            Self::Zstd => "zstd",
            Self::Lz4 => "lz4",
            Self::Lzfse => "lzfse",
            Self::Brotli => "brotli",
            Self::Snappy => "snappy",
            Self::Bzip2 => "bzip2",
            Self::Copy => "copy",
        }
    }
}

/// Metadata descriptor detailing the capabilities of a compression codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecDescriptor {
    /// Unique codec ID.
    pub id: CodecId,
    /// Canonical name.
    pub name: &'static str,
    /// Default compression level.
    pub default_level: i32,
    /// Minimum allowed level.
    pub min_level: i32,
    /// Maximum allowed level.
    pub max_level: i32,
    /// Whether compression is supported.
    pub supports_compression: bool,
    /// Whether decompression is supported.
    pub supports_decompression: bool,
    /// 7z format Method ID if applicable.
    pub sevenz_method_id: Option<u64>,
    /// ZIP format Compression Method ID if applicable.
    pub zip_method_id: Option<u16>,
}

/// Unified trait implemented by all compression codec engines.
pub trait CompressionCodec: Send + Sync {
    /// Returns metadata descriptor for this codec.
    fn descriptor(&self) -> &CodecDescriptor;

    /// Compresses `input` into an owned vector with the requested level.
    fn compress(&self, input: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus>;

    /// Decompresses `input` into an owned vector.
    fn decompress(&self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus>;
}

// ============================================================================
// Concrete Codec Implementations
// ============================================================================

struct CopyCodec {
    desc: CodecDescriptor,
}

impl CopyCodec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Copy,
                name: "copy",
                default_level: 0,
                min_level: 0,
                max_level: 0,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: Some(0x00),
                zip_method_id: Some(0),
            },
        }
    }
}

impl CompressionCodec for CopyCodec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        Ok(input.to_vec())
    }
    fn decompress(&self, input: &[u8], _expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        Ok(input.to_vec())
    }
}

struct DeflateCodec {
    desc: CodecDescriptor,
}

impl DeflateCodec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Deflate,
                name: "deflate",
                default_level: 6,
                min_level: 1,
                max_level: 12,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: Some(0x040108),
                zip_method_id: Some(8),
            },
        }
    }
}

impl CompressionCodec for DeflateCodec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = crate::deflate::deflate_compress_bound(input.len(), level);
        let mut dst = vec![0u8; bound];
        let written = crate::deflate::deflate_compress(input, &mut dst, level)?;
        dst.truncate(written);
        Ok(dst)
    }
    fn decompress(&self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        let size = expected_size.unwrap_or(input.len() * 4).max(64);
        let mut dst = vec![0u8; size];
        let written = crate::deflate::deflate_decompress(input, &mut dst)?;
        dst.truncate(written);
        Ok(dst)
    }
}

struct ZstdCodec {
    desc: CodecDescriptor,
}

impl ZstdCodec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Zstd,
                name: "zstd",
                default_level: 3,
                min_level: 1,
                max_level: 22,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: Some(0x04F71101),
                zip_method_id: Some(93),
            },
        }
    }
}

impl CompressionCodec for ZstdCodec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = crate::zstd::cctx::zstd_compress_bound(input.len());
        let mut dst = vec![0u8; bound];
        let written = crate::zstd::cctx::zstd_compress(input, &mut dst, level)?;
        dst.truncate(written);
        Ok(dst)
    }
    fn decompress(&self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        let size = expected_size.unwrap_or(input.len() * 4).max(64);
        let mut dst = vec![0u8; size];
        let written = crate::zstd::dctx::zstd_decompress(input, &mut dst)?;
        dst.truncate(written);
        Ok(dst)
    }
}

struct BrotliCodec {
    desc: CodecDescriptor,
}

impl BrotliCodec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Brotli,
                name: "brotli",
                default_level: 6,
                min_level: 0,
                max_level: 11,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: Some(0x04F71102),
                zip_method_id: None,
            },
        }
    }
}

impl CompressionCodec for BrotliCodec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        crate::brotli::brotli_compress_to_vec(input, level.clamp(0, 11) as u32, 22)
    }
    fn decompress(&self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        crate::brotli::brotli_decompress_to_vec(input, expected_size.unwrap_or(input.len() * 4).max(64))
    }
}

struct Bzip2Codec {
    desc: CodecDescriptor,
}

impl Bzip2Codec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Bzip2,
                name: "bzip2",
                default_level: 9,
                min_level: 1,
                max_level: 9,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: Some(0x040202),
                zip_method_id: Some(12),
            },
        }
    }
}

impl CompressionCodec for Bzip2Codec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        crate::bzip2::bzip2_compress_to_vec(input, level.clamp(1, 9))
    }
    fn decompress(&self, input: &[u8], _expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        crate::bzip2::bzip2_decompress_vec(input)
    }
}

struct Lz4Codec {
    desc: CodecDescriptor,
}

impl Lz4Codec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Lz4,
                name: "lz4",
                default_level: 1,
                min_level: 1,
                max_level: 12,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: Some(0x04F71104),
                zip_method_id: None,
            },
        }
    }
}

impl CompressionCodec for Lz4Codec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        crate::lz4::lz4_compress_to_vec(input, 1)
    }
    fn decompress(&self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        crate::lz4::lz4_decompress_to_vec(input, expected_size.unwrap_or(input.len() * 4).max(64))
    }
}

struct LzfseCodec {
    desc: CodecDescriptor,
}

impl LzfseCodec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Lzfse,
                name: "lzfse",
                default_level: 0,
                min_level: 0,
                max_level: 0,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: None,
                zip_method_id: None,
            },
        }
    }
}

impl CompressionCodec for LzfseCodec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        crate::lzfse::lzfse_compress_to_vec(input)
    }
    fn decompress(&self, input: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        crate::lzfse::lzfse_decompress_to_vec(input, expected_size.unwrap_or(input.len() * 4).max(64))
    }
}

struct SnappyCodec {
    desc: CodecDescriptor,
}

impl SnappyCodec {
    fn new() -> Self {
        Self {
            desc: CodecDescriptor {
                id: CodecId::Snappy,
                name: "snappy",
                default_level: 0,
                min_level: 0,
                max_level: 0,
                supports_compression: true,
                supports_decompression: true,
                sevenz_method_id: None,
                zip_method_id: None,
            },
        }
    }
}

impl CompressionCodec for SnappyCodec {
    fn descriptor(&self) -> &CodecDescriptor {
        &self.desc
    }
    fn compress(&self, input: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        crate::snappy::snappy_compress_to_vec(input)
    }
    fn decompress(&self, input: &[u8], _expected_size: Option<usize>) -> Result<Vec<u8>, TTZipStatus> {
        crate::snappy::snappy_decompress_to_vec(input)
    }
}

// ============================================================================
// Registry Engine
// ============================================================================

/// Global, immutable codec registry ensuring deterministic codec availability.
pub struct CodecRegistry {
    codecs: Vec<Box<dyn CompressionCodec>>,
}

impl Default for CodecRegistry {
    fn default() -> Self {
        Self::new_builtin()
    }
}

impl CodecRegistry {
    /// Creates and populates a new registry with all built-in industrial codecs.
    pub fn new_builtin() -> Self {
        let codecs: Vec<Box<dyn CompressionCodec>> = vec![
            Box::new(CopyCodec::new()),
            Box::new(DeflateCodec::new()),
            Box::new(ZstdCodec::new()),
            Box::new(BrotliCodec::new()),
            Box::new(Bzip2Codec::new()),
            Box::new(Lz4Codec::new()),
            Box::new(LzfseCodec::new()),
            Box::new(SnappyCodec::new()),
        ];
        Self { codecs }
    }

    /// Accesses the global singleton CodecRegistry.
    #[inline]
    pub fn global() -> &'static Self {
        static REGISTRY: LazyLock<CodecRegistry> = LazyLock::new(CodecRegistry::new_builtin);
        &REGISTRY
    }

    /// Finds a codec by its strongly-typed CodecId.
    pub fn get(&self, id: CodecId) -> Option<&dyn CompressionCodec> {
        self.codecs
            .iter()
            .find(|c| c.descriptor().id == id)
            .map(|b| b.as_ref())
    }

    /// Finds a codec by case-insensitive name.
    pub fn get_by_name(&self, name: &str) -> Option<&dyn CompressionCodec> {
        self.codecs
            .iter()
            .find(|c| c.descriptor().name.eq_ignore_ascii_case(name))
            .map(|b| b.as_ref())
    }

    /// Finds a codec by its 7z Method ID.
    pub fn get_by_7z_method_id(&self, method_id: u64) -> Option<&dyn CompressionCodec> {
        self.codecs
            .iter()
            .find(|c| c.descriptor().sevenz_method_id == Some(method_id))
            .map(|b| b.as_ref())
    }

    /// Finds a codec by its ZIP compression method ID.
    pub fn get_by_zip_method(&self, method_id: u16) -> Option<&dyn CompressionCodec> {
        self.codecs
            .iter()
            .find(|c| c.descriptor().zip_method_id == Some(method_id))
            .map(|b| b.as_ref())
    }

    /// Returns descriptors for all registered codecs.
    pub fn all_descriptors(&self) -> Vec<CodecDescriptor> {
        self.codecs.iter().map(|c| c.descriptor().clone()).collect()
    }

    /// Returns the total number of registered codecs.
    pub fn len(&self) -> usize {
        self.codecs.len()
    }

    /// Returns true if the registry has no codecs registered.
    pub fn is_empty(&self) -> bool {
        self.codecs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PAYLOAD: &[u8] = b"CodecRegistry Deterministic Built-in Verification Payload 2026! 1234567890 ABCDEFGHIJKLMNOPQRSTUVWXYZ.";

    #[test]
    fn test_codec_registry_catalog() {
        let reg = CodecRegistry::new_builtin();
        assert_eq!(reg.len(), 8);
        assert!(!reg.is_empty());

        let desc_list = reg.all_descriptors();
        assert_eq!(desc_list.len(), 8);

        // Verify lookup by ID
        assert!(reg.get(CodecId::Deflate).is_some());
        assert!(reg.get(CodecId::Zstd).is_some());
        assert!(reg.get(CodecId::Brotli).is_some());

        // Verify lookup by name
        assert!(reg.get_by_name("deflate").is_some());
        assert!(reg.get_by_name("DEFLATE").is_some());
        assert!(reg.get_by_name("zstd").is_some());
        assert!(reg.get_by_name("unknown_codec").is_none());

        // Verify 7z Method ID lookup
        assert_eq!(reg.get_by_7z_method_id(0x04F71101).map(|c| c.descriptor().id), Some(CodecId::Zstd));
        assert_eq!(reg.get_by_7z_method_id(0x040108).map(|c| c.descriptor().id), Some(CodecId::Deflate));

        // Verify ZIP Method ID lookup
        assert_eq!(reg.get_by_zip_method(8).map(|c| c.descriptor().id), Some(CodecId::Deflate));
        assert_eq!(reg.get_by_zip_method(93).map(|c| c.descriptor().id), Some(CodecId::Zstd));
    }

    #[test]
    fn test_codec_registry_roundtrip_all() {
        let reg = CodecRegistry::new_builtin();
        let payload = TEST_PAYLOAD;

        for desc in reg.all_descriptors() {
            let codec = reg.get(desc.id).expect("codec present");
            let compressed = codec
                .compress(payload, desc.default_level)
                .unwrap_or_else(|e| panic!("compress failed for {:?}: {:?}", desc.id, e));

            let decompressed = codec
                .decompress(&compressed, Some(payload.len()))
                .unwrap_or_else(|e| panic!("decompress failed for {:?}: {:?}", desc.id, e));

            assert_eq!(decompressed, payload, "roundtrip mismatch for {:?}", desc.id);
        }
    }
}
