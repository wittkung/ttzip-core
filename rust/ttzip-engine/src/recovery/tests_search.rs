// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Gates for the engine-level password search orchestration.
//!
//! The gates drive the production orchestrator, which screens candidates cheaply and
//! confirms the survivors with the archive-layer content verification.

use ttzip_archive::recovery::{ConfirmationBudget, ScreeningMaterial};
use ttzip_crypto::password_recovery::verify_zipcrypto_candidate;
use ttzip_types::{CancellationReason, CancellationToken};

use crate::recovery::{
    RecoverySearch, RecoverySearchOutcome, RecoveryStopReason, SCREEN_BATCH_SIZE,
};

use super::tests_fixtures::{
    build_aes_fixture, build_credential_less_fixture, build_sevenz_fixture,
    build_unencrypted_fixture, build_unsupported_fixture, build_zipcrypto_fixture,
    derive_screen_bait, fixture_plaintext, FIXTURE_PASSWORD, FIXTURE_SCREEN_BAIT,
};

/// Extracts the ZipCrypto screening material of a fixture the way the crypto crate
/// expects it.
fn zipcrypto_screen_material(zip_bytes: &[u8]) -> ([u8; 12], u8) {
    let archive = ttzip_archive::zip::reader::ZipArchive::open_slice(zip_bytes).expect("fixture");
    let entry = &archive.entries()[0];
    let (payload_offset, _) =
        ttzip_archive::zip::parser::parse_local_file_header(zip_bytes, entry.lfh_offset as usize)
            .expect("LFH");
    let comp_size = entry.compressed_size as usize;
    let raw = &zip_bytes[payload_offset..payload_offset + comp_size];
    let header: [u8; 12] = raw[..12].try_into().expect("header");
    (header, (entry.crc32 >> 24) as u8)
}

/// A screening hit that is not the password must not end the search.
#[test]
fn a_screening_hit_does_not_end_the_search() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);
    assert_eq!(
        bait, FIXTURE_SCREEN_BAIT,
        "the deterministic bait derivation must reproduce the pinned literal"
    );
    assert!(
        verify_zipcrypto_candidate(&bait, &header, check_byte),
        "the bait must survive the screening stage"
    );

    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let outcome = search.run([bait.as_str(), FIXTURE_PASSWORD], None);

    assert_eq!(
        outcome,
        RecoverySearchOutcome::Confirmed {
            password: FIXTURE_PASSWORD.to_string()
        },
        "the search must reject the screening hit and continue to the real password"
    );
    assert!(
        search.confirmed_attempts() >= 2,
        "both the bait and the real password must have been confirmed"
    );
}

/// A screening hit on its own must never be reported as a found password.
#[test]
fn a_screening_hit_alone_is_never_a_success() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);

    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let outcome = search.run([bait.as_str()], None);

    assert_eq!(
        outcome,
        RecoverySearchOutcome::Exhausted,
        "a rejected screening hit must leave the search exhausted, not successful"
    );
    assert!(!outcome.is_confirmed());
}

/// The AE-2 path confirms with its authentication code, not with a CRC.
#[test]
fn ae2_screening_hit_does_not_end_the_search() {
    let zip_bytes = build_aes_fixture();
    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");

    let Some(ScreeningMaterial::WinZipAes { salt, stored_pvv }) = search.screening_material()
    else {
        panic!("an AE-2 entry must expose its WinZip AES screening material");
    };
    assert!(
        ttzip_crypto::password_recovery::verify_winzip_aes_candidate(
            super::tests_fixtures::FIXTURE_AES_BAIT,
            salt,
            stored_pvv
        ),
        "the AE-2 bait must survive the screening stage"
    );

    let outcome = search.run(
        [super::tests_fixtures::FIXTURE_AES_BAIT, FIXTURE_PASSWORD],
        None,
    );
    assert_eq!(
        outcome,
        RecoverySearchOutcome::Confirmed {
            password: FIXTURE_PASSWORD.to_string()
        },
        "the search must reject the AE-2 screening hit and continue"
    );
    assert!(
        search.confirmed_attempts() >= 2,
        "the AE-2 bait must have reached the confirmation and been rejected there"
    );
}

/// The AE-2 path rejects a wrong password without reporting it as a success.
#[test]
fn ae2_wrong_password_is_not_a_success() {
    let zip_bytes = build_aes_fixture();
    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let outcome = search.run(["an-unrelated-password"], None);
    assert_eq!(outcome, RecoverySearchOutcome::Exhausted);
}

/// The 7z path has no cheap screen, so every candidate is confirmed.
#[test]
fn sevenz_confirms_every_candidate_and_has_no_screening_material() {
    let sevenz_bytes = build_sevenz_fixture();
    let search =
        RecoverySearch::prepare(&sevenz_bytes, ConfirmationBudget::default()).expect("prepared");

    assert!(
        search.screening_material().is_none(),
        "7z must not expose a screening material, and the abolished fixed magic must not come back"
    );

    assert_eq!(
        search.run([FIXTURE_PASSWORD], None),
        RecoverySearchOutcome::Confirmed {
            password: FIXTURE_PASSWORD.to_string()
        }
    );
    assert_eq!(
        search.run(["an-unrelated-password"], None),
        RecoverySearchOutcome::Exhausted
    );
}

/// An exhausted budget is reported as such and never as a wrong password.
#[test]
fn an_exhausted_budget_is_preserved() {
    let zip_bytes = build_zipcrypto_fixture();
    let budget = ConfirmationBudget {
        max_entry_bytes: 16,
        max_preceding_bytes: Some(16),
    };
    let search = RecoverySearch::prepare(&zip_bytes, budget).expect("prepared");
    let outcome = search.run([FIXTURE_PASSWORD], None);

    assert_eq!(
        outcome,
        RecoverySearchOutcome::Stopped(RecoveryStopReason::BudgetExhausted),
        "a budget limit says nothing about the candidate"
    );
}

