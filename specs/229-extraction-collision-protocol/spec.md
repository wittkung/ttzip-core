# Feature Specification: Extraction Name-Collision Protocol

**Feature Directory**: `products/ttzip/core/specs/229-extraction-collision-protocol`

**Created**: 2026-10-03

**Status**: Draft

**Pipeline Tier**: Full SDD (changes the Rust/Swift UniFFI boundary)

**Input**: Extraction currently replaces existing files at the destination without asking. All UniFFI extraction entry points either hard-code `overwrite_existing: true` or default to it; the format decoders ignore the flag and always truncate; the staging promotion step deletes existing destination files unconditionally. The `ForensicCollisionPanel` UI exists but has no producer. Design a protocol so that no existing file is ever replaced, removed, or modified without an explicit decision, while keeping extraction throughput and transactional safety.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Extracting never destroys existing files without consent (Priority: P1)

A user extracts `Project.zip` into a folder that already contains some files with the same names (for example an earlier extraction they have since edited). Before anything on disk is changed, TTZip shows one decision panel listing the conflicts, with name, size and modification date of both the existing item and the incoming item. The user chooses per item, or once for all remaining items: **Replace**, **Keep Both**, **Skip**, or **Stop**. Only then is the destination changed, exactly as decided.

**Why this priority**: Today this flow silently destroys user data. It is the only data-loss defect currently known in the product.

**Independent Test**: Prepare a destination containing edited copies of three entries from a test archive, extract the full archive, choose a different action for each conflict, and compare the destination byte-for-byte with the expected result.

**Acceptance Scenarios**:

1. **Given** a destination with no conflicting names, **When** the user extracts, **Then** no decision is requested and the result is identical to today's behavior.
2. **Given** a destination containing `notes.txt` (edited by the user) and the archive contains `notes.txt`, **When** the user extracts and chooses **Skip**, **Then** the existing `notes.txt` is byte-identical to before and all other entries are extracted.
3. **Given** the same setup, **When** the user chooses **Keep Both**, **Then** the existing `notes.txt` is unchanged and the incoming file is written as `notes 2.txt`.
4. **Given** the same setup, **When** the user chooses **Replace**, **Then** `notes.txt` contains the archive's version.
5. **Given** the same setup, **When** the user chooses **Stop**, **Then** the destination is identical to its state before the extraction started (no new files, no changed files).
6. **Given** 10,000 conflicts, **When** the user picks an action with "apply to all remaining", **Then** no further prompts appear and the action is applied to every remaining conflict.

---

### User Story 2 - Partial and single-entry extraction obey the same rules (Priority: P1)

A user extracts only selected entries, or a single entry from the archive browser, into a folder that already contains a file with the same name. The same decision rules apply.

**Why this priority**: These paths currently write directly into the destination, bypassing staging; one of them removes the existing destination file before moving the new one in. Fixing only full extraction would leave the same data loss in place.

**Independent Test**: Extract one selected entry into a folder containing an edited file of the same name; verify the decision is requested and each choice produces the expected disk state.

**Acceptance Scenarios**:

1. **Given** a destination containing `report.pdf`, **When** the user extracts the single entry `report.pdf` and chooses **Keep Both**, **Then** the result is `report.pdf` (unchanged) and `report 2.pdf`.
2. **Given** a selection of 5 entries of which 2 conflict, **When** the user chooses **Stop**, **Then** none of the 5 entries is written.

---

### User Story 3 - Non-interactive extraction uses a policy instead of prompting (Priority: P2)

Extraction started where no decision panel can be shown (Finder "Extract Here", batch extraction, plugin host, automation) resolves conflicts using a policy rather than blocking. The default policy is **Keep Both**, matching macOS Archive Utility. The user can change the default behavior for interactive extraction in Settings: **Ask** (default), **Keep Both**, **Replace**, **Skip**.

**Why this priority**: Without it, non-interactive callers would either block forever or fall back to silent replacement.

**Independent Test**: Run a batch extraction of 3 archives into a folder with conflicts; verify no prompt appears and every conflict is resolved as Keep Both.

**Acceptance Scenarios**:

1. **Given** Finder "Extract Here" into a folder with conflicts, **When** extraction runs, **Then** no prompt is shown and conflicting incoming items are written with numbered names.
2. **Given** the Settings default is **Replace**, **When** the user extracts in the app, **Then** no prompt is shown and conflicting items are replaced.

---

### User Story 4 - Interruption never leaves a half-replaced destination (Priority: P2)

If extraction is interrupted at any point (the user stops, the app crashes, the disk fills up, power is lost), every affected path at the destination holds either its complete original content or its complete new content, never a missing or partially written file. Staging data left behind by an interrupted run is removed automatically.

**Why this priority**: Collision decisions are worthless if the replacement step itself can lose the original.

**Independent Test**: Inject a failure at each step of the commit phase in a test build; after each run, verify every destination path is original-or-new and complete, and that no staging directory remains after the next extraction or launch.

**Acceptance Scenarios**:

1. **Given** a Replace decision for 100 files, **When** a failure is injected after 50 replacements, **Then** each of the 100 paths holds either the complete original or the complete new content, and the user is told which items were not completed.
2. **Given** the app was killed while a decision panel was open, **When** the app launches again, **Then** no staging data from that run remains on disk.

---

### Edge Cases

- **Type mismatch**: the archive contains directory `docs/` and the destination has a file `docs` (or the reverse). Always a conflict; Keep Both renames the incoming item; Replace replaces the whole existing item only after an explicit decision.
- **Directory onto directory**: merged recursively; only files inside that clash, or type mismatches, are conflicts.
- **Symbolic links at the destination**: never followed. A symlink at a conflicting path is treated as an existing item and handled like a file.
- **Case and Unicode normalization**: on case-insensitive or normalization-insensitive volumes, `Readme.md` vs `README.md` and NFC vs NFD spellings of the same name are conflicts.
- **Keep Both name already taken**: if `notes 2.txt` also exists, the next free number is used (`notes 3.txt`, and so on).
- **Names already present inside the archive twice**: not a destination conflict. Existing in-archive duplicate handling is unchanged.
- **Destination changes while the panel is open**: a final check runs at commit time; items that started to conflict in the meantime are resolved by the same rules (prompting again if interactive and not covered by "apply to all").
- **Destination not writable / permission denied on a single item**: that item is reported as failed; other items continue; the original stays intact.
- **Insufficient disk space**: detected before any existing item is touched; the destination stays unchanged.
- **Extraction to fresh temporary destinations** (drag-out to Finder, preview, File Provider): cannot conflict and never prompt.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST NOT replace, remove, truncate, or modify any existing item at the destination unless an explicit decision (interactive or policy) covers that item.
- **FR-002**: The system MUST determine the complete set of conflicts before any existing item is touched.
- **FR-003**: When the archive's entry names are available without decompressing content, the system MUST request the decision before content extraction starts, and MUST re-check for new conflicts immediately before committing.
- **FR-004**: The interactive decision MUST present, for each conflict: item name, relative path, size and modification date of both the existing and incoming item, and the conflict kind (same type / type mismatch), plus the total number of conflicts.
- **FR-005**: The available actions MUST be Replace, Keep Both, Skip, and Stop, each applicable to the current item or to all remaining conflicts. The default (keyboard Return) action MUST be non-destructive (Keep Both).
- **FR-006**: Keep Both MUST name the incoming item `<name> N<.ext>` with the smallest N ≥ 2 that is free at commit time, matching Finder's convention; for directories `<name> N`.
- **FR-007**: Directory-onto-directory MUST merge; only contained item clashes and type mismatches are conflicts.
- **FR-008**: Conflict detection MUST use the destination volume's name-comparison rules (case sensitivity and Unicode normalization).
- **FR-009**: The system MUST NOT follow symbolic links at the destination when detecting or resolving conflicts.
- **FR-010**: Stop MUST leave the destination identical to its state before the extraction started.
- **FR-011**: For every affected path, an interruption at any point MUST leave either the complete original or the complete new item; never neither, never partial.
- **FR-012**: Staging data from interrupted runs MUST be removed automatically no later than the next extraction into the same parent folder or the next app launch, whichever comes first.
- **FR-013**: Callers that cannot present a decision MUST supply a policy (Keep Both, Replace, Skip, Stop). If none is supplied, Keep Both applies. No caller may block indefinitely waiting for a decision that cannot be presented.
- **FR-014**: Users MUST be able to set the default interactive behavior to Ask (default), Keep Both, Replace, or Skip.
- **FR-015**: These rules MUST apply to every extraction into a user-chosen destination: full extraction, selected entries, single entry, batch, smart extract, and plugin-host extraction.
- **FR-016**: The extraction result MUST report the counts of items extracted, replaced, kept both (renamed), skipped, merged_dirs, and failed, and list failed items with reasons. Counter fields (`extracted`, `replaced`, `kept_both`, `skipped`, `merged_dirs`) MUST only be incremented after the target mutation succeeds with `Ok(())`. **Item Commit Failures** (any failure inspecting `dest_dirfd`, opening/merging a directory in `dest_dirfd`, moving/swapping an item into `dest_dirfd`, or unlinking a skipped item in `payload_dirfd`) MUST NOT abort the entire commit walk (`?`) and MUST record a `FailedItem` in `outcome.failed` without incrementing success counters. Conversely, **Staging Cleanup Failures** that occur *after* the destination item/directory in `dest_dirfd` has already been atomically committed (specifically: parking the displaced old item from `payload_dirfd` into `displaced_dirfd` after `RENAME_SWAP` succeeded, or unlinking a drained `payload_dirfd` directory after merging its children) do not affect the committed destination state, MUST NOT roll back `replaced`/`merged_dirs` counts, and are NOT recorded in `outcome.failed` (`StagingArea::drop` removes the entire `.ttzip-staging-*` root on teardown).
- **FR-017**: While waiting for a decision, extraction MUST consume no CPU and hold no memory beyond what is needed to keep already-prepared content.
- **FR-018**: Extraction with no conflicts MUST NOT become measurably slower (see SC-004) and MUST continue to satisfy the constitution's decompression throughput floor.
- **FR-019**: The system MUST distinguish five terminal states across Rust and Swift boundaries:
  - **Success (`Completed`)**: All planned items committed with `failed.is_empty()` and all integrity checks (CRC32, AEAD MAC, EOF) verified.
  - **Partial Failure (`CompletedWithFailures`)**: Stage succeeded, commit completed its walk, non-failing items were committed, and at least one item experienced an Item Commit Failure (`!failed.is_empty()`).
  - **Stopped (`Stopped`)**: Collision resolution ended with `.stop` policy or user Stop (`session.abort()`); staging is removed and zero destination items are modified.
  - **Cancelled (`Cancelled`)**: Mid-operation cancellation via `Task.isCancelled` or `CancellationToken` prior to the **Commit Point of No Return** (entry into `session.commit`); staging is removed and pre-existing target files remain untouched.
  - **Corrupted (`Corrupted` / `ErrCorruptHeader` / `ErrCrcMismatch` / `ErrInvalidPassword`)**: Integrity, framing, missing split volume, or authentication failure during `stage` or `preflight`; staging is aborted and removed before `commit` can run, leaving pre-existing destination files untouched.
