// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Comprehensive integration tests for SecurePathExtractor sandbox isolation,
//! Zip-Slip traversal defense, intermediate symlink escape interception,
//! and bottom-up POSIX metadata restoration.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
mod common;
use common::SafeTempDir;
use tempfile::tempdir;
use ttzip_engine::security::{DeferredSecureEntry, SecurePathExtractor, SecurityFlags};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_security_flags_operations() {
    let flags = SecurityFlags::SECURE_NODOTDOT | SecurityFlags::SECURE_SYMLINKS;
    assert!(flags.contains(SecurityFlags::SECURE_NODOTDOT));
    assert!(flags.contains(SecurityFlags::SECURE_SYMLINKS));
    assert!(!flags.contains(SecurityFlags::SECURE_NOABSOLUTEPATHS));

    let mut mutable_flags = SecurityFlags::empty();
    assert!(mutable_flags.is_empty());
    mutable_flags.insert(SecurityFlags::SECURE_UNLINK_FIRST);
    assert!(mutable_flags.contains(SecurityFlags::SECURE_UNLINK_FIRST));

    mutable_flags.toggle(SecurityFlags::SECURE_UNLINK_FIRST);
    assert!(mutable_flags.is_empty());

    let all = SecurityFlags::all();
    assert!(all.contains(SecurityFlags::DEFAULT));
    assert_eq!(all.bits(), 0b11111);

    let diff = all.difference(SecurityFlags::RESTORE_PERMISSIONS);
    assert!(!diff.contains(SecurityFlags::RESTORE_PERMISSIONS));
    assert!(diff.contains(SecurityFlags::SECURE_NODOTDOT));
}

#[test]
fn test_deferred_entry_depth_sorting() {
    let e1 = DeferredSecureEntry {
        rel_path: Path::new("a").to_path_buf(),
        mode: 0o755,
        mtime_epoch_secs: 100,
        mtime_nanos: 0,
        is_directory: true,
    };
    let e2 = DeferredSecureEntry {
        rel_path: Path::new("a/b/c").to_path_buf(),
        mode: 0o755,
        mtime_epoch_secs: 100,
        mtime_nanos: 0,
        is_directory: true,
    };
    assert_eq!(e1.depth(), 1);
    assert_eq!(e2.depth(), 3);
    assert!(e2.depth() > e1.depth());
}

