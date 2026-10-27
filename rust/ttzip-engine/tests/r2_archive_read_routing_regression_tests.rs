// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R2 regression test suite for unified archive browse (`inspect_archive`),
//! single-file compressed streams, selected-entry extraction (`extract_selected_entries` /
//! `extract_single_entry_memory` / `ExtractionSession`), external container classification
//! (`DMG`, `AAR`, `XAR`, `WIM`), and bounded XAR decompression / cumulative output budget.

use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};

use ttzip_engine::archive::unified::detect::detect_format;
use ttzip_engine::archive::unified::extract::extract_archive_with_metrics;
use ttzip_engine::archive::unified::inspect::ArchiveEntryRef;
use ttzip_engine::archive::unified::selection::EntrySelection;
use ttzip_engine::archive::unified::{
    create_archive, extract_archive, extract_selected_entries, extract_single_entry_memory,
    inspect_archive,
};
use ttzip_engine::fs::collision::staging::StagingRoot;
use ttzip_engine::standards::signatures::DetectedFormat;
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3,
};
use ttzip_engine::uniffi_api::archive::{
    create_archive_advanced_with_options, UniFFICreateArchiveConfig,
    UniFFICreateArchiveExtraOptions,
};
use ttzip_engine::uniffi_api::extraction_session::{
    CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession, OutcomeStatus,
    ResolutionSet,
};
use ttzip_engine::uniffi_api::types::ArchiveFormat;

const CORPUS_MAIN_README: &str =
    "TTZip Release Acceptance Standard Sample File.\nLine 2: Deterministic Content.\n";
const EXT_SMALL_HELLO: &str = "Hello from external tool archive!\n";
const EXT_SMALL_NESTED: &str = "Nested file inside external archive.\n";

fn default_opts() -> TTZipExtractOptions {
    TTZipExtractOptions {
        preserve_permissions: true,
        ..Default::default()
    }
}

fn make_create_opts(format: TTZipArchiveFormat) -> TTZipCreateOptions {
    TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 2,
        solid_block_size_mb: 16,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    }
}

fn build_corpus_main_source(root: &Path) -> PathBuf {
    let corpus = root.join("corpus_main");
    fs::create_dir_all(corpus.join("docs")).unwrap();
    fs::create_dir_all(corpus.join("bin")).unwrap();
    fs::create_dir_all(corpus.join("L1/L2/L3/L4/L5/L6")).unwrap();
    fs::write(corpus.join("docs/readme.txt"), CORPUS_MAIN_README).unwrap();
    fs::write(corpus.join("bin/run_verify.sh"), "#!/bin/sh\necho verify\n").unwrap();
    fs::write(
        corpus.join("L1/L2/L3/L4/L5/L6/deep_leaf.txt"),
        "deep leaf deterministic content\n",
    )
    .unwrap();
    corpus
}

fn build_ext_small_files(root: &Path) -> (PathBuf, PathBuf) {
    let dir = root.join("ext_small_src");
    fs::create_dir_all(dir.join("sub")).unwrap();
    let f_hello = dir.join("hello.txt");
    let f_sub = dir.join("sub");
    fs::write(&f_hello, EXT_SMALL_HELLO).unwrap();
    fs::write(f_sub.join("nested.txt"), EXT_SMALL_NESTED).unwrap();
    (f_hello, f_sub)
}

fn create_7z_fixture(
    sources: &[PathBuf],
    out_path: &Path,
    password: Option<&str>,
    encrypt_file_names: Option<bool>,
) {
    let cfg = UniFFICreateArchiveConfig {
        source_paths: sources
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        output_path: out_path.to_string_lossy().to_string(),
        format: ArchiveFormat::SevenZip,
        level: 5,
        password: password.map(|s| s.to_string()),
        split_volume_size_bytes: None,
        exclude_patterns: Vec::new(),
        include_patterns: Vec::new(),
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    };
    create_archive_advanced_with_options(
        cfg,
        UniFFICreateArchiveExtraOptions { encrypt_file_names },
        None,
        None,
    )
    .expect("create deterministic 7z fixture");
}

