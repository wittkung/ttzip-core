// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! MM15 — 7z / RAR / multi-volume behaviour consistency across production entry points.
//!
//! Deliberately *not* a re-run of R1 / R2 / `phase2_split_volume_integration_tests`:
//! 1. Head / middle / tail reads in a large solid block, verified per entry point by
//!    output path + length + SHA-256 (not by exit code or entry count).
//! 2. The two independent axes of the 64 MiB guard — *entry size* vs *preceding solid
//!    offset* — surfacing as `ErrOutOfMemory` vs `ErrSolidBudgetExceeded`.
//! 3. Content vs header encryption, with the wrong / missing / correct password matrix.
//! 4. Multi-volume chains: complete, missing middle, missing last, out-of-order seeds.
//! 5. Truncated and corrupted payloads must never report success.
//! 6. RAR only via the repository's existing fixtures; the fixtures are **required**, so a
//!    missing fixture fails loudly instead of reporting a pass.
//!
//! Fixture budget (serialised by `HEAVY_FIXTURE_LOCK`; run this file exclusively): 3 x 33 MiB
//! solid, 64 MiB + (64 MiB + 1) solid, 3 x 2 MiB split volumes, 2 x 256 KiB encrypted.
//! Peak temporary disk <= 300 MiB, peak RSS per fixture build <= 200 MiB.

use std::ffi::CString;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use tempfile::tempdir;

use ttzip_engine::archive::split::detect_volume_chain;
use ttzip_engine::archive::unified::selection::EntrySelection;
use ttzip_engine::archive::unified::{
    extract_archive_with_metrics, extract_selected_entries, extract_single_entry_memory,
};
use ttzip_engine::fs::collision::staging::StagingRoot;
use ttzip_engine::sevenz::decoder::archive::{
    extract_7z_from_reader, extract_7z_single_entry_from_reader as reader_entry, SevenZArchive,
};
use ttzip_engine::sevenz::decoder::payload::{
    decode_7z_solid_payload, extract_single_entry_bounded as bounded_entry,
};
use ttzip_engine::sevenz::writer::{
    create_7z_archive_explicit_split,
    create_7z_archive_explicit_split_with_header_encryption as create_encrypted_7z,
};
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, SevenZSeekIndex};
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions, TTZipStatus,
};
use ttzip_engine::uniffi_api::archive::{
    detect_archive_format, extract_single_entry_stream,
    extract_single_entry_stream_guarded as guarded_stream,
};
use ttzip_engine::uniffi_api::extraction::extract_single_entry_by_path as entry_by_path;
use ttzip_engine::uniffi_api::types::{ArchiveFormat as UniFFIArchiveFormat, TTZipError};
use ttzip_engine::zip::writer::ZipInputItem;

const MIB: usize = 1024 * 1024;
const LIMIT_64_MIB: u64 = 64 * 1024 * 1024;
const SOLID_ENTRY_SIZE: usize = 33 * MIB;
const RAR_A_TXT_SHA256: &str = "02dc86d8b326a1cd07526f75b66bb7207c43376b21d9ac2c20bfedf510898861";
const RAR_A_TXT_LEN: u64 = 18;
const RAR4_FIXTURE: &str = "test_read_format_rar4_encrypted.rar";
const RAR5_FIXTURE: &str = "test_read_format_rar5_encrypted.rar";

/// Serialises the heavy fixture scenarios inside this binary. The Bazel target still has to
/// be scheduled exclusively because the fixtures are shared with other integration binaries.
static HEAVY_FIXTURE_LOCK: Mutex<()> = Mutex::new(());

/// Acquires the heavy-fixture lock, recovering from a poisoned mutex so that one failing
/// scenario does not cascade into the remaining ones.
fn heavy_lock() -> std::sync::MutexGuard<'static, ()> {
    HEAVY_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Asserts that a memory entry point refused the request with `ErrOutOfMemory`.
