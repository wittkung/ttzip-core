// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! UniFFI `ExtractionSession`: the staged, collision-aware extraction state
//! machine of feature 229. Wire types live in `extraction_session_types`.
//!
//! Lifecycle: `new -> preflight -> stage -> commit`, or `abort` at any decision
//! point. When `preflight` ran, its conflict table is authoritative for `commit`
//! (each conflict is decided exactly once); post-stage detection runs only when
//! `stage` is called without a prior `preflight`. Conflicts that appear after
//! detection are caught by the `RENAME_EXCL` re-check during commit.

use std::ffi::c_void;
use std::os::fd::AsRawFd;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use ttzip_archive::archive::unified::{
    normalize_entry_path, ArchiveEntryRef, EntrySelection, UnifiedArchiveOrchestrator,
};
use ttzip_archive::fs::collision::preflight::{post_stage_detect, preflight_detect};
use ttzip_archive::fs::collision::{
    sweep_stale, CollisionError, ConflictTable, DFSCommitWalker, DetectionPhase as CorePhase,
    ExtractionOutcome as CoreOutcome, FsOps, ItemKind as CoreItemKind,
    ItemMeta as CoreItemMeta, LibcFsOps, OutcomeStatus as CoreOutcomeStatus,
    ResolutionSet as CoreResolutionSet, StagingArea,
};

use super::callback::{UniFFICancellationToken, UniFFIProgressCallback};
use super::extraction::resolve_adaptive_extraction_threads;
use super::extraction_observer::{progress_trampoline, SessionStageObserver, StageObserver};
use super::types::TTZipError;
use crate::crypto::ZeroizingString;
use crate::types::{TTZipExtractOptions, TTZipStatus};

pub use super::extraction_session_types::*;

const PREFLIGHT_STATES: &str = "Created or AwaitingDecision(Preflight)";
const COMMIT_STATES: &str = "Staged or AwaitingDecision(PostStage)";
const ABORT_STATES: &str = "Created, AwaitingDecision, Staged or Aborted";

/// Entries the archive will stage, as counted by the listing pass.
#[derive(Clone, Copy, Debug)]
struct StagePlan {
    items: u64,
    bytes: u64,
}

/// Result of `preflight`: the staging plan and the conflicts it found.
struct Preflight {
    plan: StagePlan,
    table: ConflictTable,
}

enum CommitPlan {
    StopBeforeCommit,
    Proceed {
        dest_root: PathBuf,
        table: ConflictTable,
        staging: StagingArea,
        effective_resolutions: CoreResolutionSet,
    },
}

enum SessionSlot {
    /// `preflight` is `Some` (with an empty table) once a conflict-free preflight ran.
    Created { dest_root: PathBuf, preflight: Option<Preflight> },
    AwaitingPreflightDecision { dest_root: PathBuf, preflight: Preflight },
    Staging,
    Staged { dest_root: PathBuf, table: ConflictTable, staging: StagingArea },
    AwaitingStageDecision { dest_root: PathBuf, table: ConflictTable, staging: StagingArea },
    Committing,
    Completed,
    Aborted,
    Failed,
}

impl SessionSlot {
    fn state(&self) -> SessionState {
        match self {
            Self::Created { .. } => SessionState::Created,
            Self::AwaitingPreflightDecision { .. } | Self::AwaitingStageDecision { .. } => {
                SessionState::AwaitingDecision
            }
            Self::Staging => SessionState::Staging,
            Self::Staged { .. } => SessionState::Staged,
            Self::Committing => SessionState::Committing,
            Self::Completed => SessionState::Completed,
            Self::Aborted => SessionState::Aborted,
            Self::Failed => SessionState::Failed,
        }
    }

    /// Conflict table awaiting a caller decision, if any.
    fn pending_table(&self) -> Option<&ConflictTable> {
        match self {
            Self::AwaitingPreflightDecision { preflight, .. } => Some(&preflight.table),
            Self::AwaitingStageDecision { table, .. } => Some(table),
            _ => None,
        }
    }
}

#[derive(uniffi::Object)]
pub struct ExtractionSession {
    request: ExtractionRequest,
    selection: Option<EntrySelection>,
    slot: Mutex<SessionSlot>,
}

