// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R1 Large Solid Block Entrypoint Verification Tests.
//!
//! Validates:
//! 1. Folder unpack size matrix: 60 MiB (< 64 MiB), 64 MiB (= 64 MiB), 70 MiB (> 64 MiB), and 256 MiB.
//! 2. Single large files vs multi-file solid blocks (7 x 10 MiB in 1 solid folder).
//! 3. Control non-solid archives (2 x 50 MiB in separate folders).
//! 4. Encrypted archives (AES-256-CBC + LZMA2, 70 MiB).
//! 5. Verification of entry points:
//!    - Reader streaming: `extract_7z_from_reader`
//!    - Memory-mapped disk extraction: `SevenZArchive::extract_all_with_sheet`
//!    - Unified orchestrator: `extract_archive_with_metrics`
//!    - Single entry in-memory: `SevenZArchive::extract_entry_bytes_stream`,
//!      `extract_7z_single_entry_from_reader`, `extract_single_entry_memory`
//!    - Whole solid in-memory: `decode_7z_solid_payload` (64 MiB bound enforcement)

use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use ttzip_engine::archive::unified::{
    extract_archive_with_metrics, extract_single_entry_memory,
};
use ttzip_engine::fs::collision::staging::StagingRoot;
use ttzip_engine::sevenz::decoder::archive::{
    extract_7z_from_reader, extract_7z_single_entry_from_reader, SevenZArchive,
};
use ttzip_engine::sevenz::decoder::mach_rss::get_current_rss_bytes;
use ttzip_engine::sevenz::decoder::payload::decode_7z_solid_payload;
use ttzip_engine::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn sha256_file(path: &Path) -> String {
    let mut f = File::open(path).expect("open file for sha256");
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).expect("read chunk");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    hex::encode(hasher.finalize())
}

fn find_external_7z() -> PathBuf {
    let candidates = [
        "/opt/homebrew/bin/7z",
        "/opt/homebrew/bin/7zz",
        "/usr/local/bin/7z",
        "/usr/bin/7z",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return p;
        }
    }
    panic!("external 7z binary is required for R1 solid tests");
}

fn generate_compressible_data(len: usize, pattern_id: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(len);
    let line = format!(
        "TTZIP-R1-SOLID-BLOCK-PATTERN-{:02x}-REPETITIVE-PAYLOAD-FOR-COMPRESSION-TESTING-0123456789\n",
        pattern_id
    );
    let bytes = line.as_bytes();
    while buf.len() < len {
        let rem = len - buf.len();
        buf.extend_from_slice(&bytes[..rem.min(bytes.len())]);
    }
    buf
}

fn create_7z_sample(
    work_dir: &Path,
    archive_name: &str,
    files: &[(&str, &[u8])],
    extra_args: &[&str],
) -> PathBuf {
    let bin = find_external_7z();
    let src_dir = work_dir.join(format!("{}_src", archive_name));
    fs::create_dir_all(&src_dir).expect("create src dir");
    let mut file_names = Vec::with_capacity(files.len());
    for (name, content) in files {
        let full = src_dir.join(name);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&full, content).expect("write sample file");
        file_names.push(*name);
    }
    let arc_path = work_dir.join(archive_name);
    let mut cmd = Command::new(bin);
    cmd.current_dir(&src_dir).arg("a").arg("-y").arg(&arc_path);
    for arg in extra_args {
        cmd.arg(arg);
    }
    for name in file_names {
        cmd.arg(name);
    }
    let out = cmd.output().expect("spawn 7z");
    assert!(
        out.status.success(),
        "7z create failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    arc_path
}

fn make_opts(threads: u32, pwd: Option<&CString>) -> TTZipExtractOptions {
    let mut opts = TTZipExtractOptions {
        thread_budget: threads,
        ..Default::default()
    };
    if let Some(c) = pwd {
        opts.password = c.as_ptr();
    }
    opts
}

