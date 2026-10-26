// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Gates for the engine's public password recovery entries.
//!
//! The gates drive the real entry points — the UniFFI function and the C ABI
//! function — so they prove that the production wiring is in place and not a
//! stand-in pipeline.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use ttzip_archive::zip::parser::parse_local_file_header;
use ttzip_types::{CancellationReason, CancellationToken, TTZipStatus};

use crate::ffi::crypto_ffi::password_recovery::{
    ttzip_rust_password_recovery_start_brute_force, ttzip_rust_password_recovery_start_dictionary,
};
use crate::uniffi_api::archive::recover_archive_password;

use super::tests_fixtures::{
    build_aes_fixture, build_credential_less_fixture, build_sevenz_fixture,
    build_zipcrypto_fixture, build_zipcrypto_fixture_with_password, derive_screen_bait,
    FIXTURE_PASSWORD, FIXTURE_SCREEN_BAIT,
};

/// Extracts the ZipCrypto screening material of a fixture.
fn zipcrypto_screen_material(zip_bytes: &[u8]) -> ([u8; 12], u8) {
    let archive = ttzip_archive::zip::reader::ZipArchive::open_slice(zip_bytes).expect("fixture");
    let entry = &archive.entries()[0];
    let (payload_offset, _) =
        parse_local_file_header(zip_bytes, entry.lfh_offset as usize).expect("LFH");
    let comp_size = entry.compressed_size as usize;
    let raw = &zip_bytes[payload_offset..payload_offset + comp_size];
    let header: [u8; 12] = raw[..12].try_into().expect("header");
    (header, (entry.crc32 >> 24) as u8)
}

/// Writes the archive into a temporary directory and returns its path.
fn write_fixture(bytes: &[u8], name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).expect("fixture write");
    (dir, path)
}

/// The UniFFI entry must find the real password after a screening hit.
#[test]
fn the_uniffi_entry_finds_the_password_after_a_screening_hit() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);
    assert_eq!(bait, FIXTURE_SCREEN_BAIT);
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let outcome = recover_archive_password(
        path.to_string_lossy().to_string(),
        vec![bait, FIXTURE_PASSWORD.to_string()],
    )
    .expect("the entry must succeed");

    assert_eq!(outcome.found_password.as_deref(), Some(FIXTURE_PASSWORD));
    assert_eq!(outcome.total_attempts, 2);
}

/// The UniFFI entry must not report a screening hit on its own as a success.
#[test]
fn the_uniffi_entry_does_not_report_a_screening_hit_as_a_success() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let outcome = recover_archive_password(path.to_string_lossy().to_string(), vec![bait])
        .expect("the entry must succeed");

    assert_eq!(
        outcome.found_password, None,
        "a rejected screening hit must not be reported as a found password"
    );
}

/// The UniFFI entry must find the password of an AE-2 archive.
#[test]
fn the_uniffi_entry_finds_the_password_of_an_ae2_archive() {
    let zip_bytes = build_aes_fixture();
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let outcome = recover_archive_password(
        path.to_string_lossy().to_string(),
        vec![
            "an-unrelated-password".to_string(),
            FIXTURE_PASSWORD.to_string(),
        ],
    )
    .expect("the entry must succeed");

    assert_eq!(outcome.found_password.as_deref(), Some(FIXTURE_PASSWORD));
}

/// The UniFFI entry must find the password of a 7z archive.
#[test]
fn the_uniffi_entry_finds_the_password_of_a_7z_archive() {
    let sevenz_bytes = build_sevenz_fixture();
    let (_dir, path) = write_fixture(&sevenz_bytes, "fixture.7z");

    let outcome = recover_archive_password(
        path.to_string_lossy().to_string(),
        vec![
            "an-unrelated-password".to_string(),
            FIXTURE_PASSWORD.to_string(),
        ],
    )
    .expect("the entry must succeed");

    assert_eq!(outcome.found_password.as_deref(), Some(FIXTURE_PASSWORD));
}

