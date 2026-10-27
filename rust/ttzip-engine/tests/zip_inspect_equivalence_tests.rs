// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Equivalence and differential tests comparing pure-Rust Central Directory inspection
//! against libarchive sequential streaming inspection across real-world ZIP fixtures.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use ttzip_engine::archive::unified::inspect::{
    inspect_archive, inspect_archive_via_libarchive, ArchiveEntryRef,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectedEntryRecord {
    pub path: String,
    pub raw_path: Vec<u8>,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
    pub detected_encoding: Option<String>,
}

impl InspectedEntryRecord {
    fn from_ref(e: &ArchiveEntryRef<'_>) -> Self {
        Self {
            path: e.path.to_string(),
            raw_path: e.raw_path.to_vec(),
            uncompressed_size: e.uncompressed_size,
            compressed_size: e.compressed_size,
            crc32: e.crc32,
            mtime_epoch_secs: e.mtime_epoch_secs,
            mode: e.mode,
            is_directory: e.is_directory,
            is_encrypted: e.is_encrypted,
            compression_method: e.compression_method,
            detected_encoding: e.detected_encoding.map(|s| s.to_string()),
        }
    }
}

fn collect_pure_rust(path: &Path) -> Vec<InspectedEntryRecord> {
    let mut entries = Vec::new();
    let res = inspect_archive(path, None, true, |e| {
        entries.push(InspectedEntryRecord::from_ref(e));
        true
    });
    assert!(res.is_ok(), "inspect_archive (pure-Rust) failed: {:?}", res.err());
    entries
}

fn collect_libarchive(path: &Path) -> Vec<InspectedEntryRecord> {
    unsafe {
        libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());
    }
    let mut entries = Vec::new();
    let res = inspect_archive_via_libarchive(path, None, true, |e| {
        entries.push(InspectedEntryRecord::from_ref(e));
        true
    });
    assert!(res.is_ok(), "inspect_archive_via_libarchive failed: {:?}", res.err());
    entries
}

use unicode_normalization::UnicodeNormalization;

/// Asserts 100% equivalence on all core structural metadata:
/// Order, path strings (NFC-canonical), directory flags, uncompressed size, mtime, POSIX mode, and encryption flag.
fn assert_structural_equivalence(rust_entries: &[InspectedEntryRecord], lib_entries: &[InspectedEntryRecord]) {
    assert_eq!(
        rust_entries.len(),
        lib_entries.len(),
        "entry count mismatch: rust={}, libarchive={}",
        rust_entries.len(),
        lib_entries.len()
    );

    for (i, (r, l)) in rust_entries.iter().zip(lib_entries.iter()).enumerate() {
        let r_nfc: String = r.path.nfc().collect();
        let l_nfc: String = l.path.nfc().collect();
        assert_eq!(r_nfc, l_nfc, "[entry {}] path string mismatch", i);
        assert_eq!(r.is_directory, l.is_directory, "[entry {}] directory flag mismatch for {}", i, r.path);
        assert_eq!(r.uncompressed_size, l.uncompressed_size, "[entry {}] uncompressed size mismatch for {}", i, r.path);
        // Both pure-Rust and libarchive interpret DOS timestamps via libc mktime(tm_isdst=-1)
        // or 0x5455 Extended Timestamp, yielding exact mtime equivalence without tolerance.
        assert_eq!(
            r.mtime_epoch_secs,
            l.mtime_epoch_secs,
            "[entry {}] mtime mismatch for {}: rust={}, libarchive={}",
            i,
            r.path,
            r.mtime_epoch_secs,
            l.mtime_epoch_secs
        );
        assert_eq!(r.mode, l.mode, "[entry {}] mode mismatch for {}", i, r.path);
        assert_eq!(r.is_encrypted, l.is_encrypted, "[entry {}] is_encrypted mismatch for {}", i, r.path);
    }
}