fn assert_out_of_memory(result: Result<Vec<u8>, TTZipStatus>) {
    assert_eq!(result, Err(TTZipStatus::ErrOutOfMemory));
}

/// Asserts that a UniFFI preview failure carries one of the accepted rejection shapes.
fn assert_uniffi_rejected(err: &TTZipError) {
    assert!(
        matches!(
            err,
            TTZipError::EngineError { .. }
                | TTZipError::OutOfMemory { .. }
                | TTZipError::DecompressionBombDetected { .. }
        ),
        "unexpected UniFFI rejection shape: {err:?}"
    );
}

// --- generic helpers -------------------------------------------------------

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn sha256_file(path: &Path) -> (u64, String) {
    let mut file = fs::File::open(path).expect("open file for sha256");
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).expect("read chunk");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    (total, hex::encode(hasher.finalize()))
}

/// Deterministic, highly compressible payload: keeps the compressed fixture tiny while the
/// uncompressed size stays exactly at the requested byte count.
fn make_payload(len: usize, seed: u8) -> Vec<u8> {
    let line = format!("TTZIP-MM15-SOLID-{seed:02X}-REPEAT-PAYLOAD-BLOCK-0123456789\n");
    let bytes = line.as_bytes();
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let rem = len - out.len();
        out.extend_from_slice(&bytes[..rem.min(bytes.len())]);
    }
    out
}

