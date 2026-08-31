// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for Hardware Portability, 4-Tier API Stratification, and ABI Export Guard.

use ttzip_engine::api::{
    catch_ffi_boundary, catch_ffi_status, is_symbol_in_whitelist, simple_compress,
    simple_compress_bound, simple_decompress, ttzip_abi_is_compatible, ttzip_abi_version,
    verify_symbols_whitelist, AdvancedCompressorBuilder, AdvancedDecompressorBuilder,
    CompressionContext, DecompressionContext, StreamCompressor, StreamDecompressor,
    StreamFlushMode,
};
use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipStatus};
use ttzip_engine::utils::hardware_portability::{
    detect_hardware_profile, has_aes_hardware, has_avx2, has_neon, likely,
    read_unaligned_be16, read_unaligned_be32, read_unaligned_be64, read_unaligned_le128,
    read_unaligned_le16, read_unaligned_le16_at, read_unaligned_le32, read_unaligned_le32_at,
    read_unaligned_le64, read_unaligned_le64_at, read_unaligned_le64_unchecked,
    secure_zero_memory, secure_zero_struct, unlikely, write_unaligned_be16, write_unaligned_be32,
    write_unaligned_be64, write_unaligned_le128, write_unaligned_le16, write_unaligned_le32,
    write_unaligned_le64, write_unaligned_le64_unchecked, CpuFeature, HardwareSecureBuffer,
    SecureZeroize,
};

// =============================================================================
// 1. Hardware Portability: Unaligned Memory Operations
// =============================================================================

#[test]
fn test_unaligned_le_be_read_write_roundtrip() {
    let mut buffer = [0u8; 32];

    // 16-bit LE / BE
    write_unaligned_le16(&mut buffer[1..3], 0x1234);
    assert_eq!(read_unaligned_le16(&buffer[1..3]), 0x1234);
    assert_eq!(read_unaligned_le16_at(&buffer, 1), Some(0x1234));
    assert_eq!(read_unaligned_le16_at(&buffer, 31), None);

    write_unaligned_be16(&mut buffer[3..5], 0xABCD);
    assert_eq!(read_unaligned_be16(&buffer[3..5]), 0xABCD);

    // 32-bit LE / BE
    write_unaligned_le32(&mut buffer[5..9], 0xDEADBEEF);
    assert_eq!(read_unaligned_le32(&buffer[5..9]), 0xDEADBEEF);
    assert_eq!(read_unaligned_le32_at(&buffer, 5), Some(0xDEADBEEF));
    assert_eq!(read_unaligned_le32_at(&buffer, 29), None);

    write_unaligned_be32(&mut buffer[9..13], 0xCAFEBABE);
    assert_eq!(read_unaligned_be32(&buffer[9..13]), 0xCAFEBABE);

    // 64-bit LE / BE
    write_unaligned_le64(&mut buffer[13..21], 0x0123456789ABCDEF);
    assert_eq!(read_unaligned_le64(&buffer[13..21]), 0x0123456789ABCDEF);
    assert_eq!(read_unaligned_le64_at(&buffer, 13), Some(0x0123456789ABCDEF));
    assert_eq!(read_unaligned_le64_at(&buffer, 25), None);

    write_unaligned_be64(&mut buffer[21..29], 0xFEDCBA9876543210);
    assert_eq!(read_unaligned_be64(&buffer[21..29]), 0xFEDCBA9876543210);

    // 128-bit LE
    let mut buf128 = [0u8; 16];
    write_unaligned_le128(&mut buf128, 0x112233445566778899AABBCCDDEEFF00);
    assert_eq!(
        read_unaligned_le128(&buf128),
        0x112233445566778899AABBCCDDEEFF00
    );
}

#[test]
fn test_unaligned_le64_unchecked() {
    let mut data = [0u8; 16];
    unsafe {
        write_unaligned_le64_unchecked(data.as_mut_ptr().add(3), 0xAA55AA55AA55AA55);
        let val = read_unaligned_le64_unchecked(data.as_ptr().add(3));
        assert_eq!(val, 0xAA55AA55AA55AA55);
    }
}

// =============================================================================
// 2. Hardware Portability: SecureZeroize Memory Wiping
// =============================================================================

#[test]
fn test_secure_zero_memory_and_struct() {
    let mut secret_key = vec![0x42u8; 64];
    secure_zero_memory(&mut secret_key);
    assert!(secret_key.iter().all(|&b| b == 0));

    #[derive(Debug, PartialEq, Eq)]
    struct MockKey {
        k: [u8; 16],
        iv: [u8; 16],
    }

    let mut key_struct = MockKey {
        k: [0xFF; 16],
        iv: [0xEE; 16],
    };
    secure_zero_struct(&mut key_struct);
    assert_eq!(key_struct.k, [0; 16]);
    assert_eq!(key_struct.iv, [0; 16]);
}

