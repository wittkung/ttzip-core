// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Phase-3 post-commit metadata failure contract & DirJail replacement defense tests
//! across Rust `ExtractionSession`, UniFFI `ExtractionSession`, and C-ABI entry points.

extern crate ttzip_engine;

use std::ffi::{CStr, CString};
use std::fs;
use std::mem::size_of;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use tempfile::TempDir;
use ttzip_engine::types::{
    TTZipErrorInfo, TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3,
    TTZIP_COLLISION_POLICY_KEEP_BOTH,
};

extern "C" {
    fn ttzip_rust_extract_archive(
        archive_path: *const libc::c_char,
        destination_path: *const libc::c_char,
        options: *const TTZipExtractOptions,
    ) -> TTZipStatus;

    fn ttzip_rust_archive_extract_unified_v2(
        archive_path: *const libc::c_char,
        destination_path: *const libc::c_char,
        options: *const TTZipExtractOptions,
        out_extracted_bytes: *mut u64,
        out_error: *mut TTZipErrorInfo,
    ) -> TTZipStatus;

    fn ttzip_rust_archive_extract_selected(
        archive_path: *const libc::c_char,
        target_entries: *const *const libc::c_char,
        target_count: usize,
        destination_path: *const libc::c_char,
        options: *const TTZipExtractOptions,
        out_extracted_count: *mut usize,
    ) -> TTZipStatus;

    fn ttzip_rust_get_last_error_info(out_info: *mut TTZipErrorInfo) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Full,
    UnifiedV2,
    Selective,
}

const ENTRIES: [Entry; 3] = [Entry::Full, Entry::UnifiedV2, Entry::Selective];

type CallResult = (TTZipStatus, Option<u64>);

fn call(entry: Entry, archive: &Path, dest: &Path, policy: u8, targets: &[&str]) -> CallResult {
    let c_archive = CString::new(archive.to_str().unwrap()).unwrap();
    let c_dest = CString::new(dest.to_str().unwrap()).unwrap();
    let options = TTZipExtractOptions {
        collision_policy: policy,
        ..TTZipExtractOptions::default()
    };
    unsafe {
        match entry {
            Entry::Full => (
                ttzip_rust_extract_archive(c_archive.as_ptr(), c_dest.as_ptr(), &options),
                None,
            ),
            Entry::UnifiedV2 => {
                let mut bytes = u64::MAX;
                let mut error = TTZipErrorInfo::empty();
                let status = ttzip_rust_archive_extract_unified_v2(
                    c_archive.as_ptr(),
                    c_dest.as_ptr(),
                    &options,
                    &mut bytes,
                    &mut error,
                );
                assert_eq!(error.status, status, "out_error must carry the returned status");
                assert_eq!(error.struct_size, size_of::<TTZipErrorInfo>() as u32);
                assert_eq!(error.abi_version, TTZIP_ABI_VERSION_3);
                (status, Some(bytes))
            }
            Entry::Selective => {
                let c_targets: Vec<CString> =
                    targets.iter().map(|t| CString::new(*t).unwrap()).collect();
                let ptrs: Vec<*const libc::c_char> = c_targets.iter().map(|t| t.as_ptr()).collect();
                let mut count = usize::MAX;
                let status = ttzip_rust_archive_extract_selected(
                    c_archive.as_ptr(),
                    ptrs.as_ptr(),
                    ptrs.len(),
                    c_dest.as_ptr(),
                    &options,
                    &mut count,
                );
                (status, Some(count as u64))
            }
        }
    }
}

fn c_buf_str(buf: &[libc::c_char]) -> &str {
    unsafe { CStr::from_ptr(buf.as_ptr()).to_str().expect("valid UTF-8 C buffer") }
}

fn build_meta_failure_tar(path: &Path) {
    use ttzip_engine::archive::tar::writer::TarWriter;
    let mut tar_bytes = Vec::new();
    let mut writer = TarWriter::new(&mut tar_bytes);
    writer.append_file("ok.txt", b"ok_payload", 0o640, 1_700_000_010).unwrap();
    writer
        .append_file("sub/meta_fail.txt", b"committed_payload", 0o640, 1_700_000_020)
        .unwrap();
    writer.finish().unwrap();
    fs::write(path, &tar_bytes).unwrap();
}

