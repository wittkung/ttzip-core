// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R1 Regression Tests for Q1-ISSUE-01: 7z Streaming Decoder Output Drain & Boundary Verification.
//!
//! Covers:
//! 1. Q1 C5-02a large high-compression 7z fixture (copied to isolated tempdir) across all decoder
//!    entry points (`SevenZArchive::extract_all_with_sheet` single/multi-thread,
//!    `extract_7z_from_reader`, `extract_single_entry_bounded`,
//!    `extract_7z_single_entry_from_reader`, `SolidEarlyExitExtractor`).
//! 2. Exact chunk boundary output sizes around `CHUNK_SIZE = 4 MiB`:
//!    `4 MiB - 1`, `4 MiB`, `4 MiB + 1`, `8 MiB`, and `12 MiB + 37`.
//! 3. High-compressibility, incompressible (> 4 MiB compressed), multi-file solid blocks (> 4 MiB),
//!    and encrypted (`-p` / `-mhe=on`) vs unencrypted archives.
//! 4. External 7z CLI (`7z` / `7zz`) generated archives (LZMA2, LZMA1, AES+LZMA2, multi-file solid)
//!    to verify decoding independently of TTZip's writer.
//! 5. Failure & safety cases: truncated payload, corrupted stream, output size overrun (declared
//!    unpack size smaller than actual stream), wrong password, and mid-stream cancellation without
//!    leaving committed partial output files on disk.

use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::decoder::archive::{
    extract_7z_from_reader, extract_7z_single_entry_from_reader,
};
use ttzip_engine::sevenz::decoder::payload::{
    decode_7z_folder_streaming, decode_7z_solid_streaming, extract_single_entry_bounded,
};
use ttzip_engine::sevenz::{
    create_7z_solid_archive_bytes, create_7z_solid_archive_bytes_with_password, parse_7z_metadata,
    SevenZArchive, SevenZSeekIndex,
};
use ttzip_engine::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};
use ttzip_engine::zip::writer::ZipInputItem;

const CHUNK_SIZE: usize = 4 * 1024 * 1024;
const Q1_C5_ARCHIVE_ENV: &str = "TTZIP_R1_Q1_C5_ARCHIVE";
const Q1_C5_EXPECTED_LEN: usize = 14_417_920;
const Q1_C5_EXPECTED_SHA256: &str =
    "1ab15267018b8a2a2563fef2029d479dd0f6482210f50503598dbbec86195bfb";