fn inspect_paths_and_sizes(
    archive_path: &Path,
    pwd: Option<&str>,
) -> Result<Vec<(String, u64, bool)>, TTZipStatus> {
    let mut entries = Vec::new();
    inspect_archive(archive_path, pwd, false, |e: &ArchiveEntryRef<'_>| {
        entries.push((e.path.to_string(), e.uncompressed_size, e.is_directory));
        true
    })?;
    Ok(entries)
}

fn run_session_extract(
    archive: &Path,
    dest: &Path,
    password: Option<&str>,
    filter: Option<Vec<String>>,
) -> Result<u64, ttzip_engine::uniffi_api::types::TTZipError> {
    let req = ExtractionRequest {
        archive_path: archive.to_str().unwrap().to_string(),
        destination_root: dest.to_str().unwrap().to_string(),
        password: password.map(|s| s.to_string()),
        entry_filter: filter,
        root_at_common_parent: false,
        policy: CollisionPolicy::KeepBoth,
    };
    let session = ExtractionSession::new(req)?;
    let _plan = session.preflight()?;
    session.stage(None, None)?;
    let commit = session.commit(ResolutionSet {
        default_action: None,
        overrides: Vec::new(),
    })?;
    match commit {
        CommitResult::Finished { outcome } => {
            assert_eq!(outcome.status, OutcomeStatus::Completed);
            Ok(outcome.extracted)
        }
        CommitResult::NeedsDecision { summary } => {
            panic!("unexpected NeedsDecision: {summary:?}");
        }
    }
}

fn split_into_two_volumes(src: &Path, out_dir: &Path, base_name: &str) -> PathBuf {
    let bytes = fs::read(src).unwrap();
    let mid = (bytes.len() / 2).max(1);
    let vol1 = out_dir.join(format!("{base_name}.001"));
    let vol2 = out_dir.join(format!("{base_name}.002"));
    fs::write(&vol1, &bytes[..mid]).unwrap();
    fs::write(&vol2, &bytes[mid..]).unwrap();
    vol1
}

// ---------------------------------------------------------------------------
// 1. Q1-ISSUE-02: Single-Volume & Multi-Volume Encrypted/Unencrypted 7z
// ---------------------------------------------------------------------------

struct SevenZCase<'a> {
    sample: &'a str,
    pwd: Option<&'a str>,
    encrypt_names: Option<bool>,
    use_corpus_main: bool,
    target_file: &'a str,
    target_dir: &'a str,
    dir_child: &'a str,
    excluded_file: &'a str,
    expected_text: &'a str,
}

#[test]
fn test_r2_issue02_sevenz_single_and_multivolume_selected_extract() {
    let tmp = tempfile::tempdir().unwrap();
    let corpus_src = build_corpus_main_source(tmp.path());
    let (ext_hello, ext_sub) = build_ext_small_files(tmp.path());

    let cases = [
        SevenZCase {
            sample: "c1_7z_normal.7z",
            pwd: None,
            encrypt_names: None,
            use_corpus_main: true,
            target_file: "corpus_main/docs/readme.txt",
            target_dir: "corpus_main/L1",
            dir_child: "corpus_main/L1/L2/L3/L4/L5/L6/deep_leaf.txt",
            excluded_file: "corpus_main/bin/run_verify.sh",
            expected_text: CORPUS_MAIN_README,
        },
        SevenZCase {
            sample: "c3_enc.7z",
            pwd: Some("Accept$2026_密码_🔒"),
            encrypt_names: Some(false),
            use_corpus_main: true,
            target_file: "corpus_main/docs/readme.txt",
            target_dir: "corpus_main/L1",
            dir_child: "corpus_main/L1/L2/L3/L4/L5/L6/deep_leaf.txt",
            excluded_file: "corpus_main/bin/run_verify.sh",
            expected_text: CORPUS_MAIN_README,
        },
        SevenZCase {
            sample: "ext_7z_mhe_on.7z",
            pwd: Some("Secret2026!"),
            encrypt_names: Some(true),
            use_corpus_main: false,
            target_file: "hello.txt",
            target_dir: "sub",
            dir_child: "sub/nested.txt",
            excluded_file: "nonexistent.txt",
            expected_text: EXT_SMALL_HELLO,
        },
        SevenZCase {
            sample: "ext_7z_mhe_off.7z",
            pwd: Some("Secret2026!"),
            encrypt_names: Some(false),
            use_corpus_main: false,
            target_file: "hello.txt",
            target_dir: "sub",
            dir_child: "sub/nested.txt",
            excluded_file: "nonexistent.txt",
            expected_text: EXT_SMALL_HELLO,
        },
    ];

    for case in &cases {
        let single_dir = tmp.path().join(format!("single_{}", case.sample));
        fs::create_dir_all(&single_dir).unwrap();
        let archive_path = single_dir.join(case.sample);
        if case.use_corpus_main {
            create_7z_fixture(
                std::slice::from_ref(&corpus_src),
                &archive_path,
                case.pwd,
                case.encrypt_names,
            );
        } else {
            create_7z_fixture(
                &[ext_hello.clone(), ext_sub.clone()],
                &archive_path,
                case.pwd,
                case.encrypt_names,
            );
        }
        verify_sevenz_case(&archive_path, case, &single_dir, "single");

        let split_dir = tmp.path().join(format!("split_{}", case.sample));
        fs::create_dir_all(&split_dir).unwrap();
        let vol1 = split_into_two_volumes(&archive_path, &split_dir, case.sample);
        verify_sevenz_case(&vol1, case, &split_dir, "multi");
    }
}

