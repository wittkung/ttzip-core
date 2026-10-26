// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Engine-level password search orchestration.
//!
//! The engine owns the *orchestration* of a password search, and nothing else:
//!
//! 1. the cheap screening stage, which may only **eliminate** candidates and never
//!    proves that a candidate is the password;
//! 2. the content-level confirmation stage, which is owned by the archive layer
//!    ([`ttzip_archive::recovery`]) and decides whether a candidate is accepted.
//!
//! A screening hit is therefore only a reason to spend a confirmation on a
//! candidate. A candidate that the confirmation rejects does **not** end the search,
//! and a verdict that is not about the candidate at all — an exhausted budget, an
//! unsupported unit, a missing integrity credential, a damaged container — is
//! reported as such instead of being flattened into "not found".

use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;

use ttzip_archive::recovery::{
    ConfirmationBudget, ConfirmationVerdict, PasswordConfirmation, ScreeningMaterial,
};
use ttzip_types::CancellationToken;

use crate::crypto::password_recovery::{verify_winzip_aes_candidate, verify_zipcrypto_candidate};

/// Number of candidates screened in one parallel batch.
pub const SCREEN_BATCH_SIZE: usize = 512;

/// Why a search stopped for a reason that is **not** about the candidate.
///
/// Every value here says nothing about whether the candidate was correct, so none
/// of them may be reported as a wrong password or as an exhausted search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStopReason {
    /// The archive carries no encrypted unit that could hold a password proof.
    NoEncryptedUnit,
    /// The encrypted unit carries no usable integrity credential.
    NoCredential,
    /// The encrypted unit uses a compression method or a feature the engine cannot
    /// decode.
    Unsupported,
    /// The confirmation budget was exhausted.
    BudgetExhausted,
    /// The archive container is damaged.
    ArchiveDamaged,
}

impl RecoveryStopReason {
    /// The status code that the engine's public surfaces use for this reason.
    ///
    /// The codes are the engine's existing `TTZipStatus` values, so no public shape
    /// has to change. Two reasons share a code because the current status set has
    /// no dedicated code for them; the precise interface proposal lives in the
    /// MM37 design record.
    pub fn status_code(&self) -> i32 {
        match self {
            RecoveryStopReason::NoEncryptedUnit | RecoveryStopReason::NoCredential => {
                ttzip_types::TTZipStatus::ErrInvalidParam as i32
            }
            RecoveryStopReason::Unsupported => {
                ttzip_types::TTZipStatus::ErrUnsupportedFeature as i32
            }
            RecoveryStopReason::BudgetExhausted => {
                ttzip_types::TTZipStatus::ErrSolidBudgetExceeded as i32
            }
            RecoveryStopReason::ArchiveDamaged => ttzip_types::TTZipStatus::ErrCorruptHeader as i32,
        }
    }
}

impl From<ConfirmationVerdict> for RecoveryStopReason {
    fn from(verdict: ConfirmationVerdict) -> Self {
        match verdict {
            ConfirmationVerdict::NoCredential => RecoveryStopReason::NoCredential,
            ConfirmationVerdict::Unsupported => RecoveryStopReason::Unsupported,
            ConfirmationVerdict::BudgetExhausted => RecoveryStopReason::BudgetExhausted,
            ConfirmationVerdict::ArchiveDamaged => RecoveryStopReason::ArchiveDamaged,
            ConfirmationVerdict::Confirmed | ConfirmationVerdict::NotAccepted => {
                unreachable!("a confirmed or rejected candidate is never a stop reason")
            }
        }
    }
}

/// The terminal outcome of an engine-level password search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverySearchOutcome {
    /// A candidate was confirmed by the content-level confirmation.
    Confirmed {
        /// The confirmed password.
        password: String,
    },
    /// Every candidate that was given to the search was rejected.
    Exhausted,
    /// The search stopped for a reason that is not about the candidate.
    Stopped(RecoveryStopReason),
    /// The search was cancelled.
    Cancelled,
}

impl RecoverySearchOutcome {
    /// Reports whether the outcome is a confirmed password.
    #[inline]
    pub fn is_confirmed(&self) -> bool {
        matches!(self, RecoverySearchOutcome::Confirmed { .. })
    }

    /// Reports whether the outcome is an exhausted search.
    ///
    /// A stopped search is **not** exhausted: it never ran through the candidates.
    #[inline]
    pub fn is_exhausted(&self) -> bool {
        matches!(self, RecoverySearchOutcome::Exhausted)
    }
}

/// Metrics of a finished search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecoverySearchStats {
    /// Number of candidates that went through the screening stage.
    pub screened: u64,
    /// Number of candidates that reached the content-level confirmation.
    pub confirmed_attempts: u64,
}

/// A prepared password search against one archive.
///
/// The archive is parsed once and the confirmation target is selected once, so
/// every candidate afterwards reuses the same parsed archive and the same
/// confirmation unit.
pub struct RecoverySearch<'a> {
    confirmation: PasswordConfirmation<'a>,
    screening: Option<ScreeningMaterial>,
    budget: ConfirmationBudget,
    screened: AtomicU64,
    confirmed_attempts: AtomicU64,
}