// The deterministic fallback is a distinct payload from the historical Q1 sample.
const Q1_C5_REPRO_SHA256: &str =
    "6606c4c4cb1eb952888fea2a5f3bf150ae2af15689a34f55686c6698d6d15c73";

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn find_external_7z_binary() -> Option<PathBuf> {
    let candidates = ["/opt/homebrew/bin/7z", "/opt/homebrew/bin/7zz", "/usr/local/bin/7z"];
    for candidate in candidates {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn build_q1_c5_large_payload() -> Vec<u8> {
    let mut pattern = Vec::with_capacity(61 * 130);
    for i in 0..130u32 {
        let line = format!("LINE-{:04}-ABCDEFGHIJKLMNOPQRSTUVWXYZ-0123456789-REPEAT-BLOCK\n", i);
        pattern.extend_from_slice(line.as_bytes());
    }
    let mut out = Vec::with_capacity(Q1_C5_EXPECTED_LEN);
    let mut block_130k = Vec::with_capacity(131_072);
    while block_130k.len() < 131_072 {
        let rem = 131_072 - block_130k.len();
        let take = rem.min(pattern.len());
        block_130k.extend_from_slice(&pattern[..take]);
    }
    for _ in 0..110 {
        out.extend_from_slice(&block_130k);
    }
    out
}

fn generate_compressible_bytes(len: usize, seed: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(len);
    let block = format!(
        "TTZIP-R1-STREAMING-DRAIN-SEED-{:08x}-HIGH-RATIO-REPEATING-SEGMENT\n",
        seed
    );
    let block_bytes = block.as_bytes();
    while buf.len() < len {
        let rem = len - buf.len();
        buf.extend_from_slice(&block_bytes[..rem.min(block_bytes.len())]);
    }
    buf
}

fn generate_incompressible_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    while out.len() < len {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let word = state.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes();
        let rem = len - out.len();
        out.extend_from_slice(&word[..rem.min(8)]);
    }
    out
}

fn create_external_7z_archive(
    work_dir: &Path,
    archive_name: &str,
    files: &[(&str, &[u8])],
    extra_args: &[&str],
) -> PathBuf {
    let bin = find_external_7z_binary().expect("external 7z binary required for test");
    let src_dir = work_dir.join("src_tree");
    fs::create_dir_all(&src_dir).expect("create src_tree");
    let mut rel_names = Vec::with_capacity(files.len());
    for (rel, content) in files {
        let full = src_dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&full, content).expect("write input file");
        rel_names.push(*rel);
    }
    let archive_path = work_dir.join(archive_name);
    let mut cmd = Command::new(bin);
    cmd.current_dir(&src_dir).arg("a").arg("-y").arg(&archive_path);
    for arg in extra_args {
        cmd.arg(arg);
    }
    for rel in rel_names {
        cmd.arg(rel);
    }
    let output = cmd.output().expect("spawn external 7z");
    assert!(
        output.status.success(),
        "external 7z failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    archive_path
}

fn make_extract_opts(threads: u32, c_pass: Option<&CString>) -> TTZipExtractOptions {
    let mut opts = TTZipExtractOptions {
        thread_budget: threads,
        ..Default::default()
    };
    if let Some(c) = c_pass {
        opts.password = c.as_ptr();
    }
    opts
}

fn verify_archive_all_paths(
    archive_bytes: &[u8],
    password: Option<&str>,
    expected_name: &str,
    expected_len: usize,
    expected_sha256: &str,
) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let archive = SevenZArchive::open_slice_with_password(archive_bytes, password)
        .expect("open_slice_with_password");
    let c_pass = password.map(|p| CString::new(p).expect("cstring"));

    for threads in [1u32, 4u32] {
        let dest = tmp.path().join(format!("extract_t{}", threads));
        fs::create_dir_all(&dest).expect("mkdir");
        let opts = make_extract_opts(threads, c_pass.as_ref());
        let report = archive
            .extract_all_with_sheet(&dest, &opts, None)
            .expect("extract_all_with_sheet");
        assert_eq!(report.total_uncompressed_bytes, expected_len as u64);
        let on_disk = fs::read(dest.join(expected_name)).expect("read extracted file");
        assert_eq!(on_disk.len(), expected_len);
        assert_eq!(sha256_hex(&on_disk), expected_sha256);
    }

    let reader_dest = tmp.path().join("extract_reader");
    fs::create_dir_all(&reader_dest).expect("mkdir");
    let mut cursor = Cursor::new(archive_bytes);
    let opts2 = make_extract_opts(2, c_pass.as_ref());
    let reader_extracted =
        extract_7z_from_reader(&mut cursor, archive.info(), &reader_dest, &opts2, None)
            .expect("reader extract");
    assert_eq!(reader_extracted, expected_len as u64);
    let reader_bytes = fs::read(reader_dest.join(expected_name)).expect("read reader output");
    assert_eq!(sha256_hex(&reader_bytes), expected_sha256);

    let bounded = extract_single_entry_bounded(
        archive_bytes,
        archive.info(),
        archive.seek_index(),
        0,
        password,
        0,
    )
    .expect("extract_single_entry_bounded");
    assert_eq!(bounded.len(), expected_len);
    assert_eq!(sha256_hex(&bounded), expected_sha256);

    let mut cursor2 = Cursor::new(archive_bytes);
    let streamed = extract_7z_single_entry_from_reader(
        &mut cursor2,
        archive.info(),
        archive.seek_index(),
        0,
        password,
    )
    .expect("extract_7z_single_entry_from_reader");
    assert_eq!(streamed.len(), expected_len);
    assert_eq!(sha256_hex(&streamed), expected_sha256);

    let mut sink_buf = Vec::with_capacity(expected_len);
    let extractor = archive.solid_extractor().with_threads(2);
    let stats = extractor
        .extract_streaming(0, password, |chunk| {
            sink_buf.extend_from_slice(chunk);
            Ok(())
        })
        .expect("early_exit_extractor");
    assert!(stats.crc_matched);
    assert_eq!(sink_buf.len(), expected_len);
    assert_eq!(sha256_hex(&sink_buf), expected_sha256);
}

