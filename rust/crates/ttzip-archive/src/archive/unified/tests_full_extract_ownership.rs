// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! End-to-end full extraction ownership, hardlink write-through defense, and abort cleanup tests.

#[cfg(test)]
mod tests {
    use crate::archive::tar::reader::TarArchive;
    use crate::archive::unified::create::create_archive_explicit;
    use crate::archive::unified::extract_single::extract_selected_entries;
    use crate::archive::unified::selection::EntrySelection;
    use crate::archive::unified::UnifiedArchiveOrchestrator;
    use crate::fs::collision::staging::StagingRoot;
    use crate::types::{
        TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
        TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3, TTZIP_COLLISION_POLICY_REPLACE,
    };
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use libc::c_void;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn write_ustar_header(
        name: &str,
        size: usize,
        typeflag: u8,
        linkname: &str,
    ) -> [u8; 512] {
        let mut hdr = [0u8; 512];
        hdr[..name.len()].copy_from_slice(name.as_bytes());
        hdr[100..107].copy_from_slice(b"0000644");
        hdr[108..115].copy_from_slice(b"0000000");
        hdr[116..123].copy_from_slice(b"0000000");
        let size_oct = format!("{:011o}", size);
        hdr[124..135].copy_from_slice(size_oct.as_bytes());
        hdr[136..147].copy_from_slice(b"00000000000");
        hdr[148..156].fill(b' ');
        hdr[156] = typeflag;
        if !linkname.is_empty() {
            hdr[157..157 + linkname.len()].copy_from_slice(linkname.as_bytes());
        }
        hdr[257..263].copy_from_slice(b"ustar\0");
        hdr[263..265].copy_from_slice(b"00");
        let cksum: u32 = hdr.iter().map(|&b| u32::from(b)).sum();
        let cksum_str = format!("{:06o}\0 ", cksum);
        hdr[148..156].copy_from_slice(cksum_str.as_bytes());
        hdr
    }

    fn append_tar_regular(buf: &mut Vec<u8>, name: &str, payload: &[u8]) {
        buf.extend_from_slice(&write_ustar_header(name, payload.len(), b'0', ""));
        buf.extend_from_slice(payload);
        let pad = (512 - (payload.len() % 512)) % 512;
        buf.resize(buf.len() + pad, 0);
    }

    fn append_tar_hardlink(buf: &mut Vec<u8>, name: &str, target: &str) {
        buf.extend_from_slice(&write_ustar_header(name, 0, b'1', target));
    }

    fn build_tar_a_hardlink_b_then_overwrite_b(forward_hardlink: bool) -> Vec<u8> {
        let mut tar = Vec::new();
        if forward_hardlink {
            append_tar_hardlink(&mut tar, "b", "a");
            append_tar_regular(&mut tar, "a", b"A");
            append_tar_regular(&mut tar, "b", b"B");
        } else {
            append_tar_regular(&mut tar, "a", b"A");
            append_tar_hardlink(&mut tar, "b", "a");
            append_tar_regular(&mut tar, "b", b"B");
        }
        tar.resize(tar.len() + 1024, 0);
        tar
    }