impl<'a> RecoverySearch<'a> {
    /// Prepares a search against an in-memory archive.
    ///
    /// The archive is parsed here and only here. The preparation fails when the
    /// archive cannot be confirmed at all; those failures are reported as stop
    /// reasons and never as "not found".
    pub fn prepare(
        data: &'a [u8],
        budget: ConfirmationBudget,
    ) -> Result<Self, RecoveryStopReason> {
        let confirmation = PasswordConfirmation::for_archive(data, budget)
            .map_err(|error| match error {
                ttzip_archive::recovery::PreparationError::ArchiveDamaged => {
                    RecoveryStopReason::ArchiveDamaged
                }
                ttzip_archive::recovery::PreparationError::NoEncryptedUnit => {
                    RecoveryStopReason::NoEncryptedUnit
                }
                ttzip_archive::recovery::PreparationError::NoCredential => {
                    RecoveryStopReason::NoCredential
                }
                ttzip_archive::recovery::PreparationError::Unsupported => {
                    RecoveryStopReason::Unsupported
                }
            })?;
        let screening = confirmation.screening_material();
        Ok(Self {
            confirmation,
            screening,
            budget,
            screened: AtomicU64::new(0),
            confirmed_attempts: AtomicU64::new(0),
        })
    }

    /// The budget the search confirms under.
    #[inline]
    pub fn budget(&self) -> ConfirmationBudget {
        self.budget
    }

    /// The screening material of the confirmation target, when the format has one.
    ///
    /// 7z has none on purpose: its key derivation dominates the cost of an attempt
    /// and no fixed magic can stand in for a content-level check.
    #[inline]
    pub fn screening_material(&self) -> Option<&ScreeningMaterial> {
        self.screening.as_ref()
    }

    /// Runs the search over the given candidates.
    ///
    /// The candidates are screened in parallel batches, and only the candidates that
    /// survive the screening are confirmed. A rejected candidate never ends the
    /// search; a verdict that is not about the candidate does, and is reported.
    pub fn run<'c, I>(
        &self,
        candidates: I,
        token: Option<&CancellationToken>,
    ) -> RecoverySearchOutcome
    where
        I: IntoIterator<Item = &'c str>,
    {
        let mut batch: Vec<&str> = Vec::with_capacity(SCREEN_BATCH_SIZE);
        for candidate in candidates {
            batch.push(candidate);
            if batch.len() >= SCREEN_BATCH_SIZE {
                if let Some(outcome) = self.run_batch(&batch, token) {
                    return commit_search_outcome(outcome, token);
                }
                batch.clear();
            }
        }
        let outcome = if batch.is_empty() {
            RecoverySearchOutcome::Exhausted
        } else {
            self.run_batch(&batch, token)
                .unwrap_or(RecoverySearchOutcome::Exhausted)
        };
        commit_search_outcome(outcome, token)
    }

    /// Screens and confirms one batch, returning the outcome when the batch ends
    /// the search.
    fn run_batch(
        &self,
        batch: &[&str],
        token: Option<&CancellationToken>,
    ) -> Option<RecoverySearchOutcome> {
        // Cancellation checkpoint at the real computation boundary: a batch is only
        // screened when no cancellation is pending.
        if is_cancelled(token) {
            return Some(RecoverySearchOutcome::Cancelled);
        }

        let survivors = self.screen(batch);
        self.screened
            .fetch_add(batch.len() as u64, Ordering::Relaxed);

        for candidate in survivors {
            if is_cancelled(token) {
                return Some(RecoverySearchOutcome::Cancelled);
            }
            self.confirmed_attempts.fetch_add(1, Ordering::Relaxed);
            match self.confirmation.confirm(candidate) {
                ConfirmationVerdict::Confirmed => {
                    return Some(RecoverySearchOutcome::Confirmed {
                        password: candidate.to_string(),
                    });
                }
                ConfirmationVerdict::NotAccepted => continue,
                other => return Some(RecoverySearchOutcome::Stopped(other.into())),
            }
        }
        None
    }

    /// Returns the candidates of a batch that survive the cheap screening stage.
    ///
    /// The screening stage only eliminates candidates. A survivor is not a proof and
    /// is not reported as one; it merely earns a content-level confirmation.
    fn screen<'c>(&self, batch: &[&'c str]) -> Vec<&'c str> {
        match &self.screening {
            Some(ScreeningMaterial::ZipCrypto { header, check_byte }) => batch
                .par_iter()
                .filter(|candidate| verify_zipcrypto_candidate(candidate, header, *check_byte))
                .copied()
                .collect(),
            Some(ScreeningMaterial::WinZipAes { salt, stored_pvv }) => batch
                .par_iter()
                .filter(|candidate| verify_winzip_aes_candidate(candidate, salt, stored_pvv))
                .copied()
                .collect(),
            // 7z has no sound cheap screen, so every candidate is confirmed.
            None => batch.to_vec(),
        }
    }

    /// Counts candidates on which the format's password test actually ran.
    /// ZIP candidates undergo the cheap screen; 7z has no screen and must reach
    /// content confirmation to count as an attempted candidate.
    #[inline]
    pub fn attempted_candidates(&self) -> u64 {
        if self.screening.is_some() {
            self.screened()
        } else {
            self.confirmed_attempts()
        }
    }

    /// The number of candidates that went through the screening stage.
    #[inline]
    pub fn screened(&self) -> u64 {
        self.screened.load(Ordering::Relaxed)
    }

    /// The number of candidates that reached the content-level confirmation.
    #[inline]
    pub fn confirmed_attempts(&self) -> u64 {
        self.confirmed_attempts.load(Ordering::Relaxed)
    }
}

/// Reports whether a cancellation is pending.
#[inline]
fn is_cancelled(token: Option<&CancellationToken>) -> bool {
    token.is_some_and(|t| t.is_cancelled())
}

/// Arbitrates cancellation once when the synchronous search commits its outcome.
/// Real preparation/confirmation failures retain their non-candidate meaning.
fn commit_search_outcome(
    outcome: RecoverySearchOutcome,
    token: Option<&CancellationToken>,
) -> RecoverySearchOutcome {
    match outcome {
        RecoverySearchOutcome::Confirmed { .. } | RecoverySearchOutcome::Exhausted
            if is_cancelled(token) => RecoverySearchOutcome::Cancelled,
        other => other,
    }
}
