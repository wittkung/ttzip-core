# Group G Follow-Up Contract: `CollisionPolicy::Stop` Session Consistency & TAR Permission Restoration

**Feature Directory**: `products/ttzip/core/specs/229-extraction-collision-protocol`  
**Scope**: Q1 Acceptance Follow-Up (`Q1-ISSUE-04` and the TAR permission portion of `Q1-ISSUE-08`)  
**Status**: Active Follow-Up Contract (aligned with Spec 229 FR-001, FR-010, FR-016, FR-019, FR-022, FR-023, FR-024)

---

## 1. Problem Classification & Root-Cause Analysis

### 1.1 `Q1-ISSUE-04` — Direct `ExtractionSession.commit` under `CollisionPolicy::Stop`

- **Classification**: **Confirmed Implementation Defect & Session Boundary Enforcement Gap**.
- **Observed Symptom**:
  - High-level helpers (`uniffi_api::extraction::run_policy`, `TTZipEngineFacade+Extraction.swift`, and C-ABI `staged_commit::commit_staged_with_policy`) checked `if policy == Stop && has_conflicts { return session.abort() }` before calling `commit`.
  - However, when a caller invoked `uniffi_api::ExtractionSession` directly with `ExtractionRequest { ..., policy: CollisionPolicy::Stop }`, called `preflight()` and `stage()`, and then called `session.commit(ResolutionSet { default_action: None, overrides: vec![] })` (as in Q1 acceptance test `C4-04`), two defects combined:
    1. `uniffi_api::ExtractionSession::commit` / `begin_commit` ignored `self.request.policy` and never verified whether detected conflicts in `ConflictTable` were covered by `resolutions`.
    2. `DFSCommitWalker::commit_file` (`crates/ttzip-archive/src/fs/collision/commit.rs:467`) executed `state.resolutions.action_for(record.id).unwrap_or(CollisionAction::Skip)` on conflicts present in `state.table`, silently converting uncovered file conflicts into `Skip` while moving non-conflicting files into `destination_root` (`OutcomeStatus::Completed`, `extracted: 2, skipped: 2`).
- **Contract Violation**: Violates Spec 229 **FR-001** (no destination mutation without a covering decision) and **FR-010** / **FR-019** (`Stop` must leave the destination identical to its state before extraction started and return `OutcomeStatus::Stopped`).

### 1.2 `Q1-ISSUE-08` (TAR Portion) — `0444` Read-Only File & POSIX Permission Restoration