#[uniffi::export]
impl ExtractionSession {
    /// Validates the request and resolves the destination. Touches nothing in the
    /// destination except sweeping stale staging directories of dead processes.
    #[uniffi::constructor]
    pub fn new(request: ExtractionRequest) -> Result<Arc<Self>, TTZipError> {
        if !Path::new(&request.archive_path).exists() {
            return Err(TTZipError::FileNotFound { path: request.archive_path });
        }
        let selection = request
            .entry_filter
            .as_ref()
            .map(|filter| EntrySelection::new(filter, request.root_at_common_parent));
        if selection.as_ref().is_some_and(EntrySelection::is_empty) {
            return Err(TTZipError::InvalidArgument {
                message: "entry_filter must select at least one entry".to_string(),
            });
        }
        let dest_root = resolve_destination(&request.destination_root)?;
        sweep_stale(&dest_root);
        Ok(Arc::new(Self {
            request,
            selection,
            slot: Mutex::new(SessionSlot::Created { dest_root, preflight: None }),
        }))
    }

    /// Lists the archive and detects conflicts with the destination.
    /// State: Created -> Created | AwaitingDecision.
    pub fn preflight(&self) -> Result<Option<ConflictSummary>, TTZipError> {
        let mut slot = self.lock_slot();
        let dest_root = match &*slot {
            SessionSlot::Created { dest_root, .. } => dest_root.clone(),
            SessionSlot::AwaitingPreflightDecision { preflight, .. } => {
                return Ok(Some(preflight.table.summary(CorePhase::Preflight).into()));
            }
            other => return Err(invalid_state(PREFLIGHT_STATES, other)),
        };
        let scan = self.scan_entries()?;
        let table = detect_preflight(&dest_root, &scan.items)?;
        let preflight = Preflight { plan: scan.plan, table };
        if preflight.table.is_empty() {
            *slot = SessionSlot::Created { dest_root, preflight: Some(preflight) };
            return Ok(None);
        }
        let summary = preflight.table.summary(CorePhase::Preflight).into();
        *slot = SessionSlot::AwaitingPreflightDecision { dest_root, preflight };
        Ok(Some(summary))
    }

    /// Pages through the conflict table awaiting a decision. `limit` is clamped to 1024.
    pub fn conflict_page(&self, offset: u32, limit: u32) -> Result<ConflictPage, TTZipError> {
        let slot = self.lock_slot();
        let Some(table) = slot.pending_table() else {
            return Err(invalid_state("AwaitingDecision", &slot));
        };
        let limit = limit.min(1024) as usize;
        let total = table.len();
        let records = table
            .iter_from(offset)
            .take(limit)
            .map(|r| r.to_record().into())
            .collect();
        Ok(ConflictPage {
            records,
            total_conflicts: u32::try_from(total).unwrap_or(u32::MAX),
            has_more: (offset as usize).saturating_add(limit) < total,
        })
    }

    /// Decompresses the selected entries into the session's staging directory.
    /// Blocking. `progress` returning false or `cancel` firing aborts the session.
    /// State: Created | AwaitingDecision(Preflight) -> Staged | AwaitingDecision(PostStage).
    pub fn stage(
        &self,
        progress: Option<Box<dyn UniFFIProgressCallback>>,
        cancel: Option<Arc<UniFFICancellationToken>>,
    ) -> Result<StageReport, TTZipError> {
        self.stage_observed(&SessionStageObserver {
            progress: progress.as_deref(),
            cancel: cancel.as_deref(),
        })
    }

    /// Moves staged items into the destination according to `resolutions`.
    /// State: Staged | AwaitingDecision(PostStage) -> Completed | Aborted | Failed.
    pub fn commit(&self, resolutions: ResolutionSet) -> Result<CommitResult, TTZipError> {
        let resolutions: CoreResolutionSet = resolutions.into();
        let (dest_root, table, staging, effective_resolutions) = match self.begin_commit(&resolutions)? {
            CommitPlan::StopBeforeCommit => {
                return Ok(CommitResult::Finished {
                    outcome: ExtractionOutcome::stopped(),
                });
            }
            CommitPlan::Proceed {
                dest_root,
                table,
                staging,
                effective_resolutions,
            } => (dest_root, table, staging, effective_resolutions),
        };
        let result = commit_staged(&dest_root, &table, &staging, &effective_resolutions);
        let mut slot = self.lock_slot();
        match result {
            Ok(outcome) => {
                let has_placed_items = (outcome.extracted
                    + outcome.replaced
                    + outcome.kept_both
                    + outcome.merged_dirs)
                    > 0;
                if outcome.status == CoreOutcomeStatus::Completed || has_placed_items {
                    staging.commit();
                }
                *slot = SessionSlot::Completed;
                Ok(CommitResult::Finished { outcome: outcome.into() })
            }
            Err(err) => {
                *slot = SessionSlot::Failed;
                Err(err)
            }
        }
    }