/// A missing integrity credential must be an error, not a "not found" outcome and
/// not a password error.
#[test]
fn the_uniffi_entry_reports_a_missing_credential_as_an_error() {
    let zip_bytes = build_credential_less_fixture();
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let error = recover_archive_password(
        path.to_string_lossy().to_string(),
        vec![FIXTURE_PASSWORD.to_string()],
    )
    .expect_err("a missing credential must not be reported as a search result");

    match error {
        crate::uniffi_api::types::TTZipError::EngineError { code } => {
            assert_eq!(
                code,
                crate::recovery::RecoveryStopReason::NoCredential.status_code(),
                "the error must carry the status code of the stop reason"
            );
        }
        other => panic!("a missing credential must not be a password error: {other:?}"),
    }
}

/// The C ABI entry must find the real password after a screening hit.
#[test]
fn the_c_abi_entry_finds_the_password_after_a_screening_hit() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let path_c = CString::new(path.to_string_lossy().as_ref()).expect("path");
    let bait_c = CString::new(bait).expect("bait");
    let correct_c = CString::new(FIXTURE_PASSWORD).expect("password");
    let candidates: Vec<*const c_char> = vec![bait_c.as_ptr(), correct_c.as_ptr()];
    let mut out: Vec<c_char> = vec![0; 64];
    let mut attempts: u64 = 0;

    let status = unsafe {
        ttzip_rust_password_recovery_start_dictionary(
            path_c.as_ptr(),
            candidates.as_ptr(),
            candidates.len(),
            std::ptr::null(),
            out.as_mut_ptr(),
            out.len(),
            &mut attempts,
        )
    };

    assert_eq!(status, TTZipStatus::Ok);
    let found = unsafe { CStr::from_ptr(out.as_ptr()) }
        .to_str()
        .expect("utf8");
    assert_eq!(found, FIXTURE_PASSWORD);
    assert_eq!(attempts, 2, "both candidates must have been screened");
}

/// The C ABI entry must not report a screening hit on its own as a success.
#[test]
fn the_c_abi_entry_does_not_report_a_screening_hit_as_a_success() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let path_c = CString::new(path.to_string_lossy().as_ref()).expect("path");
    let bait_c = CString::new(bait).expect("bait");
    let candidates: Vec<*const c_char> = vec![bait_c.as_ptr()];
    let mut out: Vec<c_char> = vec![0; 64];
    for (index, &byte) in b"old-password\0".iter().enumerate() {
        out[index] = byte as c_char;
    }
    assert_ne!(out[0], 0, "the reused output must start nonempty");
    let mut attempts: u64 = 0;

    let status = unsafe {
        ttzip_rust_password_recovery_start_dictionary(
            path_c.as_ptr(),
            candidates.as_ptr(),
            candidates.len(),
            std::ptr::null(),
            out.as_mut_ptr(),
            out.len(),
            &mut attempts,
        )
    };

    assert_eq!(
        status,
        TTZipStatus::ErrInvalidPassword,
        "a rejected screening hit must not be reported as a found password"
    );
    let found = unsafe { CStr::from_ptr(out.as_ptr()) }
        .to_str()
        .expect("utf8");
    assert!(found.is_empty(), "no password may be written out");
    assert_eq!(attempts, 1, "the rejected screening hit must still count once");
}

/// The C ABI entry must honour a cancellation request.
#[test]
fn the_c_abi_entry_reports_a_cancellation() {
    let zip_bytes = build_zipcrypto_fixture();
    let (_dir, path) = write_fixture(&zip_bytes, "fixture.zip");

    let path_c = CString::new(path.to_string_lossy().as_ref()).expect("path");
    let password_c = CString::new(FIXTURE_PASSWORD).expect("password");
    let candidates: Vec<*const c_char> = vec![password_c.as_ptr()];
    let mut out: Vec<c_char> = vec![0; 64];
    let mut attempts: u64 = 0;

    let token = CancellationToken::new();
    token.cancel(CancellationReason::UserRequested);

    let status = unsafe {
        ttzip_rust_password_recovery_start_dictionary(
            path_c.as_ptr(),
            candidates.as_ptr(),
            candidates.len(),
            &token,
            out.as_mut_ptr(),
            out.len(),
            &mut attempts,
        )
    };

    assert_eq!(status, TTZipStatus::Cancelled);
    let found = unsafe { CStr::from_ptr(out.as_ptr()) }
        .to_str()
        .expect("utf8");
    assert!(
        found.is_empty(),
        "a cancelled search must not write a password"
    );
}

