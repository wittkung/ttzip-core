// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Shared staged-commit driver and result mapping for the extraction C-ABI entry points
//! (spec 229 FR-023).
//!
//! `ttzip_rust_extract_archive`, `ttzip_rust_archive_extract_unified{,_v2}`, and
//! `ttzip_rust_archive_extract_selected` all commit through [`commit_staged_with_policy`], so a
//! Stop decision, a partial commit failure, and a complete commit map to the same status codes on
//! every entry point.

use ttzip_archive::fs::collision::fs_ops::LibcFsOps;
use ttzip_archive::fs::collision::{
    CollisionAction, ExtractionOutcome, ExtractionSession, OutcomeStatus, ResolutionSet, Staged,
};

use crate::types::{
    TTZipStatus, TTZIP_COLLISION_POLICY_REPLACE, TTZIP_COLLISION_POLICY_SKIP, TTZIP_COLLISION_POLICY_STOP,
};

/// Result of committing a staged extraction through the C-ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitReport {
    /// `Ok` only for a complete commit.
    pub status: TTZipStatus,
    /// Items actually placed into the destination (`extracted + replaced + kept_both`).
    pub committed_items: usize,
    /// Relative path of the first failed item when `status == ErrExtractionFailed`.
    pub first_failed_path: Option<String>,
}

impl CommitReport {
    const fn without_commit(status: TTZipStatus) -> Self {
        Self { status, committed_items: 0, first_failed_path: None }
    }
}

/// Maps a commit outcome to the C-ABI status: only `Completed` is `Ok`.
pub(crate) fn status_for_outcome(outcome: &ExtractionOutcome) -> TTZipStatus {
    match outcome.status {
        OutcomeStatus::Completed => TTZipStatus::Ok,
        OutcomeStatus::CompletedWithFailures => TTZipStatus::ErrExtractionFailed,
        OutcomeStatus::Stopped => TTZipStatus::Cancelled,
    }
}

/// Number of destination placements performed by the commit.
pub(crate) fn committed_items(outcome: &ExtractionOutcome) -> usize {
    let total = outcome
        .extracted
        .saturating_add(outcome.replaced)
        .saturating_add(outcome.kept_both);
    usize::try_from(total).unwrap_or(usize::MAX)
}

/// Resolution set for a validated `collision_policy` (`0..=3`). `Stop` carries no default action,
/// so a conflict appearing at commit time is recorded as a failed item instead of being resolved.
fn resolutions_for_policy(policy: u8) -> ResolutionSet {
    match policy {
        TTZIP_COLLISION_POLICY_REPLACE => ResolutionSet::from_action(CollisionAction::Replace),
        TTZIP_COLLISION_POLICY_SKIP => ResolutionSet::from_action(CollisionAction::Skip),
        TTZIP_COLLISION_POLICY_STOP => ResolutionSet::new(),
        _ => ResolutionSet::from_action(CollisionAction::KeepBoth),
    }
}

/// Detects conflicts against the destination, applies `policy`, commits, and maps the outcome.
///
/// - `Stop` with any detected conflict aborts the session (staging removed, destination
///   unchanged) and returns `Cancelled`.
/// - `Completed` → `Ok`; `CompletedWithFailures` → `ErrExtractionFailed` with the partial count;
///   a detection or commit I/O error before an outcome exists → `ErrExtractionFailed`, count 0.
pub(crate) fn commit_staged_with_policy(mut staged: ExtractionSession<Staged>, policy: u8) -> CommitReport {
    let conflicts = match staged.post_stage_detect(&LibcFsOps) {
        Ok(table) => table.len(),
        Err(_) => return CommitReport::without_commit(TTZipStatus::ErrExtractionFailed),
    };
    if policy == TTZIP_COLLISION_POLICY_STOP && conflicts > 0 {
        let _ = staged.abort();
        return CommitReport::without_commit(TTZipStatus::Cancelled);
    }
    match staged.commit(&LibcFsOps, &resolutions_for_policy(policy)) {
        Ok((_committed, outcome)) => CommitReport {
            status: status_for_outcome(&outcome),
            committed_items: committed_items(&outcome),
            first_failed_path: outcome.failed.first().map(|f| f.relative_path.clone()),
        },
        Err(_) => CommitReport::without_commit(TTZipStatus::ErrExtractionFailed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(status: OutcomeStatus, extracted: u64, replaced: u64, kept_both: u64, skipped: u64) -> ExtractionOutcome {
        ExtractionOutcome { status, extracted, replaced, kept_both, skipped, ..ExtractionOutcome::default() }
    }

    #[test]
    fn only_completed_outcome_maps_to_ok() {
        assert_eq!(status_for_outcome(&outcome(OutcomeStatus::Completed, 1, 0, 0, 0)), TTZipStatus::Ok);
        assert_eq!(
            status_for_outcome(&outcome(OutcomeStatus::CompletedWithFailures, 1, 0, 0, 0)),
            TTZipStatus::ErrExtractionFailed
        );
        assert_eq!(status_for_outcome(&outcome(OutcomeStatus::Stopped, 0, 0, 0, 0)), TTZipStatus::Cancelled);
    }

    #[test]
    fn committed_items_excludes_skipped() {
        assert_eq!(committed_items(&outcome(OutcomeStatus::Completed, 2, 3, 4, 5)), 9);
    }

    #[test]
    fn stop_policy_has_no_default_action() {
        assert_eq!(resolutions_for_policy(TTZIP_COLLISION_POLICY_STOP).default_action, None);
        assert_eq!(
            resolutions_for_policy(TTZIP_COLLISION_POLICY_REPLACE).default_action,
            Some(CollisionAction::Replace)
        );
    }
}