/// Deterministic pseudo-random payload: keeps a compressed fixture large enough to be split
/// into several physical volumes.
fn make_incompressible_payload(len: usize, seed: u64) -> Vec<u8> {
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

fn make_opts(threads: u32, password: Option<&CString>) -> TTZipExtractOptions {
    let mut opts = TTZipExtractOptions {
        thread_budget: threads,
        ..Default::default()
    };
    if let Some(c) = password {
        opts.password = c.as_ptr();
    }
    opts
}

// --- fixture model and builders --------------------------------------------

#[derive(Clone)]
struct EntryExpect {
    name: String,
    len: u64,
    sha256: String,
}

struct SolidFixture {
    archive: PathBuf,
    entries: Vec<EntryExpect>,
}

fn solid_items(sizes: &[usize], seed: u8) -> (Vec<ZipInputItem>, Vec<EntryExpect>) {
    let mut items = Vec::with_capacity(sizes.len());
    let mut expects = Vec::with_capacity(sizes.len());
    for (index, &size) in sizes.iter().enumerate() {
        let data = make_payload(size, seed.wrapping_add(index as u8));
        let name = format!("entry_{index}.bin");
        expects.push(EntryExpect {
            name: name.clone(),
            len: size as u64,
            sha256: sha256_hex(&data),
        });
        items.push(ZipInputItem {
            rel_path: name,
            data,
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o644,
            is_directory: false,
        });
    }
    (items, expects)
}

/// Builds a single-folder solid 7z archive from in-memory payloads.
fn build_solid_fixture(dir: &Path, name: &str, sizes: &[usize], seed: u8) -> SolidFixture {
    let (items, entries) = solid_items(sizes, seed);
    let bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create solid 7z archive");
    let archive = dir.join(name);
    fs::write(&archive, &bytes).expect("write solid 7z archive");
    SolidFixture { archive, entries }
}
fn write_source_tree(
    dir: &Path,
    tag: &str,
    sizes: &[usize],
    split: bool,
) -> Vec<(PathBuf, String)> {
    let src_dir = dir.join(format!("{tag}_src"));
    fs::create_dir_all(&src_dir).expect("create source dir");
    let mut entries = Vec::with_capacity(sizes.len());
    for (index, &size) in sizes.iter().enumerate() {
        let data = if split {
            make_incompressible_payload(size, 0xA11CE + index as u64)
        } else {
            make_payload(size, 0xB0 + index as u8)
        };
        let name = format!("payload_{index}.bin");
        fs::write(src_dir.join(&name), &data).expect("write source payload");
        entries.push((src_dir.join(&name), name));
    }
    entries
}

fn create_options(password: Option<&CString>) -> TTZipCreateOptions {
    TTZipCreateOptions {
        format: TTZipArchiveFormat::SevenZip,
        level: TTZipCompressionLevel::Fast,
        encryption: if password.is_some() {
            TTZipEncryptionMethod::Aes256
        } else {
            TTZipEncryptionMethod::None
        },
        password: password.map_or(std::ptr::null(), |c| c.as_ptr()),
        thread_budget: 1,
        ..Default::default()
    }
}

/// Builds a 7z archive whose *payload* is AES encrypted but whose *header* stays plain.
fn build_content_encrypted_7z(dir: &Path, name: &str, password: &str) -> PathBuf {
    let entries = write_source_tree(dir, name, &[256 * 1024, 256 * 1024], false);
    let pwd = CString::new(password).expect("cstring");
    let options = create_options(Some(&pwd));
    let dest = dir.join(name);
    create_encrypted_7z(&dest, &entries, &options, 0, Some(false))
        .expect("create content-encrypted 7z");
    dest
}

/// Builds a 7z archive whose *header* and *payload* are both AES encrypted.
fn build_header_encrypted_7z(dir: &Path, name: &str, password: &str) -> PathBuf {
    let entries = write_source_tree(dir, name, &[256 * 1024, 256 * 1024], false);
    let pwd = CString::new(password).expect("cstring");
    let options = create_options(Some(&pwd));
    let dest = dir.join(name);
    create_encrypted_7z(&dest, &entries, &options, 0, Some(true))
        .expect("create header-encrypted 7z");
    dest
}

/// Builds a real multi-volume 7z archive through the production split writer.
fn build_split_7z(dir: &Path, name: &str, volume_size: u64) -> (PathBuf, Vec<EntryExpect>) {
    let entries = write_source_tree(dir, name, &[2 * MIB, 2 * MIB, 2 * MIB], true);
    let mut expects = Vec::with_capacity(entries.len());
    for (path, name) in &entries {
        let (len, sha) = sha256_file(path);
        expects.push(EntryExpect {
            name: name.clone(),
            len,
            sha256: sha,
        });
    }
    let options = create_options(None);
    let dest = dir.join(name);
    create_7z_archive_explicit_split(&dest, &entries, &options, volume_size)
        .expect("create split 7z archive");
    (dest, expects)
}

// --- assertion helpers -----------------------------------------------------

fn assert_file_matches(path: &Path, expected: &EntryExpect) {
    assert!(path.exists(), "missing output file: {path:?}");
    let (len, sha) = sha256_file(path);
    assert_eq!(len, expected.len, "length mismatch for {}", path.display());
    assert_eq!(sha, expected.sha256, "sha mismatch: {path:?}");
}

fn assert_dir_matches(dir: &Path, expects: &[EntryExpect]) {
    for expected in expects {
        assert_file_matches(&dir.join(&expected.name), expected);
    }
}

fn staging_root(dir: &Path, name: &str) -> StagingRoot {
    let path = dir.join(name);
    fs::create_dir_all(&path).expect("create staging dir");
    StagingRoot(path)
}

fn total_expected(fixture: &SolidFixture) -> u64 {
    fixture.entries.iter().map(|e| e.len).sum()
}

// --- 1. head / middle / tail reads inside a large solid block ---------------

fn assert_solid_disk_entrypoints(fixture: &SolidFixture) {
    let work = tempdir().expect("tempdir");
    let bytes = fs::read(&fixture.archive).expect("read archive");
    let archive = SevenZArchive::open_slice(&bytes).expect("open 7z");
    assert_eq!(archive.info().folders.len(), 1);
    let opts = make_opts(2, None);

    let dir_a = work.path().join("decoder_all");
    fs::create_dir_all(&dir_a).expect("mkdir");
    let report = archive
        .extract_all_with_sheet(&dir_a, &opts, None)
        .expect("extract_all_with_sheet");
    assert_eq!(report.total_uncompressed_bytes, total_expected(fixture));
    assert_dir_matches(&dir_a, &fixture.entries);
    let dir_b = work.path().join("decoder_reader");
    fs::create_dir_all(&dir_b).expect("mkdir");
    let mut cursor = Cursor::new(&bytes);
    let written = extract_7z_from_reader(&mut cursor, archive.info(), &dir_b, &opts, None)
        .expect("extract_7z_from_reader");
    assert_eq!(written, total_expected(fixture));
    assert_dir_matches(&dir_b, &fixture.entries);
}

fn assert_solid_unified_entrypoints(fixture: &SolidFixture) {
    let work = tempdir().expect("tempdir");
    let opts = make_opts(2, None);
    let staging = staging_root(work.path(), "unified_all");
    let total = extract_archive_with_metrics(&fixture.archive, &staging, &opts)
        .expect("unified full extraction");
    assert_eq!(total, total_expected(fixture));
    assert_dir_matches(staging.path(), &fixture.entries);
    for expected in &fixture.entries {
        let staging = staging_root(work.path(), &format!("unified_one_{}", expected.name));
        let selection = EntrySelection::new([expected.name.as_str()], false);
        let count = extract_selected_entries(&fixture.archive, &selection, &staging, &opts)
            .expect("unified selective extraction");
        assert_eq!(count, 1);
        assert_file_matches(&staging.path().join(&expected.name), expected);
    }
}

#[test]
fn test_mm15_solid_head_mid_tail_readable_through_disk_entrypoints() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let fixture = build_solid_fixture(
        work.path(),
        "solid_head_mid_tail.7z",
        &[SOLID_ENTRY_SIZE, SOLID_ENTRY_SIZE, SOLID_ENTRY_SIZE],
        0x11,
    );
    assert_solid_disk_entrypoints(&fixture);
    assert_solid_unified_entrypoints(&fixture);
}