#[test]
fn test_01_large_solid_60m_under_threshold() {
    let temp = tempfile::tempdir().expect("tempdir");
    let size_60m = 60 * 1024 * 1024;
    let data_60m = generate_compressible_data(size_60m, 0x60);
    let exp_hash = sha256_hex(&data_60m);

    let arc = create_7z_sample(
        temp.path(),
        "solid_60m.7z",
        &[("file_60m.bin", &data_60m)],
        &["-mx=1", "-m0=lzma2", "-ms=on"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let opts = make_opts(2, None);

    // 1. extract_7z_from_reader
    let dest_reader = temp.path().join("dest_reader");
    let mut reader = Cursor::new(&arc_bytes);
    let sevenz = SevenZArchive::open_slice(&arc_bytes).expect("open slice");
    let bytes_out =
        extract_7z_from_reader(&mut reader, sevenz.info(), &dest_reader, &opts, None)
            .expect("reader extract");
    assert_eq!(bytes_out, size_60m as u64);
    assert_eq!(sha256_file(&dest_reader.join("file_60m.bin")), exp_hash);

    // 2. SevenZArchive::extract_all_with_sheet
    let dest_mmap = temp.path().join("dest_mmap");
    let rep = sevenz
        .extract_all_with_sheet(&dest_mmap, &opts, None)
        .expect("extract_all");
    assert_eq!(rep.total_uncompressed_bytes, size_60m as u64);
    assert_eq!(sha256_file(&dest_mmap.join("file_60m.bin")), exp_hash);

    // 3. extract_archive_with_metrics
    let dest_unified = temp.path().join("dest_unified");
    let staging = StagingRoot(dest_unified.clone());
    let u_bytes =
        extract_archive_with_metrics(&arc, &staging, &opts).expect("unified extract");
    assert_eq!(u_bytes, size_60m as u64);
    assert_eq!(sha256_file(&dest_unified.join("file_60m.bin")), exp_hash);

    // 4. In-memory single entry (<= 64 MiB must succeed)
    let mem_entry = sevenz
        .extract_entry_bytes_stream(0, None)
        .expect("mem entry");
    assert_eq!(mem_entry.len(), size_60m);
    assert_eq!(sha256_hex(&mem_entry), exp_hash);

    // 5. In-memory single entry via reader (<= 64 MiB must succeed)
    let mut reader2 = Cursor::new(&arc_bytes);
    let mem_entry_reader = extract_7z_single_entry_from_reader(
        &mut reader2,
        sevenz.info(),
        sevenz.seek_index(),
        0,
        None,
    )
    .expect("reader mem entry");
    assert_eq!(sha256_hex(&mem_entry_reader), exp_hash);

    // 6. decode_7z_solid_payload (<= 64 MiB must succeed)
    let payload = decode_7z_solid_payload(&arc_bytes, sevenz.info(), None, 2)
        .expect("solid payload");
    assert_eq!(payload.len(), size_60m);
}

#[test]
fn test_02_large_solid_64m_exact_boundary() {
    let temp = tempfile::tempdir().expect("tempdir");
    let size_64m = 64 * 1024 * 1024;
    let data_64m = generate_compressible_data(size_64m, 0x64);
    let exp_hash = sha256_hex(&data_64m);

    let arc = create_7z_sample(
        temp.path(),
        "solid_64m.7z",
        &[("file_64m.bin", &data_64m)],
        &["-mx=1", "-m0=lzma2", "-ms=on"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let opts = make_opts(2, None);

    let sevenz = SevenZArchive::open_slice(&arc_bytes).expect("open slice");
    let dest = temp.path().join("dest_64m");
    let rep = sevenz
        .extract_all_with_sheet(&dest, &opts, None)
        .expect("extract_all at 64MB");
    assert_eq!(rep.total_uncompressed_bytes, size_64m as u64);
    assert_eq!(sha256_file(&dest.join("file_64m.bin")), exp_hash);

    // In-memory single entry at exact 64 MiB must succeed
    let mem_entry = sevenz
        .extract_entry_bytes_stream(0, None)
        .expect("mem entry at 64MB");
    assert_eq!(mem_entry.len(), size_64m);
    assert_eq!(sha256_hex(&mem_entry), exp_hash);

    // decode_7z_solid_payload at exact 64 MiB must succeed
    let payload = decode_7z_solid_payload(&arc_bytes, sevenz.info(), None, 2)
        .expect("payload at 64MB");
    assert_eq!(payload.len(), size_64m);
}

#[test]
fn test_03_large_solid_70m_over_threshold_boundaries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let size_70m = 70 * 1024 * 1024;
    let data_70m = generate_compressible_data(size_70m, 0x70);
    let exp_hash = sha256_hex(&data_70m);

    let arc = create_7z_sample(
        temp.path(),
        "solid_70m.7z",
        &[("file_70m.bin", &data_70m)],
        &["-mx=1", "-m0=lzma2", "-ms=on"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let opts = make_opts(2, None);

    // 1. Reader streaming must succeed to disk
    let dest_reader = temp.path().join("dest_reader_70m");
    let mut reader = Cursor::new(&arc_bytes);
    let sevenz = SevenZArchive::open_slice(&arc_bytes).expect("open slice");
    let bytes_out =
        extract_7z_from_reader(&mut reader, sevenz.info(), &dest_reader, &opts, None)
            .expect("reader extract 70MB");
    assert_eq!(bytes_out, size_70m as u64);
    assert_eq!(sha256_file(&dest_reader.join("file_70m.bin")), exp_hash);

    // 2. Disk extraction via SevenZArchive must succeed to disk
    let dest_mmap = temp.path().join("dest_mmap_70m");
    let rep = sevenz
        .extract_all_with_sheet(&dest_mmap, &opts, None)
        .expect("extract_all 70MB");
    assert_eq!(rep.total_uncompressed_bytes, size_70m as u64);
    assert_eq!(sha256_file(&dest_mmap.join("file_70m.bin")), exp_hash);

    // 3. Unified extract must succeed to disk
    let dest_unified = temp.path().join("dest_unified_70m");
    let staging = StagingRoot(dest_unified.clone());
    let u_bytes =
        extract_archive_with_metrics(&arc, &staging, &opts).expect("unified extract 70MB");
    assert_eq!(u_bytes, size_70m as u64);
    assert_eq!(sha256_file(&dest_unified.join("file_70m.bin")), exp_hash);

    // 4. In-memory single entry > 64 MiB must be REJECTED with ErrOutOfMemory
    let res_mem = sevenz.extract_entry_bytes_stream(0, None);
    assert_eq!(res_mem, Err(TTZipStatus::ErrOutOfMemory));

    let mut reader3 = Cursor::new(&arc_bytes);
    let res_reader_mem = extract_7z_single_entry_from_reader(
        &mut reader3,
        sevenz.info(),
        sevenz.seek_index(),
        0,
        None,
    );
    assert_eq!(res_reader_mem, Err(TTZipStatus::ErrOutOfMemory));

    let res_unified_mem = extract_single_entry_memory(&arc, Some("file_70m.bin"), -1, None);
    assert_eq!(res_unified_mem, Err(TTZipStatus::ErrOutOfMemory));

    // 5. decode_7z_solid_payload > 64 MiB must be REJECTED with ErrOutOfMemory
    let res_payload = decode_7z_solid_payload(&arc_bytes, sevenz.info(), None, 2);
    assert_eq!(res_payload, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_04_large_solid_256m_streaming_and_rss() {
    let temp = tempfile::tempdir().expect("tempdir");
    let size_256m = 256 * 1024 * 1024;
    let data_256m = generate_compressible_data(size_256m, 0x25);
    let exp_hash = sha256_hex(&data_256m);

    let arc = create_7z_sample(
        temp.path(),
        "solid_256m.7z",
        &[("file_256m.bin", &data_256m)],
        &["-mx=1", "-m0=lzma2", "-ms=on"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let opts = make_opts(2, None);

    let rss_before = get_current_rss_bytes();

    let sevenz = SevenZArchive::open_slice(&arc_bytes).expect("open slice");
    let dest = temp.path().join("dest_256m");
    let rep = sevenz
        .extract_all_with_sheet(&dest, &opts, None)
        .expect("extract_all 256MB");
    assert_eq!(rep.total_uncompressed_bytes, size_256m as u64);
    assert_eq!(sha256_file(&dest.join("file_256m.bin")), exp_hash);

    let rss_after = get_current_rss_bytes();
    eprintln!(
        "256MB extract RSS delta: before={}MB, after={}MB",
        rss_before / (1024 * 1024),
        rss_after / (1024 * 1024)
    );

    // Single in-memory return must reject 256 MiB
    assert_eq!(
        sevenz.extract_entry_bytes_stream(0, None),
        Err(TTZipStatus::ErrOutOfMemory)
    );
    assert_eq!(
        extract_single_entry_memory(&arc, Some("file_256m.bin"), -1, None),
        Err(TTZipStatus::ErrOutOfMemory)
    );
}

#[test]
fn test_05_multi_file_solid_7x10m_small_entry_extraction() {
    let temp = tempfile::tempdir().expect("tempdir");
    let file_size = 10 * 1024 * 1024;
    let num_files = 7;
    let mut files_data = Vec::with_capacity(num_files);
    let mut hashes = Vec::with_capacity(num_files);

    for i in 0..num_files {
        let content = generate_compressible_data(file_size, i as u8);
        hashes.push(sha256_hex(&content));
        files_data.push((format!("part_{}.bin", i), content));
    }

    let files_ref: Vec<(&str, &[u8])> = files_data
        .iter()
        .map(|(n, c)| (n.as_str(), c.as_slice()))
        .collect();

    let arc = create_7z_sample(
        temp.path(),
        "solid_7x10m.7z",
        &files_ref,
        &["-mx=1", "-m0=lzma2", "-ms=256m"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let opts = make_opts(2, None);

    let sevenz = SevenZArchive::open_slice(&arc_bytes).expect("open slice");
    assert_eq!(sevenz.info().folders.len(), 1, "Must be single solid folder");
    let folder_unpack_total: u64 = sevenz.info().folders[0].unpack_sizes.iter().sum();
    assert!(
        folder_unpack_total >= (num_files * file_size) as u64,
        "Folder unpack size must exceed 64 MiB"
    );

    // 1. extract_all_with_sheet must extract all 7 files to disk
    let dest_all = temp.path().join("dest_all_7x10m");
    let rep = sevenz
        .extract_all_with_sheet(&dest_all, &opts, None)
        .expect("extract_all 7x10M");
    assert_eq!(rep.total_uncompressed_bytes, (num_files * file_size) as u64);
    for (i, (name, _)) in files_data.iter().enumerate() {
        assert_eq!(sha256_file(&dest_all.join(name)), hashes[i]);
    }

    // 2. In-memory single entry extraction: each file is 10 MiB (<= 64 MiB),
    // so extracting ANY file from this 70 MiB solid folder must SUCCEED!
    for (i, (name, _)) in files_data.iter().enumerate() {
        let mem_bytes = sevenz
            .extract_entry_bytes_stream(i, None)
            .unwrap_or_else(|e| panic!("extract_entry_bytes_stream {} failed: {:?}", i, e));
        assert_eq!(mem_bytes.len(), file_size);
        assert_eq!(sha256_hex(&mem_bytes), hashes[i]);

        let unified_mem = extract_single_entry_memory(&arc, Some(name.as_str()), -1, None)
            .unwrap_or_else(|e| panic!("unified mem {} failed: {:?}", name, e));
        assert_eq!(sha256_hex(&unified_mem), hashes[i]);
    }

    // 3. Whole-solid payload memory collection (70 MiB > 64 MiB) must REJECT
    let res_payload = decode_7z_solid_payload(&arc_bytes, sevenz.info(), None, 2);
    assert_eq!(res_payload, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_06_control_2x50m_nonsolid_folders() {
    let temp = tempfile::tempdir().expect("tempdir");
    let file_size = 50 * 1024 * 1024;
    let data_a = generate_compressible_data(file_size, 0xA1);
    let data_b = generate_compressible_data(file_size, 0xB2);
    let hash_a = sha256_hex(&data_a);
    let hash_b = sha256_hex(&data_b);

    // Non-solid archive (-ms=off): 2 folders, each 50 MiB (< 64 MiB)
    let arc = create_7z_sample(
        temp.path(),
        "nonsolid_2x50m.7z",
        &[("part_a.bin", &data_a), ("part_b.bin", &data_b)],
        &["-mx=1", "-m0=lzma2", "-ms=off"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let opts = make_opts(2, None);

    let sevenz = SevenZArchive::open_slice(&arc_bytes).expect("open slice");
    assert_eq!(sevenz.info().folders.len(), 2, "Must be 2 separate folders");

    let dest = temp.path().join("dest_nonsolid");
    let rep = sevenz
        .extract_all_with_sheet(&dest, &opts, None)
        .expect("extract nonsolid");
    assert_eq!(rep.total_uncompressed_bytes, (file_size * 2) as u64);
    assert_eq!(sha256_file(&dest.join("part_a.bin")), hash_a);
    assert_eq!(sha256_file(&dest.join("part_b.bin")), hash_b);

    let mem_a = sevenz.extract_entry_bytes_stream(0, None).expect("mem part_a");
    assert_eq!(sha256_hex(&mem_a), hash_a);
    let mem_b = sevenz.extract_entry_bytes_stream(1, None).expect("mem part_b");
    assert_eq!(sha256_hex(&mem_b), hash_b);
}

#[test]
fn test_07_large_solid_70m_encrypted_aes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let size_70m = 70 * 1024 * 1024;
    let data_70m = generate_compressible_data(size_70m, 0xEE);
    let exp_hash = sha256_hex(&data_70m);

    let arc = create_7z_sample(
        temp.path(),
        "solid_70m_aes.7z",
        &[("secret_70m.bin", &data_70m)],
        &["-mx=1", "-m0=lzma2", "-ms=on", "-pSecretPass99!", "-mhe=off"],
    );
    let arc_bytes = fs::read(&arc).expect("read archive");
    let c_pass = CString::new("SecretPass99!").unwrap();
    let opts_good = make_opts(2, Some(&c_pass));

    // 1. Correct password -> disk extraction succeeds
    let dest_good = temp.path().join("dest_encrypted_good");
    let sevenz_good =
        SevenZArchive::open_slice_with_password(&arc_bytes, Some("SecretPass99!"))
            .expect("open encrypted");
    let rep = sevenz_good
        .extract_all_with_sheet(&dest_good, &opts_good, None)
        .expect("extract encrypted 70M");
    assert_eq!(rep.total_uncompressed_bytes, size_70m as u64);
    assert_eq!(sha256_file(&dest_good.join("secret_70m.bin")), exp_hash);

    // 2. Wrong password -> ErrInvalidPassword
    let c_bad = CString::new("WrongPassword!").unwrap();
    let opts_bad = make_opts(2, Some(&c_bad));
    let dest_bad = temp.path().join("dest_encrypted_bad");
    let sevenz_bad =
        SevenZArchive::open_slice_with_password(&arc_bytes, Some("WrongPassword!"))
            .expect("open encrypted with wrong pwd");
    let res_bad = sevenz_bad.extract_all_with_sheet(&dest_bad, &opts_bad, None);
    assert_eq!(res_bad.err(), Some(TTZipStatus::ErrInvalidPassword));

    // 3. Mid-stream cancellation -> Cancelled
    let dest_cancel = temp.path().join("dest_encrypted_cancel");
    let sheet = ProgressAtomicSheet::with_total(size_70m as u64);
    sheet.cancel();
    let res_cancel =
        sevenz_good.extract_all_with_sheet(&dest_cancel, &opts_good, Some(&sheet));
    assert_eq!(res_cancel.err(), Some(TTZipStatus::Cancelled));
}