#[test]
fn metadata_failure_after_content_commit_reports_completed_with_failures_across_rust_session_uniffi_and_cabi() {
    use ttzip_engine::fs::staged_metadata::register_pre_metadata_hook_for_dest;

    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("meta_fail.tar");
    build_meta_failure_tar(&archive);

    // 1. Rust ExtractionSession
    let rust_dest = tmp.path().join("rust_dest");
    fs::create_dir_all(&rust_dest).unwrap();
    let staged = ttzip_engine::fs::collision::ExtractionSession::new(&rust_dest)
        .stage()
        .unwrap();
    ttzip_engine::archive::unified::UnifiedArchiveOrchestrator::extract_archive_with_metrics(
        &archive,
        &staged.staging().payload_dir(),
        &TTZipExtractOptions::default(),
    )
    .unwrap();
    register_pre_metadata_hook_for_dest(
        &rust_dest,
        Box::new(|d| {
            fs::set_permissions(d.join("sub/meta_fail.txt"), fs::Permissions::from_mode(0o000))
                .unwrap();
        }),
    );
    let (_committed, rust_out) = staged
        .commit(
            &ttzip_engine::fs::collision::fs_ops::LibcFsOps,
            &ttzip_engine::fs::collision::ResolutionSet::new(),
        )
        .unwrap();
    assert_eq!(
        rust_out.status,
        ttzip_engine::fs::collision::OutcomeStatus::CompletedWithFailures
    );
    assert!(rust_out
        .failed
        .iter()
        .any(|f| f.relative_path == "sub/meta_fail.txt" && f.errno == libc::EACCES));
    assert_eq!(fs::metadata(rust_dest.join("ok.txt")).unwrap().mtime(), 1_700_000_010);
    fs::set_permissions(rust_dest.join("sub/meta_fail.txt"), fs::Permissions::from_mode(0o600))
        .unwrap();
    assert_eq!(fs::read(rust_dest.join("sub/meta_fail.txt")).unwrap(), b"committed_payload");

    // 2. UniFFI ExtractionSession
    let uniffi_dest = tmp.path().join("uniffi_dest");
    fs::create_dir_all(&uniffi_dest).unwrap();
    let session = ttzip_engine::uniffi_api::extraction_session::ExtractionSession::new(
        ttzip_engine::uniffi_api::extraction_session::ExtractionRequest {
            archive_path: archive.to_str().unwrap().to_string(),
            destination_root: uniffi_dest.to_str().unwrap().to_string(),
            password: None,
            entry_filter: None,
            root_at_common_parent: false,
            policy: ttzip_engine::uniffi_api::extraction_session::CollisionPolicy::KeepBoth,
        },
    )
    .unwrap();
    session.stage(None, None).unwrap();
    register_pre_metadata_hook_for_dest(
        &uniffi_dest,
        Box::new(|d| {
            fs::set_permissions(d.join("sub/meta_fail.txt"), fs::Permissions::from_mode(0o000))
                .unwrap();
        }),
    );
    let ttzip_engine::uniffi_api::extraction_session::CommitResult::Finished { outcome } = session
        .commit(ttzip_engine::uniffi_api::extraction_session::ResolutionSet {
            default_action: Some(
                ttzip_engine::uniffi_api::extraction_session::CollisionAction::KeepBoth,
            ),
            overrides: Vec::new(),
        })
        .unwrap()
    else {
        panic!("expected Finished");
    };
    assert_eq!(
        outcome.status,
        ttzip_engine::uniffi_api::extraction_session::OutcomeStatus::CompletedWithFailures
    );
    assert!(outcome
        .failed
        .iter()
        .any(|f| f.relative_path == "sub/meta_fail.txt" && f.errno == libc::EACCES));
    fs::set_permissions(uniffi_dest.join("sub/meta_fail.txt"), fs::Permissions::from_mode(0o600))
        .unwrap();
    assert_eq!(fs::read(uniffi_dest.join("sub/meta_fail.txt")).unwrap(), b"committed_payload");

    // 3. C-ABI entry points (Full, UnifiedV2, Selective)
    for (idx, entry) in ENTRIES.iter().enumerate() {
        let cabi_dest = tmp.path().join(format!("cabi_{idx}"));
        fs::create_dir_all(&cabi_dest).unwrap();
        register_pre_metadata_hook_for_dest(
            &cabi_dest,
            Box::new(|d| {
                fs::set_permissions(d.join("sub/meta_fail.txt"), fs::Permissions::from_mode(0o000))
                    .unwrap();
            }),
        );
        let (status, out_val) = call(
            *entry,
            &archive,
            &cabi_dest,
            TTZIP_COLLISION_POLICY_KEEP_BOTH,
            &["ok.txt", "sub/meta_fail.txt"],
        );
        assert_eq!(status, TTZipStatus::ErrExtractionFailed, "{entry:?}");
        if *entry == Entry::Selective {
            assert_eq!(out_val, Some(2), "both files were committed before metadata failed");
        } else if *entry == Entry::UnifiedV2 {
            assert_eq!(out_val, Some(0));
            let mut last = TTZipErrorInfo::empty();
            assert!(unsafe { ttzip_rust_get_last_error_info(&mut last) });
            assert_eq!(c_buf_str(&last.entry_path), "sub/meta_fail.txt");
        }
        fs::set_permissions(cabi_dest.join("sub/meta_fail.txt"), fs::Permissions::from_mode(0o600))
            .unwrap();
        assert_eq!(fs::read(cabi_dest.join("sub/meta_fail.txt")).unwrap(), b"committed_payload");
    }
}