#[test]
fn test_secure_buffer_raii_drop() {
    let mut sec_buf = HardwareSecureBuffer::new(b"TopSecretPassword123");
    assert_eq!(sec_buf.len(), 20);
    assert!(!sec_buf.is_empty());
    assert_eq!(sec_buf.as_slice(), b"TopSecretPassword123");
    sec_buf.as_mut_slice()[0] = b't';
    assert_eq!(sec_buf.as_slice(), b"topSecretPassword123");

    let sec_cap = HardwareSecureBuffer::with_capacity(128);
    assert!(sec_cap.is_empty());
}

#[test]
fn test_secure_zeroize_trait() {
    let mut bytes = vec![0xABu8; 32];
    bytes.secure_zeroize();
    assert!(bytes.iter().all(|&b| b == 0));
}

// =============================================================================
// 3. Hardware Portability: CPU Detection & Branch Prediction
// =============================================================================

#[test]
fn test_cpu_feature_detection_and_branch_hints() {
    let profile = detect_hardware_profile();
    #[cfg(target_arch = "aarch64")]
    {
        assert!(profile.has_neon);
        assert!(has_neon());
        assert!(profile.supports(CpuFeature::Neon));
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        assert_eq!(profile.has_avx2, has_avx2());
    }

    let _ = has_aes_hardware();
    let _ = has_avx2();

    assert!(likely(true));
    assert!(!likely(false));
    assert!(unlikely(true));
    assert!(!unlikely(false));
}

// =============================================================================
// 4. API Stratification: Layer 1 (Simple API)
// =============================================================================

#[test]
fn test_layer1_simple_api_roundtrip_all_codecs() {
    let payload = b"TTZip Safe Rust High-Throughput 4-Tier API Stratification Test Payload 2026!";

    let formats = [
        TTZipArchiveFormat::Zip,
        TTZipArchiveFormat::Gzip,
        TTZipArchiveFormat::Zstd,
        TTZipArchiveFormat::Lz4,
        TTZipArchiveFormat::Snappy,
        TTZipArchiveFormat::Brotli,
        TTZipArchiveFormat::Bzip2,
    ];

    for fmt in formats {
        let bound = simple_compress_bound(payload.len(), fmt, TTZipCompressionLevel::Normal);
        assert!(bound >= payload.len() / 2);

        let compressed = simple_compress(payload, fmt, TTZipCompressionLevel::Normal)
            .unwrap_or_else(|e| panic!("simple_compress failed for format {:?}: {:?}", fmt, e));
        assert!(!compressed.is_empty());

        let decompressed = simple_decompress(&compressed, fmt)
            .unwrap_or_else(|e| panic!("simple_decompress failed for format {:?}: {:?}", fmt, e));
        assert_eq!(decompressed, payload, "Failed roundtrip for format {:?}", fmt);
    }
}

// =============================================================================
// 5. API Stratification: Layer 2 (Context API)
// =============================================================================

#[test]
fn test_layer2_context_api_buffer_reuse_and_reset() {
    let mut cctx = CompressionContext::new(TTZipArchiveFormat::Zstd, TTZipCompressionLevel::Normal);
    let mut dctx = DecompressionContext::new(TTZipArchiveFormat::Zstd);

    assert_eq!(cctx.format(), TTZipArchiveFormat::Zstd);
    assert_eq!(cctx.level(), TTZipCompressionLevel::Normal);
    assert_eq!(dctx.format(), TTZipArchiveFormat::Zstd);

    let mut out_comp = Vec::new();
    let mut out_decomp = Vec::new();

    for i in 1..=5 {
        let payload = format!("TTZip Iteration payload #{} with repeatable contents.", i).into_bytes();
        out_comp.clear();
        out_decomp.clear();

        let comp_sz = cctx.compress_to_vec(&payload, &mut out_comp).expect("compress_to_vec");
        assert_eq!(comp_sz, out_comp.len());

        let decomp_sz = dctx.decompress_to_vec(&out_comp, &mut out_decomp).expect("decompress_to_vec");
        assert_eq!(decomp_sz, payload.len());
        assert_eq!(out_decomp, payload);

        cctx.reset();
        dctx.reset();
    }
}

// =============================================================================
// 6. API Stratification: Layer 3 (Streaming API)
// =============================================================================