// --- 2. the two axes of the 64 MiB guard ------------------------------------

fn assert_memory_rejects_oversized(fixture: &SolidFixture) {
    let bytes = fs::read(&fixture.archive).expect("read archive");
    let archive = SevenZArchive::open_slice(&bytes).expect("open 7z");
    let (info, seek) = (archive.info(), archive.seek_index());
    let over_index = fixture.entries.len() - 1;
    let name = &fixture.entries[over_index].name;

    let unified = extract_single_entry_memory(&fixture.archive, Some(name.as_str()), -1, None);
    assert_out_of_memory(unified);

    let mut cursor = Cursor::new(&bytes);
    assert_out_of_memory(reader_entry(&mut cursor, info, seek, over_index, None));

    let solid = decode_7z_solid_payload(&bytes, info, None, 2);
    assert_out_of_memory(solid);

    assert_out_of_memory(bounded_entry(&bytes, info, seek, over_index, None, 0));
}

fn assert_memory_accepts_at_limit(fixture: &SolidFixture) {
    let bytes = fs::read(&fixture.archive).expect("read archive");
    let archive = SevenZArchive::open_slice(&bytes).expect("open 7z");
    let (info, seek) = (archive.info(), archive.seek_index());
    let expected = &fixture.entries[0];

    let unified =
        extract_single_entry_memory(&fixture.archive, Some(expected.name.as_str()), -1, None)
            .expect("unified memory entry point must accept exactly 64 MiB");
    assert_eq!(unified.len() as u64, expected.len);
    assert_eq!(sha256_hex(&unified), expected.sha256);

    let mut cursor = Cursor::new(&bytes);
    let streamed = reader_entry(&mut cursor, info, seek, 0, None)
        .expect("reader memory entry point must accept exactly 64 MiB");
    assert_eq!(streamed.len() as u64, expected.len);
    assert_eq!(sha256_hex(&streamed), expected.sha256);
}

