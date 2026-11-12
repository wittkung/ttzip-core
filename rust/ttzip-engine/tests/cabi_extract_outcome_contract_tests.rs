// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Spec 229 FR-023 & Issue #397: every extraction C-ABI entry point maps Stop to `Cancelled`, a
//! partial commit to `ErrExtractionFailed`, only a complete commit to `Ok`, reports actual commit
//! counts, and keeps `sdk/include/ttzip.h`, `bindings/cpp/ttzip_engine.hpp`, and
//! `bindings/csharp/TTZipEngine.cs` layout- and discriminant-identical to Rust.

extern crate ttzip_engine;

#[path = "common/cabi_abi_probe_support.rs"]
mod cabi_abi_probe_support;

use std::ffi::{CStr, CString};
use std::fs;
use std::mem::size_of;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use cabi_abi_probe_support::{
    assert_cross_language_enum_parity_for_manifest, build_c_abi_probe_source,
    expected_rust_abi_probe_lines, resolve_monorepo_bindings_dir, sdk_root_dir,
};
use tempfile::TempDir;
use ttzip_engine::platform::TTZipCpuCapsRaw;
use ttzip_engine::types::{
    TTZipEncryptionMethod, TTZipEntryMetadata, TTZipErrorInfo, TTZipExecutionProvenance,
    TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3, TTZIP_COLLISION_POLICY_KEEP_BOTH,
    TTZIP_COLLISION_POLICY_STOP,
};

unsafe extern "C" {
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
    fn ttzip_rust_cpu_get_capabilities(out_caps: *mut TTZipCpuCapsRaw) -> TTZipStatus;
    fn ttzip_rust_get_last_execution_provenance(out_prov: *mut TTZipExecutionProvenance) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Full,
    UnifiedV2,
    Selective,
}

const ENTRIES: [Entry; 3] = [Entry::Full, Entry::UnifiedV2, Entry::Selective];

type CallResult = (TTZipStatus, Option<u64>);

fn build_tar(path: &Path, entries: &[(&str, &[u8])]) {
    let mut builder = tar::Builder::new(fs::File::create(path).expect("create tar"));
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(1_700_000_000);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append_data(&mut header, name, *data).expect("append tar entry");
    }
    builder.finish().expect("finish tar");
}

fn call(entry: Entry, archive: &Path, dest: &Path, policy: u8, targets: &[&str]) -> CallResult {
    let c_archive = CString::new(archive.to_str().unwrap()).unwrap();
    let c_dest = CString::new(dest.to_str().unwrap()).unwrap();
    let options = TTZipExtractOptions { collision_policy: policy, ..TTZipExtractOptions::default() };
    // SAFETY: all pointers reference live, NUL-terminated buffers owned by this frame.
    unsafe {
        match entry {
            Entry::Full => (ttzip_rust_extract_archive(c_archive.as_ptr(), c_dest.as_ptr(), &options), None),
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
                let c_targets: Vec<CString> = targets.iter().map(|t| CString::new(*t).unwrap()).collect();
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

fn assert_no_staging_left(dest: &Path) {
    let leftovers: Vec<_> = fs::read_dir(dest)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(".ttzip_staging_"))
        .collect();
    assert!(leftovers.is_empty(), "staging left behind: {leftovers:?}");
}

fn names(dest: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        fs::read_dir(dest).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn c_buf_str(buf: &[libc::c_char]) -> &str {
    // SAFETY: `TTZipErrorInfo` buffers are zero-initialized and NUL-terminated by `write_c_str_buf`.
    unsafe { CStr::from_ptr(buf.as_ptr()).to_str().expect("valid UTF-8 C buffer") }
}

#[test]
fn stop_with_conflict_returns_cancelled_and_leaves_destination_unchanged() {
    for entry in ENTRIES {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("in.tar");
        build_tar(&archive, &[("a.txt", b"new"), ("b.txt", b"new b")]);
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), "old").unwrap();

        let (status, output) = call(entry, &archive, &dest, TTZIP_COLLISION_POLICY_STOP, &["a.txt", "b.txt"]);

        assert_eq!(status, TTZipStatus::Cancelled, "{entry:?}");
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "old", "{entry:?}");
        assert_eq!(names(&dest), vec!["a.txt".to_string()], "{entry:?}: destination changed");
        assert_no_staging_left(&dest);
        if entry != Entry::Full {
            assert_eq!(output, Some(0), "{entry:?}: output must be 0 on Stop");
        }
    }
}

