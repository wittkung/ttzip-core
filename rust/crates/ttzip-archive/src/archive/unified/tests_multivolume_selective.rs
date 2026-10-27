// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Selective extraction, cancellation, and multi-volume streaming roundtrip unit tests.

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use libc::c_void;
    use tempfile::tempdir;

    use crate::archive::split::detect_volume_chain;
    use crate::archive::unified::create::create_archive_explicit;
    use crate::archive::unified::extract_single::{
        extract_selected_entries, extract_single_entry_memory,
    };
    use crate::archive::unified::selection::EntrySelection;
    use crate::archive::unified::UnifiedArchiveOrchestrator;
    use crate::types::{
        TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
        TTZipExtractOptions, TTZipStatus,
    };

    #[test]
    fn test_extract_selected_entries_cancellation_during_chunk_read() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file.bin");
        let data = vec![0xA5u8; 256 * 1024];
        fs::write(&src_file, &data).unwrap();

        let archive_path = dir.path().join("test_extract_cancel.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0)
            .expect("Failed to create test archive");

        let dest_dir = dir.path().join("extracted");
        fs::create_dir_all(&dest_dir).unwrap();
        let staging = crate::fs::collision::staging::StagingArea::create(&dest_dir).unwrap();

        static CHUNK_CANCEL_TRIGGERED: AtomicBool = AtomicBool::new(false);
        CHUNK_CANCEL_TRIGGERED.store(false, Ordering::SeqCst);

        unsafe extern "C" fn cancel_on_chunk_cb(
            proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            if proc > 0 {
                CHUNK_CANCEL_TRIGGERED.store(true, Ordering::SeqCst);
                false
            } else {
                true
            }
        }

        let extract_opt = TTZipExtractOptions {
            progress_callback: Some(cancel_on_chunk_cb),
            ..Default::default()
        };

        let res = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["big_file.bin"], false),
            &staging.payload_dir(),
            &extract_opt,
        );

        assert_eq!(res, Err(TTZipStatus::Cancelled));
        assert!(CHUNK_CANCEL_TRIGGERED.load(Ordering::SeqCst));

        let target_extracted_path = staging.payload_dir().path().join("big_file.bin");
        assert!(
            !target_extracted_path.exists(),
            "Truncated file must be destroyed upon cancellation"
        );
    }

    #[test]
    fn test_extract_selected_entries_cancellation_on_header() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("small_file.bin");
        fs::write(&src_file, b"content").unwrap();

        let archive_path = dir.path().join("test_extract_cancel_header.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0)
            .expect("Failed to create test archive");

        let dest_dir = dir.path().join("extracted_header");
        fs::create_dir_all(&dest_dir).unwrap();
        let staging = crate::fs::collision::staging::StagingArea::create(&dest_dir).unwrap();

        unsafe extern "C" fn cancel_immediate_cb(
            _proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            false
        }

        let extract_opt = TTZipExtractOptions {
            progress_callback: Some(cancel_immediate_cb),
            ..Default::default()
        };

        let res = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["small_file.bin"], false),
            &staging.payload_dir(),
            &extract_opt,
        );

        assert_eq!(res, Err(TTZipStatus::Cancelled));
    }

    #[test]
    fn test_extract_selected_entries_root_at_common_parent() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("docs").join("sub");
        fs::create_dir_all(sub.join("img")).unwrap();
        fs::write(sub.join("report.txt"), b"deep report payload").unwrap();
        fs::write(sub.join("img").join("a.png"), b"png payload").unwrap();

        let archive_path = dir.path().join("test_common_parent.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(
            &[dir.path().join("docs")],
            &archive_path,
            &create_opt,
            0,
        )
        .expect("Failed to create test archive");

        let dest_dir = dir.path().join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let extract_opt = TTZipExtractOptions::default();

        let staging = crate::fs::collision::staging::StagingArea::create(&dest_dir).unwrap();
        let count = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["docs/sub/report.txt"], true),
            &staging.payload_dir(),
            &extract_opt,
        )
        .expect("Selective extraction with common parent should succeed");
        let payload = staging.payload_dir().path().to_path_buf();
        assert_eq!(count, 1);
        assert_eq!(
            fs::read(payload.join("report.txt")).unwrap(),
            b"deep report payload"
        );
        assert!(!payload.join("docs").exists());
        assert!(
            !dest_dir.join("report.txt").exists(),
            "destination must not be written directly"
        );
        staging.abort();

        let staging = crate::fs::collision::staging::StagingArea::create(&dest_dir).unwrap();
        extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["docs/sub/img/"], true),
            &staging.payload_dir(),
            &extract_opt,
        )
        .expect("Directory selection should succeed");
        let payload = staging.payload_dir().path().to_path_buf();
        assert_eq!(
            fs::read(payload.join("img").join("a.png")).unwrap(),
            b"png payload"
        );
        assert!(!payload.join("report.txt").exists());
    }

    #[test]
    fn test_create_archive_cancellation_during_chunk_write() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file_for_create.bin");
        let data = vec![0x3Cu8; 256 * 1024];
        fs::write(&src_file, &data).unwrap();

        let archive_path = dir.path().join("test_create_cancel.tar.gz");

        static CREATE_CHUNK_CANCEL: AtomicBool = AtomicBool::new(false);
        CREATE_CHUNK_CANCEL.store(false, Ordering::SeqCst);

        unsafe extern "C" fn cancel_after_first_chunk(
            proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            if proc >= 65536 {
                CREATE_CHUNK_CANCEL.store(true, Ordering::SeqCst);
                false
            } else {
                true
            }
        }

        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::TarGz,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: Some(cancel_after_first_chunk),
            user_data: std::ptr::null_mut(),
        };

        let res =
            UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0);

        assert_eq!(res, Err(TTZipStatus::Cancelled));
        assert!(CREATE_CHUNK_CANCEL.load(Ordering::SeqCst));
    }

    #[test]
    fn test_extract_single_memory_budget_limit() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("budget_test.bin");
        let data = b"selective in-memory extraction payload".to_vec();
        fs::write(&src_file, &data).unwrap();

        let archive_path = dir.path().join("test_budget.tar.gz");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::TarGz,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0)
            .expect("Failed to create test archive");

        let res = extract_single_entry_memory(&archive_path, Some("budget_test.bin"), -1, None);
        assert_eq!(res.unwrap(), data);
    }

    fn pseudo_random_bytes(seed: u64, len: usize) -> Vec<u8> {
        let mut x = seed | 1;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            out.push((x & 0xFF) as u8);
        }
        out
    }

    #[test]
    fn test_split_zip_streaming_create_inspect_and_extract_roundtrip() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("alpha.bin");
        let f2 = dir.path().join("beta.bin");
        let data_a = pseudo_random_bytes(0x1234_5678_9ABC_DEF0, 18_000);
        let data_b = pseudo_random_bytes(0xFEDC_BA98_7654_3210, 22_000);
        fs::write(&f1, &data_a).unwrap();
        fs::write(&f2, &data_b).unwrap();

        let zip_out = dir.path().join("spanned_encrypted.zip");
        let pwd = CString::new("SplitZipSecret!2026").unwrap();
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Fast,
            encryption: TTZipEncryptionMethod::Aes256,
            password: pwd.as_ptr(),
            thread_budget: 2,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        create_archive_explicit(
            &[(f1, "nested/alpha.bin".to_string()), (f2, "nested/beta.bin".to_string())],
            &zip_out,
            &create_opt,
            12 * 1024,
        )
        .expect("Split ZIP creation should succeed");

        let chain = detect_volume_chain(&zip_out).expect("Detect PKZIP spanned chain");
        assert!(chain.len() >= 2, "Expected multiple ZIP volumes, got {:?}", chain);

        let mut inspected_paths = Vec::new();
        let count = crate::archive::unified::inspect::inspect_archive(
            &chain[0],
            Some("SplitZipSecret!2026"),
            false,
            |entry| {
                inspected_paths.push(entry.path.to_string());
                true
            },
        )
        .expect("Inspect multi-volume ZIP via .z01 seed");
        assert_eq!(count, 2);
        assert!(inspected_paths.contains(&"nested/alpha.bin".to_string()));
        assert!(inspected_paths.contains(&"nested/beta.bin".to_string()));

        let single_bytes = extract_single_entry_memory(
            &chain[0],
            Some("nested/beta.bin"),
            -1,
            Some("SplitZipSecret!2026"),
        )
        .expect("Single-entry memory extract from split ZIP");
        assert_eq!(single_bytes, data_b);

        let extract_opt = TTZipExtractOptions {
            password: pwd.as_ptr(),
            ..Default::default()
        };
        let sel_dir = dir.path().join("sel_out");
        fs::create_dir_all(&sel_dir).unwrap();
        let staging = crate::fs::collision::staging::StagingArea::create(&sel_dir).unwrap();
        let sel_count = extract_selected_entries(
            &zip_out,
            &EntrySelection::new(["nested/alpha.bin"], true),
            &staging.payload_dir(),
            &extract_opt,
        )
        .expect("Selective extract from split ZIP");
        assert_eq!(sel_count, 1);
        assert_eq!(
            fs::read(staging.payload_dir().path().join("alpha.bin")).unwrap(),
            data_a
        );
        staging.abort();

        let full_dir = dir.path().join("full_out");
        fs::create_dir_all(&full_dir).unwrap();
        UnifiedArchiveOrchestrator::extract_archive(
            &chain[0],
            &crate::fs::collision::staging::StagingRoot(full_dir.clone()),
            &extract_opt,
        )
        .expect("Full extract from split ZIP");
        assert_eq!(fs::read(full_dir.join("nested/alpha.bin")).unwrap(), data_a);
        assert_eq!(fs::read(full_dir.join("nested/beta.bin")).unwrap(), data_b);
    }

    #[test]
    fn test_split_tar_zstd_and_brotli_streaming_roundtrip() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("part1.dat");
        let f2 = dir.path().join("part2.dat");
        let payload1 = pseudo_random_bytes(0xCAFE_BABE_1111_2222, 25_000);
        let payload2 = pseudo_random_bytes(0xDEAD_BEEF_3333_4444, 25_000);
        fs::write(&f1, &payload1).unwrap();
        fs::write(&f2, &payload2).unwrap();

        for (fmt, name) in [
            (TTZipArchiveFormat::TarZstd, "bundle.tar.zst"),
            (TTZipArchiveFormat::Brotli, "bundle.tar.br"),
        ] {
            let out_base = dir.path().join(name);
            let create_opt = TTZipCreateOptions {
                struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
                abi_version: crate::types::TTZIP_ABI_VERSION_3,
                format: fmt,
                level: TTZipCompressionLevel::Fast,
                encryption: TTZipEncryptionMethod::None,
                password: std::ptr::null(),
                thread_budget: 2,
                solid_block_size_mb: 0,
                progress_callback: None,
                user_data: std::ptr::null_mut(),
            };

            create_archive_explicit(
                &[
                    (f1.clone(), "dir/part1.dat".to_string()),
                    (f2.clone(), "dir/part2.dat".to_string()),
                ],
                &out_base,
                &create_opt,
                10 * 1024,
            )
            .expect("Streaming split tar creation should succeed");

            assert!(
                !out_base.exists(),
                "Single-pass split must not leave unsplit intermediate file"
            );
            let chain = detect_volume_chain(&out_base).expect("Detect .001 chain from base path");
            assert!(chain.len() >= 2, "Expected >= 2 volumes for {}, got {:?}", name, chain);

            let count = crate::archive::unified::inspect::inspect_archive(&out_base, None, false, |_| true)
                .expect("Inspect split tar archive");
            assert_eq!(count, 2);

            let mem = extract_single_entry_memory(&chain[0], Some("dir/part2.dat"), -1, None)
                .expect("Single-entry memory extract from split tar");
            assert_eq!(mem, payload2);

            let dest = dir.path().join(format!("ext_{}", name));
            fs::create_dir_all(&dest).unwrap();
            UnifiedArchiveOrchestrator::extract_archive(
                &chain[0],
                &crate::fs::collision::staging::StagingRoot(dest.clone()),
                &TTZipExtractOptions::default(),
            )
            .expect("Full extract from split tar");
            assert_eq!(fs::read(dest.join("dir/part1.dat")).unwrap(), payload1);
            assert_eq!(fs::read(dest.join("dir/part2.dat")).unwrap(), payload2);
        }
    }

    #[test]
    fn test_extract_selected_entries_error_exit_cleans_partial_and_preserves_existing() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file.bin");
        fs::write(&src_file, vec![0x5Au8; 256 * 1024]).unwrap();

        let archive_path = dir.path().join("truncated_selected.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };
        UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0)
            .expect("create tar");
        let mut raw_tar = fs::read(&archive_path).unwrap();
        raw_tar.truncate(100 * 1024);
        fs::write(&archive_path, &raw_tar).unwrap();

        let empty_dest = dir.path().join("empty_dest");
        fs::create_dir_all(&empty_dest).unwrap();
        let staging = crate::fs::collision::staging::StagingArea::create(&empty_dest).unwrap();
        let res = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["big_file.bin"], false),
            &staging.payload_dir(),
            &TTZipExtractOptions::default(),
        );
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
        assert!(
            !staging.payload_dir().path().join("big_file.bin").exists(),
            "Uncommitted partial file must be cleaned up on error exit"
        );

        let existing_root = dir.path().join("existing_root");
        fs::create_dir_all(&existing_root).unwrap();
        fs::write(existing_root.join("big_file.bin"), b"EXISTING_FORMAL_PAYLOAD").unwrap();
        fs::write(existing_root.join("sibling.txt"), b"EXISTING_SIBLING").unwrap();
        let replace_opt = TTZipExtractOptions {
            collision_policy: crate::types::TTZIP_COLLISION_POLICY_REPLACE,
            ..Default::default()
        };
        let res_existing = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["big_file.bin"], false),
            &crate::fs::collision::staging::StagingRoot(existing_root.clone()),
            &replace_opt,
        );
        assert_eq!(res_existing, Err(TTZipStatus::ErrCorruptHeader));
        assert_eq!(
            fs::read(existing_root.join("big_file.bin")).unwrap(),
            b"EXISTING_FORMAL_PAYLOAD"
        );
        assert_eq!(
            fs::read(existing_root.join("sibling.txt")).unwrap(),
            b"EXISTING_SIBLING"
        );
        let entry_names: Vec<String> = fs::read_dir(&existing_root)
            .unwrap()
            .filter_map(|e| e.ok().and_then(|d| d.file_name().into_string().ok()))
            .collect();
        assert!(
            !entry_names.iter().any(|n| n.starts_with(".ttzip_inflight_")),
            "No in-flight temporary file may remain after error: {:?}",
            entry_names
        );
    }

    #[test]
    fn test_extract_selected_entries_cancel_preserves_preexisting_formal_files() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file.bin");
        fs::write(&src_file, vec![0x7Eu8; 256 * 1024]).unwrap();

        let archive_path = dir.path().join("cancel_existing.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };
        UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0)
            .expect("create tar");

        unsafe extern "C" fn cancel_after_chunk_cb(
            proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            proc == 0
        }

        let target_root = dir.path().join("target_with_formal_files");
        fs::create_dir_all(&target_root).unwrap();
        fs::write(target_root.join("big_file.bin"), b"FORMAL_ORIGINAL_V1").unwrap();
        fs::write(target_root.join("keep_me.txt"), b"FORMAL_SIBLING_V1").unwrap();

        let replace_cancel_opt = TTZipExtractOptions {
            collision_policy: crate::types::TTZIP_COLLISION_POLICY_REPLACE,
            progress_callback: Some(cancel_after_chunk_cb),
            ..Default::default()
        };
        let res = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["big_file.bin"], false),
            &crate::fs::collision::staging::StagingRoot(target_root.clone()),
            &replace_cancel_opt,
        );
        assert_eq!(res, Err(TTZipStatus::Cancelled));
        assert_eq!(
            fs::read(target_root.join("big_file.bin")).unwrap(),
            b"FORMAL_ORIGINAL_V1"
        );
        assert_eq!(
            fs::read(target_root.join("keep_me.txt")).unwrap(),
            b"FORMAL_SIBLING_V1"
        );
        let entry_names: Vec<String> = fs::read_dir(&target_root)
            .unwrap()
            .filter_map(|e| e.ok().and_then(|d| d.file_name().into_string().ok()))
            .collect();
        assert!(
            !entry_names.iter().any(|n| n.starts_with(".ttzip_inflight_")),
            "No in-flight temporary file may remain after cancellation: {:?}",
            entry_names
        );
    }

    #[test]
    fn test_inflight_temp_name_collision_retries_exclusively_without_truncation() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("big_file.bin");
        let new_payload = vec![0x33u8; 256 * 1024];
        fs::write(&src_file, &new_payload).unwrap();

        let archive_path = dir.path().join("temp_collision.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };
        UnifiedArchiveOrchestrator::create_archive(&[src_file], &archive_path, &create_opt, 0)
            .expect("create tar");

        unsafe extern "C" fn cancel_after_chunk_cb(
            proc: u64,
            _tot: u64,
            _entry: *const libc::c_char,
            _user_data: *mut c_void,
        ) -> bool {
            proc == 0
        }

        let target_root = dir.path().join("collision_root");
        fs::create_dir_all(&target_root).unwrap();
        let colliding_tmp = target_root.join(".ttzip_inflight_big_file.bin_0");
        fs::write(&colliding_tmp, b"COLLIDING_TEMP_MUST_NOT_TRUNCATE").unwrap();
        fs::write(target_root.join("big_file.bin"), b"FORMAL_BEFORE_REPLACE").unwrap();

        let cancel_opt = TTZipExtractOptions {
            collision_policy: crate::types::TTZIP_COLLISION_POLICY_REPLACE,
            progress_callback: Some(cancel_after_chunk_cb),
            ..Default::default()
        };
        let res_cancel = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["big_file.bin"], false),
            &crate::fs::collision::staging::StagingRoot(target_root.clone()),
            &cancel_opt,
        );
        assert_eq!(res_cancel, Err(TTZipStatus::Cancelled));
        assert_eq!(
            fs::read(&colliding_tmp).unwrap(),
            b"COLLIDING_TEMP_MUST_NOT_TRUNCATE"
        );
        assert_eq!(
            fs::read(target_root.join("big_file.bin")).unwrap(),
            b"FORMAL_BEFORE_REPLACE"
        );

        let ok_opt = TTZipExtractOptions {
            collision_policy: crate::types::TTZIP_COLLISION_POLICY_REPLACE,
            ..Default::default()
        };
        let count = extract_selected_entries(
            &archive_path,
            &EntrySelection::new(["big_file.bin"], false),
            &crate::fs::collision::staging::StagingRoot(target_root.clone()),
            &ok_opt,
        )
        .expect("replace succeeds via retried exclusive temp name");
        assert_eq!(count, 1);
        assert_eq!(
            fs::read(&colliding_tmp).unwrap(),
            b"COLLIDING_TEMP_MUST_NOT_TRUNCATE"
        );
        assert_eq!(fs::read(target_root.join("big_file.bin")).unwrap(), new_payload);
    }

    struct SwapParentCtx {
        root: std::path::PathBuf,
        outside: std::path::PathBuf,
        cancel: bool,
        swapped: bool,
    }

    unsafe extern "C" fn swap_parent_on_chunk_cb(
        proc: u64,
        _tot: u64,
        _entry: *const libc::c_char,
        user_data: *mut c_void,
    ) -> bool {
        if proc == 0 || user_data.is_null() {
            return true;
        }
        let ctx = &mut *(user_data as *mut SwapParentCtx);
        if !ctx.swapped {
            ctx.swapped = true;
            let sub = ctx.root.join("sub");
            let detached = ctx.root.join("sub_detached");
            let _ = fs::rename(&sub, &detached);
            let _ = std::os::unix::fs::symlink(&ctx.outside, &sub);
        }
        !ctx.cancel
    }

    #[test]
    fn test_inflight_parent_dir_replaced_uses_parent_fd_and_protects_outside_and_formal() {
        let dir = tempdir().unwrap();
        let f_in = dir.path().join("big_file.bin");
        fs::write(&f_in, vec![0x99u8; 256 * 1024]).unwrap();

        let archive_path = dir.path().join("parent_swap.tar");
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_3,
            format: TTZipArchiveFormat::Tar,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };
        create_archive_explicit(
            &[(f_in, "sub/big_file.bin".to_string())],
            &archive_path,
            &create_opt,
            0,
        )
        .expect("create tar with sub/big_file.bin");

        let outside = dir.path().join("outside_sensitive");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("big_file.bin"), b"OUTSIDE_FORMAL_FILE").unwrap();
        fs::write(
            outside.join(".ttzip_inflight_big_file.bin_0"),
            b"OUTSIDE_TEMP_FILE",
        )
        .unwrap();

        for (cancel_mode, expected_err) in [
            (true, TTZipStatus::Cancelled),
            (false, TTZipStatus::ErrSecurityViolation),
        ] {
            let root = dir.path().join(if cancel_mode { "root_cancel" } else { "root_commit" });
            fs::create_dir_all(root.join("sub")).unwrap();
            fs::write(root.join("sub/big_file.bin"), b"FORMAL_INSIDE_SUB").unwrap();

            let mut ctx = SwapParentCtx {
                root: root.clone(),
                outside: outside.clone(),
                cancel: cancel_mode,
                swapped: false,
            };
            let opt = TTZipExtractOptions {
                collision_policy: crate::types::TTZIP_COLLISION_POLICY_REPLACE,
                progress_callback: Some(swap_parent_on_chunk_cb),
                user_data: (&mut ctx as *mut SwapParentCtx).cast(),
                ..Default::default()
            };
            let res = extract_selected_entries(
                &archive_path,
                &EntrySelection::new(["sub/big_file.bin"], false),
                &crate::fs::collision::staging::StagingRoot(root.clone()),
                &opt,
            );
            assert_eq!(res, Err(expected_err));
            assert_eq!(fs::read(outside.join("big_file.bin")).unwrap(), b"OUTSIDE_FORMAL_FILE");
            assert_eq!(
                fs::read(outside.join(".ttzip_inflight_big_file.bin_0")).unwrap(),
                b"OUTSIDE_TEMP_FILE"
            );
            let detached = root.join("sub_detached");
            assert_eq!(fs::read(detached.join("big_file.bin")).unwrap(), b"FORMAL_INSIDE_SUB");
            let names: Vec<String> = fs::read_dir(&detached)
                .unwrap()
                .filter_map(|e| e.ok().and_then(|d| d.file_name().into_string().ok()))
                .collect();
            assert!(!names.iter().any(|n| n.starts_with(".ttzip_inflight_")));
        }
    }
}