fn assert_disk_accepts_oversized(fixture: &SolidFixture) {
    let work = tempdir().expect("tempdir");
    let bytes = fs::read(&fixture.archive).expect("read archive");
    let archive = SevenZArchive::open_slice(&bytes).expect("open 7z");
    let expected = fixture.entries.last().expect("tail entry");
    let opts = make_opts(2, None);

    let dir = work.path().join("disk_over");
    fs::create_dir_all(&dir).expect("mkdir");
    let report = archive
        .extract_all_with_sheet(&dir, &opts, None)
        .expect("streaming disk extraction must accept an entry above 64 MiB");
    assert_eq!(report.total_uncompressed_bytes, total_expected(fixture));
    assert_file_matches(&dir.join(&expected.name), expected);

    let staging = staging_root(work.path(), "unified_over");
    let total = extract_archive_with_metrics(&fixture.archive, &staging, &opts)
        .expect("unified streaming extraction must accept an entry above 64 MiB");
    assert_eq!(total, total_expected(fixture));
    assert_file_matches(&staging.path().join(&expected.name), expected);
}

fn assert_uniffi_preview_contract(fixture: &SolidFixture) {
    let at_limit = &fixture.entries[0];
    let over = fixture.entries.last().expect("tail entry");
    let path = fixture.archive.to_string_lossy().to_string();

    let ok = extract_single_entry_stream(path.clone(), 0, None)
        .expect("UniFFI preview must accept an entry of exactly 64 MiB");
    assert_eq!(ok.len() as u64, at_limit.len);
    assert_eq!(sha256_hex(&ok), at_limit.sha256);

    let over_index = (fixture.entries.len() - 1) as u64;
    let err = extract_single_entry_stream(path.clone(), over_index, None)
        .expect_err("UniFFI preview must reject an entry above 64 MiB");
    assert_uniffi_rejected(&err);

    let by_path = entry_by_path(path, over.name.clone(), None)
        .expect_err("UniFFI preview by path must reject an entry above 64 MiB");
    assert_uniffi_rejected(&by_path);
}
#[test]
fn test_mm15_oversized_entry_rejected_in_memory_but_accepted_on_disk() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let fixture = build_solid_fixture(
        work.path(),
        "solid_size_axis.7z",
        &[LIMIT_64_MIB as usize, LIMIT_64_MIB as usize + 1],
        0x22,
    );
    assert_memory_accepts_at_limit(&fixture);
    assert_memory_rejects_oversized(&fixture);
    assert_disk_accepts_oversized(&fixture);
    assert_uniffi_preview_contract(&fixture);
}

// --- 3. preceding-solid budget is a separate axis from the entry size -------

fn assert_preceding_budget_axis(fixture: &SolidFixture) {
    let bytes = fs::read(&fixture.archive).expect("read archive");
    let archive = SevenZArchive::open_slice(&bytes).expect("open 7z");
    let info = archive.info();
    let seek: &SevenZSeekIndex = archive.seek_index();
    let tail_index = fixture.entries.len() - 1;
    let tail = &fixture.entries[tail_index];

    // The tail entry is 33 MiB (well below 64 MiB) but starts 66 MiB into the solid block.
    let err = bounded_entry(&bytes, info, seek, tail_index, None, LIMIT_64_MIB);
    assert_eq!(
        err,
        Err(TTZipStatus::ErrSolidBudgetExceeded),
        "64 MiB budget rejects tail"
    );

    let ok = bounded_entry(&bytes, info, seek, tail_index, None, 0)
        .expect("disabling the preceding budget must allow the tail entry");
    assert_eq!(ok.len() as u64, tail.len);
    assert_eq!(sha256_hex(&ok), tail.sha256);
}