fn verify_sevenz_case(archive_path: &Path, case: &SevenZCase<'_>, work_dir: &Path, tag: &str) {
    let entries = inspect_paths_and_sizes(archive_path, case.pwd).expect("7z inspect must succeed");
    assert!(!entries.is_empty(), "{tag} {}: expected entries", case.sample);

    let full_dest = work_dir.join(format!("full_{tag}"));
    fs::create_dir_all(&full_dest).unwrap();
    let mut opts = default_opts();
    let pwd_c = case.pwd.map(|p| CString::new(p).unwrap());
    if let Some(ref c) = pwd_c {
        opts.password = c.as_ptr();
    }
    extract_archive(archive_path, &StagingRoot(full_dest.clone()), &opts)
        .expect("7z full extract must succeed");
    assert_eq!(
        fs::read_to_string(full_dest.join(case.target_file)).unwrap(),
        case.expected_text
    );

    let mem_bytes = extract_single_entry_memory(archive_path, Some(case.target_file), -1, case.pwd)
        .expect("7z single entry memory must succeed");
    assert_eq!(std::str::from_utf8(&mem_bytes).unwrap(), case.expected_text);

    let sel_dest = work_dir.join(format!("sel_{tag}"));
    fs::create_dir_all(&sel_dest).unwrap();
    let selection = EntrySelection::new([case.target_file, case.target_dir], false);
    let count = extract_selected_entries(
        archive_path,
        &selection,
        &StagingRoot(sel_dest.clone()),
        &opts,
    )
    .expect("7z selected extract must succeed");
    assert!(count >= 2, "{tag} {}: expected >=2 selected entries", case.sample);
    assert_eq!(
        fs::read_to_string(sel_dest.join(case.target_file)).unwrap(),
        case.expected_text
    );
    assert!(sel_dest.join(case.dir_child).exists());
    assert!(!sel_dest.join(case.excluded_file).exists());

    let sess_dest = work_dir.join(format!("sess_{tag}"));
    fs::create_dir_all(&sess_dest).unwrap();
    let committed = run_session_extract(
        archive_path,
        &sess_dest,
        case.pwd,
        Some(vec![case.target_file.to_string()]),
    )
    .expect("7z ExtractionSession selected extract must succeed");
    assert_eq!(committed, 1);
    assert_eq!(
        fs::read_to_string(sess_dest.join(case.target_file)).unwrap(),
        case.expected_text
    );

    if case.pwd.is_some() {
        verify_sevenz_wrong_and_missing_password(archive_path, &selection, work_dir, tag);
    }
}