// Keep each archive bound to its independent plaintext oracle.
struct Q1C5Fixture {
    archive_path: PathBuf,
    expected_sha256: &'static str,
}

#[test]
fn test_q1_c5_02a_large_high_ratio_7z_all_entrypoints() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let fixture = if let Some(source) = std::env::var_os(Q1_C5_ARCHIVE_ENV) {
        let copied = tmp.path().join("c5_large_copy.7z");
        fs::copy(PathBuf::from(source), &copied)
            .expect("copy explicitly requested historical Q1 archive");
        Q1C5Fixture {
            archive_path: copied,
            expected_sha256: Q1_C5_EXPECTED_SHA256,
        }
    } else {
        let payload = build_q1_c5_large_payload();
        assert_eq!(payload.len(), Q1_C5_EXPECTED_LEN);
        assert_eq!(sha256_hex(&payload), Q1_C5_REPRO_SHA256);
        let generated = create_external_7z_archive(
            tmp.path(),
            "c5_large_repro.7z",
            &[("large_16mb.bin", &payload)],
            &["-m0=lzma2", "-mx=5"],
        );
        Q1C5Fixture {
            archive_path: generated,
            expected_sha256: Q1_C5_REPRO_SHA256,
        }
    };

    let archive_bytes = fs::read(&fixture.archive_path).expect("read c5_large.7z");
    assert!(
        archive_bytes.len() < CHUNK_SIZE,
        "compressed size ({}) must be smaller than 4 MiB chunk to trigger drain path",
        archive_bytes.len()
    );
    verify_archive_all_paths(
        &archive_bytes,
        None,
        "large_16mb.bin",
        Q1_C5_EXPECTED_LEN,
        fixture.expected_sha256,
    );
}

#[test]
fn test_chunk_boundary_sizes_lzma2_and_lzma1() {
    let sizes = [
        CHUNK_SIZE - 1,
        CHUNK_SIZE,
        CHUNK_SIZE + 1,
        CHUNK_SIZE * 2,
        CHUNK_SIZE * 3 + 37,
    ];
    let tmp = tempfile::tempdir().expect("tempdir");

    for (idx, &size) in sizes.iter().enumerate() {
        let data = generate_compressible_bytes(size, idx as u32 + 100);
        let expected_hash = sha256_hex(&data);
        let case_dir = tmp.path().join(format!("boundary_{}", size));
        fs::create_dir_all(&case_dir).expect("mkdir");

        let lzma2_path = create_external_7z_archive(
            &case_dir,
            "lzma2.7z",
            &[("boundary.bin", &data)],
            &["-m0=lzma2", "-mx=3"],
        );
        let lzma2_bytes = fs::read(&lzma2_path).expect("read lzma2.7z");
        verify_archive_all_paths(&lzma2_bytes, None, "boundary.bin", size, &expected_hash);

        let lzma1_path = create_external_7z_archive(
            &case_dir,
            "lzma1.7z",
            &[("boundary.bin", &data)],
            &["-m0=lzma", "-mx=3"],
        );
        let lzma1_bytes = fs::read(&lzma1_path).expect("read lzma1.7z");
        verify_archive_all_paths(&lzma1_bytes, None, "boundary.bin", size, &expected_hash);
    }
}

