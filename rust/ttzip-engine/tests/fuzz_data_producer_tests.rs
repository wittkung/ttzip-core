// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Fuzz Data Producer & Multi-Format Fuzz Matrix Integration Tests (Task 12.2).
//!
//! Validates:
//! 1. `test_fuzz_zip_parameter_orthogonality`: Verifies payload prefix mutations do not disrupt tail parameters.
//! 2. `test_fuzz_tar_streaming_chunks`: Verifies arbitrary streaming chunk size consumption and TAR roundtrip.
//! 3. `test_fuzz_seekable_vfs_random_seek`: Verifies 64-bit random seek offsets and buffer boundaries on VFS.
//! 4. `test_fuzz_password_key_derivation`: Verifies variable-length password & salt reverse extraction in KDF.
//! 5. `test_fuzz_archive_roundtrip_params`: Verifies dynamic codec levels & parameters roundtrip fidelity.
//! 6. `test_fuzz_malformed_archive_hunter`: Verifies graceful error handling and zero panics under mutated payloads.

use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::archive::find_next_pk_signature;
use ttzip_engine::archive::nested_vfs::VirtualFileStream;
use ttzip_engine::archive::tar::{TarArchive, TarWriter};
use ttzip_engine::codecs::deflate::{deflate_compress, deflate_decompress, zlib_compress, zlib_decompress};
use ttzip_engine::codecs::zstd::{zstd_compress, zstd_compress_bound, zstd_decompress};
use ttzip_engine::crypto::sha1::winzip_aes256_derive_keys;
use ttzip_engine::crypto::vault::pbkdf2_hmac_sha256;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::testing::{
    mutate_stream, FuzzDataProducer, MutationOperator, SplitMix64,
};
use ttzip_engine::types::{TTZipEncryptionMethod, TTZipStatus};
use ttzip_engine::zip::reader::ZipArchive;
use ttzip_engine::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

fn generate_synthetic_payload(seed: u64, len: usize) -> Vec<u8> {
    let mut prng = SplitMix64::new(seed);
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(prng.next_range(0, 256) as u8);
    }
    out
}

