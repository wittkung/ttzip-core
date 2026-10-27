// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Engine-level password search orchestration.
//!
//! The engine owns the orchestration of a password search and nothing else: the
//! cheap screening stage may only eliminate candidates, and the content-level
//! confirmation stage — owned by the archive layer — decides whether a candidate is
//! accepted. A screening hit is never reported as a confirmed password.

pub mod candidates;
pub mod search;

#[cfg(test)]
mod tests_fixtures;
#[cfg(test)]
mod tests_public_entries;
#[cfg(test)]
mod tests_search;

pub use candidates::BruteForceCandidates;
pub use ttzip_archive::recovery::ConfirmationBudget;
pub use search::{
    RecoverySearch, RecoverySearchOutcome, RecoverySearchStats, RecoveryStopReason,
    SCREEN_BATCH_SIZE,
};