#[test]
fn phase3_dir_jail_metadata_rejects_parent_dir_and_leaf_replacement_preserving_external_target() {
    use ttzip_engine::fs::staged_metadata::register_pre_metadata_hook_for_dest;
    use ttzip_engine::uniffi_api::extraction_session::{
        CollisionAction, CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession,
        OutcomeStatus, ResolutionSet,
    };

    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("replace_defense.tar");
    build_meta_failure_tar(&archive);

    for mode_case in ["parent_symlink", "parent_imposter_dir", "leaf_symlink", "leaf_imposter_file"]
    {
        let outside_dir = tmp.path().join(format!("outside_{mode_case}"));
        fs::create_dir_all(&outside_dir).unwrap();
        fs::set_permissions(&outside_dir, fs::Permissions::from_mode(0o700)).unwrap();
        let outside_file = outside_dir.join("meta_fail.txt");
        fs::write(&outside_file, b"outside_untouched").unwrap();
        fs::set_permissions(&outside_file, fs::Permissions::from_mode(0o600)).unwrap();
        let orig_mtime = fs::metadata(&outside_file).unwrap().mtime();

        let dest = tmp.path().join(format!("dest_{mode_case}"));
        fs::create_dir_all(&dest).unwrap();
        let session = ExtractionSession::new(ExtractionRequest {
            archive_path: archive.to_str().unwrap().to_string(),
            destination_root: dest.to_str().unwrap().to_string(),
            password: None,
            entry_filter: None,
            root_at_common_parent: false,
            policy: CollisionPolicy::KeepBoth,
        })
        .unwrap();
        session.stage(None, None).unwrap();

        let out_dir_clone = outside_dir.clone();
        let out_file_clone = outside_file.clone();
        register_pre_metadata_hook_for_dest(
            &dest,
            Box::new(move |d| match mode_case {
                "parent_symlink" => {
                    fs::remove_dir_all(d.join("sub")).unwrap();
                    std::os::unix::fs::symlink(&out_dir_clone, d.join("sub")).unwrap();
                }
                "parent_imposter_dir" => {
                    fs::remove_dir_all(d.join("sub")).unwrap();
                    fs::create_dir_all(d.join("sub")).unwrap();
                    fs::hard_link(&out_file_clone, d.join("sub/meta_fail.txt")).unwrap();
                }
                "leaf_symlink" => {
                    fs::remove_file(d.join("sub/meta_fail.txt")).unwrap();
                    std::os::unix::fs::symlink(&out_file_clone, d.join("sub/meta_fail.txt"))
                        .unwrap();
                }
                _ => {
                    fs::remove_file(d.join("sub/meta_fail.txt")).unwrap();
                    fs::hard_link(&out_file_clone, d.join("sub/meta_fail.txt")).unwrap();
                }
            }),
        );

        let CommitResult::Finished { outcome } = session
            .commit(ResolutionSet {
                default_action: Some(CollisionAction::KeepBoth),
                overrides: Vec::new(),
            })
            .unwrap()
        else {
            panic!("expected Finished");
        };
        assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures, "{mode_case}");
        assert!(
            outcome.failed.iter().any(|f| f.relative_path == "sub/meta_fail.txt"),
            "{mode_case}"
        );

        let after_meta = fs::metadata(&outside_file).unwrap();
        assert_eq!(
            after_meta.permissions().mode() & 0o777,
            0o600,
            "{mode_case}: external mode changed"
        );
        assert_eq!(after_meta.mtime(), orig_mtime, "{mode_case}: external mtime changed");
        assert_eq!(
            fs::metadata(&outside_dir).unwrap().permissions().mode() & 0o777,
            0o700,
            "{mode_case}"
        );
    }
}