#[test]
fn partial_commit_failure_returns_extraction_failed_with_actual_count() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    for entry in ENTRIES {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("in.tar");
        build_tar(&archive, &[("ok.txt", b"ok"), ("sub/new.txt", b"n")]);
        let dest = tmp.path().join("dest");
        fs::create_dir_all(dest.join("sub")).unwrap();
        fs::set_permissions(dest.join("sub"), fs::Permissions::from_mode(0o555)).unwrap();

        let result = call(entry, &archive, &dest, TTZIP_COLLISION_POLICY_KEEP_BOTH, &["ok.txt", "sub/new.txt"]);
        fs::set_permissions(dest.join("sub"), fs::Permissions::from_mode(0o755)).unwrap();
        let (status, output) = result;

        assert_eq!(status, TTZipStatus::ErrExtractionFailed, "{entry:?}");
        assert_eq!(fs::read_to_string(dest.join("ok.txt")).unwrap(), "ok", "{entry:?}");
        assert!(!dest.join("sub/new.txt").exists(), "{entry:?}");
        match entry {
            Entry::Selective => assert_eq!(output, Some(1), "count must reflect the committed item"),
            Entry::UnifiedV2 => assert_eq!(output, Some(0), "bytes must be 0 unless Ok"),
            Entry::Full => {}
        }
    }
}

#[test]
fn keep_both_success_returns_ok_with_committed_count() {
    for entry in ENTRIES {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("in.tar");
        build_tar(&archive, &[("a.txt", b"new"), ("b.txt", b"new b")]);
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), "old").unwrap();

        let (status, output) = call(entry, &archive, &dest, TTZIP_COLLISION_POLICY_KEEP_BOTH, &["a.txt", "b.txt"]);

        assert_eq!(status, TTZipStatus::Ok, "{entry:?}");
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "old", "{entry:?}");
        assert_eq!(fs::read_to_string(dest.join("a 2.txt")).unwrap(), "new", "{entry:?}");
        assert_eq!(fs::read_to_string(dest.join("b.txt")).unwrap(), "new b", "{entry:?}");
        assert_no_staging_left(&dest);
        match entry {
            Entry::Selective => assert_eq!(output, Some(2), "kept_both + extracted"),
            Entry::UnifiedV2 => assert_eq!(output, Some(8), "decoded bytes reported on Ok"),
            Entry::Full => {}
        }
    }
}

#[test]
fn stop_without_conflict_commits_everything() {
    for entry in ENTRIES {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("in.tar");
        build_tar(&archive, &[("a.txt", b"new"), ("b.txt", b"new b")]);
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();

        let (status, output) = call(entry, &archive, &dest, TTZIP_COLLISION_POLICY_STOP, &["a.txt", "b.txt"]);

        assert_eq!(status, TTZipStatus::Ok, "{entry:?}");
        assert_eq!(names(&dest), vec!["a.txt".to_string(), "b.txt".to_string()], "{entry:?}");
        if entry == Entry::Selective {
            assert_eq!(output, Some(2));
        }
    }
}

