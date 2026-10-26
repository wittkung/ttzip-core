// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive-layer password confirmation.
//!
//! This module owns the content-level proof that a candidate is the password of an
//! archive. It reuses the archive engine's real decrypt, decompress, CRC and HMAC
//! entries and never re-implements them.
//!
//! The cheap screening stage stays in the crypto crate: it can only *eliminate*
//! candidates, and a screening hit is never evidence that a candidate is correct.

pub mod confirm;
pub mod target;
pub mod verdict;

#[cfg(test)]
mod tests_search_semantics;
#[cfg(test)]
mod tests_sevenz_confirmation;
#[cfg(test)]
mod tests_support;
#[cfg(test)]
mod tests_zip_confirmation;

pub use confirm::{ConfirmationBudget, PasswordConfirmation};
pub use target::{ConfirmationTarget, IntegrityCredential, ScreeningMaterial};
pub use verdict::{ConfirmationVerdict, PreparationError};
