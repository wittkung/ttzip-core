// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Gates for the search semantics of the archive-layer confirmation.
//!\n//! A candidate that is merely rejected must never stop the search, and a reason
//! that is not about the candidate must never be reported as an exhausted
//! search.

use crate::crypto::password_recovery::{recover_dictionary_rayon, RecoveryTarget};
use crate::recovery::{ConfirmationBudget, ConfirmationVerdict, PasswordConfirmation};
use super::tests_support::*;

#[test]
fn screening_only_search_reports_the_false_positive_but_confirmation_does_not() {
    let zip_bytes = build_zipcrypto_fixture();
    let (header, check_byte) = zipcrypto_screen_material(&zip_bytes);
    let bait = derive_screen_bait(&header, check_byte);

    let target = RecoveryTarget::ZipCrypto { header, check_byte };
    let candidates = [bait.clone(), FIXTURE_PASSWORD.to_string()];
    let refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();

    let found = recover_dictionary_rayon(&refs, &target, None, None);
    assert_eq!(
        found.as_deref(),
        Some(bait.as_str()),
        "the screening-only search must terminate on the false positive it finds first"
    );

    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");
    assert_eq!(
        confirmation.confirm_first(refs.iter().copied()),
        Ok(Some(FIXTURE_PASSWORD)),
        "the confirmation must reject the bait and continue the search"
    );
}

/// An ordinary entry in front of an encrypted one must not be mistaken for the
/// confirmation target.
#[test]
fn exhausted_budget_is_not_a_rejection() {
    let zip_bytes = build_zipcrypto_fixture();
    let budget = ConfirmationBudget {
        max_entry_bytes: 16,
        max_preceding_bytes: Some(16),
    };
    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, budget).expect("prepared");

    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::BudgetExhausted
    );
    assert_eq!(
        confirmation.confirm("an-unrelated-password"),
        ConfirmationVerdict::BudgetExhausted,
        "a budget limit says nothing about the candidate"
    );
    assert_eq!(
        confirmation.confirm_first([FIXTURE_PASSWORD]),
        Err(ConfirmationVerdict::BudgetExhausted),
        "a budget limit must not be reported as an exhausted search"
    );
}

/// 7z separates the correct password from a wrong one at the content level.
#[test]
fn search_preserves_the_non_candidate_reason() {
    let zip_bytes = build_zipcrypto_fixture();
    let budget = ConfirmationBudget {
        max_entry_bytes: 0,
        max_preceding_bytes: Some(0),
    };
    let confirmation = PasswordConfirmation::for_zip(&zip_bytes, budget).expect("prepared");

    assert_eq!(
        confirmation.confirm(FIXTURE_PASSWORD),
        ConfirmationVerdict::BudgetExhausted
    );
    assert_eq!(
        confirmation.confirm_first([FIXTURE_PASSWORD]),
        Err(ConfirmationVerdict::BudgetExhausted),
        "a budget limit must not be reported as an exhausted search"
    );
}

/// A search that rejected every candidate is genuinely exhausted.
#[test]
fn search_reports_a_genuine_exhaustion() {
    let zip_bytes = build_zipcrypto_fixture();
    let confirmation =
        PasswordConfirmation::for_zip(&zip_bytes, ConfirmationBudget::default()).expect("prepared");

    assert_eq!(
        confirmation.confirm_first(["wrong-a", "wrong-b"]),
        Ok(None),
        "a search that rejected every candidate is exhausted"
    );
    assert_eq!(
        confirmation.confirm_first(["wrong-a", FIXTURE_PASSWORD]),
        Ok(Some(FIXTURE_PASSWORD)),
        "a rejected candidate must not stop the search"
    );
}