#[test]
fn test_zipslip_traversal_interception() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let extractor = SecurePathExtractor::new(tmp.path(), SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    // 1. Classic dot-dot escape
    assert_eq!(
        extractor.sanitize_and_validate_path("../../../../etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 2. Nested parent traversal escape
    assert_eq!(
        extractor.sanitize_and_validate_path("foo/bar/../../../etc/shadow"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. Current dir dot-dot escape
    assert_eq!(
        extractor.sanitize_and_validate_path("./../secret"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 4. Backslash escaped traversal
    assert_eq!(
        extractor.sanitize_and_validate_path(r"a\b\..\..\..\..\escape.sh"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Multi-dot obfuscation
    assert_eq!(
        extractor.sanitize_and_validate_path(".../evil.sh"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path("..../payload"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 6. Null byte injection
    assert_eq!(
        extractor.sanitize_and_validate_path("valid_file.txt\0/malicious"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 7. URI schemes
    assert_eq!(
        extractor.sanitize_and_validate_path("file:///etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path("http://evil.com/exploit"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_absolute_paths_interception_and_sandboxing() {
    let tmp = tempdir().expect("Failed to create tempdir");

    // Extractor with SECURE_NOABSOLUTEPATHS (default)
    let extractor = SecurePathExtractor::new(tmp.path(), SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    assert_eq!(
        extractor.sanitize_and_validate_path("/etc/hosts"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path(r"\Windows\System32"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path(r"C:\Windows\System32"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path(r"\\remote_host\share\data"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Valid relative path passes
    let rel = extractor
        .sanitize_and_validate_path("subfolder/file.txt")
        .expect("Valid path should pass");
    assert_eq!(rel, Path::new("subfolder/file.txt"));
}

#[test]
fn test_symlink_escape_attack_interception() {
    let tmp_sandbox = tempdir().expect("Failed to create sandbox tempdir");
    let tmp_victim = tempdir().expect("Failed to create victim tempdir");

    let sandbox_path = tmp_sandbox.path();
    let victim_path = tmp_victim.path();

    let mut extractor = SecurePathExtractor::new(sandbox_path, SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    // 1. Attempt to create a symlink pointing to an external directory (victim dir)
    let symlink_name = Path::new("link_dir");
    let victim_target_str = victim_path.to_str().expect("Path to str");

    // create_symlink_secure with absolute target must be rejected
    assert_eq!(
        extractor.create_symlink_secure(symlink_name, victim_target_str),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Manually create symlink inside sandbox to simulate an archive containing an intermediate symlink
    let full_symlink = sandbox_path.join("external_link");
    std::os::unix::fs::symlink(victim_path, &full_symlink).expect("Create symlink");

    // 2. Now attempt to extract a file inside the symlink directory ("external_link/payload.txt")
    let payload_path = Path::new("external_link/payload.txt");
    let res = extractor.create_file_secure(payload_path, 0o644, 1700000000, 0, true);

    // Assert that intermediate path validation intercepts the symlink and rejects the write
    assert_eq!(res.map(|_| ()), Err(TTZipStatus::ErrSecurityViolation));

    // Ensure victim directory was NOT modified
    let victim_payload = victim_path.join("payload.txt");
    assert!(!victim_payload.exists());

    // 3. Attempt to create directory through intermediate symlink
    let payload_dir = Path::new("external_link/nested_dir");
    let dir_res = extractor.create_dir_all_secure(payload_dir, 0o755, 1700000000, 0);
    assert_eq!(dir_res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_toctou_atomic_unlink_and_file_creation() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let sandbox = tmp.path();

    let mut extractor = SecurePathExtractor::new(sandbox, SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    let file_rel = Path::new("output/data.bin");

    // Create file
    let mut f = extractor
        .create_file_secure(file_rel, 0o644, 1700000000, 0, false)
        .expect("File creation should succeed");
    f.write_all(b"Initial content").expect("Write should succeed");
    drop(f);

    let full_path = sandbox.join(file_rel);
    assert!(full_path.exists());
    let content = fs::read_to_string(&full_path).expect("Read file");
    assert_eq!(content, "Initial content");

    // Overwrite file with SECURE_UNLINK_FIRST active
    let mut f2 = extractor
        .create_file_secure(file_rel, 0o600, 1700000001, 0, true)
        .expect("Overwrite should succeed");
    f2.write_all(b"Updated content").expect("Write should succeed");
    drop(f2);

    let updated = fs::read_to_string(&full_path).expect("Read file");
    assert_eq!(updated, "Updated content");
}

#[test]
fn test_two_stage_bottom_up_permission_and_timestamp_restoration() {
    let tmp = SafeTempDir::new().expect("Failed to create safe tempdir");
    let sandbox = tmp.path();

    let mut extractor = SecurePathExtractor::new(sandbox, SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    // Create multi-tier nested directory: parent/child/grandchild
    let parent_dir = Path::new("parent");
    let child_dir = Path::new("parent/child");
    let file_path = Path::new("parent/child/test.txt");

    // Set parent directory to restrictive read-only mode (0555) in archive metadata
    extractor
        .create_dir_all_secure(parent_dir, 0o555, 1700000000, 0)
        .expect("Parent dir creation should succeed");
    extractor
        .create_dir_all_secure(child_dir, 0o755, 1700000000, 0)
        .expect("Child dir creation should succeed");

    // Create child file with 0644 mode
    let mut f = extractor
        .create_file_secure(file_path, 0o644, 1700000000, 0, true)
        .expect("File creation should succeed");
    f.write_all(b"Nested secure file content")
        .expect("Write file");
    drop(f);

    // Before apply_deferred_metadata, stage 1 permissions are user-accessible (0700/0600)
    let parent_meta = fs::metadata(sandbox.join(parent_dir)).expect("Metadata");
    assert_eq!(parent_meta.permissions().mode() & 0o777, 0o700);

    let file_meta = fs::metadata(sandbox.join(file_path)).expect("Metadata");
    assert_eq!(file_meta.permissions().mode() & 0o777, 0o600);

    // Apply deferred metadata in bottom-up order
    extractor
        .apply_deferred_metadata()
        .expect("Metadata application should succeed");

    // Verify final restored permissions
    let final_file_meta = fs::metadata(sandbox.join(file_path)).expect("Metadata");
    assert_eq!(final_file_meta.permissions().mode() & 0o777, 0o644);

    let final_child_meta = fs::metadata(sandbox.join(child_dir)).expect("Metadata");
    assert_eq!(final_child_meta.permissions().mode() & 0o777, 0o755);

    let final_parent_meta = fs::metadata(sandbox.join(parent_dir)).expect("Metadata");
    assert_eq!(final_parent_meta.permissions().mode() & 0o777, 0o555);
}

fn build_synthetic_zip_with_entries(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();

    for &(name, payload, unix_mode) in entries {
        let lfh_offset = out.len() as u32;
        let name_bytes = name.as_bytes();
        let crc = crc32fast::hash(payload);
        let size = payload.len() as u32;

        // Local File Header
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(payload);

        // Central Directory Header (made by UNIX = 3 << 8)
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&((3u16 << 8) | 20u16).to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&size.to_le_bytes());
        cd.extend_from_slice(&size.to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        let ext_attr = unix_mode << 16;
        cd.extend_from_slice(&ext_attr.to_le_bytes());
        cd.extend_from_slice(&lfh_offset.to_le_bytes());
        cd.extend_from_slice(name_bytes);
    }

    let cd_offset = out.len() as u32;
    let cd_size = cd.len() as u32;
    out.extend_from_slice(&cd);

    let count = entries.len() as u16;
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn append_tar_entry(tar: &mut Vec<u8>, name: &str, typeflag: u8, linkname: &str, payload: &[u8]) {
    let mut header = [0u8; 512];
    let name_bytes = name.as_bytes();
    header[..name_bytes.len().min(100)].copy_from_slice(&name_bytes[..name_bytes.len().min(100)]);
    header[100..108].copy_from_slice(b"0000755\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    let size_octal = format!("{:011o}\0", if typeflag == b'0' { payload.len() } else { 0 });
    header[124..136].copy_from_slice(size_octal.as_bytes());
    header[136..148].copy_from_slice(b"14000000000\0");
    header[148..156].fill(b' ');
    header[156] = typeflag;
    if !linkname.is_empty() {
        let lb = linkname.as_bytes();
        header[157..157 + lb.len().min(100)].copy_from_slice(&lb[..lb.len().min(100)]);
    }
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    let chk = format!("{:06o}\0 ", sum);
    header[148..156].copy_from_slice(chk.as_bytes());
    tar.extend_from_slice(&header);
    if typeflag == b'0' && !payload.is_empty() {
        tar.extend_from_slice(payload);
        let rem = payload.len() % 512;
        if rem != 0 {
            tar.resize(tar.len() + (512 - rem), 0);
        }
    }
}

#[test]
fn test_symlink_escape_rejected_across_zip_tar_and_selective_extract() {
    use ttzip_engine::archive::tar::reader::TarArchive;
    use ttzip_engine::archive::unified::{EntrySelection, UnifiedArchiveOrchestrator};
    use ttzip_engine::fs::collision::StagingRoot;
    use ttzip_engine::types::TTZipExtractOptions;
    use ttzip_engine::zip::ZipArchive;

    let tmp = tempdir().unwrap();
    let zip_bytes = build_synthetic_zip_with_entries(&[
        ("dir_link", b"..", 0o120777),
        ("sub/escape", b"../../outside.txt", 0o120777),
    ]);
    let zip_path = tmp.path().join("symlink_escape.zip");
    fs::write(&zip_path, &zip_bytes).unwrap();

    let out_zip = tmp.path().join("out_zip");
    fs::create_dir_all(&out_zip).unwrap();
    let out_zip_staging = StagingRoot(out_zip.clone());
    let sel_dir_link = EntrySelection::new(["dir_link"], false);
    let opts = TTZipExtractOptions::default();
    let zip_arch = ZipArchive::open_slice(&zip_bytes).unwrap();
    assert_eq!(
        zip_arch.extract_all(&out_zip, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_archive(&zip_path, &out_zip_staging, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_selected_entries(
            &zip_path,
            &sel_dir_link,
            &out_zip_staging,
            &opts
        )
        .map(|_| ()),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    let mut tar_bytes = Vec::new();
    append_tar_entry(&mut tar_bytes, "dir_link", b'2', "..", b"");
    tar_bytes.resize(tar_bytes.len() + 1024, 0);
    let tar_path = tmp.path().join("symlink_escape.tar");
    fs::write(&tar_path, &tar_bytes).unwrap();

    let out_tar = tmp.path().join("out_tar");
    fs::create_dir_all(&out_tar).unwrap();
    let out_tar_staging = StagingRoot(out_tar.clone());
    let tar_arch = TarArchive::open_slice(&tar_bytes).unwrap();
    assert_eq!(
        tar_arch.extract_all(&out_tar, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_archive(&tar_path, &out_tar_staging, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_selected_entries(
            &tar_path,
            &sel_dir_link,
            &out_tar_staging,
            &opts
        )
        .map(|_| ()),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_symlink_chain_and_parent_escape_preserve_outside_marker_across_formats() {
    use ttzip_engine::archive::unified::{EntrySelection, UnifiedArchiveOrchestrator};
    use ttzip_engine::fs::collision::StagingRoot;
    use ttzip_engine::fs::dir_jail::DirJail;
    use ttzip_engine::types::TTZipExtractOptions;

    let tmp = tempdir().unwrap();
    let outside_marker = tmp.path().join("harmless_marker.txt");
    fs::write(&outside_marker, "SAFE_ORIGINAL_CONTENT").unwrap();

    let opts = TTZipExtractOptions::default();

    // 1. Direct parent symlink + overwrite payload in ZIP & TAR
    let zip_direct = build_synthetic_zip_with_entries(&[
        ("dir_link", b"..", 0o120777),
        ("dir_link/harmless_marker.txt", b"PWNED_DIRECT", 0o100644),
    ]);
    let zip_direct_path = tmp.path().join("direct_attack.zip");
    fs::write(&zip_direct_path, &zip_direct).unwrap();
    let dest_zip_direct = tmp.path().join("dest_zip_direct");
    fs::create_dir_all(&dest_zip_direct).unwrap();
    let staging_zip_direct = StagingRoot(dest_zip_direct);
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_archive(&zip_direct_path, &staging_zip_direct, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(fs::read_to_string(&outside_marker).unwrap(), "SAFE_ORIGINAL_CONTENT");

    // 2. Chained symlink attack: a/b -> .. (valid 1st hop to dest_dir), a/c -> b/.. (escapes via a/b!), a/c/harmless_marker.txt
    let zip_chain = build_synthetic_zip_with_entries(&[
        ("a/b", b"..", 0o120777),
        ("a/c", b"b/..", 0o120777),
        ("a/c/harmless_marker.txt", b"PWNED_CHAIN", 0o100644),
    ]);
    let zip_chain_path = tmp.path().join("chain_attack.zip");
    fs::write(&zip_chain_path, &zip_chain).unwrap();
    let dest_zip_chain = tmp.path().join("dest_zip_chain");
    fs::create_dir_all(&dest_zip_chain).unwrap();
    let staging_zip_chain = StagingRoot(dest_zip_chain);
    let sel_chain = EntrySelection::new(["a/b", "a/c", "a/c/harmless_marker.txt"], false);
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_archive(&zip_chain_path, &staging_zip_chain, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_selected_entries(
            &zip_chain_path,
            &sel_chain,
            &staging_zip_chain,
            &opts
        )
        .map(|_| ()),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(fs::read_to_string(&outside_marker).unwrap(), "SAFE_ORIGINAL_CONTENT");

    let mut tar_chain = Vec::new();
    append_tar_entry(&mut tar_chain, "a/", b'5', "", b"");
    append_tar_entry(&mut tar_chain, "a/b", b'2', "..", b"");
    append_tar_entry(&mut tar_chain, "a/c", b'2', "b/..", b"");
    append_tar_entry(&mut tar_chain, "a/c/harmless_marker.txt", b'0', "", b"PWNED_TAR_CHAIN");
    tar_chain.resize(tar_chain.len() + 1024, 0);
    let tar_chain_path = tmp.path().join("chain_attack.tar");
    fs::write(&tar_chain_path, &tar_chain).unwrap();
    let dest_tar_chain = tmp.path().join("dest_tar_chain");
    fs::create_dir_all(&dest_tar_chain).unwrap();
    let staging_tar_chain = StagingRoot(dest_tar_chain);
    assert_eq!(
        UnifiedArchiveOrchestrator::extract_archive(&tar_chain_path, &staging_tar_chain, &opts),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(fs::read_to_string(&outside_marker).unwrap(), "SAFE_ORIGINAL_CONTENT");

    // 3. Direct DirJail step-by-step verification on 7z/generic symlink chain
    let dest_jail_dir = tmp.path().join("dest_jail_7z");
    let jail = DirJail::open(&dest_jail_dir).unwrap();
    assert_eq!(
        jail.create_symlink_in_jail("a/b", ".."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    jail.create_symlink_in_jail("a/sub/b", "../sibling_dir").unwrap();
    assert_eq!(
        jail.create_symlink_in_jail("a/sub/c", "b/../../.."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        jail.open_file_in_jail("a/sub/b/harmless_marker.txt", 0o600, true).map(|_| ()),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(fs::read_to_string(&outside_marker).unwrap(), "SAFE_ORIGINAL_CONTENT");
}

#[test]
fn test_refusal_to_downgrade_on_security_corruption_password_or_cancel() {
    use ttzip_engine::archive::unified::fallback_policy::should_abort_without_fallback;

    for fatal_status in [
        TTZipStatus::ErrSecurityViolation,
        TTZipStatus::ErrCorruptHeader,
        TTZipStatus::ErrInvalidPassword,
        TTZipStatus::Cancelled,
        TTZipStatus::ErrOutOfMemory,
        TTZipStatus::ErrInvalidOffset,
        TTZipStatus::ErrUnsupportedFeature,
    ] {
        assert!(
            should_abort_without_fallback(fatal_status),
            "Status {fatal_status:?} must abort immediately without falling back to secondary engines"
        );
    }
    assert!(
        !should_abort_without_fallback(TTZipStatus::ErrArchiveInitFailed),
        "Only pure format/codec init capability gaps may fall back to secondary engines"
    );
}

#[test]
fn test_preexisting_0700_new_implicit_and_explicit_dir_permissions_e2e() {
    let tmp = tempdir().unwrap();
    let dest_dir = tmp.path().join("dest_coexistence");
    fs::create_dir_all(&dest_dir).unwrap();

    // Pre-existing private directory (`0o700`) already present before archive extraction
    let preexisting_priv = dest_dir.join("existing_private");
    fs::create_dir(&preexisting_priv).unwrap();
    fs::set_permissions(&preexisting_priv, fs::Permissions::from_mode(0o700)).unwrap();

    // Build a TAR archive with:
    // 1. `existing_private/new_implicit/payload.txt` (no explicit dir entry for `existing_private` or `new_implicit`)
    // 2. `explicit_dir/` (explicit directory entry with mode 0o750)
    // 3. `explicit_strict/` (explicit directory entry with mode 0o700)
    let mut tar_bytes = Vec::new();
    {
        let mut writer = ttzip_engine::archive::tar::writer::TarWriter::new(&mut tar_bytes);
        writer
            .append_file(
                "existing_private/new_implicit/payload.txt",
                b"coexistence test payload",
                0o644,
                1700000000,
            )
            .unwrap();
        writer
            .append_dir("explicit_dir/", 0o750, 1700000000)
            .unwrap();
        writer
            .append_dir("explicit_strict/", 0o700, 1700000000)
            .unwrap();
        writer.finish().unwrap();
    }

    let tar_arch = ttzip_engine::archive::tar::reader::TarArchive::open_slice(&tar_bytes).unwrap();
    let opts = ttzip_engine::types::TTZipExtractOptions {
        preserve_permissions: true,
        ..Default::default()
    };
    tar_arch.extract_all(&dest_dir, &opts).unwrap();

    let new_implicit_dir = dest_dir.join("existing_private/new_implicit");
    let explicit_dir = dest_dir.join("explicit_dir");
    let explicit_strict = dest_dir.join("explicit_strict");
    let payload_file = dest_dir.join("existing_private/new_implicit/payload.txt");

    // Pre-existing directory keeps 0o700; newly created implicit directory gets 0o755; explicit dirs get 0o750 and 0o700
    assert_eq!(fs::metadata(&preexisting_priv).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&new_implicit_dir).unwrap().permissions().mode() & 0o777, 0o755);
    assert_eq!(fs::metadata(&explicit_dir).unwrap().permissions().mode() & 0o777, 0o750);
    assert_eq!(fs::metadata(&explicit_strict).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&payload_file).unwrap().permissions().mode() & 0o777, 0o644);
}