#[test]
fn c_sdk_header_abi_v3_layout_and_init_match_rust() {
    let sdk = sdk_root_dir();
    let include_dir = sdk.join("include");
    let tmp = TempDir::new().unwrap();
    let probe_c = tmp.path().join("abi_probe.c");
    let probe_bin = tmp.path().join("abi_probe");
    fs::write(&probe_c, build_c_abi_probe_source()).unwrap();

    let cc_out = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(&include_dir)
        .arg(&probe_c)
        .arg("-o")
        .arg(&probe_bin)
        .output()
        .expect("invoke cc for ABI probe");
    assert!(cc_out.status.success(), "cc failed: {}", String::from_utf8_lossy(&cc_out.stderr));

    let test_sdk_obj = tmp.path().join("test_c_sdk.o");
    let sdk_compile = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-c", "-I"])
        .arg(&include_dir)
        .arg(sdk.join("c/test_c_sdk.c"))
        .arg("-o")
        .arg(&test_sdk_obj)
        .output()
        .expect("compile test_c_sdk.c");
    assert!(sdk_compile.status.success(), "test_c_sdk.c compile failed: {}", String::from_utf8_lossy(&sdk_compile.stderr));

    let run_out = Command::new(&probe_bin).output().expect("run ABI probe");
    assert!(run_out.status.success());
    let stdout = String::from_utf8(run_out.stdout).unwrap();
    let actual_lines: Vec<String> = stdout.lines().map(str::to_owned).collect();
    assert_eq!(actual_lines, expected_rust_abi_probe_lines());

    let mut caps = TTZipCpuCapsRaw::default();
    let mut prov = TTZipExecutionProvenance::default();
    // SAFETY: pointers reference valid stack-allocated repr(C) structs.
    unsafe {
        assert_eq!(ttzip_rust_cpu_get_capabilities(&mut caps), TTZipStatus::Ok);
        assert!(ttzip_rust_get_last_execution_provenance(&mut prov));
    }
    assert_eq!(caps.struct_size, size_of::<TTZipCpuCapsRaw>() as u32);
    assert_eq!(caps.abi_version, TTZIP_ABI_VERSION_3);
    assert_eq!(prov.struct_size, size_of::<TTZipExecutionProvenance>() as u32);
    assert_eq!(prov.abi_version, TTZIP_ABI_VERSION_3);
    assert_eq!(TTZipEntryMetadata::default().struct_size, size_of::<TTZipEntryMetadata>() as u32);
    assert_eq!(TTZipEntryMetadata::default().abi_version, TTZIP_ABI_VERSION_3);
    assert_eq!(TTZipEncryptionMethod::None as i32, 0);
}

#[test]
fn cross_language_enum_discriminants_match_rust_c_cpp_and_csharp() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let _checked_host_bindings = assert_cross_language_enum_parity_for_manifest(manifest_dir);

    // Also verify standalone `ttzip-core` export layout where `../../sdk/include/ttzip.h`
    // exists relative to `CARGO_MANIFEST_DIR`, while monorepo `bindings/` siblings are absent.
    let tmp = TempDir::new().unwrap();
    let standalone_manifest = tmp.path().join("rust/ttzip-engine");
    let standalone_include = tmp.path().join("sdk/include");
    fs::create_dir_all(&standalone_manifest).unwrap();
    fs::create_dir_all(&standalone_include).unwrap();
    let src_hdr = sdk_root_dir().join("include/ttzip.h");
    fs::write(
        standalone_include.join("ttzip.h"),
        fs::read(&src_hdr).expect("read source ttzip.h"),
    )
    .unwrap();
    assert!(resolve_monorepo_bindings_dir(&standalone_manifest).is_none());
    assert!(!assert_cross_language_enum_parity_for_manifest(&standalone_manifest));
}