- **Classification**: **Report Attribution Conflation on `c5_posix.tar` + Explicit Contract & Regression Hardening for `preserve_permissions = true / false`**.
- **Evidence from Q1 Acceptance Artifacts**:
  1. **Sample Inspection (`/tmp/ttzip-q1-acceptance-20261017/failed_samples/C5-04_05_c5_posix.tar`)**:
     - `tar -tvf C5-04_05_c5_posix.tar` confirms the USTAR header stores `-r--r--r--` (`0o444`, `mtime = 1680000000`) on `corpus_posix/readonly/immutable.cfg` and `-rwxr-xr-x` (`0o755`, `mtime = 1700001234`) on `corpus_posix/bin/tool.sh`.
  2. **Q1 Raw Execution Record (`/tmp/ttzip-q1-acceptance-20261017/reports/acceptance_results.json`, Test `#66` `C5-03a`)**:
     - The raw acceptance JSON recorded:
       `actual: "ext=Ok(Completed), is_symlink=true, tool.sh(mode=0o755, mtime=1700001234), immutable.cfg(mode=0o444, mtime=1680000000)"`, `status: "PASS"`.
      - The prose in `issue-tickets.md` line 161 conflated the 7z creation metadata case (`c5_posix.7z`, which yielded `0o644` and belongs to **R4**'s creation metadata pipeline) with `c5_posix.tar` (whose original Q1 record `C5-03a` was **PASS**).
   3. **R2 Reader & Stream Adapter Audit (Read-Only)**:
      - `crates/ttzip-archive/src/archive/tar/reader.rs` (`TarArchive::extract_all`), `crates/ttzip-archive/src/archive/unified/extract.rs` (`extract_from_libarchive_handle`), and `crates/ttzip-archive/src/archive/unified/extract_single.rs` (`extract_selected_from_libarchive_handle`) all pass `(entry.mode, entry.mtime, is_dir, is_symlink)` into `SafeExtractEngine::register_entry_typed` and invoke `engine.apply_deferred_metadata(options.preserve_permissions)?`. No metadata is dropped in R2's TAR readers; no R2 handoff modification is required.
   4. **Group G Hardening Scope**:
      - Explicitly document and lock down via automated regressions the complete TAR metadata restoration matrix across `preserve_permissions = true` and `preserve_permissions = false` (including strict caller `umask` such as `0o077` and `0o027`), covering `0o444` read-only files, `0o755` executables, `0o555` restricted directories with nested children, security bit filtering (`S_ISUID`, `S_ISGID`, `S_IWOTH`), `KeepBoth` renamed targets, `Skip` untouched originals, `MergedExistingDir` directory preservation, and Phase 3 metadata failure propagation (`CompletedWithFailures`).

---

## 2. Collision Policy, Explicit Resolutions, and Unresolved Conflict Contract

### 2.1 Precedence Hierarchy at `ExtractionSession::commit`

When `ExtractionSession::commit(resolutions: ResolutionSet)` is called on a staged session holding `(request.policy, table: ConflictTable, staging: StagingArea)`:

1. **Structural Validation (`ResolutionSet::validate(&table)`)**:
   - Every `ResolutionOverride { conflict_id, action }` in `resolutions.overrides` MUST satisfy `conflict_id < table.len()` and be strictly sorted in ascending `conflict_id` order without duplicates.
   - If validation fails, `commit` MUST return `Err(TTZipError::InvalidResolution)` **before** transitioning out of `SessionSlot::Staged` / `SessionSlot::AwaitingStageDecision`, keeping the session committable and leaving `destination_root` untouched.
2. **Resolution Precedence for Detected Conflicts (`record.id in 0..table.len()`)**:
   - **Priority 1 — Explicit Per-Item Override (`resolutions.overrides`)**: If `resolutions.overrides` contains `conflict_id == record.id`, its `CollisionAction` (`Replace`, `KeepBoth`, or `Skip`) governs `record`.
   - **Priority 2 — Explicit Default Decision (`resolutions.default_action = Some(action)`)**: If `record.id` is not overridden in `resolutions.overrides` and `resolutions.default_action` is `Some(action)`, `action` governs `record` (as well as any dynamic commit-time race conflict).
   - **Priority 3 — Session Request Policy (`request.policy: CollisionPolicy`) when `resolutions.default_action == None`**:
     - `CollisionPolicy::KeepBoth` $\rightarrow$ supplies fallback `default_action = Some(CollisionAction::KeepBoth)`.
     - `CollisionPolicy::Replace` $\rightarrow$ supplies fallback `default_action = Some(CollisionAction::Replace)`.
     - `CollisionPolicy::Skip` $\rightarrow$ supplies fallback `default_action = Some(CollisionAction::Skip)`.
     - `CollisionPolicy::Stop` $\rightarrow$ supplies **no** fallback `CollisionAction` (`default_action = None`):
       - **Uncovered Detected Conflicts (`!table.is_empty()` and `resolutions.overrides.len() < table.len()`)**: At least one conflict detected in `preflight` or `post_stage` has no explicit resolution. `ExtractionSession::commit` MUST execute a **Pre-Commit Stop**:
         - Do **not** invoke `DFSCommitWalker`; move zero non-conflicting files.
         - Drop/abort `StagingArea` (removing `.ttzip_staging_*` and removing `destination_root` if created by this session).
         - Transition session state to `SessionState::Aborted`.
         - Return `Ok(CommitResult::Finished { outcome: ExtractionOutcome::stopped() })` (`status = OutcomeStatus::Stopped`, `extracted = replaced = kept_both = skipped = merged_dirs = 0`, `failed = []`).
       - **All Detected Conflicts Explicitly Covered (`!table.is_empty()` and (`resolutions.default_action.is_some()` or `resolutions.overrides.len() == table.len()`))**: Explicit caller decisions override `request.policy`; `commit` proceeds with those decisions.
       - **Conflict-Free Session (`table.is_empty()`)**: No pre-commit conflicts exist; `commit` proceeds with `default_action = resolutions.default_action` (`None` unless explicitly supplied) and commits all staged entries (`OutcomeStatus::Completed`).
     - `CollisionPolicy::Ask` $\rightarrow$ supplies **no** fallback `CollisionAction` (`default_action = None`):
       - If `!table.is_empty()` and `resolutions.default_action.is_none()` and `resolutions.overrides.len() < table.len()`, `commit` MUST reject the incomplete resolution set with `Err(TTZipError::InvalidResolution)` without mutating session state or `destination_root`.

### 2.2 Core Commit Walker Defense-in-Depth (`DFSCommitWalker` & `session::ExtractionSession<Staged>`)

- In `crates/ttzip-archive/src/fs/collision/commit.rs` (`DFSCommitWalker::commit_file`), if a caller invokes the low-level walker directly with a `ConflictTable` containing `record` where `state.resolutions.action_for(record.id)` is `None` (neither per-item override nor `default_action`), the walker MUST NOT silently default to `CollisionAction::Skip`.
- Instead, matching `resolve_dir_type_mismatch` and dynamic `EEXIST` race handling, `commit_file` leaves the existing destination item untouched, leaves the staged item unplaced, and records `FailedItem { relative_path, reason: FailureReason::Io, errno: libc::EEXIST }`, causing the commit outcome to report `OutcomeStatus::CompletedWithFailures`.

### 2.3 Dynamic Commit-Time Conflicts (`CommitRecheck` Race Contract)

When a path was free during `preflight` / `post_stage` (`table` has no record for it) and an item appears at `destination_root` before `place_exclusive` claims the path during Phase 2:

1. `place_exclusive` uses atomic kernel primitives (`renameatx_np(RENAME_EXCL | RENAME_NOFOLLOW_ANY)` or `linkat` for regular files) that fail with `EEXIST` and **never** overwrite the newly appeared destination item.
2. **Staged Directory vs. Newly Appeared Directory**: Merges recursively (`CommittedEntryDisposition::MergedExistingDir`, `outcome.merged_dirs += 1`) without overwriting existing directory metadata or children.
3. **Staged File/Symlink vs. Newly Appeared Item, or Staged Directory vs. Newly Appeared File/Symlink**:
   - If `effective_resolutions.default_action` is `Some(action)`, resolves according to `action` (`KeepBoth` $\rightarrow$ `<stem> N<.ext>`, `Replace` $\rightarrow$ atomic rename/swap, `Skip` $\rightarrow$ unlink staged item and increment `skipped`).
   - If `effective_resolutions.default_action` is `None` (e.g., conflict-free `CollisionPolicy::Stop` session where a racer file appeared mid-commit, or `CollisionPolicy::Ask` with per-item overrides only), preserves the destination item untouched, records `FailedItem { relative_path, reason: FailureReason::Io, errno: libc::EEXIST }`, and finishes the walk with `OutcomeStatus::CompletedWithFailures`.

### 2.4 Pre-Commit Stop vs. Mid-Commit Partial Failure

| Dimension | Pre-Commit Stop (`OutcomeStatus::Stopped`) | Mid-Commit Partial Failure (`OutcomeStatus::CompletedWithFailures`) |
|---|---|---|
| **When it occurs** | Before `DFSCommitWalker` starts (`abort()`, or `commit()` under `CollisionPolicy::Stop` with uncovered conflicts in `ConflictTable`) | After `DFSCommitWalker` starts (Phase 2 placement error / uncovered commit-time race `EEXIST`, or Phase 3 FD-bound metadata error) |
| **Destination state** | **100% untouched** (`extracted = replaced = kept_both = skipped = merged_dirs = 0`) | Already-committed items are **preserved**; failed items leave original destination items intact; every path holds complete original or complete new content |
| **Rollback guarantee** | Full cleanup of `.ttzip_staging_*` (and `destination_root` if newly created by the session) | Per-item atomic transition via kernel rename/swap/linkat; **no** mid-commit rollback of already-placed items; `.ttzip_staging_*` is cleaned up on teardown |
| **C-ABI / Swift mapping** | C-ABI: `TTZipStatus::Cancelled`; Swift: `ArchiveError.extractionStopped` | C-ABI: `TTZipStatus::ErrExtractionFailed`; Swift: `ArchiveError.partialExtractionFailure(outcome)` |

---

## 3. Three-Phase Permission & Timestamp Restoration Contract (TAR & POSIX Metadata)

### 3.1 Staging Isolation & Race-Free `umask` Observation (Phase 1)

- All directories created inside `.ttzip_staging_*/payload` use `0o700` and all regular files use `0o600` (via `InFlightJailFile` exclusive sibling temp files `.ttzip_inflight_*` + atomic `renameat`).
- Restrictive archive modes (`0o444`, `0o555`, `0o000`) are **never** applied to `staging/payload`, ensuring `renameatx_np`, `unlinkat`, and `remove_tree` during Phase 2 never fail with `EACCES`.
- **Race-Free Environment Creation Mask Observation (`creation_mode_mask = 0o777 & !umask`)**:
  - Calling `libc::umask(...)` to read and restore the process creation mask is **prohibited** because `umask(2)` mutates process-global state and introduces a concurrency race across threads creating files concurrently.
  - Instead, `probe_creation_mode_mask_in_fd` creates a transient exclusive probe inode (`openat(dir_fd, ".ttzip_mode_probe_<pid>_<seq>", O_RDONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o777)`), reads the kernel-masked `st.st_mode & 0o0777` via `fstat(fd)`, and immediately unlinks and closes it. The kernel applies `& !umask` atomically without mutating process-global state. If probing fails, it fails closed to `0o700`.
- At the end of Phase 1, `SafeExtractEngine::apply_deferred_metadata` deduplicates intra-archive duplicates (Last-Entry-Wins), collects newly created implicit parent directories, probes `creation_mode_mask`, and captures each staged object's `(st_dev, st_ino)` identity (`expected_identity`) via step-by-step no-follow `DirJail` traversal (`populate_staged_identities`). Any identity capture failure aborts Phase 1 with `Err(TTZipStatus)`.

### 3.2 `preserve_permissions = true` vs. `preserve_permissions = false` (Phase 3)

Let `creation_mode_mask = 0o777 & !umask` be the kernel-observed creation mask for the calling environment (e.g. `0o755` under `umask 0o022`, `0o750` under `umask 0o027`, `0o700` under `umask 0o077`).

| Attribute / Target | `preserve_permissions = true` | `preserve_permissions = false` |
|---|---|---|
| **Security bit filtering** | `target_mode = (effective_mode & 0o0777) & !0o002` — strips `S_ISUID` (`0o4000`), `S_ISGID` (`0o2000`), `S_ISVTX` (`0o1000`), and world-writable `S_IWOTH` (`0o0002`) | Archive mode bits (`0o444`, `0o755`, `0o555`, `0o4755`, etc.) are **ignored**; permissions are bounded by `creation_mode_mask` |
| **Regular files (`0o444`, `0o755`, `0o644`, etc.)** | Restores filtered archive mode (`0o444` stays `0o444`, `0o755` stays `0o755`, `0o666` becomes `0o664`, `0o4755` becomes `0o755`) via `fchmod(fd)` on the verified FD | Applies `(0o644 & creation_mode_mask) & !0o002` (`DEFAULT_REGULAR_FILE_MODE & !umask`): `0o644` under `umask 0o022`, `0o640` under `umask 0o027`, `0o600` under `umask 0o077` |
| **Newly created explicit directories (`0o555`, `0o755`, `0o700`, etc.)** | Restores filtered archive mode (or `(0o755 & creation_mode_mask)` if `entry.mode == 0`) bottom-up (deepest child directory first, after all files) | Applies `(0o755 & creation_mode_mask) & !0o002` (`DEFAULT_IMPLICIT_DIR_MODE & !umask`) bottom-up: `0o755` under `umask 0o022`, `0o750` under `umask 0o027`, `0o700` under `umask 0o077` |
| **Newly created implicit parent directories** | Applies `(0o755 & creation_mode_mask) & !0o002` bottom-up | Applies `(0o755 & creation_mode_mask) & !0o002` bottom-up |
| **Pre-existing directories (`MergedExistingDir` / `preexisting_dirs`)** | **Never modified** — existing permissions (e.g. `0o700`, `0o750`) and existing `mtime` are strictly preserved | **Never modified** — existing permissions and existing `mtime` are strictly preserved |
| **Symlinks** | Never calls `chmod`; applies `mtime` via no-follow symlink FD (`O_RDONLY \| O_SYMLINK` + `futimens` on macOS, `utimensat(..., AT_SYMLINK_NOFOLLOW)` elsewhere) | Same as `preserve_permissions = true` |
| **Timestamps (`mtime`)** | Applied via `futimens(fd)` when `mtime_epoch_secs > 0` | Applied via `futimens(fd)` when `mtime_epoch_secs > 0` |

> [!IMPORTANT]
> **Why `preserve_permissions = false` masks `0o644`/`0o755` by `creation_mode_mask` (`!umask`)**:
> Disabling archive permission preservation opts out of archive-supplied mode bits (such as `0o444`, `0o555`, or `0o755`), but MUST NOT widen permissions beyond what the calling environment's `umask` allows. By intersecting the standard non-executable file default (`0o644`) and directory default (`0o755`) with the kernel-observed `creation_mode_mask` (`0o777 & !umask`), standard `umask = 0o022` produces `0o644`/`0o755` while restrictive `umask = 0o077` produces `0o600`/`0o700` (and `umask = 0o027` produces `0o640`/`0o750`), without ever calling `libc::umask` to toggle process-global state.

### 3.3 Collision Resolution Interaction with Phase 3 Metadata

- **`KeepBoth`**: `CommitPathMapping` maps `staged_rel_path` to the actual renamed target (`<stem> N<.ext>` or `<dir> N`, including descendants of a renamed directory subtree). Phase 3 applies `mode` (archive mode when `preserve_permissions = true`, or `(0o644 | 0o755) & !umask` when `preserve_permissions = false`) and `mtime` to the new renamed target while leaving the pre-existing original file's content, permissions, and `mtime` untouched.
- **`Skip`**: Omitted from `CommitPathMapping`; Phase 3 never touches the pre-existing destination item.
- **`Replace`**: `CommitPathMapping` maps `staged_rel_path` to `committed_rel_path` with the newly placed inode's `expected_identity`; Phase 3 verifies `(st_dev, st_ino)` matches the replacement object and applies `mode` and `mtime` to it.
- **`MergedExistingDir`**: `CommitPathMapping::resolve_committed_path` returns `None` for any directory with `CommittedEntryDisposition::MergedExistingDir`, so Phase 3 never overwrites the pre-existing directory's permissions or `mtime`.
- **Phase 3 Metadata Failure**: Any failure during `DirJail` no-follow traversal, `(st_dev, st_ino)` verification, `futimens`, or `fchmod` records a `FailedItem` in `outcome.failed` and transitions `outcome.status` to `OutcomeStatus::CompletedWithFailures` while preserving all already-committed file contents.

---

## 4. Shared Documentation Sync Recommendations (for Coordinator & Group H)

Per strict file ownership boundaries, Group G does not directly edit shared `spec.md`, `contracts/uniffi-api.md`, or `docs/twin/05_archive_transfer.md` in this follow-up. Group H should merge the following concise updates during final integration:

1. **`products/ttzip/core/specs/229-extraction-collision-protocol/contracts/uniffi-api.md` (§1 `ExtractionSession::commit`)**:
   - Clarify that `session.commit(resolutions)` enforces `self.request.policy` when `resolutions.default_action` is `None`:
     - Under `CollisionPolicy::Stop`, any conflict in `ConflictTable` not covered by `resolutions.overrides` or `resolutions.default_action` triggers an immediate pre-commit stop (`OutcomeStatus::Stopped`, `SessionState::Aborted`, staging removed, zero destination changes).
     - Under `CollisionPolicy::KeepBoth | Replace | Skip`, `request.policy` supplies the fallback `default_action` when `resolutions.default_action` is `None`.
     - Under `CollisionPolicy::Ask`, uncovered conflicts in `ConflictTable` return `Err(TTZipError::InvalidResolution)` without mutating session state or destination.
2. **`products/ttzip/core/specs/229-extraction-collision-protocol/spec.md` (FR-024)**:
   - Note that when `preserve_permissions == false`, Phase 3 normalizes newly created regular files to `(0o644 & !umask) & !0o002` and newly created directories to `(0o755 & !umask) & !0o002` using a race-free kernel creation probe (ignoring archive mode bits such as `0o444`, `0o755`, `0o555`, `0o4755` while never widening permissions restricted by caller `umask`), and preserves pre-existing directories (`MergedExistingDir`) and `mtime`.

