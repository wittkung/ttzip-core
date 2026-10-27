// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R2 regression test suite for external container classification and read routing
//! (`DMG`, `AAR`, `XAR`, `WIM`), bounded XAR TOC/entry decompression and cumulative
//! output budget enforcement, and corrupt/missing volume rejection.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::ZlibEncoder;
use flate2::Compression;
use ttzip_engine::archive::unified::detect::detect_archive_sniff;
use ttzip_engine::archive::unified::extract::extract_archive_with_metrics;
use ttzip_engine::archive::unified::inspect::ArchiveEntryRef;
use ttzip_engine::archive::unified::{
    create_archive, extract_archive, extract_single_entry_memory, inspect_archive,
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

fn write_synthetic_udif_dmg(path: &Path) {
    let mut bytes = vec![0u8; 1024];
    let trailer = &mut bytes[512..1024];
    trailer[0..4].copy_from_slice(b"koly");
    trailer[4..8].copy_from_slice(&4u32.to_be_bytes());
    trailer[8..12].copy_from_slice(&512u32.to_be_bytes());
    fs::write(path, bytes).unwrap();
}

fn write_synthetic_xar(path: &Path, toc_xml: &str, declared_toc_uncomp: Option<usize>, heap: &[u8]) {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(toc_xml.as_bytes()).unwrap();
    let comp_toc = enc.finish().unwrap();
    let toc_uncomp = declared_toc_uncomp.unwrap_or(toc_xml.len());

    let mut out = Vec::with_capacity(28 + comp_toc.len() + heap.len());
    out.extend_from_slice(b"xar!");
    out.extend_from_slice(&28u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&(comp_toc.len() as u64).to_be_bytes());
    out.extend_from_slice(&(toc_uncomp as u64).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&comp_toc);
    out.extend_from_slice(heap);
    fs::write(path, out).unwrap();
}

fn push_wim_direntry(buf: &mut Vec<u8>, name: &str, is_dir: bool, subdir_off: u64, sha1: [u8; 20]) {
    let utf16: Vec<u8> = name.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let raw_len = 102 + utf16.len();
    let li_len = (raw_len + 7) & !7;
    let start = buf.len();
    buf.resize(start + li_len, 0);
    let entry = &mut buf[start..start + li_len];
    entry[0..8].copy_from_slice(&(li_len as u64).to_le_bytes());
    let attrs: u32 = if is_dir { 0x10 } else { 0x80 };
    entry[8..12].copy_from_slice(&attrs.to_le_bytes());
    entry[16..24].copy_from_slice(&subdir_off.to_le_bytes());
    entry[64..84].copy_from_slice(&sha1);
    entry[100..102].copy_from_slice(&(utf16.len() as u16).to_le_bytes());
    entry[102..102 + utf16.len()].copy_from_slice(&utf16);
}

fn push_wim_reshdr(buf: &mut Vec<u8>, size: usize, flags: u8, offset: usize, sha1: [u8; 20]) {
    let start = buf.len();
    buf.resize(start + 50, 0);
    let entry = &mut buf[start..start + 50];
    let size_and_flags = ((size as u64) & 0x00FF_FFFF_FFFF_FFFF) | ((flags as u64) << 56);
    entry[0..8].copy_from_slice(&size_and_flags.to_le_bytes());
    entry[8..16].copy_from_slice(&(offset as u64).to_le_bytes());
    entry[16..24].copy_from_slice(&(size as u64).to_le_bytes());
    entry[30..50].copy_from_slice(&sha1);
}

fn write_synthetic_wim(path: &Path) {
    let sha_hello = [0x11u8; 20];
    let sha_nested = [0x22u8; 20];
    let sha_meta = [0x33u8; 20];

    let mut meta = vec![0u8; 8];
    push_wim_direntry(&mut meta, "", true, 120, [0u8; 20]);
    meta.extend_from_slice(&0u64.to_le_bytes());
    assert_eq!(meta.len(), 120);
    push_wim_direntry(&mut meta, "hello.txt", false, 0, sha_hello);
    push_wim_direntry(&mut meta, "sub", true, 360, [0u8; 20]);
    meta.extend_from_slice(&0u64.to_le_bytes());
    assert_eq!(meta.len(), 360);
    push_wim_direntry(&mut meta, "nested.txt", false, 0, sha_nested);
    meta.extend_from_slice(&0u64.to_le_bytes());

    let mut wim = vec![0u8; 208];
    wim[0..8].copy_from_slice(b"MSWIM\0\0\0");
    wim[8..12].copy_from_slice(&208u32.to_le_bytes());
    wim[12..16].copy_from_slice(&1u32.to_le_bytes());
    wim[40..42].copy_from_slice(&1u16.to_le_bytes());
    wim[42..44].copy_from_slice(&1u16.to_le_bytes());

    let off_hello = wim.len();
    wim.extend_from_slice(EXT_SMALL_HELLO.as_bytes());
    let off_nested = wim.len();
    wim.extend_from_slice(EXT_SMALL_NESTED.as_bytes());
    let off_meta = wim.len();
    wim.extend_from_slice(&meta);

    let mut ot = Vec::new();
    push_wim_reshdr(&mut ot, EXT_SMALL_HELLO.len(), 0x00, off_hello, sha_hello);
    push_wim_reshdr(&mut ot, EXT_SMALL_NESTED.len(), 0x00, off_nested, sha_nested);
    push_wim_reshdr(&mut ot, meta.len(), 0x02, off_meta, sha_meta);

    let off_ot = wim.len();
    let ot_len = ot.len();
    wim.extend_from_slice(&ot);
    wim[48..56].copy_from_slice(&(ot_len as u64).to_le_bytes());
    wim[56..64].copy_from_slice(&(off_ot as u64).to_le_bytes());
    wim[64..72].copy_from_slice(&(ot_len as u64).to_le_bytes());
    fs::write(path, wim).unwrap();
}

#[test]
fn test_r2_issue06_dmg_explicit_unsupported_vs_corrupt() {
    let tmp = tempfile::tempdir().unwrap();
    let dmg_path = tmp.path().join("ext_created.dmg");
    write_synthetic_udif_dmg(&dmg_path);
    assert_eq!(
        detect_archive_sniff(&dmg_path).unwrap().format(),
        DetectedFormat::Dmg
    );
    assert_eq!(
        inspect_paths_and_sizes(&dmg_path, None).unwrap_err(),
        TTZipStatus::ErrUnsupportedFeature
    );
    let out_dir = tmp.path().join("dmg_out");
    fs::create_dir_all(&out_dir).unwrap();
    assert_eq!(
        extract_archive(&dmg_path, &StagingRoot(out_dir.clone()), &default_opts()).unwrap_err(),
        TTZipStatus::ErrUnsupportedFeature
    );
    assert_eq!(fs::read_dir(&out_dir).unwrap().count(), 0);
    assert_eq!(
        extract_single_entry_memory(&dmg_path, None, 0, None).unwrap_err(),
        TTZipStatus::ErrUnsupportedFeature
    );

    let corrupt_dmg = tmp.path().join("broken.dmg");
    fs::write(&corrupt_dmg, b"not a valid udif koly image").unwrap();
    assert_eq!(
        inspect_paths_and_sizes(&corrupt_dmg, None).unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );
}

#[test]
fn test_r2_issue06_aar_and_xar_and_wim_read_paths() {
    let tmp = tempfile::tempdir().unwrap();
    verify_aar_read_path(tmp.path());
    verify_xar_read_paths(tmp.path());
    verify_wim_read_path(tmp.path());
}

fn verify_aar_read_path(work_dir: &Path) {
    let (ext_hello, ext_sub) = build_ext_small_files(work_dir);
    let aar_path = work_dir.join("ext_created.aar");
    create_archive(
        &[ext_hello, ext_sub],
        &aar_path,
        &make_create_opts(TTZipArchiveFormat::Aar),
        0,
    )
    .expect("create deterministic AAR fixture");
    assert_eq!(
        detect_archive_sniff(&aar_path).unwrap().format(),
        DetectedFormat::Aar
    );
    let entries = inspect_paths_and_sizes(&aar_path, None).expect("AAR inspect must succeed");
    assert_eq!(entries.len(), 3);
    let aar_full = work_dir.join("aar_full");
    fs::create_dir_all(&aar_full).unwrap();
    assert_eq!(run_session_extract(&aar_path, &aar_full, None, None).unwrap(), 2);
    assert_eq!(
        fs::read_to_string(aar_full.join("hello.txt")).unwrap(),
        EXT_SMALL_HELLO
    );
    assert_eq!(
        fs::read_to_string(aar_full.join("sub/nested.txt")).unwrap(),
        EXT_SMALL_NESTED
    );

    let aar_sel = work_dir.join("aar_sel");
    fs::create_dir_all(&aar_sel).unwrap();
    assert_eq!(
        run_session_extract(
            &aar_path,
            &aar_sel,
            None,
            Some(vec!["sub/nested.txt".to_string()]),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        fs::read_to_string(aar_sel.join("sub/nested.txt")).unwrap(),
        EXT_SMALL_NESTED
    );
    assert!(!aar_sel.join("hello.txt").exists());
}

fn verify_xar_read_paths(work_dir: &Path) {
    let xar_empty = work_dir.join("ext_created.xar");
    write_synthetic_xar(&xar_empty, "<xar><toc></toc></xar>", None, &[]);
    assert_eq!(
        detect_archive_sniff(&xar_empty).unwrap().format(),
        DetectedFormat::Xar
    );
    assert_eq!(inspect_paths_and_sizes(&xar_empty, None).unwrap().len(), 0);
    let xar_empty_out = work_dir.join("xar_empty_out");
    fs::create_dir_all(&xar_empty_out).unwrap();
    assert_eq!(
        extract_archive_with_metrics(&xar_empty, &StagingRoot(xar_empty_out), &default_opts())
            .unwrap(),
        0
    );

    let corpus_src = build_corpus_main_source(work_dir);
    let xar_full = work_dir.join("c1_xar.xar");
    create_archive(
        std::slice::from_ref(&corpus_src),
        &xar_full,
        &make_create_opts(TTZipArchiveFormat::Xar),
        0,
    )
    .expect("create deterministic XAR fixture");
    assert_eq!(
        detect_archive_sniff(&xar_full).unwrap().format(),
        DetectedFormat::Xar
    );
    let entries = inspect_paths_and_sizes(&xar_full, None).expect("c1_xar inspect");
    assert!(entries.len() >= 10);
    let mem = extract_single_entry_memory(
        &xar_full,
        Some("corpus_main/docs/readme.txt"),
        -1,
        None,
    )
    .expect("c1_xar single entry memory");
    assert_eq!(std::str::from_utf8(&mem).unwrap(), CORPUS_MAIN_README);

    let xar_sel = work_dir.join("xar_sel");
    fs::create_dir_all(&xar_sel).unwrap();
    assert_eq!(
        run_session_extract(
            &xar_full,
            &xar_sel,
            None,
            Some(vec!["corpus_main/docs/readme.txt".to_string()]),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        fs::read_to_string(xar_sel.join("corpus_main/docs/readme.txt")).unwrap(),
        CORPUS_MAIN_README
    );
}

fn verify_wim_read_path(work_dir: &Path) {
    let wim_path = work_dir.join("ext_created.wim");
    write_synthetic_wim(&wim_path);
    assert_eq!(
        detect_archive_sniff(&wim_path).unwrap().format(),
        DetectedFormat::Wim
    );
    let entries = inspect_paths_and_sizes(&wim_path, None).expect("WIM inspect");
    assert_eq!(entries.len(), 3);
    let wim_full = work_dir.join("wim_full");
    fs::create_dir_all(&wim_full).unwrap();
    assert_eq!(run_session_extract(&wim_path, &wim_full, None, None).unwrap(), 2);
    assert_eq!(
        fs::read_to_string(wim_full.join("hello.txt")).unwrap(),
        EXT_SMALL_HELLO
    );
    assert_eq!(
        fs::read_to_string(wim_full.join("sub/nested.txt")).unwrap(),
        EXT_SMALL_NESTED
    );

    let wim_sel = work_dir.join("wim_sel");
    fs::create_dir_all(&wim_sel).unwrap();
    assert_eq!(
        run_session_extract(
            &wim_path,
            &wim_sel,
            None,
            Some(vec!["hello.txt".to_string()]),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        fs::read_to_string(wim_sel.join("hello.txt")).unwrap(),
        EXT_SMALL_HELLO
    );
    assert!(!wim_sel.join("sub/nested.txt").exists());
}

#[test]
fn test_r2_xar_bounded_toc_and_entry_decompression_and_cumulative_budget() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. XAR TOC declared smaller than actual decompressed stream must fail during bounded read
    let big_xml = format!("<xar><toc><!-- {} --></toc></xar>", "A".repeat(256 * 1024));
    let xar_lying_toc = tmp.path().join("lying_toc.xar");
    write_synthetic_xar(&xar_lying_toc, &big_xml, Some(128), &[]);
    assert_eq!(
        inspect_paths_and_sizes(&xar_lying_toc, None).unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );

    // 2. XAR entry extent declared <size>64</size> while zlib stream expands to 128 KiB must fail
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&vec![b'Z'; 128 * 1024]).unwrap();
    let bomb_heap = enc.finish().unwrap();
    let entry_toc = format!(
        "<xar><toc><file id=\"1\"><name>bomb.bin</name><type>file</type><data><encoding style=\"application/x-gzip\"/><offset>0</offset><length>{}</length><size>64</size></data></file></toc></xar>",
        bomb_heap.len()
    );
    let xar_lying_entry = tmp.path().join("lying_entry.xar");
    write_synthetic_xar(&xar_lying_entry, &entry_toc, None, &bomb_heap);
    assert_eq!(
        inspect_paths_and_sizes(&xar_lying_entry, None).unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );

    // 3. Cumulative output budget across repeated extents referencing a shared 1 MiB compressed block
    //    must reject with ErrOutOfMemory once total output exceeds MAX_TRANSCODE_CONTAINER_BYTES (256 MiB).
    let chunk_size = 4 * 1024 * 1024;
    let mut chunk_enc = ZlibEncoder::new(Vec::new(), Compression::best());
    chunk_enc.write_all(&vec![b'X'; chunk_size]).unwrap();
    let shared_heap = chunk_enc.finish().unwrap();
    let mut files_xml = String::from("<xar><toc>");
    for i in 0..70 {
        files_xml.push_str(&format!(
            "<file id=\"{i}\"><name>f_{i}.bin</name><type>file</type><data><encoding style=\"application/x-gzip\"/><offset>0</offset><length>{}</length><size>{chunk_size}</size></data></file>",
            shared_heap.len()
        ));
    }
    files_xml.push_str("</toc></xar>");
    let xar_budget_overflow = tmp.path().join("budget_overflow.xar");
    write_synthetic_xar(&xar_budget_overflow, &files_xml, None, &shared_heap);
    assert_eq!(
        inspect_paths_and_sizes(&xar_budget_overflow, None).unwrap_err(),
        TTZipStatus::ErrOutOfMemory
    );
}

#[test]
fn test_r2_raw_format_and_corrupt_or_missing_volumes_reject_cleanly() {
    let tmp = tempfile::tempdir().unwrap();

    let not_arch = tmp.path().join("not_an_archive.bin");
    fs::write(&not_arch, b"arbitrary non-archive payload bytes 1234567890").unwrap();
    assert!(inspect_paths_and_sizes(&not_arch, None).is_err());
    let out = tmp.path().join("not_arch_out");
    fs::create_dir_all(&out).unwrap();
    assert!(extract_archive(&not_arch, &StagingRoot(out.clone()), &default_opts()).is_err());
    assert!(!out.join("data").exists());

    for ext in ["gz", "bz2", "xz", "zst", "lz4", "br", "lz", "lrz", "sz"] {
        let bogus = tmp.path().join(format!("corrupt_stream.{ext}"));
        let mut f = fs::File::create(&bogus).unwrap();
        f.write_all(b"\x1F\x8B\x08\x00\x00\x00\x00\x00\x00\x03\xFF\xFF").unwrap();
        drop(f);
        assert!(
            inspect_paths_and_sizes(&bogus, None).is_err(),
            "truncated .{ext} must fail inspect"
        );
        let out = tmp.path().join(format!("out_{ext}"));
        fs::create_dir_all(&out).unwrap();
        assert!(
            extract_archive(&bogus, &StagingRoot(out), &default_opts()).is_err(),
            "truncated .{ext} must fail extract"
        );
    }

    let (ext_hello, ext_sub) = build_ext_small_files(tmp.path());
    let base_7z = tmp.path().join("base_split.7z");
    create_7z_fixture(&[ext_hello, ext_sub], &base_7z, None, None);

    let miss_dir = tmp.path().join("miss_vol");
    fs::create_dir_all(&miss_dir).unwrap();
    let miss_v1 = split_into_two_volumes(&base_7z, &miss_dir, "c3_split.7z");
    fs::remove_file(miss_dir.join("c3_split.7z.002")).unwrap();
    assert!(inspect_paths_and_sizes(&miss_v1, None).is_err());

    let corr_dir = tmp.path().join("corr_vol");
    fs::create_dir_all(&corr_dir).unwrap();
    let corr_v1 = split_into_two_volumes(&base_7z, &corr_dir, "c3_split.7z");
    fs::write(corr_dir.join("c3_split.7z.002"), b"CORRUPTED_VOLUME_2_BYTES").unwrap();
    let corr_out = corr_dir.join("out");
    fs::create_dir_all(&corr_out).unwrap();
    assert!(run_session_extract(&corr_v1, &corr_out, None, None).is_err());
    assert_eq!(fs::read_dir(&corr_out).unwrap().count(), 0);
}
