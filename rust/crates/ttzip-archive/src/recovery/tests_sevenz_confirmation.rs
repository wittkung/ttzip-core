// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive-layer confirmation gates for the 7z format.
//!\n//! The gates cover the per-folder encryption fact, the integrity credential that
//! the engine actually compares, and the preceding-decode budget of a solid
//! archive.

use crate::recovery::{
    ConfirmationBudget, ConfirmationVerdict, PasswordConfirmation, PreparationError,
};
use crate::sevenz::decoder::SevenZArchive;
use super::tests_support::*;

#[test]
fn sevenz_correct_and_wrong_password_are_distinguishable() {
    let sevenz_bytes = build_sevenz_fixture();
    let archive = SevenZArchive::open_slice(&sevenz_bytes).expect("7z fixture must open");
    assert!(archive.info().is_encrypted);

    let confirmation =
        PasswordConfirmation::for_seven_zip(&sevenz_bytes, ConfirmationBudget::default())
            .expect("prepared");

    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );
}

/// A 7z stream without a digest carries no integrity credential, so no candidate can
/// be confirmed.
#[test]
fn sevenz_without_digest_has_no_credential() {
    let sevenz_bytes = build_sevenz_fixture_without_digest();

    // The fixture must be a real encrypted 7z: it parses and the correct password
    // decrypts it. Only the integrity credential is missing.
    let archive = SevenZArchive::open_slice(&sevenz_bytes).expect("7z fixture must open");
    assert!(archive.info().is_encrypted);
    assert!(
        archive
            .extract_entry_bytes_stream(0, Some(FIXTURE_PASSWORD))
            .is_ok(),
        "the digest-less fixture must still decrypt correctly"
    );

    assert_eq!(
        PasswordConfirmation::for_seven_zip(&sevenz_bytes, ConfirmationBudget::default()).err(),
        Some(PreparationError::NoCredential)
    );
}

/// AE-2 separates the correct password from a wrong one without any CRC.
///
/// AE-2 stores `crc32 = 0`, so the authentication code is the only credential. The
/// confirmation must not reject such an entry because of its missing CRC.
#[test]
fn sevenz_preceding_decode_budget_boundary() {
    let sevenz_bytes = build_sevenz_two_entry_fixture();
    let archive = SevenZArchive::open_slice(&sevenz_bytes).expect("7z fixture must open");
    let second = archive.seek_index().get_by_index(1).expect("second entry");
    assert!(
        second.offset_in_folder > 0,
        "the second entry must sit behind the first one"
    );

    // A zero preceding budget forbids any decode in front of the target entry.
    let zero_budget = ConfirmationBudget {
        max_entry_bytes: 1024 * 1024,
        max_preceding_bytes: Some(0),
    };
    let confirmation =
        PasswordConfirmation::for_seven_zip_entry(&sevenz_bytes, zero_budget, 1).expect("prepared");
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::BudgetExhausted,
        "a zero preceding budget must forbid the preceding decode"
    );

    // A preceding budget that is too small for the preceding decode is exhausted too.
    let tight_budget = ConfirmationBudget {
        max_entry_bytes: 1024 * 1024,
        max_preceding_bytes: Some(16),
    };
    let confirmation =
        PasswordConfirmation::for_seven_zip_entry(&sevenz_bytes, tight_budget, 1).expect("prepared");
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::BudgetExhausted
    );

    // A budget that covers the preceding decode lets the confirmation through.
    let roomy_budget = ConfirmationBudget {
        max_entry_bytes: 1024 * 1024,
        max_preceding_bytes: Some(1024 * 1024),
    };
    let confirmation =
        PasswordConfirmation::for_seven_zip_entry(&sevenz_bytes, roomy_budget, 1).expect("prepared");
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::NotAccepted
    );

    // An unbounded preceding budget behaves like a roomy one.
    let unbounded_budget = ConfirmationBudget {
        max_entry_bytes: 1024 * 1024,
        max_preceding_bytes: None,
    };
    let confirmation =
        PasswordConfirmation::for_seven_zip_entry(&sevenz_bytes, unbounded_budget, 1).expect("prepared");
    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::Confirmed
    );
}