#[test]
fn unified_v2_and_thread_local_error_info_report_real_abi_and_commit_diagnostics() {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("in.tar");
    build_tar(&archive, &[("ok.txt", b"ok"), ("sub/new.txt", b"n")]);
    let dest = tmp.path().join("dest");
    fs::create_dir(&dest).unwrap();

    let c_archive = CString::new(archive.to_str().unwrap()).unwrap();
    let c_dest = CString::new(dest.to_str().unwrap()).unwrap();

    let bad_opt = TTZipExtractOptions { abi_version: 2, ..TTZipExtractOptions::default() };
    let mut bytes = 99u64;
    let mut err = TTZipErrorInfo::empty();
    // SAFETY: valid pointers to stack-allocated structs and NUL-terminated CStrings.
    let st = unsafe {
        ttzip_rust_archive_extract_unified_v2(
            c_archive.as_ptr(),
            c_dest.as_ptr(),
            &bad_opt,
            &mut bytes,
            &mut err,
        )
    };
    assert_eq!(st, TTZipStatus::ErrAbiMismatch);
    assert_eq!(bytes, 0);
    assert_eq!(err.struct_size, size_of::<TTZipErrorInfo>() as u32);
    assert_eq!(err.abi_version, TTZIP_ABI_VERSION_3);
    assert_eq!(err.status, TTZipStatus::ErrAbiMismatch);
    assert!(c_buf_str(&err.message).contains("ABI"), "unexpected message: {}", c_buf_str(&err.message));

    let mut tls_err = TTZipErrorInfo::empty();
    // SAFETY: valid pointer to stack-allocated `TTZipErrorInfo`.
    assert!(unsafe { ttzip_rust_get_last_error_info(&mut tls_err) });
    assert_eq!(tls_err.struct_size, size_of::<TTZipErrorInfo>() as u32);
    assert_eq!(tls_err.abi_version, TTZIP_ABI_VERSION_3);
    assert_eq!(tls_err.status, TTZipStatus::ErrAbiMismatch);
    assert_eq!(c_buf_str(&tls_err.message), c_buf_str(&err.message));

    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        fs::create_dir_all(dest.join("sub")).unwrap();
        fs::set_permissions(dest.join("sub"), fs::Permissions::from_mode(0o555)).unwrap();
        let opt = TTZipExtractOptions {
            collision_policy: TTZIP_COLLISION_POLICY_KEEP_BOTH,
            ..TTZipExtractOptions::default()
        };
        let mut commit_err = TTZipErrorInfo::empty();
        // SAFETY: valid pointers to stack-allocated structs and NUL-terminated CStrings.
        let st_commit = unsafe {
            ttzip_rust_archive_extract_unified_v2(
                c_archive.as_ptr(),
                c_dest.as_ptr(),
                &opt,
                &mut bytes,
                &mut commit_err,
            )
        };
        fs::set_permissions(dest.join("sub"), fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(st_commit, TTZipStatus::ErrExtractionFailed);
        assert_eq!(commit_err.struct_size, size_of::<TTZipErrorInfo>() as u32);
        assert_eq!(commit_err.abi_version, TTZIP_ABI_VERSION_3);
        assert_eq!(commit_err.status, TTZipStatus::ErrExtractionFailed);
        assert_eq!(c_buf_str(&commit_err.entry_path), "sub/new.txt");
        assert!(!c_buf_str(&commit_err.message).is_empty());

        let mut tls_commit_err = TTZipErrorInfo::empty();
        // SAFETY: valid pointer to stack-allocated `TTZipErrorInfo`.
        assert!(unsafe { ttzip_rust_get_last_error_info(&mut tls_commit_err) });
        assert_eq!(tls_commit_err.status, TTZipStatus::ErrExtractionFailed);
        assert_eq!(c_buf_str(&tls_commit_err.entry_path), "sub/new.txt");
    }
}