/// Public attempt metrics report executed screening, not unvisited input length.
#[test]
fn the_uniffi_entry_counts_only_screened_candidates() {
    let bytes = build_zipcrypto_fixture();
    let (_dir, path) = write_fixture(&bytes, "early-hit.zip");
    let mut candidates =
        vec!["codex-unused-candidate".to_string(); crate::recovery::SCREEN_BATCH_SIZE * 2];
    candidates[0] = FIXTURE_PASSWORD.to_string();
    let outcome = recover_archive_password(path.to_string_lossy().to_string(), candidates)
        .expect("the first batch contains the confirmed password");
    assert_eq!(outcome.found_password.as_deref(), Some(FIXTURE_PASSWORD));
    assert_eq!(
        outcome.total_attempts,
        crate::recovery::SCREEN_BATCH_SIZE as u64
    );
}

/// Formats without a cheap screen count confirmations actually attempted.
#[test]
fn the_uniffi_entry_does_not_count_unconfirmed_sevenz_candidates() {
    let bytes = build_sevenz_fixture();
    let (_dir, path) = write_fixture(&bytes, "early-sevenz-hit.7z");
    let mut candidates =
        vec!["codex-unused-candidate".to_string(); crate::recovery::SCREEN_BATCH_SIZE * 2];
    candidates[0] = FIXTURE_PASSWORD.to_string();
    let outcome = recover_archive_password(path.to_string_lossy().to_string(), candidates)
        .expect("the first candidate is confirmed");
    assert_eq!(outcome.found_password.as_deref(), Some(FIXTURE_PASSWORD));
    assert_eq!(outcome.total_attempts, 1);
}

/// The second archive-level C ABI starter must reach content confirmation too.
#[test]
fn the_brute_force_c_abi_confirms_the_archive_password() {
    let bytes = build_zipcrypto_fixture_with_password("b");
    let (_dir, path) = write_fixture(&bytes, "brute-force.zip");
    let path = CString::new(path.to_string_lossy().as_ref()).expect("path");
    let charset = CString::new("ab").expect("charset");
    let mut output: [c_char; 64] = [0; 64];
    let mut attempts = 0;
    let status = unsafe {
        ttzip_rust_password_recovery_start_brute_force(
            path.as_ptr(),
            charset.as_ptr(),
            1,
            1,
            std::ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut attempts,
        )
    };
    assert_eq!(status, TTZipStatus::Ok);
    assert_eq!(
        unsafe { CStr::from_ptr(output.as_ptr()) }
            .to_str()
            .expect("utf8"),
        "b"
    );
    assert_eq!(
        attempts, 2,
        "only the two screened candidates were attempted"
    );
}

/// Exhaustion and pre-computation cancellation retain different public outcomes.
#[test]
fn the_brute_force_c_abi_preserves_exhaustion_and_cancellation() {
    let bytes = build_zipcrypto_fixture_with_password("b");
    let (_dir, path) = write_fixture(&bytes, "brute-force-stop.zip");
    let path = CString::new(path.to_string_lossy().as_ref()).expect("path");
    let charset = CString::new("a").expect("charset");
    for cancelled in [false, true] {
        let token = CancellationToken::new();
        if cancelled {
            token.cancel(CancellationReason::UserRequested);
        }
        let mut output: [c_char; 64] = [0; 64];
        if !cancelled {
            for (index, &byte) in b"old-password\0".iter().enumerate() {
                output[index] = byte as c_char;
            }
            assert_ne!(output[0], 0, "the exhausted path must reuse a nonempty output");
        }
        let mut attempts = u64::MAX;
        let status = unsafe {
            ttzip_rust_password_recovery_start_brute_force(
                path.as_ptr(),
                charset.as_ptr(),
                1,
                1,
                &token,
                output.as_mut_ptr(),
                output.len(),
                &mut attempts,
            )
        };
        let expected = if cancelled {
            TTZipStatus::Cancelled
        } else {
            TTZipStatus::ErrInvalidPassword
        };
        assert_eq!(status, expected);
        assert_eq!(attempts, if cancelled { 0 } else { 1 });
        assert!(unsafe { CStr::from_ptr(output.as_ptr()) }.is_empty());
    }
}