fn assert_uniffi_preceding_budget(fixture: &SolidFixture) {
    let tail_index = fixture.entries.len() - 1;
    let path = fixture.archive.to_string_lossy().to_string();
    let tail = &fixture.entries[tail_index];

    let err = guarded_stream(path.clone(), tail_index as u64, None, 64)
        .expect_err("a 64 MB preceding budget must reject the tail entry");
    assert_uniffi_rejected(&err);

    let ok = guarded_stream(path, tail_index as u64, None, 128)
        .expect("a 128 MB preceding budget must accept the tail entry");
    assert_eq!(ok.len() as u64, tail.len);
    assert_eq!(sha256_hex(&ok), tail.sha256);

    let by_path = entry_by_path(
        fixture.archive.to_string_lossy().to_string(),
        tail.name.clone(),
        None,
    );
    assert_uniffi_rejected(&by_path.expect_err("preview by path must not succeed"));
}
#[test]
fn test_mm15_preceding_solid_budget_is_independent_of_entry_size() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let fixture = build_solid_fixture(
        work.path(),
        "solid_preceding_axis.7z",
        &[SOLID_ENTRY_SIZE, SOLID_ENTRY_SIZE, SOLID_ENTRY_SIZE],
        0x33,
    );
    assert_preceding_budget_axis(&fixture);
    assert_uniffi_preceding_budget(&fixture);
}

// --- 4. content encryption versus header encryption -------------------------

fn assert_correct_password_succeeds(archive: &Path, password: &str) {
    let work = tempdir().expect("tempdir");
    let pwd = CString::new(password).expect("cstring");
    let staging = staging_root(work.path(), "out");
    let total = extract_archive_with_metrics(archive, &staging, &make_opts(2, Some(&pwd)))
        .expect("correct password must extract to disk");
    assert_eq!(total, 2 * 256 * 1024);
    let expected = make_payload(256 * 1024, 0xB0);
    let (len, sha) = sha256_file(&staging.path().join("payload_0.bin"));
    assert_eq!(len, 256 * 1024);
    assert_eq!(sha, sha256_hex(&expected));
}

fn assert_wrong_password_fails(archive: &Path) {
    let work = tempdir().expect("tempdir");
    let pwd = CString::new("WrongPassword!2026").expect("cstring");
    let staging = staging_root(work.path(), "out");
    let err = extract_archive_with_metrics(archive, &staging, &make_opts(2, Some(&pwd)))
        .expect_err("a wrong password must not extract");
    assert_eq!(err, TTZipStatus::ErrInvalidPassword);
    let mem = extract_single_entry_memory(
        archive,
        Some("payload_0.bin"),
        -1,
        Some("WrongPassword!2026"),
    )
    .expect_err("a wrong password must not produce an in-memory entry");
    assert_eq!(mem, TTZipStatus::ErrInvalidPassword);
}

fn assert_header_encryption_needs_password(archive: &Path) {
    let bytes = fs::read(archive).expect("read archive");
    let err = match SevenZArchive::open_slice(&bytes) {
        Ok(_) => panic!("a header-encrypted archive must not open without a password"),
        Err(e) => e,
    };
    assert_eq!(err, TTZipStatus::ErrInvalidPassword);
    let work = tempdir().expect("tempdir");
    let staging = staging_root(work.path(), "out");
    let err = extract_archive_with_metrics(archive, &staging, &make_opts(2, None))
        .expect_err("a header-encrypted archive must not extract without a password");
    assert_eq!(err, TTZipStatus::ErrInvalidPassword);
}

#[test]
fn test_mm15_content_encryption_password_matrix() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let archive =
        build_content_encrypted_7z(work.path(), "content_encrypted.7z", "ContentPass!2026");
    assert_correct_password_succeeds(&archive, "ContentPass!2026");
    assert_wrong_password_fails(&archive);
}

#[test]
fn test_mm15_header_encryption_password_matrix() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let archive = build_header_encrypted_7z(work.path(), "header_encrypted.7z", "HeaderPass!2026");
    assert_header_encryption_needs_password(&archive);
    assert_correct_password_succeeds(&archive, "HeaderPass!2026");
    assert_wrong_password_fails(&archive);
}

// --- 5. multi-volume chains -------------------------------------------------