#[test]
fn three_phase_metadata_keep_both_skip_and_partial_failure_preserve_permissions_and_mtime() {
    use std::os::unix::fs::MetadataExt;
    use ttzip_engine::uniffi_api::extraction_session::{
        CollisionAction, CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession,
        FailureReason, OutcomeStatus, ResolutionOverride, ResolutionSet,
    };

    if unsafe { libc::geteuid() } == 0 {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("three_phase.tar");
    let mut tar_bytes = Vec::new();
    let mut writer = ttzip_engine::archive::tar::writer::TarWriter::new(&mut tar_bytes);
    writer.append_dir("ro_dir/", 0o555, 1_700_000_000).unwrap();
    writer.append_file("ro_dir/keep.txt", b"incoming keep", 0o640, 1_700_000_100).unwrap();
    writer.append_file("ro_dir/skip.txt", b"incoming skip", 0o750, 1_700_000_200).unwrap();
    writer.append_file("locked_sub/fail.txt", b"incoming fail", 0o644, 1_700_000_300).unwrap();
    writer.finish().unwrap();
    fs::write(&tar_path, &tar_bytes).unwrap();

    let dest = tmp.path().join("dest");
    let pre_ro = dest.join("ro_dir");
    let locked_sub = dest.join("locked_sub");
    fs::create_dir_all(&pre_ro).unwrap();
    fs::set_permissions(&pre_ro, fs::Permissions::from_mode(0o700)).unwrap();
    for (name, data) in [("keep.txt", b"orig keep".as_slice()), ("skip.txt", b"orig skip".as_slice())] {
        fs::write(pre_ro.join(name), data).unwrap();
        fs::set_permissions(pre_ro.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    fs::create_dir_all(&locked_sub).unwrap();
    fs::set_permissions(&locked_sub, fs::Permissions::from_mode(0o555)).unwrap();

    let session = ExtractionSession::new(ExtractionRequest {
        archive_path: tar_path.to_str().unwrap().to_string(),
        destination_root: dest.to_str().unwrap().to_string(),
        password: None,
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::Ask,
    })
    .unwrap();
    let stage = session.stage(None, None).unwrap();
    assert_eq!(stage.post_stage_conflicts.unwrap().total, 2);
    let page = session.conflict_page(0, 10).unwrap();
    let mut overrides: Vec<ResolutionOverride> = page
        .records
        .iter()
        .map(|r| ResolutionOverride {
            conflict_id: r.id,
            action: if r.relative_path == "ro_dir/keep.txt" { CollisionAction::KeepBoth } else { CollisionAction::Skip },
        })
        .collect();
    overrides.sort_by_key(|o| o.conflict_id);

    let CommitResult::Finished { outcome } = session.commit(ResolutionSet { default_action: None, overrides }).unwrap() else {
        panic!("expected Finished");
    };
    fs::set_permissions(&locked_sub, fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!((outcome.kept_both, outcome.skipped, outcome.failed.len()), (1, 1, 1));
    assert_eq!((outcome.failed[0].relative_path.as_str(), outcome.failed[0].reason), ("locked_sub/fail.txt", FailureReason::PermissionDenied));
    assert_eq!(fs::metadata(&pre_ro).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::read(pre_ro.join("keep.txt")).unwrap(), b"orig keep");
    assert_eq!(fs::metadata(pre_ro.join("keep.txt")).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::read(pre_ro.join("skip.txt")).unwrap(), b"orig skip");
    assert_eq!(fs::metadata(pre_ro.join("skip.txt")).unwrap().permissions().mode() & 0o777, 0o600);
    let keep2_meta = fs::metadata(pre_ro.join("keep 2.txt")).unwrap();
    assert_eq!(fs::read(pre_ro.join("keep 2.txt")).unwrap(), b"incoming keep");
    assert_eq!((keep2_meta.permissions().mode() & 0o777, keep2_meta.mtime()), (0o640, 1_700_000_100));
}

#[test]
fn intra_archive_duplicate_uses_safe_replacement_without_truncating_hardlinked_sibling() {
    use std::os::unix::fs::MetadataExt;
    use ttzip_engine::archive::tar::header::TYPE_HARDLINK;
    use ttzip_engine::archive::tar::writer::{TarCustomEntryParams, TarWriter};
    use ttzip_engine::uniffi_api::extraction_session::{
        CollisionAction, CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession,
        OutcomeStatus, ResolutionSet,
    };

    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("dup_hardlink.tar");
    let mut tar_bytes = Vec::new();
    let mut writer = TarWriter::new(&mut tar_bytes);
    writer
        .append_file("a.txt", b"original_shared_inode_payload", 0o600, 1_700_000_010)
        .unwrap();
    writer
        .append_custom_entry(TarCustomEntryParams {
            rel_path: "b.txt",
            data: &[],
            mode: 0o600,
            mtime: 1_700_000_010,
            typeflag: TYPE_HARDLINK,
            link_target: Some("a.txt"),
            pax_extra: None,
        })
        .unwrap();
    writer
        .append_file("b.txt", b"overwritten_second_entry", 0o640, 1_700_000_099)
        .unwrap();
    writer.finish().unwrap();
    fs::write(&tar_path, &tar_bytes).unwrap();

    for (sub_name, filter) in [
        ("full", None),
        ("selective", Some(vec!["a.txt".to_string(), "b.txt".to_string()])),
    ] {
        let dest = tmp.path().join(sub_name);
        fs::create_dir_all(&dest).unwrap();
        let session = ExtractionSession::new(ExtractionRequest {
            archive_path: tar_path.to_str().unwrap().to_string(),
            destination_root: dest.to_str().unwrap().to_string(),
            password: None,
            entry_filter: filter,
            root_at_common_parent: false,
            policy: CollisionPolicy::Skip,
        })
        .unwrap();
        session.stage(None, None).unwrap();
        let CommitResult::Finished { outcome } = session
            .commit(ResolutionSet {
                default_action: Some(CollisionAction::Skip),
                overrides: Vec::new(),
            })
            .unwrap()
        else {
            panic!("expected Finished");
        };
        assert_eq!(outcome.status, OutcomeStatus::Completed);

        let a_meta = fs::metadata(dest.join("a.txt")).unwrap();
        let b_meta = fs::metadata(dest.join("b.txt")).unwrap();
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"original_shared_inode_payload");
        assert_eq!(a_meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(a_meta.mtime(), 1_700_000_010);

        assert_eq!(fs::read(dest.join("b.txt")).unwrap(), b"overwritten_second_entry");
        assert_eq!(b_meta.permissions().mode() & 0o777, 0o640);
        assert_eq!(b_meta.mtime(), 1_700_000_099);
        assert_ne!(a_meta.ino(), b_meta.ino(), "safe replacement must break hardlink inode sharing");
    }
}
