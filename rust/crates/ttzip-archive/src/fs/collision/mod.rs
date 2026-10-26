// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

pub mod fs_ops;
pub mod naming;
pub mod probe;
pub mod staging;
pub mod table;
pub mod preflight;
pub mod commit;
pub mod session;

pub use fs_ops::{map_errno, FsOps, LibcFsOps, VolCaps};
pub use staging::{StagingArea, StagingRoot, sweep_stale};
pub use table::{CollisionRecord, ConflictRef, ConflictTable, NewConflict};
pub use commit::DFSCommitWalker;
pub use session::{ExtractionSession, Created, Preflighted, Staged, Committed, Aborted};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemMeta {
    pub kind: ItemKind,
    pub size: u64,
    pub mtime_unix_ns: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    SameType,
    TypeMismatch,
}

impl ConflictKind {
    pub fn classify(existing: ItemKind, incoming: ItemKind) -> Option<Self> {
        match (existing == ItemKind::Directory, incoming == ItemKind::Directory) {
            (true, true) => None,
            (false, false) => Some(Self::SameType),
            _ => Some(Self::TypeMismatch),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionPhase {
    Preflight,
    PostStage,
    CommitRecheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictSummary {
    pub total: u32,
    pub same_type: u32,
    pub type_mismatch: u32,
    pub phase: DetectionPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionAction {
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionPolicy {
    Ask,
    KeepBoth,
    Replace,
    Skip,
    Stop,
}

impl CollisionPolicy {
    pub fn to_resolution_set(self) -> Option<ResolutionSet> {
        match self {
            Self::Ask | Self::Stop => None,
            Self::KeepBoth => Some(ResolutionSet::from_action(CollisionAction::KeepBoth)),
            Self::Replace => Some(ResolutionSet::from_action(CollisionAction::Replace)),
            Self::Skip => Some(ResolutionSet::from_action(CollisionAction::Skip)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutcomeStatus {
    #[default]
    Completed,
    CompletedWithFailures,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    PermissionDenied,
    NotSupportedByVolume,
    NameTooLong,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedItem {
    pub relative_path: String,
    pub reason: FailureReason,
    pub errno: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractionOutcome {
    pub status: OutcomeStatus,
    pub extracted: u64,
    pub replaced: u64,
    pub kept_both: u64,
    pub skipped: u64,
    pub merged_dirs: u64,
    pub failed: Vec<FailedItem>,
}

/// Disposition recorded by [`DFSCommitWalker`] for a staged entry during commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommittedEntryDisposition {
    /// Placed at its original relative path where no destination entry existed.
    Extracted,
    /// Replaced an existing destination entry at its original relative path.
    Replaced,
    /// Placed under a collision-resolved sibling name (e.g. `<stem> <n>.<ext>`).
    KeptBoth,
    /// Staged directory merged into a pre-existing destination directory (`outcome.merged_dirs`).
    MergedExistingDir,
}

/// Mapping record from a staged relative path in `staging/payload` to its destination relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedPathEntry {
    pub staged_rel_path: std::path::PathBuf,
    pub committed_rel_path: std::path::PathBuf,
    pub kind: ItemKind,
    pub disposition: CommittedEntryDisposition,
    /// True when a directory subtree was moved as a single unit (`Extracted`, `Replaced`, or
    /// `KeptBoth` on a directory), so descendant paths under `staged_rel_path` map by prefix
    /// substitution onto `committed_rel_path`.
    pub whole_subtree: bool,
}

/// Commit-time path mapping produced by [`DFSCommitWalker::commit_with_mapping`].
///
/// Allows post-commit consumers (such as `SafeExtractEngine` deferred permission/timestamp
/// restoration and selective/single-entry callers) to:
/// 1. Resolve any staged relative path to its actual committed destination relative path (including
///    when the item or an ancestor directory was renamed by `KeepBoth`);
/// 2. Distinguish newly placed directories from pre-existing destination directories that were
///    merged (`MergedExistingDir`), preserving pre-existing directory permissions;
/// 3. Skip items that were `Skip`ped or failed to commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitPathMapping {
    entries: Vec<CommittedPathEntry>,
}

impl CommitPathMapping {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: CommittedPathEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[CommittedPathEntry] {
        &self.entries
    }

    fn normalize_rel(path: &std::path::Path) -> std::path::PathBuf {
        path.components()
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .collect()
    }

    /// Resolves `staged_rel_path` to `(committed_rel_path, disposition)`.
    ///
    /// Returns `None` for items that were skipped or failed to commit.
    /// For `MergedExistingDir`, returns `Some((committed_rel_path, MergedExistingDir))` only for
    /// the exact directory path; children inside a merged directory are resolved by their own
    /// individual commit entries.
    pub fn resolve_with_disposition(
        &self,
        staged_rel_path: &std::path::Path,
    ) -> Option<(std::path::PathBuf, CommittedEntryDisposition)> {
        let norm = Self::normalize_rel(staged_rel_path);
        if norm.as_os_str().is_empty() {
            return None;
        }
        for entry in &self.entries {
            if entry.staged_rel_path == norm {
                return Some((entry.committed_rel_path.clone(), entry.disposition));
            }
        }
        for entry in &self.entries {
            if entry.whole_subtree {
                if let Ok(suffix) = norm.strip_prefix(&entry.staged_rel_path) {
                    if !suffix.as_os_str().is_empty() {
                        return Some((entry.committed_rel_path.join(suffix), entry.disposition));
                    }
                }
            }
        }
        None
    }

    /// Resolves `staged_rel_path` to its committed relative path in `destination_root` if and only
    /// if the item was newly placed (`Extracted`, `Replaced`, `KeptBoth`, or a descendant of a
    /// whole-subtree directory placement).
    ///
    /// Returns `None` for `MergedExistingDir` (pre-existing destination directories), `Skip`ped
    /// items, and failed items.
    pub fn resolve_committed_path(
        &self,
        staged_rel_path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        match self.resolve_with_disposition(staged_rel_path) {
            Some(( _, CommittedEntryDisposition::MergedExistingDir)) | None => None,
            Some((committed, _)) => Some(committed),
        }
    }

    /// Returns `true` if `rel_path` was a pre-existing directory in `destination_root` into which
    /// a staged directory was merged (`MergedExistingDir`).
    pub fn is_merged_existing_dir(&self, rel_path: &std::path::Path) -> bool {
        let norm = Self::normalize_rel(rel_path);
        self.entries.iter().any(|e| {
            e.disposition == CommittedEntryDisposition::MergedExistingDir
                && e.committed_rel_path == norm
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionOverride {
    pub conflict_id: u32,
    pub action: CollisionAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolutionSet {
    pub default_action: Option<CollisionAction>,
    pub overrides: Vec<ResolutionOverride>,
}

impl ResolutionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_action(action: CollisionAction) -> Self {
        Self {
            default_action: Some(action),
            overrides: Vec::new(),
        }
    }

    pub fn validate(&self, table: &ConflictTable) -> Result<(), CollisionError> {
        let mut last_id: Option<u32> = None;
        for o in &self.overrides {
            if o.conflict_id >= table.len() as u32 {
                return Err(CollisionError::InvalidResolution(format!(
                    "id out of bounds: {}",
                    o.conflict_id
                )));
            }
            if let Some(last) = last_id {
                if o.conflict_id <= last {
                    return Err(CollisionError::InvalidResolution(
                        "overrides not strictly sorted".to_string(),
                    ));
                }
            }
            last_id = Some(o.conflict_id);
        }
        Ok(())
    }

    pub fn action_for(&self, id: u32) -> Option<CollisionAction> {
        if let Ok(idx) = self.overrides.binary_search_by_key(&id, |o| o.conflict_id) {
            Some(self.overrides[idx].action)
        } else {
            self.default_action
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollisionError {
    #[error("CapacityExceeded")]
    CapacityExceeded,
    #[error("defect: {0}")]
    Defect(#[from] table::ConflictDefect),
    #[error("invalid resolution: {0}")]
    InvalidResolution(String),
    #[error("destination unavailable, errno {0}")]
    DestinationUnavailable(i32),
    #[error("invalid session state: expected {expected}, actual {actual}")]
    InvalidSessionState { expected: String, actual: String },
}