#[test]
fn test_equivalence_stored_deflate_and_empty_dirs() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let f1 = src_dir.join("stored_file.txt");
    fs::write(&f1, b"uncompressed raw stored content bytes 12345").unwrap();

    let f2 = src_dir.join("deflated_file.bin");
    fs::write(&f2, vec![0xAB; 8192]).unwrap();

    let empty_d1 = src_dir.join("empty_folder");
    fs::create_dir_all(&empty_d1).unwrap();

    let nested_d = src_dir.join("nested").join("sub");
    fs::create_dir_all(&nested_d).unwrap();
    fs::write(nested_d.join("inner.txt"), b"inner nested content").unwrap();

    let zip_path = dir.path().join("archive_mix.zip");

    // Create with /usr/bin/zip
    let status = Command::new("/usr/bin/zip")
        .arg("-r")
        .arg("-q")
        .arg(&zip_path)
        .arg("stored_file.txt")
        .arg("deflated_file.bin")
        .arg("empty_folder")
        .arg("nested")
        .current_dir(&src_dir)
        .status()
        .expect("failed to execute /usr/bin/zip");
    assert!(status.success());

    let rust_entries = collect_pure_rust(&zip_path);
    let lib_entries = collect_libarchive(&zip_path);

    assert_structural_equivalence(&rust_entries, &lib_entries);

    // Verify pure-Rust populated accurate compressed size and CRC
    for r in &rust_entries {
        if !r.is_directory && r.uncompressed_size > 0 {
            assert!(r.crc32 > 0, "expected non-zero CRC32 for {}", r.path);
            assert!(r.compressed_size > 0, "expected non-zero compressed_size for {}", r.path);
        }
    }
}

#[test]
fn test_equivalence_utf8_multilingual_names() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("utf8_names.zip");

    let script = r#"
import zipfile, datetime
dt = (2026, 10, 3, 12, 0, 0)
with zipfile.ZipFile('REPLACE_ZIP', 'w') as z:
    for name in ['文档/测试资料_2026.pdf', '日本語/写真_京都.jpg', '한국어/보고서.docx', 'Umlauts_äöü_ß.txt']:
        info = zipfile.ZipInfo(name, date_time=dt)
        info.compress_type = zipfile.ZIP_DEFLATED
        z.writestr(info, b"Sample UTF-8 Content")
"#.replace("REPLACE_ZIP", zip_path.to_str().unwrap());

    let py_status = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .status()
        .expect("failed to run python3 script");
    assert!(py_status.success());

    let rust_entries = collect_pure_rust(&zip_path);
    let lib_entries = collect_libarchive(&zip_path);

    assert_structural_equivalence(&rust_entries, &lib_entries);
    assert_eq!(rust_entries.len(), 4);
    assert_eq!(rust_entries[0].path, "文档/测试资料_2026.pdf");
    assert_eq!(rust_entries[1].path, "日本語/写真_京都.jpg");
    assert_eq!(rust_entries[2].path, "한국어/보고서.docx");
    assert_eq!(rust_entries[3].path, "Umlauts_äöü_ß.txt");
}

#[test]
fn test_equivalence_cp437_legacy_encoding() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("cp437_names.zip");

    // Use Python to create ZIP with non-UTF-8 bytes (bit 11 clear)
    let script = r#"
import zipfile
with zipfile.ZipFile('REPLACE_ZIP', 'w') as z:
    # 0x84 is 'ä', 0x94 is 'ö', 0x81 is 'ü' in CP437
    raw_cp437 = b"\x84\x94\x81_german.txt"
    info = zipfile.ZipInfo()
    info.filename = raw_cp437.decode('cp437')
    info.flag_bits = 0 # bit 11 clear
    z.writestr(info, b"CP437 text payload")
"#.replace("REPLACE_ZIP", zip_path.to_str().unwrap());

    let py_status = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .status()
        .expect("failed to run python3 script");
    assert!(py_status.success());

    let rust_entries = collect_pure_rust(&zip_path);
    let lib_entries = collect_libarchive(&zip_path);

    assert_structural_equivalence(&rust_entries, &lib_entries);
    assert_eq!(rust_entries.len(), 1);
    let r_nfc: String = rust_entries[0].path.nfc().collect();
    let l_nfc: String = lib_entries[0].path.nfc().collect();
    assert_eq!(r_nfc, l_nfc);
}

