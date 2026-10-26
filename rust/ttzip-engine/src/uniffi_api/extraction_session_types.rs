// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! UniFFI wire records and enums of the extraction session protocol (feature 229).
//!
//! Each type mirrors a core type in `ttzip_archive::fs::collision` and converts
//! from or into it losslessly. The session state machine lives in
//! `extraction_session`, which re-exports everything declared here.

use ttzip_archive::fs::collision::{
    CollisionAction as CoreAction, CollisionPolicy as CorePolicy,
    ConflictKind as CoreConflictKind, DetectionPhase as CoreDetectionPhase,
    FailureReason as CoreFailureReason, ItemKind as CoreItemKind, ItemMeta as CoreItemMeta,
    OutcomeStatus as CoreOutcomeStatus, ResolutionOverride as CoreOverride,
    ResolutionSet as CoreResolutionSet,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ItemKind {
    File,
    Directory,
    Symlink,
}

impl From<CoreItemKind> for ItemKind {
    fn from(core: CoreItemKind) -> Self {
        match core {
            CoreItemKind::File => Self::File,
            CoreItemKind::Directory => Self::Directory,
            CoreItemKind::Symlink => Self::Symlink,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ItemMeta {
    pub kind: ItemKind,
    pub size: u64,
    pub mtime_unix_ns: i64,
}

impl From<CoreItemMeta> for ItemMeta {
    fn from(core: CoreItemMeta) -> Self {
        Self {
            kind: core.kind.into(),
            size: core.size,
            mtime_unix_ns: core.mtime_unix_ns,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ConflictKind {
    SameType,
    TypeMismatch,
}

impl From<CoreConflictKind> for ConflictKind {
    fn from(core: CoreConflictKind) -> Self {
        match core {
            CoreConflictKind::SameType => Self::SameType,
            CoreConflictKind::TypeMismatch => Self::TypeMismatch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DetectionPhase {
    Preflight,
    PostStage,
    CommitRecheck,
}

impl From<CoreDetectionPhase> for DetectionPhase {
    fn from(core: CoreDetectionPhase) -> Self {
        match core {
            CoreDetectionPhase::Preflight => Self::Preflight,
            CoreDetectionPhase::PostStage => Self::PostStage,
            CoreDetectionPhase::CommitRecheck => Self::CommitRecheck,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ConflictSummary {
    pub total: u32,
    pub same_type: u32,
    pub type_mismatch: u32,
    pub phase: DetectionPhase,
}

impl From<ttzip_archive::fs::collision::ConflictSummary> for ConflictSummary {
    fn from(core: ttzip_archive::fs::collision::ConflictSummary) -> Self {
        Self {
            total: core.total,
            same_type: core.same_type,
            type_mismatch: core.type_mismatch,
            phase: core.phase.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CollisionRecord {
    pub id: u32,
    pub relative_path: String,
    pub existing_name: String,
    pub kind: ConflictKind,
    pub existing: ItemMeta,
    pub incoming: ItemMeta,
}

impl From<ttzip_archive::fs::collision::CollisionRecord> for CollisionRecord {
    fn from(core: ttzip_archive::fs::collision::CollisionRecord) -> Self {
        Self {
            id: core.id,
            relative_path: core.relative_path,
            existing_name: core.existing_name,
            kind: core.kind.into(),
            existing: core.existing.into(),
            incoming: core.incoming.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ConflictPage {
    pub records: Vec<CollisionRecord>,
    pub total_conflicts: u32,
    pub has_more: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CollisionAction {
    Replace,
    KeepBoth,
    Skip,
}

impl From<CollisionAction> for CoreAction {
    fn from(val: CollisionAction) -> Self {
        match val {
            CollisionAction::Replace => CoreAction::Replace,
            CollisionAction::KeepBoth => CoreAction::KeepBoth,
            CollisionAction::Skip => CoreAction::Skip,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CollisionPolicy {
    Ask,
    KeepBoth,
    Replace,
    Skip,
    Stop,
}

impl CollisionPolicy {
    /// Action applied to every conflict by a non-interactive policy.
    /// `Ask` and `Stop` resolve nothing: `Ask` defers to the user, `Stop` aborts.
    pub fn default_action(self) -> Option<CollisionAction> {
        match self {
            Self::KeepBoth => Some(CollisionAction::KeepBoth),
            Self::Replace => Some(CollisionAction::Replace),
            Self::Skip => Some(CollisionAction::Skip),
            Self::Ask | Self::Stop => None,
        }
    }
}

impl From<CollisionPolicy> for CorePolicy {
    fn from(val: CollisionPolicy) -> Self {
        match val {
            CollisionPolicy::Ask => CorePolicy::Ask,
            CollisionPolicy::KeepBoth => CorePolicy::KeepBoth,
            CollisionPolicy::Replace => CorePolicy::Replace,
            CollisionPolicy::Skip => CorePolicy::Skip,
            CollisionPolicy::Stop => CorePolicy::Stop,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ResolutionOverride {
    pub conflict_id: u32,
    pub action: CollisionAction,
}

impl From<ResolutionOverride> for CoreOverride {
    fn from(val: ResolutionOverride) -> Self {
        Self {
            conflict_id: val.conflict_id,
            action: val.action.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ResolutionSet {
    pub default_action: Option<CollisionAction>,
    pub overrides: Vec<ResolutionOverride>,
}

impl From<ResolutionSet> for CoreResolutionSet {
    fn from(val: ResolutionSet) -> Self {
        Self {
            default_action: val.default_action.map(Into::into),
            overrides: val.overrides.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ExtractionRequest {
    pub archive_path: String,
    pub destination_root: String,
    pub password: Option<String>,
    pub entry_filter: Option<Vec<String>>,
    pub root_at_common_parent: bool,
    pub policy: CollisionPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct StageReport {
    pub staged_items: u64,
    pub staged_bytes: u64,
    pub post_stage_conflicts: Option<ConflictSummary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum OutcomeStatus {
    Completed,
    CompletedWithFailures,
    Stopped,
}

impl From<CoreOutcomeStatus> for OutcomeStatus {
    fn from(core: CoreOutcomeStatus) -> Self {
        match core {
            CoreOutcomeStatus::Completed => Self::Completed,
            CoreOutcomeStatus::CompletedWithFailures => Self::CompletedWithFailures,
            CoreOutcomeStatus::Stopped => Self::Stopped,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FailureReason {
    PermissionDenied,
    NotSupportedByVolume,
    NameTooLong,
    Io,
}

impl From<CoreFailureReason> for FailureReason {
    fn from(core: CoreFailureReason) -> Self {
        match core {
            CoreFailureReason::PermissionDenied => Self::PermissionDenied,
            CoreFailureReason::NotSupportedByVolume => Self::NotSupportedByVolume,
            CoreFailureReason::NameTooLong => Self::NameTooLong,
            CoreFailureReason::Io => Self::Io,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FailedItem {
    pub relative_path: String,
    pub reason: FailureReason,
    pub errno: i32,
}

impl From<ttzip_archive::fs::collision::FailedItem> for FailedItem {
    fn from(core: ttzip_archive::fs::collision::FailedItem) -> Self {
        Self {
            relative_path: core.relative_path,
            reason: core.reason.into(),
            errno: core.errno,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ExtractionOutcome {
    pub status: OutcomeStatus,
    pub extracted: u64,
    pub replaced: u64,
    pub kept_both: u64,
    pub skipped: u64,
    pub failed: Vec<FailedItem>,
}

impl ExtractionOutcome {
    /// Outcome of a session that ended before anything reached the destination.
    pub fn stopped() -> Self {
        Self {
            status: OutcomeStatus::Stopped,
            extracted: 0,
            replaced: 0,
            kept_both: 0,
            skipped: 0,
            failed: Vec::new(),
        }
    }
}

impl From<ttzip_archive::fs::collision::ExtractionOutcome> for ExtractionOutcome {
    fn from(core: ttzip_archive::fs::collision::ExtractionOutcome) -> Self {
        Self {
            status: core.status.into(),
            extracted: core.extracted,
            replaced: core.replaced,
            kept_both: core.kept_both,
            skipped: core.skipped,
            failed: core.failed.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SessionState {
    Created,
    AwaitingDecision,
    Staging,
    Staged,
    Committing,
    Completed,
    Aborted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CommitResult {
    Finished { outcome: ExtractionOutcome },
    NeedsDecision { summary: ConflictSummary },
}
