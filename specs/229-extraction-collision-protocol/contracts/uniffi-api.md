# UniFFI API Contract: Extraction Session

**Feature**: `229-extraction-collision-protocol` | Wire records: [extraction-session.schema.json](./extraction-session.schema.json)

This contract replaces every silent-overwrite extraction entry point. It is a breaking change of the `ttzip_engine` UniFFI surface; generated bindings (Swift, C++, C#, Python) are regenerated in the same change.

## 1. Interactive path: `ExtractionSession` object

```rust
#[derive(uniffi::Object)]
pub struct ExtractionSession { /* Mutex<SessionSlot> */ }

#[uniffi::export]
impl ExtractionSession {
    /// Validates the request, resolves `destination_root` with realpath, sweeps stale
    /// staging directories, registers the session. Touches nothing in the destination.
    #[uniffi::constructor]
    pub fn new(request: ExtractionRequest) -> Result<Arc<Self>, TTZipError>;

    /// Cheap conflict detection from the archive entry list (ZIP, 7z). Returns
    /// `None` when the format has no cheap listing; the caller then calls `stage`
    /// and reads post-stage conflicts. State: Created -> Created | AwaitingDecision.
    pub fn preflight(&self) -> Result<Option<ConflictSummary>, TTZipError>;

    /// Pages through the current conflict table. `limit` is clamped to 1024.
    pub fn conflict_page(&self, offset: u32, limit: u32) -> Result<ConflictPage, TTZipError>;

    /// Decompresses into the session's staging directory using the same parallel
    /// decoders as today. Skipped items are staged and discarded at commit; filtering
    /// them out here would force the slower selective path. Blocking; call off the
    /// main thread. Progress and cancellation reuse the existing callback types;
    /// a fired token or a callback returning `false` ends in `Cancelled` / Aborted.
    /// When `preflight` ran, its table is authoritative for `commit` and
    /// `post_stage_conflicts` is `None`, so every conflict is decided exactly once;
    /// post-stage detection runs only when `stage` is called without `preflight`.
    /// State: Created | AwaitingDecision(Preflight) -> Staged | AwaitingDecision(PostStage).
    pub fn stage(
        &self,
        progress: Option<Box<dyn UniFFIProgressCallback>>,
        cancel: Option<Arc<UniFFICancellationToken>>,
    ) -> Result<StageReport, TTZipError>;

    /// Moves staged items into the destination according to `resolutions`. The caller
    /// passes the union of every decision made so far in this session. Re-checks every
    /// target at rename time (`RENAME_EXCL`). Uncovered new conflicts stop the walk
    /// and return `CommitResult::NeedsDecision`; already-moved items stay moved and the
    /// next `commit` resumes from the cursor.
    /// State: Staged | AwaitingDecision(CommitRecheck) -> Completed | AwaitingDecision.
    pub fn commit(&self, resolutions: ResolutionSet) -> Result<CommitResult, TTZipError>;

    /// Stop. Deletes staging, removes `destination_root` if this session created it.
    /// Valid in Created, AwaitingDecision and Staged; idempotent in Aborted. A running
    /// `stage` is stopped through its cancellation token; `commit` cannot be interrupted.
    pub fn abort(&self) -> Result<ExtractionOutcome, TTZipError>;

    pub fn state(&self) -> SessionState;
}

#[derive(uniffi::Enum)]
pub enum CommitResult {
    Finished { outcome: ExtractionOutcome },
    NeedsDecision { summary: ConflictSummary },
}
```

Threading: every method is a blocking call on the caller's thread; `ExtractionSession: Send + Sync`. `stage` is long-running and must be called from the extraction queue, never from the main actor. `preflight`, `conflict_page`, `commit` and `abort` are bounded by the number of conflicts / staged top-level items.

Drop semantics: dropping the last reference in a non-terminal state performs `abort()`.

## 2. Non-interactive path: one-shot functions

Every disk-writing one-shot function gains a required `policy: CollisionPolicy` parameter; `overwrite_existing` is removed from every options record. `CollisionPolicy::Ask` returns `TTZipError::InvalidArgument`. Internally each function is `ExtractionSession::new → preflight → stage → commit` with the implicit resolution set of the policy.

| Function | Change |
|---|---|
| `extract_archive_stream` (extraction.rs) | `+ policy: CollisionPolicy`; return type gains `ExtractionOutcome` counts |
| `extract_selected_entries` (extraction.rs) | **Deleted** — no caller outside generated bindings after US2; selective and single-entry extraction use `ExtractionSession` with `entry_filter` (directories select their descendants) and `root_at_common_parent` |
| `extract_archive_stream_advanced` (extraction.rs) | **Deleted** — no caller outside generated bindings |
| `extract_single_entry_to_path` (extraction.rs) | **Deleted** — no caller outside generated bindings; its remove-then-rename sequence disappears with it |
| `extract_single_entry_by_path` (extraction.rs) | Unchanged — returns bytes in memory, writes nothing |
| `resolve_smart_extract_decision` (smart_extract.rs) | Unchanged — decides the wrapper folder name only; extraction itself goes through the session |
| `verify_archive_integrity` (integrity.rs) | Unchanged behavior — `dry_run` into a private temp dir; only the removed field is dropped from its options literal |
| `sweep_stale_staging(destination_root: String) -> Result<u32, TTZipError>` | **New** — see data-model §3.2 |

C-ABI (`ttzip-types/src/options/mod.rs`, `#[repr(C)] TTZipExtractOptions`): `overwrite_existing: bool` becomes `collision_policy: u8` at the same offset and size (`0 = KeepBoth, 1 = Replace, 2 = Skip, 3 = Stop`; any other value → `ErrInvalidParam`); `TTZIP_ABI_VERSION_3`; version-2 structs are rejected with `ErrAbiMismatch` so no caller can keep silent overwrite by accident. Callers: `ffi/archive_ffi/{extract,unified}.rs`, `ttzip-python`, `ttzip-tui` (gains `--on-conflict keep-both|replace|skip|stop`, default `keep-both`), benches, tests.

## 3. Errors (additions to `TTZipError`)

| Variant | When |
|---|---|
| `InvalidSessionState { expected: String, actual: SessionState }` | Method called in the wrong state |
| `InvalidResolution { detail: String }` | Override id out of range, duplicate id, unsorted overrides |
| `DestinationUnavailable { errno: i32 }` | `destination_root` cannot be created/opened |

Per-item filesystem errors during commit are not errors of the call; they are `ExtractionOutcome.failed` entries.

## 4. Swift facade contract (TTZipCore)

```swift
public enum CollisionHandling: Sendable {
    case policy(CollisionPolicy)                 // non-interactive callers
    case interactive(CollisionDecisionProvider)  // UI callers
}

public protocol CollisionDecisionProvider: Sendable {
    /// Called on the main actor. Pages via `fetch`; returns the user's decision or
    /// nil for Stop. Must not be called for policy-driven sessions.
    @MainActor
    func decide(summary: ConflictSummary,
                fetch: @escaping @Sendable (UInt32, UInt32) async throws -> ConflictPage)
        async -> ResolutionSet?
}

extension TTZipEngineFacade {
    public func extract(_ request: ExtractionRequest,
                        handling: CollisionHandling,
                        progress: ProgressSink?) async throws -> ExtractionOutcome

    public func quickExtract(archivePath: String,
                             destinationDir: String,
                             password: String?,
                             autoVaultUnlock: Bool,
                             handling: CollisionHandling,
                             filterOptions: ArchiveFilterOptions,
                             progress: (@Sendable (ArchiveProgress) -> Void)?,
                             token: CancellationToken?) async throws -> ExtractResult
}
```

The facade owns the state-machine loop (preflight → decide → stage → decide → commit → decide …). No Rust thread ever waits for Swift.

### 4.1 High-level Swift return model & outcome propagation (`quickExtract` / `ExtractResult` / `ArchiveError`)

1. **`ExtractResult` carries `outcome: ExtractionOutcome?`**:
   - When `TTZipEngineFacade.quickExtract` completes with `outcome.status == .completed`, it returns `ExtractResult` populated with `outcome: outcome` (exposing `extracted`, `replaced`, `keptBoth`, `skipped`, `failed`, and `status`).
2. **Non-success terminal outcomes in `quickExtract` and `executePipelineExtract`**:
   - `outcome.status == .stopped`: throws `ArchiveError.extractionStopped` (never returns ordinary success, never triggers post-extract reveal or source archive deletion).
   - `outcome.status == .completedWithFailures`: throws `ArchiveError.partialExtractionFailure(outcome: outcome)` carrying the full `ExtractionOutcome` and `FailedItem` list (never returns ordinary success).
   - Mid-operation cancellation (`Task.isCancelled` / `CancellationToken.isCancelled()` / `TTZipError.Cancelled`): throws `ArchiveError.cancelled`.
3. **Commit Point of No Return & Dual Cancellation Checks (`TTZipEngineFacade.extract`)**:
   - Each call to `session.commit(resolutions:)` is the **Commit Point of No Return** for the resolved items.
   - Immediately before every `session.commit(resolutions:)` invocation (after `stage`, after `postStageConflicts` `provider.decide`, and inside the `.needsDecision` loop after `provider.decide` or `.policy`), `extract` checks `if Task.isCancelled || cancelToken.isCancelled()` (where `cancelToken` reflects external `CancellationToken`), aborts the session (`session.abort()`), and throws `ArchiveError.cancelled` without mutating `destination_root`.
4. **Caller Inventory for `quickExtract` / `ExtractResult`**:

| Caller | Location | Handling of `.completed` / `.stopped` / `.completedWithFailures` |
|---|---|---|
| `TTZipEngineFacade.quickExtract` | `products/ttzip/core/Sources/TTZipCore/Facades/TTZipEngineFacade.swift` | Inspects `outcome.status`; returns `ExtractResult(..., outcome: outcome)` on `.completed`, throws `.extractionStopped` on `.stopped`, throws `.partialExtractionFailure(outcome:)` on `.completedWithFailures` |
| `TTZipEngineFacade.executePipelineExtract` | `products/ttzip/core/Sources/TTZipCore/Facades/TTZipEngineFacade.swift` | Inspects `outcome.status`; throws `.extractionStopped` on `.stopped` and `.partialExtractionFailure(outcome:)` on `.completedWithFailures` before constructing `ArchiveOperationResult` |
| `AppViewState.quickExtractArchive` | `products/ttzip/apple/Sources/TTZipApp/ViewModels/AppViewState+ArchiveOperations.swift` | On `.completed`: updates status, reveals in Finder, and optionally trashes source archive. On `.extractionStopped`: sets status to `"Extraction stopped: <name>"`, does not reveal or trash source. On `.partialExtractionFailure(let outcome)`: sets status to `"Extracted <name> with <N> failed item(s)"`, reports error, never trashes source |
| `AppViewState.extractArchive` | `products/ttzip/apple/Sources/TTZipApp/ViewModels/AppViewState+ArchiveOperations.swift` | Updates `taskState.statusMessage` distinctly for `.completed`, `.extractionStopped`, `.partialExtractionFailure`, and `.cancelled`, then rethrows non-completed errors to caller |
| `ExtractCommand.execute` | `products/ttzip/core/Sources/TTZipCore/Commands/ExtractCommand.swift` | Calls `engineFacade.quickExtract`; on any thrown error (including `.extractionStopped` and `.partialExtractionFailure`), rolls back newly created paths while preserving pre-existing files |
| `ArchiveBatchFacade.executeSingleExtractTask` | `products/ttzip/core/Sources/TTZipCore/Facades/ArchiveBatchFacade.swift` | Catches thrown `.extractionStopped` / `.partialExtractionFailure` and records `BatchTaskResult(success: false, errorMessage: ...)` |
| `TTZipEngine.extract` / `extractDirect` & `TTZipArchive.extract` / `extractDirect` | `products/ttzip/core/Sources/TTZipCore/TTZipEngine.swift` | Propagates `ExtractResult` (with `outcome`) on `.completed` and propagates thrown `ArchiveError` on `.extractionStopped` / `.partialExtractionFailure` / `.cancelled` |

### 4.2 Archive Creation Transaction Contract (`create_archive_stream` / `create_archive_advanced`)

Callers pass an explicit `CreateCommitMode` (`SingleFile` when `split_volume_size_bytes == 0`, or `SplitVolume(VolumeNamingScheme)` when `split_volume_size_bytes > 0`) rather than inferring split mode from filenames:

1. **Single-File Archive Creation (`CreateCommitMode::SingleFile`, POSIX Atomic Replacement)**:
   - Staged in `<parent>/.ttzip_create_txn_<pid>_<ts>_<seq>/<file_name>` and installed via a single `rename(staged_file, destination_path)`, even if `<file_name>` ends with `.001` (e.g., `report.001`).
   - Zero missing-target window; neighboring files (`report.002`, `report.003`, `report.zip.1`, `report.zip.2026`, etc.) are never inspected or modified.
2. **Multi-Volume Split Archive Creation (`CreateCommitMode::SplitVolume(VolumeNamingScheme)`, Two-Phase Rollback Transaction & Backup Preservation)**:
   - Enters the two-phase `.ttzip_create_bak_*` transaction whenever `split_volume_size_bytes > 0`, even if the compressed payload fits into the first volume alone (`report.001`).
   - Only backs up existing files that match newly staged volume names or form an unbroken contiguous sequence from volume 1 in the caller-supplied `VolumeNamingScheme` (`<canon_base>.001, .002, ...` for `NumberedExtension`/`RawSplit`, or `<stem>.z01, .z02, ..., .zip` for `PkzipSpanned`).
   - If installation fails and rollback restores all backed-up volumes, `.ttzip_create_bak_*` is removed.
   - If any volume fails to restore during rollback, `.ttzip_create_bak_*` is **preserved** on disk and `create_archive_stream` / `create_archive_advanced` returns `Err(TTZipError::IoError { message })` containing the exact `backup_dir` path and un-restored file list, recoverable via `recover_preserved_create_backup(&backup_dir)`.

### 4.3 Three-Phase Extraction Metadata & Intra-Archive Duplicate Contract (FR-024)

1. **Phase 1 — Decode & Stage (`stage`)**:
   - Staged directories remain `0o700` and staged regular files remain `0o600` in `<destination_root>/.ttzip_staging_<pid>_<session>_<rand>/payload`. Restrictive directory modes (e.g., `0o555`) are never applied to `staging/payload`.
   - Full and selective extraction uniformly record explicit entry `(mode, mtime_epoch_secs, mtime_nanos, is_directory, is_symlink)` and implicit parent directories (`0o755`) in a `StagedMetadataPlan` keyed by the staging payload directory.
   - Intra-archive duplicate regular files in Stage 1 use Last-Entry-Wins via `InFlightJailFile::open(jail, path, true)`: each entry writes to an exclusive sibling temporary file (`.ttzip_inflight_<leaf>_<pid>_<seq>` opened with `O_CREAT | O_EXCL | O_NOFOLLOW`, **never `O_TRUNC`**) and atomically replaces the staged path via `renameat` on completion, preserving hardlinked siblings and earlier entries on mid-stream failure.
2. **Phase 2 — Transactional Commit (`commit`)**:
   - `DFSCommitWalker` moves/merges operable `0o700`/`0o600` staged entries into `destination_root` and records `CommittedPathRecord` mappings (`staged_rel_path -> committed_rel_path`, `CommittedDirPlacement::CreatedNew` vs. `CommittedDirPlacement::MergedExisting`).
3. **Phase 3 — Final Metadata Application (`CommitResult::Finished` only)**:
   - Runs **only** when `commit` returns `CommitResult::Finished` (never on partial batches when `CommitResult::NeedsDecision` pauses the walk).
   - Restores `mode` and `mtime` on committed files/symlinks and bottom-up on `CreatedNew` directories (including `<stem> N<.ext>` and `<dir> N` `KeepBoth` targets), while preserving pre-existing directory permissions on `MergedExisting` directories.
   - `StagingArea::drop` purges any unconsumed `StagedMetadataPlan` on `abort()` or drop.