#[test]
fn test_incompressible_multi_chunk_and_encrypted_archives() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let incomp_len = CHUNK_SIZE + 524_289;
    let incomp_data = generate_incompressible_bytes(incomp_len, 0xCAFE_BABE_1234_5678);
    let incomp_hash = sha256_hex(&incomp_data);

    let incomp_path = create_external_7z_archive(
        &tmp.path().join("incomp"),
        "incomp.7z",
        &[("random.bin", &incomp_data)],
        &["-m0=lzma2", "-mx=1"],
    );
    let incomp_bytes = fs::read(&incomp_path).expect("read incomp.7z");
    assert!(incomp_bytes.len() > CHUNK_SIZE);
    verify_archive_all_paths(&incomp_bytes, None, "random.bin", incomp_len, &incomp_hash);

    let enc_len = CHUNK_SIZE * 2 + 19;
    let enc_data = generate_compressible_bytes(enc_len, 0x7788);
    let enc_hash = sha256_hex(&enc_data);
    let pass = "R1-Stream-Secret!2026";

    let enc_path = create_external_7z_archive(
        &tmp.path().join("enc_external"),
        "encrypted.7z",
        &[("secret.bin", &enc_data)],
        &["-m0=lzma2", "-mx=5", &format!("-p{}", pass), "-mhe=on"],
    );
    let enc_bytes = fs::read(&enc_path).expect("read encrypted.7z");
    verify_archive_all_paths(&enc_bytes, Some(pass), "secret.bin", enc_len, &enc_hash);

    let items = vec![ZipInputItem {
        rel_path: "secret.bin".to_string(),
        data: enc_data,
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o644,
        is_directory: false,
    }];
    let ttzip_enc_bytes =
        create_7z_solid_archive_bytes_with_password(&items, 3, 2, Some(pass)).expect("enc 7z");
    verify_archive_all_paths(&ttzip_enc_bytes, Some(pass), "secret.bin", enc_len, &enc_hash);
}

#[test]
fn test_multi_file_solid_block_crossing_4mib_and_selective_early_exit() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let f0 = generate_compressible_bytes(2 * 1024 * 1024 + 111, 1);
    let f1 = generate_compressible_bytes(3 * 1024 * 1024 + 222, 2);
    let f2 = generate_compressible_bytes(4 * 1024 * 1024 + 333, 3);
    let h0 = sha256_hex(&f0);
    let h1 = sha256_hex(&f1);
    let h2 = sha256_hex(&f2);

    let solid_path = create_external_7z_archive(
        tmp.path(),
        "solid_multi.7z",
        &[("a_first.bin", &f0), ("b_second.bin", &f1), ("c_third.bin", &f2)],
        &["-m0=lzma2", "-mx=5", "-ms=on"],
    );
    let solid_bytes = fs::read(&solid_path).expect("read solid_multi.7z");
    let archive = SevenZArchive::open_slice(&solid_bytes).expect("open solid_multi.7z");

    let full_dest = tmp.path().join("full_out");
    fs::create_dir_all(&full_dest).expect("mkdir");
    let opts = make_extract_opts(2, None);
    archive
        .extract_all_with_sheet(&full_dest, &opts, None)
        .expect("extract_all solid");
    assert_eq!(sha256_hex(&fs::read(full_dest.join("a_first.bin")).unwrap()), h0);
    assert_eq!(sha256_hex(&fs::read(full_dest.join("b_second.bin")).unwrap()), h1);
    assert_eq!(sha256_hex(&fs::read(full_dest.join("c_third.bin")).unwrap()), h2);

    let extractor = archive.solid_extractor().with_threads(2);
    let (sel_vec, stats) = extractor
        .extract_to_vec(1, None)
        .expect("selective early exit on middle file");
    assert_eq!(sel_vec.len(), f1.len());
    assert_eq!(sha256_hex(&sel_vec), h1);
    assert!(stats.early_exit_triggered);
}

fn build_single_large_solid_archive(len: usize) -> (Vec<u8>, Vec<u8>) {
    let data = generate_compressible_bytes(len, 42);
    let items = vec![ZipInputItem {
        rel_path: "payload.bin".to_string(),
        data: data.clone(),
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o644,
        is_directory: false,
    }];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 5, 2).expect("create 7z bytes");
    (archive_bytes, data)
}