    /// Stops the session: deletes staging and a destination root this session
    /// created. Idempotent. A running `stage` is stopped through its cancellation
    /// token instead, and a running `commit` cannot be interrupted.
    pub fn abort(&self) -> Result<ExtractionOutcome, TTZipError> {
        let mut slot = self.lock_slot();
        match &*slot {
            SessionSlot::Created { .. }
            | SessionSlot::AwaitingPreflightDecision { .. }
            | SessionSlot::AwaitingStageDecision { .. }
            | SessionSlot::Staged { .. }
            | SessionSlot::Aborted => {}
            other => return Err(invalid_state(ABORT_STATES, other)),
        }
        // Dropping the previous slot drops its `StagingArea`, which removes the staging tree.
        drop(std::mem::replace(&mut *slot, SessionSlot::Aborted));
        Ok(ExtractionOutcome::stopped())
    }

    pub fn state(&self) -> SessionState {
        self.lock_slot().state()
    }
}

impl ExtractionSession {
    /// `stage` with an arbitrary progress/cancellation observer; shared with the
    /// non-interactive one-shot functions.
    pub(crate) fn stage_observed<O: StageObserver>(
        &self,
        observer: &O,
    ) -> Result<StageReport, TTZipError> {
        let (dest_root, preflight) = self.begin_staging()?;
        let result = self.stage_into(dest_root, preflight, observer);
        let mut slot = self.lock_slot();
        match result {
            Ok((report, next)) => {
                *slot = next;
                Ok(report)
            }
            Err(err) => {
                *slot = match err {
                    TTZipError::Cancelled => SessionSlot::Aborted,
                    _ => SessionSlot::Failed,
                };
                Err(err)
            }
        }
    }