#[test]
fn test_equivalence_encrypted_zipcrypto() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let f1 = src_dir.join("secret_data.txt");
    fs::write(&f1, b"Top secret confidential password-protected content").unwrap();

    let zip_path = dir.path().join("encrypted.zip");

    let status = Command::new("/usr/bin/zip")
        .arg("-q")
        .arg("-P")
        .arg("MasterPassword2026")
        .arg(&zip_path)
        .arg("secret_data.txt")
        .current_dir(&src_dir)
        .status()
        .expect("failed to execute /usr/bin/zip");
    assert!(status.success());

    let rust_entries = collect_pure_rust(&zip_path);
    let lib_entries = collect_libarchive(&zip_path);

    assert_structural_equivalence(&rust_entries, &lib_entries);
    assert_eq!(rust_entries.len(), 1);
    assert!(rust_entries[0].is_encrypted, "pure-Rust entry must be marked encrypted");
    assert!(lib_entries[0].is_encrypted, "libarchive entry must be marked encrypted");
    assert_eq!(rust_entries[0].uncompressed_size, 50);
}

#[test]
fn test_equivalence_zip64_more_than_65535_entries() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("zip64_large.zip");

    // Generate 65,540 entries (> 65535, triggering Zip64 EOCD and Locator)
    let script = r#"
import zipfile, datetime
dt = (2026, 10, 3, 12, 0, 0)
with zipfile.ZipFile('REPLACE_ZIP', 'w', allowZip64=True) as z:
    for i in range(65540):
        info = zipfile.ZipInfo(f"folder/sub/entry_{i:06d}.dat", date_time=dt)
        info.compress_type = zipfile.ZIP_STORED
        z.writestr(info, b"")
"#.replace("REPLACE_ZIP", zip_path.to_str().unwrap());

    let py_status = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .status()
        .expect("failed to run python3 script");
    assert!(py_status.success());

    let rust_entries = collect_pure_rust(&zip_path);
    let lib_entries = collect_libarchive(&zip_path);

    assert_eq!(rust_entries.len(), 65540);
    assert_eq!(lib_entries.len(), 65540);
    assert_structural_equivalence(&rust_entries, &lib_entries);
}

#[test]
fn test_equivalence_winzip_aes256() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let src_file = src_dir.join("aes_classified.dat");
    fs::write(&src_file, b"Highly classified WinZip AES-256 encrypted payload bytes").unwrap();

    let zip_path = dir.path().join("winzip_aes.zip");

    let pwd = std::ffi::CString::new("SecurePassphrase2026").unwrap();
    let create_opts = ttzip_engine::types::TTZipCreateOptions {
        struct_size: std::mem::size_of::<ttzip_engine::types::TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        format: ttzip_engine::types::TTZipArchiveFormat::Zip,
        level: ttzip_engine::types::TTZipCompressionLevel::Normal,
        encryption: ttzip_engine::types::TTZipEncryptionMethod::Aes256,
        password: pwd.as_ptr(),
        thread_budget: 2,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    ttzip_engine::archive::unified::UnifiedArchiveOrchestrator::create_archive(
        &[src_file],
        &zip_path,
        &create_opts,
        0,
    )
    .expect("Failed to create WinZip AES archive");

    let rust_entries = collect_pure_rust(&zip_path);
    let lib_entries = collect_libarchive(&zip_path);

    assert_structural_equivalence(&rust_entries, &lib_entries);
    assert_eq!(rust_entries.len(), 1);
    assert!(rust_entries[0].is_encrypted, "WinZip AES entry must be marked encrypted");
    assert!(lib_entries[0].is_encrypted, "libarchive entry must be marked encrypted");
    assert_eq!(rust_entries[0].uncompressed_size, 56);
}