#[test]
fn test_truncated_corrupted_overrun_and_cancellation_safety() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (archive_bytes, raw_data) = build_single_large_solid_archive(CHUNK_SIZE * 2 + 65_536);
    let info = parse_7z_metadata(&archive_bytes, None).expect("parse metadata");
    let folder = &info.folders[0];
    let default_opts = TTZipExtractOptions::default();

    // 1. Corrupted compressed payload bytes must fail and leave no destination file
    let mut corrupted = archive_bytes.clone();
    let mid_payload = folder.packed_offset + folder.packed_len / 2;
    for b in &mut corrupted[mid_payload..mid_payload + 32] {
        *b ^= 0xFF;
    }
    let corrupt_dest = tmp.path().join("corrupt_dest");
    fs::create_dir_all(&corrupt_dest).expect("mkdir");
    let corrupt_arch = SevenZArchive::open_slice(&corrupted).expect("open corrupted payload");
    let res = corrupt_arch.extract_all_with_sheet(&corrupt_dest, &default_opts, None);
    assert!(res.is_err(), "corrupted payload must fail extraction");
    assert!(!corrupt_dest.join("payload.bin").exists());

    // 2. Truncated packed payload must fail
    let mut trunc_info = info.clone();
    trunc_info.folders[0].packed_len = folder.packed_len / 2;
    let res_trunc = decode_7z_folder_streaming(&archive_bytes, &trunc_info, 0, None, 1, |_| Ok(()));
    assert_eq!(res_trunc, Err(TTZipStatus::ErrCorruptHeader));

    // 3. Output size overrun (declared unpack size smaller than actual decompressed stream)
    let mut overrun_info = info.clone();
    let smaller_size = (raw_data.len() - 1024) as u64;
    overrun_info.folders[0].unpack_sizes = vec![smaller_size];
    overrun_info.stream_sizes = vec![smaller_size];
    overrun_info.stream_crcs = vec![crc32_fast(0, &raw_data[..smaller_size as usize])];
    let overrun_seek = SevenZSeekIndex::build(&overrun_info);
    let res_overrun =
        decode_7z_folder_streaming(&archive_bytes, &overrun_info, 0, None, 1, |_| Ok(()));
    assert_eq!(res_overrun, Err(TTZipStatus::ErrCorruptHeader));

    let mut cursor = Cursor::new(&archive_bytes);
    let overrun_extract =
        extract_7z_single_entry_from_reader(&mut cursor, &overrun_info, &overrun_seek, 0, None);
    assert_eq!(overrun_extract, Err(TTZipStatus::ErrCorruptHeader));

    // 4. Pre-cancelled sheet & mid-stream progress callback cancellation leave no committed file
    let cancel_dest = tmp.path().join("cancel_dest");
    fs::create_dir_all(&cancel_dest).expect("mkdir");
    let sheet = ProgressAtomicSheet::with_total(raw_data.len() as u64);
    sheet.cancel();
    let valid_arch = SevenZArchive::open_slice(&archive_bytes).expect("open valid");
    let cancel_res = valid_arch.extract_all_with_sheet(&cancel_dest, &default_opts, Some(&sheet));
    assert_eq!(cancel_res.err(), Some(TTZipStatus::Cancelled));
    assert!(!cancel_dest.join("payload.bin").exists());

    unsafe extern "C" fn cancel_after_first_chunk(
        processed: u64,
        _total: u64,
        _entry: *const std::os::raw::c_char,
        _ctx: *mut std::ffi::c_void,
    ) -> bool {
        processed < CHUNK_SIZE as u64
    }
    let mid_cancel_dest = tmp.path().join("mid_cancel_dest");
    fs::create_dir_all(&mid_cancel_dest).expect("mkdir");
    let mid_opts = TTZipExtractOptions {
        progress_callback: Some(cancel_after_first_chunk),
        ..Default::default()
    };
    let mid_res = valid_arch.extract_all_with_sheet(&mid_cancel_dest, &mid_opts, None);
    assert_eq!(mid_res.err(), Some(TTZipStatus::Cancelled));
    assert!(!mid_cancel_dest.join("payload.bin").exists());
}

