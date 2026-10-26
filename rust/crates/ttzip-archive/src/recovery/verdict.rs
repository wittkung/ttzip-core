// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Verdict vocabulary of the archive-layer password confirmation.
//!
//! The archive layer cannot tell a wrong candidate apart from a tampered ciphertext
//! payload: both make the integrity credential fail. The verdict therefore reports
//! "not accepted" instead of "wrong password", and keeps every reason that is *not*
//! about the candidate in a variant of its own, so that a budget limit, an
//! unsupported format or a missing credential can never be presented as a rejected
//! password.

/// Verdict of one content-level confirmation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationVerdict {
    /// The selected encrypted unit decrypted and its integrity credential matched.
    Confirmed,
    /// The candidate was not accepted by the selected encrypted unit.
    ///
    /// This covers a wrong candidate and a tampered ciphertext payload alike. The
    /// archive layer has no evidence that separates the two, so neither may be
    /// reported as a proven "wrong password".
    NotAccepted,
    /// The selected encrypted unit carries no usable integrity credential, so no
    /// candidate can be confirmed or rejected on content grounds.
    NoCredential,
    /// The archive container could not be parsed, or its layout is inconsistent.
    ArchiveDamaged,
    /// The encrypted unit uses a compression method or a feature the engine cannot
    /// decode. This is a limitation of the archive, not a statement about the
    /// candidate.
    Unsupported,
    /// The confirmation attempt exceeded the configured budget before it could reach
    /// a verdict. This is not evidence about the candidate.
    BudgetExhausted,
}

impl ConfirmationVerdict {
    /// Reports whether the verdict is a positive proof that the candidate is correct.
    ///
    /// Only [`ConfirmationVerdict::Confirmed`] qualifies. Every other variant is
    /// either a rejection or a statement that no statement about the candidate could
    /// be made.
    #[inline]
    pub fn is_confirmed(&self) -> bool {
        matches!(self, ConfirmationVerdict::Confirmed)
    }

    /// Reports whether the verdict carries evidence that the candidate was rejected.
    ///
    /// A budget limit, a missing credential and a damaged container are explicitly
    /// **not** rejections: they say nothing about the candidate.
    #[inline]
    pub fn is_rejection(&self) -> bool {
        matches!(self, ConfirmationVerdict::NotAccepted)
    }
}

/// Reason why a confirmation could not be prepared at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationError {
    /// The archive container could not be parsed, or its layout is inconsistent.
    ArchiveDamaged,
    /// The archive carries no encrypted unit that could hold a password proof.
    NoEncryptedUnit,
    /// An encrypted unit exists but carries no usable integrity credential.
    NoCredential,
    /// The encrypted unit uses a compression method or a feature the engine cannot
    /// decode.
    Unsupported,
}