fn verify_sevenz_wrong_and_missing_password(
    archive_path: &Path,
    selection: &EntrySelection,
    work_dir: &Path,
    tag: &str,
) {
    for (sub_tag, bad_pw) in [("wrong", Some("WrongPassword!")), ("missing", None)] {
        let bad_dest = work_dir.join(format!("bad_{tag}_{sub_tag}"));
        fs::create_dir_all(&bad_dest).unwrap();
        let bad_c = bad_pw.map(|p| CString::new(p).unwrap());
        let mut bad_opts = default_opts();
        if let Some(ref c) = bad_c {
            bad_opts.password = c.as_ptr();
        }
        let err = extract_selected_entries(
            archive_path,
            selection,
            &StagingRoot(bad_dest.clone()),
            &bad_opts,
        )
        .expect_err("wrong or missing password must fail");
        assert_eq!(err, TTZipStatus::ErrInvalidPassword);
        assert_eq!(fs::read_dir(&bad_dest).unwrap().count(), 0);
    }
}

// ---------------------------------------------------------------------------
// 2. Q1-ISSUE-02 & Q1-ISSUE-05: Single-Volume & Multi-Volume `.tar.br`
// ---------------------------------------------------------------------------

#[test]
fn test_r2_issue02_and_05_tar_br_inspect_and_selected_extract() {
    let tmp = tempfile::tempdir().unwrap();
    let corpus_src = build_corpus_main_source(tmp.path());
    let tarbr_path = tmp.path().join("c1_tarbr.tar.br");
    create_archive(
        std::slice::from_ref(&corpus_src),
        &tarbr_path,
        &make_create_opts(TTZipArchiveFormat::TarBrotli),
        0,
    )
    .expect("create tar.br fixture");
    verify_tar_br_archive(&tarbr_path, tmp.path(), "single");

    let split_dir = tmp.path().join("split_tarbr");
    fs::create_dir_all(&split_dir).unwrap();
    let vol1 = split_into_two_volumes(&tarbr_path, &split_dir, "c1_tarbr.tar.br");
    verify_tar_br_archive(&vol1, &split_dir, "multi");
}

fn verify_tar_br_archive(archive_path: &Path, work_dir: &Path, tag: &str) {
    assert_eq!(
        detect_format(archive_path).unwrap().0,
        DetectedFormat::Brotli
    );
    let entries = inspect_paths_and_sizes(archive_path, None).expect("tar.br inspect must succeed");
    assert!(
        entries.len() >= 10,
        "{tag}: tar.br inspect must list internal TAR entries, got {}",
        entries.len()
    );
    assert!(entries
        .iter()
        .any(|(p, _, _)| p == "corpus_main/docs/readme.txt"));
    assert!(entries
        .iter()
        .any(|(p, _, _)| p == "corpus_main/L1/L2/L3/L4/L5/L6/deep_leaf.txt"));

    let sel_dest = work_dir.join(format!("sel_tarbr_{tag}"));
    fs::create_dir_all(&sel_dest).unwrap();
    let committed = run_session_extract(
        archive_path,
        &sel_dest,
        None,
        Some(vec![
            "corpus_main/docs/readme.txt".to_string(),
            "corpus_main/L1".to_string(),
        ]),
    )
    .expect("tar.br ExtractionSession selected extract should succeed");
    assert!(committed >= 1);
    assert_eq!(
        fs::read_to_string(sel_dest.join("corpus_main/docs/readme.txt")).unwrap(),
        CORPUS_MAIN_README
    );
    assert!(sel_dest
        .join("corpus_main/L1/L2/L3/L4/L5/L6/deep_leaf.txt")
        .exists());
    assert!(!sel_dest.join("corpus_main/bin/run_verify.sh").exists());

    let raw_sel_dest = work_dir.join(format!("raw_sel_tarbr_{tag}"));
    fs::create_dir_all(&raw_sel_dest).unwrap();
    let selection = EntrySelection::new(["corpus_main/docs/readme.txt", "corpus_main/L1"], false);
    let sel_count = extract_selected_entries(
        archive_path,
        &selection,
        &StagingRoot(raw_sel_dest.clone()),
        &default_opts(),
    )
    .expect("tar.br extract_selected_entries should succeed");
    assert!(sel_count >= 2);

    let mem = extract_single_entry_memory(
        archive_path,
        Some("corpus_main/docs/readme.txt"),
        -1,
        None,
    )
    .expect("tar.br single entry memory should succeed");
    assert_eq!(std::str::from_utf8(&mem).unwrap(), CORPUS_MAIN_README);
}