/// An encrypted unit without an integrity credential is reported as such.
#[test]
fn a_missing_credential_is_preserved() {
    let zip_bytes = build_credential_less_fixture();
    let outcome = RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).err();

    assert_eq!(outcome, Some(RecoveryStopReason::NoCredential));
}

/// An archive without any encrypted unit is reported as such.
#[test]
fn a_missing_encrypted_unit_is_preserved() {
    let zip_bytes = build_unencrypted_fixture();
    let outcome = RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).err();

    assert_eq!(outcome, Some(RecoveryStopReason::NoEncryptedUnit));
}

/// An encrypted unit the engine cannot decode is reported as unsupported.
#[test]
fn an_unsupported_unit_is_preserved() {
    let zip_bytes = build_unsupported_fixture();
    let outcome = RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).err();

    assert_eq!(outcome, Some(RecoveryStopReason::Unsupported));
}

/// A damaged archive is reported as damaged, not as a wrong password.
#[test]
fn a_damaged_archive_is_preserved() {
    let zip_bytes = build_zipcrypto_fixture();
    let truncated = &zip_bytes[..zip_bytes.len() / 2];
    let outcome = RecoverySearch::prepare(truncated, ConfirmationBudget::default()).err();

    assert_eq!(outcome, Some(RecoveryStopReason::ArchiveDamaged));
}

/// A cancellation stops the search at the computation boundary.
#[test]
fn a_cancellation_stops_the_search() {
    let zip_bytes = build_zipcrypto_fixture();
    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");

    let token = CancellationToken::new();
    token.cancel(CancellationReason::UserRequested);
    let outcome = search.run([FIXTURE_PASSWORD], Some(&token));

    assert_eq!(outcome, RecoverySearchOutcome::Cancelled);
}

/// A live token must not cancel the search on its own.
#[test]
fn a_live_token_does_not_cancel_the_search() {
    let zip_bytes = build_zipcrypto_fixture();
    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");

    let token = CancellationToken::new();
    let candidates: Vec<String> = (0..(SCREEN_BATCH_SIZE * 4))
        .map(|i| format!("candidate-{i}"))
        .chain([FIXTURE_PASSWORD.to_string()])
        .collect();
    let outcome = search.run(candidates.iter().map(|c| c.as_str()), Some(&token));

    assert_eq!(
        outcome,
        RecoverySearchOutcome::Confirmed {
            password: FIXTURE_PASSWORD.to_string()
        },
        "a live token must not stop the search"
    );
    assert!(!token.is_cancelled());
}

/// The search screens the candidates in batches and confirms the survivors.
#[test]
fn the_search_screens_in_batches_and_confirms_the_survivors() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);

    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let candidates: Vec<String> = (0..(SCREEN_BATCH_SIZE * 3))
        .map(|i| format!("candidate-{i}"))
        .chain([bait.clone(), FIXTURE_PASSWORD.to_string()])
        .collect();
    let outcome = search.run(candidates.iter().map(|c| c.as_str()), None);

    assert_eq!(
        outcome,
        RecoverySearchOutcome::Confirmed {
            password: FIXTURE_PASSWORD.to_string()
        }
    );
    assert_eq!(
        search.screened(),
        (SCREEN_BATCH_SIZE * 3 + 2) as u64,
        "every candidate must go through the screening stage"
    );
}

/// The confirmation reads the real archive, so the payload has to be the fixture's.
#[test]
fn the_confirmed_password_is_the_archives_own() {
    let zip_bytes = build_zipcrypto_fixture();
    let search =
        RecoverySearch::prepare(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    let outcome = search.run([FIXTURE_PASSWORD], None);

    assert_eq!(
        outcome,
        RecoverySearchOutcome::Confirmed {
            password: FIXTURE_PASSWORD.to_string()
        }
    );
    assert_eq!(fixture_plaintext().len(), 512);
}

/// Cancellation observed before exhaustion is committed must not become exhaustion.
#[test]
fn cancellation_before_exhaustion_commit_is_not_lost() {
    let bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&bytes);
    let rejected = "codex-screen-miss";
    assert!(!verify_zipcrypto_candidate(rejected, &header, check_byte));
    let search = RecoverySearch::prepare(&bytes, ConfirmationBudget::default()).expect("prepared");
    let token = CancellationToken::new();
    let mut emitted = 0;
    let candidates = std::iter::from_fn(|| {
        if emitted == SCREEN_BATCH_SIZE {
            // The first batch has finished; cancellation precedes the terminal return.
            token.cancel(CancellationReason::UserRequested);
            None
        } else {
            emitted += 1;
            Some(rejected)
        }
    });
    let outcome = search.run(candidates, Some(&token));
    assert_eq!(search.screened(), SCREEN_BATCH_SIZE as u64);
    assert_eq!(search.confirmed_attempts(), 0);
    assert!(token.is_cancelled());
    assert_eq!(outcome, RecoverySearchOutcome::Cancelled);
}

/// Candidate generation resumes across batches without skipping a length boundary.
#[test]
fn brute_force_candidates_resume_in_length_order() {
    let mut source = crate::recovery::BruteForceCandidates::new("ab", 1, 2).expect("source");
    let mut batch = Vec::new();
    assert!(source.fill_batch(&mut batch, 3));
    assert_eq!(batch, ["a", "b", "aa"]);
    assert!(source.fill_batch(&mut batch, 3));
    assert_eq!(batch, ["ab", "ba", "bb"]);
    assert!(!source.fill_batch(&mut batch, 3));
    assert!(batch.is_empty());
}