#[test]
fn test_layer3_streaming_compressor_decompressor() {
    let data = b"Streaming chunk 1... Streaming chunk 2... Streaming chunk 3... Finished!";
    let mut compressed_sink = Vec::new();

    let mut stream_comp = StreamCompressor::with_chunk_size(
        &mut compressed_sink,
        TTZipArchiveFormat::Zstd,
        TTZipCompressionLevel::Normal,
        32,
    );

    stream_comp
        .write_chunk(&data[..20], StreamFlushMode::None)
        .expect("write_chunk 1");
    stream_comp
        .write_chunk(&data[20..40], StreamFlushMode::SyncFlush)
        .expect("write_chunk 2");
    stream_comp
        .write_chunk(&data[40..], StreamFlushMode::Finish)
        .expect("write_chunk 3");

    assert!(stream_comp.cursor().bytes_in >= data.len() as u64);
    stream_comp.finish().expect("stream_comp finish");

    assert!(!compressed_sink.is_empty());

    let mut stream_decomp =
        StreamDecompressor::new(compressed_sink.as_slice(), TTZipArchiveFormat::Zstd);
    let mut recovered = Vec::new();
    let mut chunk_buf = [0u8; 16];

    loop {
        let n = stream_decomp.read_chunk(&mut chunk_buf).expect("read_chunk");
        if n == 0 {
            break;
        }
        recovered.extend_from_slice(&chunk_buf[..n]);
    }

    assert_eq!(recovered, data);
}

// =============================================================================
// 7. API Stratification: Layer 4 (Advanced API)
// =============================================================================

#[test]
fn test_layer4_advanced_builder_pipeline() {
    let payload = b"Advanced API configuration test payload with custom parameters and block sizes.";

    let compressor = AdvancedCompressorBuilder::new(TTZipArchiveFormat::Zstd)
        .level(TTZipCompressionLevel::Fast)
        .threads(2)
        .checksum(true)
        .block_size(2 * 1024 * 1024)
        .memory_budget_mb(128)
        .build();

    assert_eq!(compressor.config().threads, 2);
    assert_eq!(compressor.config().memory_budget_mb, 128);

    let compressed = compressor.compress_to_vec(payload).expect("advanced compress");
    assert!(!compressed.is_empty());

    let decompressor = AdvancedDecompressorBuilder::new(TTZipArchiveFormat::Zstd)
        .threads(2)
        .memory_budget_mb(128)
        .verify_checksum(true)
        .build();

    let decompressed = decompressor.decompress_to_vec(&compressed).expect("advanced decompress");
    assert_eq!(decompressed, payload);
}

// =============================================================================
// 8. ABI Export Guard & Whitelist Verification
// =============================================================================

#[test]
fn test_abi_export_guard_and_whitelist() {
    assert_eq!(unsafe { ttzip_abi_version() }, 2);
    assert!(unsafe { ttzip_abi_is_compatible(2) });
    assert!(!unsafe { ttzip_abi_is_compatible(1) });
    assert!(!unsafe { ttzip_abi_is_compatible(999) });

    assert!(is_symbol_in_whitelist("ttzip_rust_crc32"));
    assert!(is_symbol_in_whitelist("ttzip_rust_deflate_compress"));
    assert!(is_symbol_in_whitelist("ttzip_abi_version"));
    assert!(!is_symbol_in_whitelist("uniffi_private_internal_symbol"));
    assert!(!is_symbol_in_whitelist("rogue_symbol"));

    let valid_symbols = ["ttzip_rust_crc32", "ttzip_rust_zstd_compress"];
    assert!(verify_symbols_whitelist(&valid_symbols).is_ok());

    let invalid_symbols = ["ttzip_rust_crc32", "unauthorized_export"];
    let err = verify_symbols_whitelist(&invalid_symbols).expect_err("should reject");
    assert_eq!(err, vec!["unauthorized_export"]);

    // Panic boundary protection
    let normal_val = catch_ffi_boundary(42, || 100);
    assert_eq!(normal_val, 100);

    let panic_caught_val = catch_ffi_boundary(42, || {
        panic!("Controlled test panic inside FFI boundary");
    });
    assert_eq!(panic_caught_val, 42);

    let status_res = catch_ffi_status(|| Ok(TTZipStatus::Ok));
    assert_eq!(status_res, 0);

    let panic_status = catch_ffi_status(|| {
        panic!("Controlled panic for status check");
    });
    assert_eq!(panic_status, TTZipStatus::ErrPanicCaught.to_i32());
}