- **FR-020**: Public entrypoints for both archive creation (`create_archive`, `create_archive_explicit`, `create_archive_stream`, `create_archive_advanced`) and extraction (`ExtractionSession`, `TTZipEngineFacade.extract` / `quickExtract` / `extractSingleEntry`, `ExtractCommand`) MUST stage into an isolated sibling temporary directory guarded by RAII cleanup, and archive creation callers MUST pass an explicit `CreateCommitMode` (`SingleFile` vs. `SplitVolume(VolumeNamingScheme)`) rather than inferring split mode from filenames:
  - **Single-File Archive Creation (`CreateCommitMode::SingleFile`, `split_volume_size_bytes == 0`)**: Commit MUST use a single direct POSIX `rename(staged_file, destination_path)` with zero missing-target window, even when the user explicitly names a non-split archive with a `.001` suffix (e.g., `report.001`), and MUST NOT inspect, back up, or delete any neighboring files (`report.002`, `report.003`, `report.zip.1`, `report.zip.2026`, `report.zip.001`, etc.).
  - **Multi-Volume Split Archive Creation (`CreateCommitMode::SplitVolume(VolumeNamingScheme)`, `split_volume_size_bytes > 0`)**: Commit MUST use a two-phase backup-and-rollback transaction via a sibling `.ttzip_create_bak_<pid>_<ts>_<seq>/` directory, even when the compressed output fits within the first volume alone (`report.001`). Only files matching newly staged volume names or belonging to a confirmed contiguous split volume chain of the caller-specified `VolumeNamingScheme` (`NumberedExtension`/`RawSplit`: `<canon_base>.001, .002, ...` 3-digit consecutive sequence starting at `.001`; `PkzipSpanned`: `<stem>.z01, .z02, ..., .zip` 2-digit consecutive sequence starting at `.z01`) may be backed up and retired; unrelated numeric-suffixed neighbors (`report.zip.1`, `report.zip.2026`, non-contiguous suffixes) MUST remain untouched. If any volume installation fails, rollback MUST verify every restore `rename`; if any backed-up volume fails to restore, `.ttzip_create_bak_*` MUST be preserved on disk and a locatable, recoverable error (`CreateStageCommitError::RollbackIncomplete` / `TTZipError::IoError`) naming `backup_dir` and the un-restored paths MUST be returned (with programmatic recovery via `recover_preserved_create_backup`).
