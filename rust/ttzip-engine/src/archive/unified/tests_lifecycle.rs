// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Creation, extraction, split volume, cancellation, and repair unit tests for Unified Orchestrator.

#[cfg(test)]
mod tests {
    use crate::archive::split::detect_volume_chain;
    use crate::archive::unified::UnifiedArchiveOrchestrator;
    use crate::types::{
        TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
        TTZipExtractOptions, TTZipStatus,
    };
    use libc::c_void;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_unified_create_and_extract_roundtrip_zip() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("hello.txt");
        fs::write(&src_file, b"Hello Unified Archive Engine!").unwrap();

        let zip_out = dir.path().join("test.zip");
        let extract_out = dir.path().join("extracted_zip");

        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[src_file],
            &zip_out,
            &create_opt,
            0,
        )
        .expect("Create zip failed");

        assert!(zip_out.exists());

        let extract_opt = TTZipExtractOptions {
            struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            destination_path: std::ptr::null(),
            password: std::ptr::null(),
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::extract_archive(
            &zip_out,
            &extract_out,
            &extract_opt,
        )
        .expect("Extract zip failed");

        let restored_file = extract_out.join("hello.txt");
        assert!(restored_file.exists());
        assert_eq!(
            fs::read(&restored_file).unwrap(),
            b"Hello Unified Archive Engine!"
        );
    }

    #[test]
    fn test_unified_create_and_extract_tar_variants() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("payload.dat");
        fs::write(&src_file, b"TAR.GZ payload data content").unwrap();

        let variants = [
            (TTZipArchiveFormat::TarGz, "archive.tar.gz"),
            (TTZipArchiveFormat::TarBz2, "archive.tar.bz2"),
            (TTZipArchiveFormat::TarXz, "archive.tar.xz"),
            (TTZipArchiveFormat::TarZstd, "archive.tar.zst"),
        ];

        for (fmt, filename) in variants {
            let out_archive = dir.path().join(filename);
            let extract_dir = dir.path().join(format!("ext_{}", filename));

            let create_opt = TTZipCreateOptions {
                struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
                abi_version: crate::types::TTZIP_ABI_VERSION_2,
                format: fmt,
                level: TTZipCompressionLevel::Fast,
                encryption: TTZipEncryptionMethod::None,
                password: std::ptr::null(),
                thread_budget: 0,
                solid_block_size_mb: 0,
                progress_callback: None,
                user_data: std::ptr::null_mut(),
            };

            UnifiedArchiveOrchestrator::create_archive(
                std::slice::from_ref(&src_file),
                &out_archive,
                &create_opt,
                0,
            )
            .unwrap_or_else(|_| panic!("Failed to create {}", filename));

            assert!(out_archive.exists());

            let extract_opt = TTZipExtractOptions {
                struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
                abi_version: crate::types::TTZIP_ABI_VERSION_2,
                destination_path: std::ptr::null(),
                password: std::ptr::null(),
                thread_budget: 0,
                overwrite_existing: true,
                preserve_permissions: true,
                dry_run: false,
                progress_callback: None,
                user_data: std::ptr::null_mut(),
            };

            UnifiedArchiveOrchestrator::extract_archive(
                &out_archive,
                &extract_dir,
                &extract_opt,
            )
            .unwrap_or_else(|_| panic!("Failed to extract {}", filename));

            let restored = extract_dir.join("payload.dat");
            assert!(restored.exists());
            assert_eq!(fs::read(&restored).unwrap(), b"TAR.GZ payload data content");
        }
    }

    #[test]
    fn test_unified_split_volume_creation_and_extraction() {
        let dir = tempdir().unwrap();
        let payload_file = dir.path().join("large_payload.bin");
        let payload_data = vec![0x77u8; 3000];
        fs::write(&payload_file, &payload_data).unwrap();

        let split_out = dir.path().join("archive.zip");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Store,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[payload_file],
            &split_out,
            &create_opt,
            1000,
        )
        .expect("Split create failed");

        let chain = detect_volume_chain(&split_out).expect("detect volume chain");
        assert!(chain.len() >= 2);

        let ext_dir = dir.path().join("split_extracted");
        let extract_opt = TTZipExtractOptions {
            struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            destination_path: std::ptr::null(),
            password: std::ptr::null(),
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::extract_archive(
            &chain[0],
            &ext_dir,
            &extract_opt,
        )
        .expect("Split extract failed");

        let restored = ext_dir.join("large_payload.bin");
        assert!(restored.exists());
        assert_eq!(fs::read(&restored).unwrap(), payload_data);
    }

    #[test]
    fn test_unified_repair_damaged_archive() {
        let dir = tempdir().unwrap();
        let payload_file = dir.path().join("repair_source.txt");
        fs::write(&payload_file, b"Repairable Payload Data Content").unwrap();

        let zip_path = dir.path().join("corrupt.zip");
        let repaired_path = dir.path().join("repaired.zip");

        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Store,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[payload_file],
            &zip_path,
            &create_opt,
            0,
        )
        .expect("Create zip");

        let data = fs::read(&zip_path).unwrap();
        let truncated_len = data.len().saturating_sub(22);
        fs::write(&zip_path, &data[..truncated_len]).unwrap();

        let salvaged = UnifiedArchiveOrchestrator::repair_archive(&zip_path, &repaired_path)
            .expect("Repair archive");
        assert!(salvaged >= 1);
        assert!(repaired_path.exists());
    }

    #[test]
    fn test_unified_cancellation_callback() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("cancel1.txt");
        let f2 = dir.path().join("cancel2.txt");
        fs::write(&f1, b"File 1 content").unwrap();
        fs::write(&f2, b"File 2 content").unwrap();

        let zip_out = dir.path().join("cancelled.zip");

        unsafe extern "C" fn cancel_cb(
            _proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            false
        }

        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: Some(cancel_cb),
            user_data: std::ptr::null_mut(),
        };

        let res = UnifiedArchiveOrchestrator::create_archive(
            &[f1, f2],
            &zip_out,
            &create_opt,
            0,
        );

        assert_eq!(res, Err(TTZipStatus::Cancelled));
    }

    #[test]
    fn test_unified_streaming_compound_tar_roundtrips_and_zero_disk_leak() {
        let dir = tempdir().unwrap();
        let src_file1 = dir.path().join("file1.txt");
        let src_file2 = dir.path().join("file2.log");
        let data1 = b"Pure Streaming TAR and Codec Pipeline payload 1 - eliminating 200% write amplification!";
        let data2 = b"Secondary file payload in compressed archive for zero-copy block extraction verification.";
        fs::write(&src_file1, data1).unwrap();
        fs::write(&src_file2, data2).unwrap();

        let cases = [
            (TTZipArchiveFormat::TarZstd, "test_stream.tar.zst"),
            (TTZipArchiveFormat::TarBrotli, "test_stream.tar.br"),
            (TTZipArchiveFormat::Snappy, "test_stream.sz"),
        ];

        for (fmt, filename) in cases {
            let archive_out = dir.path().join(filename);
            let extract_out = dir.path().join(format!("ext_{}", filename));

            let create_opt = TTZipCreateOptions {
                struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
                abi_version: crate::types::TTZIP_ABI_VERSION_2,
                format: fmt,
                level: TTZipCompressionLevel::Normal,
                encryption: TTZipEncryptionMethod::None,
                password: std::ptr::null(),
                thread_budget: 0,
                solid_block_size_mb: 0,
                progress_callback: None,
                user_data: std::ptr::null_mut(),
            };

            UnifiedArchiveOrchestrator::create_archive(
                &[src_file1.clone(), src_file2.clone()],
                &archive_out,
                &create_opt,
                0,
            )
            .unwrap_or_else(|_| panic!("Failed to stream create {}", filename));

            assert!(archive_out.exists(), "Archive {} must exist", filename);

            // Verify no temporary files exist in the parent folder
            let mut tmp_files_count = 0;
            if let Ok(entries) = fs::read_dir(dir.path()) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.contains("ttzip_tmp_") || name.contains("ttzip_decomp_") {
                        tmp_files_count += 1;
                    }
                }
            }
            assert_eq!(tmp_files_count, 0, "No temporary tar files should ever be created on disk");

            let extract_opt = TTZipExtractOptions {
                struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
                abi_version: crate::types::TTZIP_ABI_VERSION_2,
                destination_path: std::ptr::null(),
                password: std::ptr::null(),
                thread_budget: 0,
                overwrite_existing: true,
                preserve_permissions: true,
                dry_run: false,
                progress_callback: None,
                user_data: std::ptr::null_mut(),
            };

            let extracted_bytes = UnifiedArchiveOrchestrator::extract_archive_with_metrics(
                &archive_out,
                &extract_out,
                &extract_opt,
            )
            .unwrap_or_else(|_| panic!("Failed to stream extract {}", filename));

            assert!(extracted_bytes >= (data1.len() + data2.len()) as u64);

            let rest1 = extract_out.join("file1.txt");
            let rest2 = extract_out.join("file2.log");
            assert!(rest1.exists(), "restored file1 must exist for {}", filename);
            assert!(rest2.exists(), "restored file2 must exist for {}", filename);
            assert_eq!(fs::read(&rest1).unwrap(), data1);
            assert_eq!(fs::read(&rest2).unwrap(), data2);
        }
    }

    #[test]
    fn test_compression_levels_propagation_difference() {
        let dir = tempdir().unwrap();
        let payload_file = dir.path().join("repetitive.txt");
        let rep_data = b"TTZip High-Performance Native Archiving Engine Compression Level Verification 2026! ".repeat(5000);
        fs::write(&payload_file, &rep_data).unwrap();

        let fast_out = dir.path().join("archive_fast.tar.zst");
        let ultra_out = dir.path().join("archive_ultra.tar.zst");

        let opt_fast = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::TarZstd,
            level: TTZipCompressionLevel::Fastest,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(&[payload_file.clone()], &fast_out, &opt_fast, 0)
            .expect("create fast archive");

        let mut opt_ultra = opt_fast;
        opt_ultra.level = TTZipCompressionLevel::Ultra;
        UnifiedArchiveOrchestrator::create_archive(&[payload_file], &ultra_out, &opt_ultra, 0)
            .expect("create ultra archive");

        let size_fast = fs::metadata(&fast_out).unwrap().len();
        let size_ultra = fs::metadata(&ultra_out).unwrap().len();

        assert!(
            size_ultra <= size_fast,
            "Ultra compression (level 22) must be <= Fastest (level 1): ultra={}, fast={}",
            size_ultra,
            size_fast
        );
    }

    #[test]
    fn test_unsupported_compound_in_place_edit_error() {
        use crate::archive::in_place_edit::compound::in_place_edit_compound_stream;
        use crate::standards::signatures::CompoundFormat;

        let dir = tempdir().unwrap();
        let fake_archive = dir.path().join("fake.tar.br");
        fs::write(&fake_archive, b"dummy content").unwrap();
        let shadow = dir.path().join("shadow.tar.br");

        let res = in_place_edit_compound_stream(
            &fake_archive,
            &shadow,
            CompoundFormat::TarBrotli,
            &[],
        );

        assert_eq!(res, Err(TTZipStatus::ErrUnsupportedFeature));
    }

}