    fn lock_slot(&self) -> MutexGuard<'_, SessionSlot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn begin_staging(&self) -> Result<(PathBuf, Option<Preflight>), TTZipError> {
        let mut slot = self.lock_slot();
        match std::mem::replace(&mut *slot, SessionSlot::Staging) {
            SessionSlot::Created { dest_root, preflight } => Ok((dest_root, preflight)),
            SessionSlot::AwaitingPreflightDecision { dest_root, preflight } => {
                Ok((dest_root, Some(preflight)))
            }
            other => {
                let err = invalid_state(PREFLIGHT_STATES, &other);
                *slot = other;
                Err(err)
            }
        }
    }

    fn stage_into<O: StageObserver>(
        &self,
        dest_root: PathBuf,
        preflight: Option<Preflight>,
        observer: &O,
    ) -> Result<(StageReport, SessionSlot), TTZipError> {
        let plan = match &preflight {
            Some(p) => p.plan,
            None => self.scan_entries()?.plan,
        };
        let staging = StagingArea::create(&dest_root).map_err(destination_error)?;
        self.extract_into(&staging, observer)?;
        if observer.is_cancelled() {
            return Err(TTZipError::Cancelled);
        }
        let report = |post_stage_conflicts| StageReport {
            staged_items: plan.items,
            staged_bytes: plan.bytes,
            post_stage_conflicts,
        };
        if let Some(Preflight { table, .. }) = preflight {
            return Ok((report(None), SessionSlot::Staged { dest_root, table, staging }));
        }
        let table = detect_post_stage(&dest_root, &staging)?;
        if table.is_empty() {
            return Ok((report(None), SessionSlot::Staged { dest_root, table, staging }));
        }
        let summary = table.summary(CorePhase::PostStage).into();
        let next = SessionSlot::AwaitingStageDecision { dest_root, table, staging };
        Ok((report(Some(summary)), next))
    }

    fn extract_into<O: StageObserver>(
        &self,
        staging: &StagingArea,
        observer: &O,
    ) -> Result<(), TTZipError> {
        let secret = self.request.password.clone().map(ZeroizingString::new);
        let password = secret
            .as_ref()
            .map(ZeroizingString::to_c_string)
            .transpose()
            .map_err(|_| TTZipError::InvalidArgument {
                message: "password must not contain NUL bytes".to_string(),
            })?;
        let options = TTZipExtractOptions {
            password: password.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            thread_budget: resolve_adaptive_extraction_threads(0),
            progress_callback: Some(progress_trampoline::<O>),
            user_data: (observer as *const O).cast_mut().cast::<c_void>(),
            ..TTZipExtractOptions::default()
        };
        let archive = Path::new(&self.request.archive_path);
        let payload = staging.payload_dir();
        match &self.selection {
            Some(selection) => {
                UnifiedArchiveOrchestrator::extract_selected_entries(archive, selection, &payload, &options)
                    .map(|_| ())
            }
            None => UnifiedArchiveOrchestrator::extract_archive_with_metrics(archive, &payload, &options)
                .map(|_| ()),
        }
        .map_err(status_error)
    }

    fn begin_commit(
        &self,
        resolutions: &CoreResolutionSet,
    ) -> Result<CommitPlan, TTZipError> {
        let mut slot = self.lock_slot();
        let all_covered = match &*slot {
            SessionSlot::Staged { table, .. } | SessionSlot::AwaitingStageDecision { table, .. } => {
                resolutions.validate(table).map_err(collision_error)?;
                table.is_empty()
                    || resolutions.default_action.is_some()
                    || resolutions.overrides.len() == table.len()
            }
            other => return Err(invalid_state(COMMIT_STATES, other)),
        };

        if !all_covered {
            match self.request.policy {
                CollisionPolicy::Stop => {
                    drop(std::mem::replace(&mut *slot, SessionSlot::Aborted));
                    return Ok(CommitPlan::StopBeforeCommit);
                }
                CollisionPolicy::Ask => {
                    return Err(TTZipError::InvalidResolution {
                        detail: "unresolved conflicts remain without default_action or override"
                            .to_string(),
                    });
                }
                CollisionPolicy::KeepBoth
                | CollisionPolicy::Replace
                | CollisionPolicy::Skip => {}
            }
        }

        let mut effective_resolutions = resolutions.clone();
        if effective_resolutions.default_action.is_none() {
            effective_resolutions.default_action =
                self.request.policy.default_action().map(Into::into);
        }

        match std::mem::replace(&mut *slot, SessionSlot::Committing) {
            SessionSlot::Staged { dest_root, table, staging }
            | SessionSlot::AwaitingStageDecision { dest_root, table, staging } => {
                Ok(CommitPlan::Proceed {
                    dest_root,
                    table,
                    staging,
                    effective_resolutions,
                })
            }
            other => {
                let err = invalid_state(COMMIT_STATES, &other);
                *slot = other;
                Err(err)
            }
        }
    }

    /// Lists the archive once: destination-relative target paths, sorted
    /// component-wise as `preflight_detect` requires, plus the staging plan.
    fn scan_entries(&self) -> Result<EntryScan, TTZipError> {
        let mut items = Vec::new();
        let mut bytes = 0u64;
        UnifiedArchiveOrchestrator::inspect_archive_safe(
            Path::new(&self.request.archive_path),
            self.request.password.as_deref(),
            true,
            |entry| {
                if let Some(target) = self.target_path(entry.path) {
                    bytes = bytes.saturating_add(entry.uncompressed_size);
                    items.push((PathBuf::from(target), item_meta(entry)));
                }
                true
            },
        )
        .map_err(status_error)?;
        items.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let plan = StagePlan { items: items.len() as u64, bytes };
        Ok(EntryScan { items, plan })
    }

    /// Destination-relative path of an entry, or `None` when the entry is not
    /// selected or cannot be placed inside the destination (empty or `..`).
    fn target_path<'a>(&self, entry_path: &'a str) -> Option<&'a str> {
        let path = match &self.selection {
            Some(selection) => selection.output_path(entry_path)?,
            None => normalize_entry_path(entry_path),
        }
        .trim_end_matches('/');
        let contained = !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        contained.then_some(path)
    }
}

struct EntryScan {
    items: Vec<(PathBuf, CoreItemMeta)>,
    plan: StagePlan,
}

fn item_meta(entry: &ArchiveEntryRef<'_>) -> CoreItemMeta {
    let kind = if entry.is_directory {
        CoreItemKind::Directory
    } else if (entry.mode & libc::S_IFMT as u32) == libc::S_IFLNK as u32 {
        CoreItemKind::Symlink
    } else {
        CoreItemKind::File
    };
    CoreItemMeta {
        kind,
        size: entry.uncompressed_size,
        mtime_unix_ns: entry.mtime_epoch_secs.saturating_mul(1_000_000_000),
    }
}

/// Canonicalizes the deepest existing ancestor of `raw` and re-appends the
/// missing tail. The tail is created later by staging, which records that it
/// did so and removes it again on abort.
fn resolve_destination(raw: &str) -> Result<PathBuf, TTZipError> {
    let mut existing = PathBuf::from(raw);
    let mut missing = Vec::new();
    while !existing.exists() {
        let (Some(name), Some(parent)) = (existing.file_name(), existing.parent()) else {
            return Err(TTZipError::InvalidArgument {
                message: format!("destination_root has no existing ancestor: {raw}"),
            });
        };
        missing.push(name.to_owned());
        existing = if parent.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            parent.to_path_buf()
        };
    }
    let mut resolved = std::fs::canonicalize(&existing).map_err(destination_error)?;
    resolved.extend(missing.iter().rev());
    Ok(resolved)
}