- **FR-021**: High-level Swift convenience methods (`TTZipEngineFacade.extract`, `quickExtract`, `executePipelineExtract`, `ExtractCommand.execute`, `ArchiveBatchFacade`, `AppViewState.quickExtractArchive`, `AppViewState.extractArchive`) MUST check both `Task.isCancelled` and external `CancellationToken.isCancelled()` immediately before every `session.commit(resolutions:)` invocation (including after interactive `decide` returns), and MUST propagate `ExtractionOutcome` so `Stopped` or `CompletedWithFailures` never reports ordinary success, never reveals in Finder, and never trashes the source archive.
- **FR-022**: Every placement of a staged item into a destination name that no Replace decision covers (non-conflicting item, `CommitRecheck` item, Keep Both candidate `<name> N<.ext>`, staged directory whose destination is absent) MUST be decided by an atomic kernel primitive that fails with `EEXIST` when the name is occupied and leaves the destination holding either the complete existing item or the complete new item (never a 0-byte reservation or empty placeholder directory). Neither a prior existence lookup (`fstatat`, `access`, …) nor a two-step "create reservation then plain `rename`" sequence may be used. Primitive order:
  1. `renameatx_np(RENAME_EXCL | RENAME_NOFOLLOW_ANY)` when the volume reports `VOL_CAP_INT_RENAME_EXCL`; `ENOTSUP` / `EOPNOTSUPP` / `ENOSYS` at runtime falls through to step 2 for that item.
  2. Regular files only: `linkat(payload, name, dest, target, 0)` (atomic, fails with `EEXIST` if occupied, never replaces an existing entry), then unlink the staged name (staging cleanup only).
  3. Unsupported volume capability: when hard links are unsupported for a regular file (`ENOTSUP`, `EOPNOTSUPP`, `ENOSYS`, `EPERM`, `EMLINK`), and always for directories and symlinks when `RENAME_EXCL` is unavailable or returns `ENOTSUP` / `EOPNOTSUPP` / `ENOSYS`, placement MUST fail with `ENOTSUP` and be recorded as `FailedItem { reason: FailureReason::NotSupportedByVolume, errno: ENOTSUP }`. Both the destination (including any existing empty directory) and the staged source in `payload/` MUST remain untouched, with zero reservation artifacts created.
  - Unknown volume capabilities (probe error, capability bit not valid) MUST be treated as "unsupported" (`VolCaps::default()`), which routes through steps 2–3 and never selects a plain overwriting `rename`.