    fn assert_a_is_a_and_b_is_b_distinct_inodes(dest: &Path) {
        let a_path = dest.join("a");
        let b_path = dest.join("b");
        assert_eq!(fs::read(&a_path).unwrap(), b"A", "a must remain 'A'");
        assert_eq!(fs::read(&b_path).unwrap(), b"B", "b must be overwritten with 'B'");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let ino_a = fs::metadata(&a_path).unwrap().ino();
            let ino_b = fs::metadata(&b_path).unwrap().ino();
            assert_ne!(ino_a, ino_b, "b replacement must break shared hardlink inode");
        }
    }

    #[test]
    fn test_tar_hardlink_then_overwrite_does_not_write_through_or_reapply_stale_link() {
        let dir = tempdir().unwrap();
        let opt = TTZipExtractOptions::default();

        for (idx, forward) in [false, true].into_iter().enumerate() {
            let raw_tar = build_tar_a_hardlink_b_then_overwrite_b(forward);
            let tar_path = dir.path().join(format!("seq_{idx}.tar"));
            fs::write(&tar_path, &raw_tar).unwrap();

            // 1. Full extraction via UnifiedArchiveOrchestrator on .tar (TarArchive fast path)
            let out_tar = dir.path().join(format!("out_tar_{idx}"));
            UnifiedArchiveOrchestrator::extract_archive(
                &tar_path,
                &StagingRoot(out_tar.clone()),
                &opt,
            )
            .expect("full extract .tar");
            assert_a_is_a_and_b_is_b_distinct_inodes(&out_tar);

            // 2. Direct TarArchive::extract_all
            let out_direct = dir.path().join(format!("out_direct_{idx}"));
            let parsed = TarArchive::open_slice(&raw_tar).expect("parse tar");
            parsed.extract_all(&out_direct, &opt).expect("direct TarArchive::extract_all");
            assert_a_is_a_and_b_is_b_distinct_inodes(&out_direct);

            // 3. Full extraction via UnifiedArchiveOrchestrator on .tar.gz (libarchive handle path)
            let tgz_path = dir.path().join(format!("seq_{idx}.tar.gz"));
            let mut gz = GzEncoder::new(Vec::new(), Compression::default());
            gz.write_all(&raw_tar).unwrap();
            fs::write(&tgz_path, gz.finish().unwrap()).unwrap();

            let out_tgz = dir.path().join(format!("out_tgz_{idx}"));
            UnifiedArchiveOrchestrator::extract_archive(
                &tgz_path,
                &StagingRoot(out_tgz.clone()),
                &opt,
            )
            .expect("full extract .tar.gz via libarchive");
            assert_a_is_a_and_b_is_b_distinct_inodes(&out_tgz);

            // 4. Selective extraction covering ["a", "b"]
            let out_sel = dir.path().join(format!("out_sel_{idx}"));
            let count = extract_selected_entries(
                &tar_path,
                &EntrySelection::new(["a", "b"], false),
                &StagingRoot(out_sel.clone()),
                &opt,
            )
            .expect("selective extract a and b");
            assert!(count >= 2);
            assert_a_is_a_and_b_is_b_distinct_inodes(&out_sel);
        }
    }

    fn assert_no_inflight_temps(dir_path: &Path) {
        if !dir_path.exists() {
            return;
        }
        let names: Vec<String> = fs::read_dir(dir_path)
            .unwrap()
            .filter_map(|e| e.ok().and_then(|d| d.file_name().into_string().ok()))
            .collect();
        assert!(
            !names.iter().any(|n| n.starts_with(".ttzip_inflight_")),
            "uncommitted temp file leaked in {:?}: {:?}",
            dir_path,
            names
        );
    }

    fn create_single_entry_archive(
        dir: &Path,
        file_name: &str,
        entry_rel: &str,
        payload: &[u8],
        format: TTZipArchiveFormat,
    ) -> PathBuf {
        let src = dir.join("src_payload.bin");
        fs::write(&src, payload).unwrap();
        let archive_path = dir.join(file_name);
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_3,
            format,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 1,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };
        create_archive_explicit(
            &[(src, entry_rel.to_string())],
            &archive_path,
            &create_opt,
            0,
        )
        .expect("create single-entry test archive");
        archive_path
    }

    unsafe extern "C" fn cancel_on_progress_cb(
        proc: u64,
        _tot: u64,
        _entry: *const libc::c_char,
        _user_data: *mut c_void,
    ) -> bool {
        proc == 0
    }

    #[test]
    fn test_full_extract_cancellation_cleans_partial_and_preserves_formal_across_formats() {
        let dir = tempdir().unwrap();
        let payload = vec![0x5Au8; 256 * 1024];

        let archives = [
            create_single_entry_archive(dir.path(), "c.tar", "big_file.bin", &payload, TTZipArchiveFormat::Tar),
            create_single_entry_archive(dir.path(), "c.tar.gz", "big_file.bin", &payload, TTZipArchiveFormat::TarGz),
            create_single_entry_archive(dir.path(), "c.zip", "big_file.bin", &payload, TTZipArchiveFormat::Zip),
            create_single_entry_archive(dir.path(), "c.7z", "big_file.bin", &payload, TTZipArchiveFormat::SevenZip),
        ];

        let cancel_opt = TTZipExtractOptions {
            collision_policy: TTZIP_COLLISION_POLICY_REPLACE,
            progress_callback: Some(cancel_on_progress_cb),
            ..Default::default()
        };

        for (i, archive_path) in archives.iter().enumerate() {
            // Case A: empty destination -> no partial file or temp file left behind
            let empty_dest = dir.path().join(format!("cancel_empty_{i}"));
            let res_empty = UnifiedArchiveOrchestrator::extract_archive(
                archive_path,
                &StagingRoot(empty_dest.clone()),
                &cancel_opt,
            );
            assert_eq!(res_empty, Err(TTZipStatus::Cancelled));
            assert!(!empty_dest.join("big_file.bin").exists());
            assert_no_inflight_temps(&empty_dest);

            // Case B: pre-existing formal file -> preserved intact, temp file cleaned up
            let formal_dest = dir.path().join(format!("cancel_formal_{i}"));
            fs::create_dir_all(&formal_dest).unwrap();
            fs::write(formal_dest.join("big_file.bin"), b"PREEXISTING_FORMAL_FILE").unwrap();
            let res_formal = UnifiedArchiveOrchestrator::extract_archive(
                archive_path,
                &StagingRoot(formal_dest.clone()),
                &cancel_opt,
            );
            assert_eq!(res_formal, Err(TTZipStatus::Cancelled));
            assert_eq!(
                fs::read(formal_dest.join("big_file.bin")).unwrap(),
                b"PREEXISTING_FORMAL_FILE"
            );
            assert_no_inflight_temps(&formal_dest);
        }
    }

    #[test]
    fn test_full_extract_read_and_crc_error_cleans_partial_and_preserves_formal() {
        let dir = tempdir().unwrap();
        let payload: Vec<u8> = (0..256 * 1024).map(|i| ((i * 31 + 7) & 0xFF) as u8).collect();

        let tgz = create_single_entry_archive(
            dir.path(),
            "err.tar.gz",
            "big_file.bin",
            &payload,
            TTZipArchiveFormat::TarGz,
        );
        let mut tgz_bytes = fs::read(&tgz).unwrap();
        tgz_bytes.truncate(tgz_bytes.len() / 2);
        let corrupt_tgz = dir.path().join("corrupt.tar.gz");
        fs::write(&corrupt_tgz, &tgz_bytes).unwrap();

        let zip = create_single_entry_archive(
            dir.path(),
            "err.zip",
            "big_file.bin",
            &payload,
            TTZipArchiveFormat::Zip,
        );
        let mut zip_bytes = fs::read(&zip).unwrap();
        let mid = zip_bytes.len() / 2;
        zip_bytes[mid] ^= 0xFF;
        zip_bytes[mid + 1] ^= 0xFF;
        let corrupt_zip = dir.path().join("corrupt.zip");
        fs::write(&corrupt_zip, &zip_bytes).unwrap();

        let sevenz = create_single_entry_archive(
            dir.path(),
            "err.7z",
            "big_file.bin",
            &payload,
            TTZipArchiveFormat::SevenZip,
        );
        let mut sz_bytes = fs::read(&sevenz).unwrap();
        sz_bytes[40] ^= 0xFF;
        let corrupt_7z = dir.path().join("corrupt.7z");
        fs::write(&corrupt_7z, &sz_bytes).unwrap();

        let opt = TTZipExtractOptions {
            collision_policy: TTZIP_COLLISION_POLICY_REPLACE,
            ..Default::default()
        };

        for (i, corrupt_archive) in [&corrupt_tgz, &corrupt_zip, &corrupt_7z].into_iter().enumerate() {
            let empty_dest = dir.path().join(format!("corrupt_empty_{i}"));
            let res_empty = UnifiedArchiveOrchestrator::extract_archive(
                corrupt_archive,
                &StagingRoot(empty_dest.clone()),
                &opt,
            );
            assert!(res_empty.is_err());
            assert!(!empty_dest.join("big_file.bin").exists());
            assert_no_inflight_temps(&empty_dest);

            let formal_dest = dir.path().join(format!("corrupt_formal_{i}"));
            fs::create_dir_all(&formal_dest).unwrap();
            fs::write(formal_dest.join("big_file.bin"), b"FORMAL_BEFORE_CORRUPT").unwrap();
            let res_formal = UnifiedArchiveOrchestrator::extract_archive(
                corrupt_archive,
                &StagingRoot(formal_dest.clone()),
                &opt,
            );
            assert!(res_formal.is_err());
            assert_eq!(
                fs::read(formal_dest.join("big_file.bin")).unwrap(),
                b"FORMAL_BEFORE_CORRUPT"
            );
            assert_no_inflight_temps(&formal_dest);
        }
    }

    struct SwapParentCtx {
        root: PathBuf,
        outside: PathBuf,
        cancel: bool,
        swapped: bool,
    }

    unsafe extern "C" fn swap_parent_on_progress_cb(
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
    fn test_full_extract_temp_collision_and_parent_fd_swap_defense_across_formats() {
        let dir = tempdir().unwrap();
        let payload = vec![0x7Cu8; 256 * 1024];

        let archives = [
            create_single_entry_archive(dir.path(), "p.tar", "sub/big_file.bin", &payload, TTZipArchiveFormat::Tar),
            create_single_entry_archive(dir.path(), "p.zip", "sub/big_file.bin", &payload, TTZipArchiveFormat::Zip),
            create_single_entry_archive(dir.path(), "p.7z", "sub/big_file.bin", &payload, TTZipArchiveFormat::SevenZip),
        ];

        let outside = dir.path().join("outside_target");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("big_file.bin"), b"OUTSIDE_FORMAL").unwrap();
        fs::write(outside.join(".ttzip_inflight_big_file.bin_0"), b"OUTSIDE_TEMP").unwrap();

        for (i, archive_path) in archives.iter().enumerate() {
            // 1. Exclusive temp collision retry without truncating existing .ttzip_inflight_*_0
            let coll_root = dir.path().join(format!("coll_root_{i}"));
            let coll_sub = coll_root.join("sub");
            fs::create_dir_all(&coll_sub).unwrap();
            fs::write(coll_sub.join("big_file.bin"), b"OLD_FORMAL").unwrap();
            let colliding_tmp = coll_sub.join(".ttzip_inflight_big_file.bin_0");
            fs::write(&colliding_tmp, b"DO_NOT_TRUNCATE_COLLIDING_TEMP").unwrap();

            let ok_opt = TTZipExtractOptions {
                collision_policy: TTZIP_COLLISION_POLICY_REPLACE,
                ..Default::default()
            };
            UnifiedArchiveOrchestrator::extract_archive(
                archive_path,
                &StagingRoot(coll_root.clone()),
                &ok_opt,
            )
            .expect("full extract succeeds via retried exclusive temp");
            assert_eq!(fs::read(&colliding_tmp).unwrap(), b"DO_NOT_TRUNCATE_COLLIDING_TEMP");
            assert_eq!(fs::read(coll_sub.join("big_file.bin")).unwrap(), payload);

            // 2. Parent directory replaced with symlink before commit / drop
            for (cancel_mode, expected_err) in [
                (true, TTZipStatus::Cancelled),
                (false, TTZipStatus::ErrSecurityViolation),
            ] {
                let root = dir.path().join(format!("swap_{i}_{cancel_mode}"));
                fs::create_dir_all(root.join("sub")).unwrap();
                fs::write(root.join("sub/big_file.bin"), b"FORMAL_IN_SUB").unwrap();

                let mut ctx = SwapParentCtx {
                    root: root.clone(),
                    outside: outside.clone(),
                    cancel: cancel_mode,
                    swapped: false,
                };
                let opt = TTZipExtractOptions {
                    collision_policy: TTZIP_COLLISION_POLICY_REPLACE,
                    progress_callback: Some(swap_parent_on_progress_cb),
                    user_data: (&mut ctx as *mut SwapParentCtx).cast(),
                    ..Default::default()
                };
                let res = UnifiedArchiveOrchestrator::extract_archive(
                    archive_path,
                    &StagingRoot(root.clone()),
                    &opt,
                );
                assert_eq!(res, Err(expected_err));
                assert_eq!(fs::read(outside.join("big_file.bin")).unwrap(), b"OUTSIDE_FORMAL");
                assert_eq!(
                    fs::read(outside.join(".ttzip_inflight_big_file.bin_0")).unwrap(),
                    b"OUTSIDE_TEMP"
                );
                let detached = root.join("sub_detached");
                assert_eq!(fs::read(detached.join("big_file.bin")).unwrap(), b"FORMAL_IN_SUB");
                assert_no_inflight_temps(&detached);
            }
        }
    }

    fn prepare_split_volume_entry(
        dir: &Path,
        src: &Path,
        name: &str,
        fmt: TTZipArchiveFormat,
    ) -> PathBuf {
        let base = dir.join(name);
        let create_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_3,
            format: fmt,
            level: TTZipCompressionLevel::Store,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 1,
            solid_block_size_mb: 0,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };
        create_archive_explicit(
            &[(src.to_path_buf(), "sub/big_file.bin".to_string())],
            &base,
            &create_opt,
            64 * 1024,
        )
        .expect("create split archive");
        let chain = crate::archive::split::detect_volume_chain(&base)
            .expect("detect split volume chain from base path");
        assert!(chain.len() >= 2, "expected multi-volume chain for {name}, got {chain:?}");
        let entry_vol = chain[0].clone();
        assert!(entry_vol.exists(), "split entry volume must exist on disk: {entry_vol:?}");
        UnifiedArchiveOrchestrator::detect_format(&entry_vol)
            .expect("split entry volume must be recognized by detect_format");
        let inspected = crate::archive::unified::inspect::inspect_archive(
            &entry_vol,
            None,
            false,
            |_| true,
        )
        .expect("split entry volume must be inspectable");
        assert_eq!(inspected, 1);
        entry_vol
    }

    #[test]
    fn test_split_volume_full_extract_cancel_and_parent_swap_cleanup() {
        let dir = tempdir().unwrap();
        let payload: Vec<u8> = (0..180 * 1024).map(|i| ((i * 73 + 19) & 0xFF) as u8).collect();
        let src = dir.path().join("split_src.bin");
        fs::write(&src, &payload).unwrap();

        let split_archives = [
            prepare_split_volume_entry(dir.path(), &src, "split.zip", TTZipArchiveFormat::Zip),
            prepare_split_volume_entry(dir.path(), &src, "split.7z", TTZipArchiveFormat::SevenZip),
        ];

        let outside = dir.path().join("outside_split");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("big_file.bin"), b"OUTSIDE_SPLIT_FORMAL").unwrap();

        for (i, vol1) in split_archives.iter().enumerate() {
            for (cancel_mode, expected_err) in [
                (true, TTZipStatus::Cancelled),
                (false, TTZipStatus::ErrSecurityViolation),
            ] {
                let root = dir.path().join(format!("split_swap_{i}_{cancel_mode}"));
                fs::create_dir_all(root.join("sub")).unwrap();
                fs::write(root.join("sub/big_file.bin"), b"SPLIT_FORMAL_IN_SUB").unwrap();

                let mut ctx = SwapParentCtx {
                    root: root.clone(),
                    outside: outside.clone(),
                    cancel: cancel_mode,
                    swapped: false,
                };
                let opt = TTZipExtractOptions {
                    collision_policy: TTZIP_COLLISION_POLICY_REPLACE,
                    progress_callback: Some(swap_parent_on_progress_cb),
                    user_data: (&mut ctx as *mut SwapParentCtx).cast(),
                    ..Default::default()
                };
                let res = UnifiedArchiveOrchestrator::extract_archive(
                    vol1,
                    &StagingRoot(root.clone()),
                    &opt,
                );
                assert_eq!(res, Err(expected_err));
                assert_eq!(
                    fs::read(outside.join("big_file.bin")).unwrap(),
                    b"OUTSIDE_SPLIT_FORMAL"
                );
                let detached = root.join("sub_detached");
                assert_eq!(
                    fs::read(detached.join("big_file.bin")).unwrap(),
                    b"SPLIT_FORMAL_IN_SUB"
                );
                assert_no_inflight_temps(&detached);
            }
        }
    }
}