// ---------------------------------------------------------------------------
// 3. Q1-ISSUE-05: Pure Single-File Compressed Streams (`.gz`..`.sz`)
// ---------------------------------------------------------------------------

#[test]
fn test_r2_issue05_q1_single_file_streams_browse_session_and_extract() {
    let tmp = tempfile::tempdir().unwrap();
    let payload_src = tmp.path().join("single_payload.bin");
    let expected_payload: Vec<u8> = (0..43_008)
        .map(|i| ((i * 31 + 7) & 0xFF) as u8)
        .collect();
    fs::write(&payload_src, &expected_payload).unwrap();

    let cases = [
        ("single_payload.bin.gz", "single_payload.bin"),
        ("single_payload.bin.bz2", "single_payload.bin"),
        ("single_payload.bin.xz", "single_payload.bin"),
        ("single_payload.bin.zst", "single_payload.bin"),
        ("single_payload.bin.lz4", "single_payload.bin"),
        ("single_payload.bin.br", "single_payload.bin"),
        ("single_payload.bin.lz", "single_payload.bin"),
        ("single_payload.bin.lrz", "single_payload.bin"),
        ("single_payload.bin.sz", "single_payload.bin"),
    ];

    for (file_name, expected_entry) in cases {
        let stream_dir = tmp.path().join(format!("case_{file_name}"));
        fs::create_dir_all(&stream_dir).unwrap();
        let archive_path = stream_dir.join(file_name);
        create_archive(
            std::slice::from_ref(&payload_src),
            &archive_path,
            &TTZipCreateOptions::default(),
            0,
        )
        .expect("create single-file stream fixture");
        verify_single_stream_end_to_end(
            &archive_path,
            expected_entry,
            &expected_payload,
            &stream_dir,
        );
    }
}

fn verify_single_stream_end_to_end(
    archive_path: &Path,
    expected_entry: &str,
    expected_payload: &[u8],
    work_dir: &Path,
) {
    let entries = inspect_paths_and_sizes(archive_path, None)
        .unwrap_or_else(|e| panic!("inspect failed for {}: {e:?}", archive_path.display()));
    assert_eq!(entries.len(), 1, "expected 1 entry for {}", archive_path.display());
    assert_eq!(entries[0].0, expected_entry);
    assert_eq!(entries[0].1, expected_payload.len() as u64);
    assert!(!entries[0].2);

    let full_dir = work_dir.join("full_out");
    fs::create_dir_all(&full_dir).unwrap();
    let written = extract_archive_with_metrics(
        archive_path,
        &StagingRoot(full_dir.clone()),
        &default_opts(),
    )
    .unwrap_or_else(|e| panic!("extract_archive failed for {}: {e:?}", archive_path.display()));
    assert_eq!(written, expected_payload.len() as u64);
    assert_eq!(fs::read(full_dir.join(expected_entry)).unwrap(), expected_payload);
    assert!(!full_dir.join("data").exists());

    for (tag, filter) in [
        ("sess_all", None),
        ("sess_sel", Some(vec![expected_entry.to_string()])),
    ] {
        let sess_dir = work_dir.join(tag);
        fs::create_dir_all(&sess_dir).unwrap();
        let committed = run_session_extract(archive_path, &sess_dir, None, filter)
            .unwrap_or_else(|e| panic!("{tag} failed for {}: {e:?}", archive_path.display()));
        assert_eq!(committed, 1);
        assert_eq!(fs::read(sess_dir.join(expected_entry)).unwrap(), expected_payload);
        assert!(!sess_dir.join("data").exists());
    }

    let mem_by_name = extract_single_entry_memory(archive_path, Some(expected_entry), -1, None)
        .expect("memory extract by name");
    assert_eq!(mem_by_name, expected_payload);
    let mem_by_idx = extract_single_entry_memory(archive_path, None, 0, None)
        .expect("memory extract by index 0");
    assert_eq!(mem_by_idx, expected_payload);
}