- **FR-023**: The extraction C-ABI entry points `ttzip_rust_extract_archive`, `ttzip_rust_archive_extract_unified`, `ttzip_rust_archive_extract_unified_v2`, and `ttzip_rust_archive_extract_selected` MUST share one staged-commit path and one result mapping:
  - Policy `Stop` (`collision_policy == 3`): run post-stage detection; any conflict → return `Cancelled`, destination unchanged. Without conflicts, commit with no default action, so a conflict that appears at commit time becomes a `FailedItem` (never Keep Both, never Replace).
  - `OutcomeStatus::Completed` → `Ok`; `CompletedWithFailures` → `ErrExtractionFailed`; `Stopped` → `Cancelled`; commit I/O error before an outcome exists → `ErrExtractionFailed`. Staging / decode errors keep their own status. Only a complete commit returns `Ok`.
  - `out_extracted_count` (selective) is zeroed on entry and therefore defined on every return: `extracted + replaced + kept_both` of the actual commit outcome (one per placement; a staged directory moved in one `rename` counts once, children committed into a merged directory count individually); `0` before commit, on Stop, and on commit I/O error; the partial count on `ErrExtractionFailed`. With `dry_run` no commit runs and the count is the number of matched entries.
  - `out_extracted_bytes` (`_v2`) is the decoded byte total of the staged payload, written only when the result is `Ok`; `0` otherwise. `out_error` (`TTZipErrorInfo`, ABI v3: `struct_size = 792`, `abi_version = 3`, `status`, `error_code`, `message[512]`, `entry_path[256]`, `offset`, identical layout in `ttzip-types` and `sdk/include/ttzip.h`) and the thread-local last error carry the non-`Ok` status and diagnostic message.
- **FR-024**: Extraction MUST execute in three strictly ordered phases across all full and selective entry points:
  1. **Phase 1 — Decode & Stage (`stage`)**:
     - All directories inside `staging/payload` MUST remain at `0o700` and all regular files at `0o600` throughout staging and commit so `renameatx_np` (which requires write permission on moved directories on Darwin/POSIX to update `..`), `unlinkat`, and `remove_tree` never fail with `EACCES` on restrictive archive directory modes (such as `0o555` or `0o500`). Restrictive permissions MUST NEVER be applied inside `staging/payload`.
     - Both full extraction (`extract_archive`, `extract_archive_with_metrics`, `extract_archive_with_sheet`) and selective extraction (`extract_selected_entries`, covering resident/streaming/multi-volume ZIP, single/multi-volume 7z, TAR, and single/multi-volume libarchive) MUST record a `StagedMetadataPlan` capturing explicit entry `(mode, mtime_epoch_secs, mtime_nanos, is_directory, is_symlink)` and implicit parent directories (`DEFAULT_IMPLICIT_DIR_MODE = 0o755`).
     - **Intra-Archive Duplicate Rules (Stage 1) vs. Destination Collision Policy (Stage 2)**: Stage 1 (`staging/payload`) is strictly decoupled from `CollisionPolicy` (which governs Stage 2 destination collisions). Inside the same archive:
       - Duplicate regular files use **Last-Entry-Wins via `InFlightJailFile` exclusive sibling temporary files** (`.ttzip_inflight_<leaf>_<pid>_<seq>` opened with `O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC`, retrying on `EEXIST`, **never `O_TRUNC`**) and atomically replace the staged target upon entry completion via `renameat(parent_fd, tmp_c, parent_fd, target_c)`. This prevents write-through corruption of shared hardlink inodes and preserves the earlier entry if the later duplicate fails or is cancelled mid-stream. The last duplicate entry's `(mode, mtime)` wins in `StagedMetadataPlan`.
       - Duplicate directory entries and implicit parent directories followed by an explicit directory entry merge idempotently; an explicit directory entry overrides an implicit parent directory (`0o755`), and among multiple explicit directory entries for the same path, the last entry's `(mode, mtime)` wins.
       - Same-path type conflicts within the same archive (file vs. directory or symlink) are rejected by `DirJail` (`ErrSecurityViolation` / `ErrExtractionFailed`).
       - At the end of Phase 1, `SafeExtractEngine::apply_deferred_metadata` captures each staged item's `(st_dev, st_ino)` identity (`expected_identity`) via `DirJail` step-by-step no-follow traversal (`populate_staged_identities`). If capturing `expected_identity` fails for any staged item, Phase 1 MUST fail explicitly (`Err(TTZipStatus)`) and MUST NOT store a `StagedMetadataPlan` with `expected_identity = None`.
  2. **Phase 2 — Transactional Commit (`commit` via `DFSCommitWalker::commit_with_mapping`)**:
     - `DFSCommitWalker` walks `staging/payload` (`0o700` directories, `0o600` files) and records a `CommittedPathEntry` in `CommitPathMapping` for every item actually placed into `destination_root` (`Extracted`, `Replaced`, `KeptBoth` mapped to its final `<stem> N<.ext>` or `<dir> N` relative path, and `MergedExistingDir`), distinguishing newly placed directories from directories that merged into an already-existing directory at `destination_root` (`CommittedEntryDisposition::MergedExistingDir`). Items that are `Skip`ped or `Failed` are omitted.
  3. **Phase 3 — Final Metadata Application (`apply_committed_metadata`)**:
     - Phase 3 executes after `commit` finishes (`OutcomeStatus::Completed` or `OutcomeStatus::CompletedWithFailures`).
     - `apply_committed_metadata` resolves each committed item step-by-step from `DirJail::root_fd()` using `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC` on every parent directory component, requires `expected_identity: Some((exp_dev, exp_ino))` (rejecting `None` with `EPERM`), verifies `fstatat(..., AT_SYMLINK_NOFOLLOW)` matches `(exp_dev, exp_ino)`, opens regular files/directories with `O_NOFOLLOW`, verifies `fstat(fd)` matches `fstatat`, and applies `futimens(fd)` and `fchmod(fd)` (stripping `S_ISUID | S_ISGID | 0o002`) directly on the held FD. Symlink timestamps use explicit no-follow semantics (`O_RDONLY | O_SYMLINK | O_CLOEXEC` + `fstat` identity check + `futimens` on macOS, `utimensat(..., AT_SYMLINK_NOFOLLOW)` elsewhere) and never call `chmod`. Directory `mode` and `mtime` are applied bottom-up (deepest first) **only** to newly placed directories while strictly preserving pre-existing permissions and `mtime` of `MergedExistingDir` directories.
     - Any Phase 3 metadata failure (including missing/mismatched `expected_identity`, parent/leaf symlink or imposter replacement, or `fchmod`/`futimens` error) is recorded into `outcome.failed` and sets `outcome.status = OutcomeStatus::CompletedWithFailures` (mapped to `TTZipStatus::ErrExtractionFailed` in C-ABI) while preserving already-committed file contents in `destination_root`.