#[test]
fn test_encrypted_archive_sink_fault_injection_preserves_error_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pass = "R1-FaultInject-Key#2026";
    let wrong_pass = "Wrong-Key#2026";
    let payload = generate_compressible_bytes(CHUNK_SIZE * 2 + 4096, 0x99AA);

    let enc_path = create_external_7z_archive(
        &tmp.path().join("enc_fault"),
        "enc_fault.7z",
        &[("secret_large.bin", &payload)],
        &["-m0=lzma2", "-mx=5", &format!("-p{}", pass)],
    );
    let enc_bytes = fs::read(&enc_path).expect("read enc_fault.7z");
    let archive = SevenZArchive::open_slice_with_password(&enc_bytes, Some(pass))
        .expect("open encrypted archive");
    let info = archive.info();

    // 1. Sink returning ErrExtractionFailed, ErrSecurityViolation, Cancelled, ErrOutOfMemory,
    //    and ErrSolidBudgetExceeded on an AES-encrypted archive must preserve original status.
    let injected_statuses = [
        TTZipStatus::ErrExtractionFailed,
        TTZipStatus::ErrSecurityViolation,
        TTZipStatus::Cancelled,
        TTZipStatus::ErrOutOfMemory,
        TTZipStatus::ErrSolidBudgetExceeded,
    ];
    for &expected_status in &injected_statuses {
        let res_folder =
            decode_7z_folder_streaming(&enc_bytes, info, 0, Some(pass), 2, |_chunk| {
                Err(expected_status)
            });
        assert_eq!(res_folder, Err(expected_status));

        let mut seen_bytes = 0usize;
        let res_solid_second_chunk =
            decode_7z_solid_streaming(&enc_bytes, info, Some(pass), 2, |chunk| {
                seen_bytes += chunk.len();
                if seen_bytes > CHUNK_SIZE {
                    Err(expected_status)
                } else {
                    Ok(())
                }
            });
        assert_eq!(res_solid_second_chunk, Err(expected_status));

        let extractor = archive.solid_extractor().with_threads(2);
        let res_extractor =
            extractor.extract_streaming(0, Some(pass), |_chunk| Err(expected_status));
        assert_eq!(res_extractor.err(), Some(expected_status));
    }

    // 2. Reader-based streaming extraction on encrypted archive with read-only / unwritable target
    //    or path traversal metadata must not misreport as ErrInvalidPassword.
    let mut traversal_info = info.clone();
    traversal_info.files[0].rel_path = "../../escape_encrypted.bin".to_string();
    let c_pass = CString::new(pass).expect("cstring");
    let opts = make_extract_opts(2, Some(&c_pass));
    let sec_dest = tmp.path().join("sec_dest");
    fs::create_dir_all(&sec_dest).expect("mkdir");
    let mut cursor = Cursor::new(&enc_bytes);
    let sec_res = extract_7z_from_reader(&mut cursor, &traversal_info, &sec_dest, &opts, None);
    assert_eq!(sec_res, Err(TTZipStatus::ErrSecurityViolation));

    // 3. Wrong password on the same encrypted archive must still return ErrInvalidPassword.
    let bad_folder = decode_7z_folder_streaming(&enc_bytes, info, 0, Some(wrong_pass), 2, |_| Ok(()));
    assert_eq!(bad_folder, Err(TTZipStatus::ErrInvalidPassword));

    let bad_c_pass = CString::new(wrong_pass).expect("cstring");
    let bad_opts = make_extract_opts(2, Some(&bad_c_pass));
    let bad_dest = tmp.path().join("bad_pass_dest");
    fs::create_dir_all(&bad_dest).expect("mkdir");
    let bad_extract = archive.extract_all_with_sheet(&bad_dest, &bad_opts, None);
    assert_eq!(bad_extract.err(), Some(TTZipStatus::ErrInvalidPassword));
    assert!(!bad_dest.join("secret_large.bin").exists());

    let mut bad_cursor = Cursor::new(&enc_bytes);
    let bad_reader = extract_7z_from_reader(&mut bad_cursor, info, &bad_dest, &bad_opts, None);
    assert_eq!(bad_reader, Err(TTZipStatus::ErrInvalidPassword));
}