/// Returns the physical volume files of the chain rooted at `base` (`<base>.001`, `.002`, ...).
///
/// Only regular files whose name is `<base>` plus exactly three digits qualify, so the
/// `<base>_src` staging directory is never mistaken for a volume.
fn volume_paths(base: &Path) -> Vec<PathBuf> {
    let parent = base.parent().expect("base parent");
    let stem = base
        .file_name()
        .expect("base name")
        .to_string_lossy()
        .to_string();
    let mut paths: Vec<PathBuf> = parent
        .read_dir()
        .expect("read dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| {
            let suffix = p
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix(&stem));
            suffix
                .and_then(|s| s.strip_prefix('.'))
                .is_some_and(|s| s.len() == 3 && s.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    paths.sort();
    paths
}

/// First physical volume of the chain rooted at `base`.
fn first_volume(base: &Path) -> PathBuf {
    let parent = base.parent().expect("base parent");
    let stem = base
        .file_name()
        .expect("base name")
        .to_string_lossy()
        .to_string();
    parent.join(format!("{stem}.001"))
}

fn assert_complete_chain_extracts(base: &Path, expects: &[EntryExpect]) {
    let work = tempdir().expect("tempdir");
    let staging = staging_root(work.path(), "out");
    let total = extract_archive_with_metrics(&first_volume(base), &staging, &make_opts(2, None))
        .expect("a complete volume chain must extract");
    assert_eq!(total, expects.iter().map(|e| e.len).sum::<u64>());
    assert_dir_matches(staging.path(), expects);
}

fn copy_volumes(volumes: &[PathBuf], holder: &Path) -> Vec<PathBuf> {
    let mut copied = Vec::with_capacity(volumes.len());
    for volume in volumes {
        let target = holder.join(volume.file_name().expect("volume name"));
        fs::copy(volume, &target).expect("copy volume");
        copied.push(target);
    }
    copied
}

fn assert_missing_volume_fails(volumes: &[PathBuf], missing_index: usize) {
    let work = tempdir().expect("tempdir");
    let holder = work.path().join("chain");
    fs::create_dir_all(&holder).expect("mkdir");
    let copied = copy_volumes(volumes, &holder);
    assert!(copied.len() > missing_index,);
    fs::remove_file(&copied[missing_index]).expect("remove volume");

    let staging = staging_root(work.path(), "out");
    let result = extract_archive_with_metrics(&copied[0], &staging, &make_opts(2, None));
    assert!(result.is_err(),);
    assert_no_output_produced(staging.path());
}

fn assert_out_of_order_seed_is_canonical(base: &Path) {
    let chain = detect_volume_chain(&first_volume(base)).expect("detect chain");
    assert!(chain.len() >= 3, "fixture must have at least three volumes");
    let mut sorted = chain.clone();
    sorted.sort();
    assert_eq!(chain, sorted,);
    let from_middle = detect_volume_chain(&chain[1]).expect("detect chain from middle volume");
    assert_eq!(from_middle, chain,);
}

#[test]
fn test_mm15_split_volume_chain_matrix() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let (base, expects) = build_split_7z(work.path(), "vol.7z", MIB as u64);
    let volumes = volume_paths(&base);
    assert!(volumes.len() >= 3, "split fixture produced too few volumes");

    assert_complete_chain_extracts(&base, &expects);
    assert_out_of_order_seed_is_canonical(&base);
    assert_missing_volume_fails(&volumes, 1);
    assert_missing_volume_fails(&volumes, volumes.len() - 1);
}

// --- 6. truncated and corrupted payloads never report success ---------------

fn assert_no_output_produced(dir: &Path) {
    let produced: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read staging")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    assert!(produced.is_empty(),);
}