#[test]
fn test_fallback_on_corrupted_cd() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("corrupted.zip");

    // Create valid zip first
    let script = r#"
import zipfile
with zipfile.ZipFile('REPLACE_ZIP', 'w') as z:
    z.writestr('file1.txt', b'valid file')
"#.replace("REPLACE_ZIP", zip_path.to_str().unwrap());

    Command::new("python3").arg("-c").arg(&script).status().unwrap();

    // Corrupt Central Directory signature (replace PK\x01\x02 with XX\x01\x02)
    let mut bytes = fs::read(&zip_path).unwrap();
    if let Some(pos) = bytes.windows(4).position(|w| w == b"PK\x01\x02") {
        bytes[pos] = b'X';
        bytes[pos + 1] = b'X';
        fs::write(&zip_path, &bytes).unwrap();
    }

    // Pure-Rust Central Directory detection will fail on corrupt CDFH signature,
    // and must fall back gracefully to libarchive (which also handles/reports or attempts extraction).
    let mut count = 0;
    let _ = inspect_archive(&zip_path, None, false, |_| {
        count += 1;
        true
    });
    // Fallback executed without panic or crash.
}

#[test]
fn test_dos_mtime_across_timezones_orchestrator() {
    let current_exe = std::env::current_exe().expect("failed to obtain current test executable");

    for tz in &["UTC", "Asia/Shanghai", "America/New_York"] {
        let output = Command::new(&current_exe)
            .arg("test_dos_mtime_worker")
            .arg("--exact")
            .arg("--nocapture")
            .env("TZ", tz)
            .env("TEST_RUN_TZ_CHECK", tz)
            .output()
            .expect("failed to spawn timezone worker process");

        assert!(
            output.status.success(),
            "Timezone worker failed for TZ={}:\nSTDOUT:\n{}\nSTDERR:\n{}",
            tz,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn test_dos_mtime_worker() {
    let dir = tempdir().unwrap();

    // 1. Archive WITH 0x5455 (created via /usr/bin/zip)
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("ext_time.txt"), b"file with 0x5455 extra field").unwrap();
    let zip_with_5455 = dir.path().join("with_5455.zip");
    let status = Command::new("/usr/bin/zip")
        .arg("-j")
        .arg("-q")
        .arg(&zip_with_5455)
        .arg(src_dir.join("ext_time.txt"))
        .status()
        .expect("failed to run /usr/bin/zip");
    assert!(status.success());

    // 2. Archive WITHOUT 0x5455 (pure DOS datetime, created via python zipfile)
    let zip_without_5455 = dir.path().join("without_5455.zip");
    let script = r#"
import zipfile
with zipfile.ZipFile('REPLACE_ZIP', 'w') as zf:
    zf.writestr('dos_only.txt', b'file with pure DOS timestamp')
"#.replace("REPLACE_ZIP", zip_without_5455.to_str().unwrap());
    let status2 = Command::new("python3").arg("-c").arg(&script).status().unwrap();
    assert!(status2.success());

    // Assert exact mtime equivalence for archive WITH 0x5455
    let rust_with = collect_pure_rust(&zip_with_5455);
    let lib_with = collect_libarchive(&zip_with_5455);
    assert_eq!(rust_with.len(), 1);
    assert_eq!(lib_with.len(), 1);
    assert_eq!(
        rust_with[0].mtime_epoch_secs,
        lib_with[0].mtime_epoch_secs,
        "mtime mismatch on archive with 0x5455 under TZ={:?}",
        std::env::var("TZ")
    );

    // Assert exact mtime equivalence (zero tolerance) for archive WITHOUT 0x5455
    let rust_without = collect_pure_rust(&zip_without_5455);
    let lib_without = collect_libarchive(&zip_without_5455);
    assert_eq!(rust_without.len(), 1);
    assert_eq!(lib_without.len(), 1);
    assert_eq!(
        rust_without[0].mtime_epoch_secs,
        lib_without[0].mtime_epoch_secs,
        "mtime mismatch on archive without 0x5455 under TZ={:?}",
        std::env::var("TZ")
    );
}

#[test]
fn test_unicode_roundtrip_nfc_and_nfd_extraction() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("unicode_nfc_nfd.zip");

    let nfc_name = "münchen_nfc.txt";
    let nfd_name = "mu\u{0308}nchen_nfd.txt";
    let nfc_content = b"content for NFC entry";
    let nfd_content = b"content for NFD entry";

    let script = r#"
import zipfile
with zipfile.ZipFile('REPLACE_ZIP', 'w') as zf:
    zf.writestr('münchen_nfc.txt', b'content for NFC entry')
    zf.writestr('mu\u0308nchen_nfd.txt', b'content for NFD entry')
"#.replace("REPLACE_ZIP", zip_path.to_str().unwrap());
    let status = Command::new("python3").arg("-c").arg(&script).status().unwrap();
    assert!(status.success());

    // 1. Inspect using pure-Rust inspect_archive
    let entries = collect_pure_rust(&zip_path);

    assert_eq!(entries.len(), 2);

    let nfc_inspected = entries.iter().find(|e| e.path.contains("nfc")).expect("nfc entry not found");
    let nfd_inspected = entries.iter().find(|e| e.path.contains("nfd")).expect("nfd entry not found");

    assert_eq!(nfc_inspected.path, nfc_name, "NFC path must match exact stored bytes without normalization");
    assert_eq!(nfd_inspected.path, nfd_name, "NFD path must match exact stored bytes without normalization");

    // 2. Round-trip: pass path returned by inspect to extract_single_entry_by_path
    let nfd_extracted = ttzip_engine::uniffi_api::extraction::extract_single_entry_by_path(
        zip_path.to_str().unwrap().to_string(),
        nfd_inspected.path.clone(),
        None,
    ).expect("extract_single_entry_by_path failed for NFD entry");
    assert_eq!(nfd_extracted, nfd_content);

    let res_nfc = ttzip_engine::uniffi_api::extraction::extract_single_entry_by_path(
        zip_path.to_str().unwrap().to_string(),
        nfc_inspected.path.clone(),
        None,
    );
    assert!(res_nfc.is_ok());
    assert_eq!(res_nfc.unwrap(), nfc_content);

    // Also verify extract_single_entry_memory round-trips with pure-Rust inspect paths:
    let mem_nfc = ttzip_engine::archive::unified::extract_single::extract_single_entry_memory(
        &zip_path,
        Some(&nfc_inspected.path),
        -1,
        None,
    ).expect("extract_single_entry_memory failed for NFC entry");
    assert_eq!(mem_nfc, nfc_content);

    let mem_nfd = ttzip_engine::archive::unified::extract_single::extract_single_entry_memory(
        &zip_path,
        Some(&nfd_inspected.path),
        -1,
        None,
    ).expect("extract_single_entry_memory failed for NFD entry");
    assert_eq!(mem_nfd, nfd_content);

    // 3. Differential check on libarchive behavior:
    let lib_entries = collect_libarchive(&zip_path);
    let lib_nfc_entry = lib_entries.iter().find(|e| e.path.contains("nfc")).unwrap();
    let is_darwin_libarchive_nfd = lib_nfc_entry.path != nfc_name;
    if is_darwin_libarchive_nfd {
        // Confirming pre-existing bug: passing libarchive's NFD pathname for an NFC entry fails in extract_single_entry_memory
        let mem_lib_fail = ttzip_engine::archive::unified::extract_single::extract_single_entry_memory(
            &zip_path,
            Some(&lib_nfc_entry.path),
            -1,
            None,
        );
        assert_eq!(
            mem_lib_fail,
            Err(ttzip_engine::types::TTZipStatus::ErrFileNotFound),
            "Expected extract_single_entry_memory to fail when passed Darwin libarchive's NFD path"
        );
    }
}