fn detect_preflight(
    dest_root: &Path,
    items: &[(PathBuf, CoreItemMeta)],
) -> Result<ConflictTable, TTZipError> {
    let fs = LibcFsOps;
    let dest = match fs.openat_dir_nofollow(libc::AT_FDCWD, dest_root) {
        Ok(fd) => fd,
        // A destination that does not exist yet cannot conflict with anything.
        Err(e) if e.raw_os_error() == Some(libc::ENOENT) => {
            return ConflictTable::builder().build().map_err(collision_error);
        }
        Err(e) => return Err(destination_error(e)),
    };
    preflight_detect(&fs, dest.as_raw_fd(), dest_root, items).map_err(collision_error)
}

fn detect_post_stage(dest_root: &Path, staging: &StagingArea) -> Result<ConflictTable, TTZipError> {
    let fs = LibcFsOps;
    let dest = fs
        .openat_dir_nofollow(libc::AT_FDCWD, dest_root)
        .map_err(destination_error)?;
    post_stage_detect(&fs, dest.as_raw_fd(), &staging.payload_dir().0).map_err(collision_error)
}

fn commit_staged(
    dest_root: &Path,
    table: &ConflictTable,
    staging: &StagingArea,
    resolutions: &CoreResolutionSet,
) -> Result<CoreOutcome, TTZipError> {
    let fs = LibcFsOps;
    let dest = fs
        .openat_dir_nofollow(libc::AT_FDCWD, dest_root)
        .map_err(destination_error)?;
    let payload = fs
        .openat_dir_nofollow(libc::AT_FDCWD, &staging.payload_dir().0)
        .map_err(|e| TTZipError::io_error(e, "open staging payload"))?;
    let displaced = fs
        .openat_dir_nofollow(libc::AT_FDCWD, &staging.displaced_dir())
        .map_err(|e| TTZipError::io_error(e, "open staging displaced"))?;
    // Unknown capabilities select the conservative non-atomic fallbacks.
    let caps = fs.probe_vol_caps(dest_root).unwrap_or_default();
    let payload_path = staging.payload_dir().0;
    let (mut outcome, committed_mapping) =
        DFSCommitWalker::new(&fs, dest.as_raw_fd(), payload.as_raw_fd(), displaced.as_raw_fd(), caps)
            .commit_with_mapping(table, resolutions)
            .map_err(|e| TTZipError::io_error(e, "commit staged items"))?;
    ttzip_archive::fs::staged_metadata::apply_committed_metadata(
        &payload_path,
        dest_root,
        &committed_mapping,
        &mut outcome,
    );
    Ok(outcome)
}



fn invalid_state(expected: &str, actual: &SessionSlot) -> TTZipError {
    TTZipError::InvalidSessionState {
        expected: expected.to_string(),
        actual: actual.state(),
    }
}

fn destination_error(err: std::io::Error) -> TTZipError {
    TTZipError::DestinationUnavailable { errno: err.raw_os_error().unwrap_or(0) }
}

fn collision_error(err: CollisionError) -> TTZipError {
    match err {
        CollisionError::DestinationUnavailable(errno) => TTZipError::DestinationUnavailable { errno },
        CollisionError::InvalidResolution(detail) => TTZipError::InvalidResolution { detail },
        other => TTZipError::IoError { message: other.to_string() },
    }
}

/// Maps a decoder status to the typed UniFFI error.
pub(crate) fn status_error(status: TTZipStatus) -> TTZipError {
    match status {
        TTZipStatus::Cancelled => TTZipError::Cancelled,
        TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        TTZipStatus::ErrOutOfMemory => {
            TTZipError::out_of_memory("Extraction memory allocation limit exceeded")
        }
        TTZipStatus::ErrSecurityViolation => TTZipError::decompression_bomb(
            "Decompression ratio threshold exceeded or overlapping entry detected",
        ),
        TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
            details: "Corrupted archive header or checksum mismatch".to_string(),
            offset: 0,
        },
        other => TTZipError::EngineError { code: other as i32 },
    }
}

#[uniffi::export]
pub fn sweep_stale_staging(destination_root: String) -> Result<u32, TTZipError> {
    Ok(sweep_stale(Path::new(&destination_root)))
}