#[test]
fn test_mm15_truncated_and_corrupted_archive_never_report_success() {
    let _guard = heavy_lock();
    let work = tempdir().expect("tempdir");
    let fixture = build_solid_fixture(
        work.path(),
        "truncation_target.7z",
        &[2 * MIB, 2 * MIB, 2 * MIB],
        0x44,
    );
    let bytes = fs::read(&fixture.archive).expect("read archive");

    for cut in [bytes.len() / 4, bytes.len() / 2, bytes.len() - 64] {
        let staging = staging_root(work.path(), &format!("trunc_{cut}"));
        let truncated = work.path().join(format!("trunc_{cut}.7z"));
        fs::write(&truncated, &bytes[..cut]).expect("write truncated archive");
        let result = extract_archive_with_metrics(&truncated, &staging, &make_opts(2, None));
        assert!(result.is_err(),);
        assert_no_output_produced(staging.path());
    }

    let mut corrupted = bytes.clone();
    let payload_start = 32usize;
    for byte in corrupted[payload_start..payload_start + 64].iter_mut() {
        *byte ^= 0xFF;
    }
    let staging = staging_root(work.path(), "corrupt");
    let corrupt_path = work.path().join("corrupt.7z");
    fs::write(&corrupt_path, &corrupted).expect("write corrupted archive");
    let result = extract_archive_with_metrics(&corrupt_path, &staging, &make_opts(2, None));
    assert!(result.is_err(),);
    assert_no_output_produced(staging.path());
}

// --- 7. RAR — existing repository fixtures only -----------------------------

/// Resolves the directory holding the repository's RAR fixtures.
///
/// The fixtures are **required**: when they cannot be located this panics with the probed
/// paths instead of reporting a pass. The main agent wires them into the Bazel runfiles (or
/// points `TTZIP_MM15_RAR_FIXTURE_DIR` at a controlled copy of the same directory).
fn rar_fixture_dir() -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("TTZIP_MM15_RAR_FIXTURE_DIR") {
        candidates.push(PathBuf::from(dir));
    }
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let dir = Path::new(manifest).join("../../Tests/TTZipTests/Fixtures/Encrypted");
        candidates.push(dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("../../Tests/TTZipTests/Fixtures/Encrypted"));
        candidates.push(cwd.join("Tests/TTZipTests/Fixtures/Encrypted"));
    }
    let probed: Vec<String> = candidates.iter().map(|c| c.display().to_string()).collect();
    candidates
        .into_iter()
        .find(|dir| dir.join(RAR4_FIXTURE).exists())
        .unwrap_or_else(|| {
            panic!(
                "required RAR fixture '{RAR4_FIXTURE}' was not found; probed {probed:?}. \
             Wire products/ttzip/core/Tests/TTZipTests/Fixtures/Encrypted into the test \
             runfiles or set TTZIP_MM15_RAR_FIXTURE_DIR to that directory."
            )
        })
}

fn assert_rar_is_detected(archive: &Path) {
    let detected =
        detect_archive_format(archive.to_string_lossy().to_string()).expect("detect RAR format");
    assert_eq!(detected, UniFFIArchiveFormat::Rar);
}

/// RAR read support is provided by the linked libarchive, which is compiled with its RAR
/// reader. A failure here is a regression in that path, not a capability gap.
fn assert_rar_entry_is_exact(archive: &Path) {
    let work = tempdir().expect("tempdir");
    let staging = staging_root(work.path(), "out");
    let selection = EntrySelection::new(["a.txt"], false);
    let count = extract_selected_entries(archive, &selection, &staging, &make_opts(2, None))
        .expect("RAR fixture must extract through the production entry point");
    assert_eq!(count, 1, "selecting a.txt must commit exactly one entry");
    let (len, sha) = sha256_file(&staging.path().join("a.txt"));
    assert_eq!(len, RAR_A_TXT_LEN, "RAR entry length mismatch");
    assert_eq!(sha, RAR_A_TXT_SHA256, "RAR entry sha256 mismatch");
}

#[test]
fn test_mm15_rar_existing_fixture_only() {
    let dir = rar_fixture_dir();
    for name in [RAR4_FIXTURE, RAR5_FIXTURE] {
        let archive = dir.join(name);
        assert_rar_is_detected(&archive);
        assert_rar_entry_is_exact(&archive);
    }
}