### Key Entities

- **Extraction Session**: one extraction in progress; owns its staging area, the planned set of destination paths, and its lifecycle (preparing, awaiting decision, committing, completed, stopped, failed).
- **Conflict**: one destination path where an incoming item meets an existing item; attributes: relative path, conflict kind, existing item metadata, incoming item metadata.
- **Resolution**: the action chosen for a conflict (Replace, Keep Both, Skip, Stop) and its scope (this item / all remaining).
- **Conflict Policy**: a resolution chosen in advance for callers without UI or from the user's Settings default.
- **Extraction Outcome**: per-category counts (`extracted`, `replaced`, `kept_both`, `skipped`), `failed` items list, and terminal `status` (`Completed`, `CompletedWithFailures`, `Stopped`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Across a conflict test corpus of at least 200 scenarios (each format × each action × each edge case above), zero existing items are changed without a covering decision.
- **SC-002**: A user facing 10,000 conflicts can resolve all of them with at most 2 interactions.
- **SC-003**: With a failure injected at every step of the commit phase, 100% of affected paths hold a complete original or complete new item, and zero staging directories remain after the next launch.
- **SC-004**: Extracting an archive with no conflicts takes no more than 3% longer than the current release on the standard benchmark corpus.
- **SC-005**: For archives with up to 100,000 entries whose names can be listed cheaply, the decision is shown within 0.5 s of the user starting extraction.
- **SC-006**: Non-interactive extraction never shows a prompt and never waits on user input (verified for Finder Extract Here, batch, and plugin host).

## Assumptions

- Staging continues to live on the same volume as the destination, so moving staged items into place does not copy data.
- Disk space for the staged content is required before commit, as today.
- Archive Utility's behavior (keep both, numbered names) is the expected default for non-interactive extraction on macOS.
- In-archive duplicate entry names keep their current handling; this feature only concerns clashes with items already at the destination.
- Drag-out, preview, QuickLook, and File Provider extraction target fresh temporary folders and are out of scope except for the guarantee that they never prompt.
- The existing `ForensicCollisionPanel` component is the UI surface for interactive decisions; its visual design is out of scope.
- Settings UI for the default behavior reuses the existing Settings screen.