#[test]
fn test_fuzz_zip_parameter_orthogonality() {
    let base_payload = b"TTZip High-Performance Rust Archive Engine Fuzzing Payload 2026";
    // Tail layout: [level: u32 (4 bytes), chunk_size: u64 (8 bytes), is_enc: u8 (1 byte)]
    let mut tail_config = Vec::new();
    tail_config.extend_from_slice(&5u32.to_le_bytes()); // level: 1 + (5 % 9) = 6
    tail_config.extend_from_slice(&3584u64.to_le_bytes()); // chunk: 512 + 3584 = 4096
    tail_config.push(0x01); // is_enc: true

    let mut combined = Vec::new();
    combined.extend_from_slice(base_payload);
    combined.extend_from_slice(&tail_config);

    // Initial extraction baseline
    let mut producer = FuzzDataProducer::new(&combined);
    let is_enc = producer.consume_bool();
    let chunk_size = producer.consume_usize_range(512, 65536);
    let level = producer.consume_u32_range(1, 9);
    let prefix = producer.reserve_data_prefix();

    assert!(is_enc);
    assert_eq!(chunk_size, 4096);
    assert_eq!(level, 6);
    assert_eq!(prefix, base_payload);

    // Mutate prefix only, verifying tail parameters remain 100% orthogonal and invariant
    let mut prng = SplitMix64::new(0xDEADBEEFCAFE0001);
    for op_idx in 0..10 {
        let op = MutationOperator::from_u32(op_idx).unwrap();
        let mutated_prefix = mutate_stream(base_payload, op, &mut prng);

        let mut mutated_combined = Vec::new();
        mutated_combined.extend_from_slice(&mutated_prefix);
        mutated_combined.extend_from_slice(&tail_config);

        let mut mut_producer = FuzzDataProducer::new(&mutated_combined);
        let m_enc = mut_producer.consume_bool();
        let m_chunk = mut_producer.consume_usize_range(512, 65536);
        let m_level = mut_producer.consume_u32_range(1, 9);
        let m_prefix = mut_producer.reserve_data_prefix();

        assert_eq!(m_enc, is_enc, "Encryption flag must not change when prefix mutates");
        assert_eq!(m_chunk, chunk_size, "Chunk size must not change when prefix mutates");
        assert_eq!(m_level, level, "Compression level must not change when prefix mutates");
        assert_eq!(m_prefix, mutated_prefix.as_slice());

        // Assemble and parse ZIP with fuzzed parameters safely
        let items = vec![ZipInputItem {
            rel_path: "fuzzed_entry.dat".to_string(),
            data: m_prefix.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];

        let enc_method = if m_enc {
            TTZipEncryptionMethod::Aes256
        } else {
            TTZipEncryptionMethod::None
        };
        let password = if m_enc { Some("FuzzPass123") } else { None };

        let comp_res = compress_items_parallel(items, m_level as i32, enc_method, password, 2);
        if let Ok(compressed) = comp_res {
            if let Ok(archive_bytes) = assemble_zip_archive(&compressed) {
                let open_res = ZipArchive::open_slice(&archive_bytes);
                assert!(open_res.is_ok(), "Assembled ZIP must parse successfully");
            }
        }
    }
}

#[test]
fn test_fuzz_tar_streaming_chunks() {
    let mut prng = SplitMix64::new(0x2026AABBCCDDEEFF);

    for iter in 0..100 {
        let payload_len = prng.next_range(64, 4096);
        let raw_fuzz_bytes = generate_synthetic_payload(prng.next_u64(), payload_len + 32);

        let mut producer = FuzzDataProducer::new(&raw_fuzz_bytes);
        let in_chunk_size = producer.consume_usize_range(1, 65536);
        let out_chunk_size = producer.consume_usize_range(1, 65536);
        let mode = producer.consume_u32_range(0o600, 0o777);
        let mtime = producer.consume_u64_range(1000000000, 2000000000) as i64;
        let file_payload = producer.reserve_data_prefix();

        assert!((1..=65536).contains(&in_chunk_size));
        assert!((1..=65536).contains(&out_chunk_size));
        assert!((0o600..=0o777).contains(&mode));

        // Create TAR archive in memory using consumed parameters
        let mut tar_writer = TarWriter::new(Vec::new());
        let file_name = format!("stream_file_{}.bin", iter);
        let append_res = tar_writer.append_file(&file_name, file_payload, mode, mtime);
        assert!(append_res.is_ok(), "TAR append_file must succeed");

        let finish_res = tar_writer.finish();
        assert!(finish_res.is_ok(), "TAR finish must succeed");

        let tar_bytes = tar_writer.into_inner();
        assert!(!tar_bytes.is_empty());
        assert_eq!(tar_bytes.len() % 512, 0, "TAR archive size must be 512-byte aligned");

        // Parse and verify using TarArchive
        let open_res = TarArchive::open_slice(&tar_bytes);
        assert!(open_res.is_ok(), "Generated TAR must be parsed cleanly");
        let archive = open_res.unwrap();
        assert_eq!(archive.len(), 1);
        let entry = &archive.entries()[0];
        assert_eq!(entry.path.as_ref(), file_name.as_str());
        assert_eq!(entry.size as usize, file_payload.len());
        let entry_payload = archive.extract_entry_bytes(0).expect("extract entry failed");
        assert_eq!(entry_payload, file_payload);
    }
}

#[test]
fn test_fuzz_seekable_vfs_random_seek() {
    let payload = generate_synthetic_payload(0x55AA112233445566, 128 * 1024); // 128 KB buffer
    let vfs = VirtualFileStream::from_vec(payload.clone());
    assert_eq!(vfs.size(), payload.len() as u64);

    let mut prng = SplitMix64::new(0x9988776655443322);

    for _ in 0..500 {
        let fuzz_seed_bytes = generate_synthetic_payload(prng.next_u64(), 32);
        let mut producer = FuzzDataProducer::new(&fuzz_seed_bytes);

        let random_seek_offset = producer.consume_u64();
        let read_len = producer.consume_u32_range(0, 32 * 1024);
        let whence_offset = producer.consume_u64_range(0, payload.len() as u64 + 1024);

        // 1. Direct seek validation (must clamp to size without panic)
        let seek_res = catch_unwind(AssertUnwindSafe(|| {
            let actual_seek = vfs.seek(random_seek_offset).expect("VFS seek must not error");
            assert_eq!(actual_seek, random_seek_offset.min(vfs.size()));

            let read_data = vfs.read(read_len).expect("VFS read must not error");
            let expected_read_len = (read_len as u64).min(vfs.size().saturating_sub(actual_seek)) as usize;
            assert_eq!(read_data.len(), expected_read_len);
        }));
        assert!(seek_res.is_ok(), "VFS random seek and read panicked!");

        // 2. read_exact_at validation
        let exact_res = catch_unwind(AssertUnwindSafe(|| {
            let chunk = vfs.read_exact_at(whence_offset, read_len).expect("read_exact_at must succeed");
            if whence_offset >= vfs.size() {
                assert!(chunk.is_empty());
            } else {
                let max_avail = (vfs.size() - whence_offset) as usize;
                let expected = (read_len as usize).min(max_avail);
                assert_eq!(chunk.len(), expected);
                let start = whence_offset as usize;
                assert_eq!(&chunk[..], &payload[start..start + expected]);
            }
        }));
        assert!(exact_res.is_ok(), "VFS read_exact_at panicked!");
    }
}

#[test]
fn test_fuzz_password_key_derivation() {
    let mut prng = SplitMix64::new(0xFEEDBEEF01020304);

    for _ in 0..200 {
        let fuzz_bytes = generate_synthetic_payload(prng.next_u64(), 96);
        let mut producer = FuzzDataProducer::new(&fuzz_bytes);

        let rounds = producer.consume_u32_range(1, 100);
        let key_len = producer.consume_usize_range(16, 64);
        let salt_len = producer.consume_usize_range(0, 32);
        let salt = producer.consume_bytes(salt_len);
        let pass_len = producer.consume_usize_range(0, 64);
        let password = producer.consume_string(pass_len);

        let mut derived_key = vec![0u8; key_len];

        let kdf_res = catch_unwind(AssertUnwindSafe(|| {
            // PBKDF2 HMAC-SHA256
            if password.is_empty() && salt.is_empty() {
                let res = pbkdf2_hmac_sha256(password.as_bytes(), salt, rounds, &mut derived_key);
                assert_eq!(res, Err(TTZipStatus::ErrInvalidParam));
            } else {
                let res = pbkdf2_hmac_sha256(password.as_bytes(), salt, rounds, &mut derived_key);
                assert_eq!(res, Ok(()));
                assert!(!derived_key.iter().all(|&b| b == 0));
            }

            // WinZip AES-256 Key Derivation (requires 16-byte salt and non-empty password)
            if salt.len() >= 16 && !password.is_empty() {
                let mut salt_16 = [0u8; 16];
                salt_16.copy_from_slice(&salt[..16]);
                let winzip_res = winzip_aes256_derive_keys(&password, &salt_16);
                assert!(winzip_res.is_ok(), "WinZip AES-256 key derivation must succeed");
            }
        }));

        assert!(kdf_res.is_ok(), "Key derivation panicked on fuzz input!");
    }
}

#[test]
fn test_fuzz_archive_roundtrip_params() {
    let mut prng = SplitMix64::new(0x778899AABBCCDDEE);

    for _ in 0..100 {
        let payload_len = prng.next_range(32, 2048);
        let fuzz_bytes = generate_synthetic_payload(prng.next_u64(), payload_len + 16);

        let mut producer = FuzzDataProducer::new(&fuzz_bytes);
        let zstd_level = producer.consume_u32_range(1, 19) as i32;
        let deflate_level = producer.consume_u32_range(1, 9) as i32;
        let codec_choice = producer.consume_u8_range(0, 2);
        let raw_payload = producer.reserve_data_prefix();

        if raw_payload.is_empty() {
            continue;
        }

        let roundtrip_res = catch_unwind(AssertUnwindSafe(|| {
            match codec_choice {
                0 => {
                    // Raw DEFLATE roundtrip
                    let mut comp_buf = vec![0u8; raw_payload.len() + 1024];
                    let comp_len = deflate_compress(raw_payload, &mut comp_buf, deflate_level)
                        .expect("DEFLATE compress failed");
                    let mut decomp_buf = vec![0u8; raw_payload.len()];
                    let decomp_len = deflate_decompress(&comp_buf[..comp_len], &mut decomp_buf)
                        .expect("DEFLATE decompress failed");
                    assert_eq!(decomp_len, raw_payload.len());
                    assert_eq!(&decomp_buf[..], raw_payload);
                }
                1 => {
                    // Zstandard roundtrip
                    let mut comp_buf = vec![0u8; zstd_compress_bound(raw_payload.len())];
                    let comp_len = zstd_compress(raw_payload, &mut comp_buf, zstd_level)
                        .expect("Zstd compress failed");
                    let mut decomp_buf = vec![0u8; raw_payload.len()];
                    let decomp_len = zstd_decompress(&comp_buf[..comp_len], &mut decomp_buf)
                        .expect("Zstd decompress failed");
                    assert_eq!(decomp_len, raw_payload.len());
                    assert_eq!(&decomp_buf[..], raw_payload);
                }
                _ => {
                    // zlib roundtrip
                    let mut comp_buf = vec![0u8; raw_payload.len() + 1024];
                    let comp_len = zlib_compress(raw_payload, &mut comp_buf, deflate_level)
                        .expect("zlib compress failed");
                    let mut decomp_buf = vec![0u8; raw_payload.len()];
                    let decomp_len = zlib_decompress(&comp_buf[..comp_len], &mut decomp_buf)
                        .expect("zlib decompress failed");
                    assert_eq!(decomp_len, raw_payload.len());
                    assert_eq!(&decomp_buf[..], raw_payload);
                }
            }
        }));

        assert!(roundtrip_res.is_ok(), "Codec roundtrip panicked with level!");
    }
}

#[test]
fn test_fuzz_malformed_archive_hunter() {
    // Generate baseline valid ZIP archive
    let items = vec![
        ZipInputItem {
            rel_path: "document.txt".to_string(),
            data: b"TTZip Safe Archive Robustness Hunter".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "subdir/data.bin".to_string(),
            data: vec![0xAB; 512],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];
    let compressed = compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 2)
        .expect("compression failed");
    let baseline_zip = assemble_zip_archive(&compressed).expect("zip assembly failed");

    let mut prng = SplitMix64::new(0xCAFE1234567890AB);
    let mut graceful_rejections = 0u64;
    let mut successful_parses = 0u64;
    let mut panics_caught = 0u64;

    for i in 0..500 {
        let op_idx = (i % 10) as u32;
        let op = MutationOperator::from_u32(op_idx).unwrap();
        let mutated = mutate_stream(&baseline_zip, op, &mut prng);

        let mut producer = FuzzDataProducer::new(&mutated);
        let _pwd_hint = producer.consume_string(16);
        let _seek_probe = producer.consume_u64();
        let payload_slice = producer.reserve_data_prefix();

        let hunter_result = catch_unwind(AssertUnwindSafe(|| {
            // 1. Attempt opening as ZipArchive
            let zip_res = ZipArchive::open_slice(payload_slice);
            if let Ok(zip) = zip_res {
                for idx in 0..zip.len() {
                    let _ = zip.extract_entry_bytes(idx, None);
                }
                successful_parses += 1;
            } else {
                graceful_rejections += 1;
            }

            // 2. Attempt opening as TarArchive
            let _ = TarArchive::open_slice(payload_slice);

            // 3. Attempt opening as SevenZArchive
            let _ = SevenZArchive::open_slice(payload_slice);

            // 4. Attempt scanning for valid PK signature
            let _ = find_next_pk_signature(payload_slice, 0);
        }));

        if hunter_result.is_err() {
            panics_caught += 1;
        }
    }

    assert_eq!(panics_caught, 0, "Malformed archive hunter encountered panic!");
    assert!(graceful_rejections > 0, "Corrupted streams must trigger graceful rejections");
}
