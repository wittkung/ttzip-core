# Implementation Plan: Extraction Name-Collision Protocol

**Branch**: `main` (atomic commits per task group) | **Date**: 2026-10-03 | **Spec**: [spec.md](./spec.md)

**Input**: `products/ttzip/core/specs/229-extraction-collision-protocol/spec.md` | **Tier**: Full SDD (changes the UniFFI and C-ABI boundary)

## Summary

Every extraction into a user-chosen folder becomes an `ExtractionSession`: decompress into a private staging directory inside the destination, detect name collisions against the destination with the filesystem's own lookup rules, obtain a decision (interactive panel or caller policy), then move items into place with exclusive / atomic `renameatx_np` calls on directory file descriptors that never follow symlinks. No code path other than the commit walker can write into a user destination. ZIP and 7z detect collisions before decompressing (central-directory listing, now pure Rust after Phase 11 task 1B). The Rust side never waits for Swift: the session is a state machine driven by the Swift facade, so there is no blocking callback and no deadlock surface.

## Technical Context

**Language/Version**: Rust (workspace `products/ttzip/core/rust`, edition per workspace), Swift 6 strict concurrency (macOS 14+).

**Primary Dependencies**: `libc` (`renameatx_np`, `openat`, `fstatat`, `getattrlistat`), UniFFI (existing version in `MODULE.bazel` / Cargo workspace), libarchive (existing fallback decoders), TTKitUI `ForensicCollisionPanel`.

**Storage**: Filesystem only. Staging under `<destination_root>/.ttzip_staging_<pid>_<session_id>/`; Swift `UserDefaults` key `TTZip_ActiveStagingRoots`; `@AppStorage("ttzip_collision_default")`.

**Testing**: `cargo test` (collision corpus, fault injection, property tests), Bazel `//products/ttzip/apple:all_tests` and `//products/ttzip/core` Swift tests, existing differential oracle suites, benchmark corpus for SC-004.

**Target Platform**: macOS 14+ on APFS (primary), HFS+, exFAT/FAT32, SMB (degraded guarantees documented in research R1).

**Project Type**: Desktop app (SwiftUI) over a Rust engine exposed via UniFFI and a C-ABI.

**Performance Goals**: No-conflict extraction ≤ 3 % slower than today (SC-004); decision panel shown ≤ 0.5 s after start for ≤ 100k cheaply listable entries (SC-005); commit cost O(number of top-level staged items + conflicts), not O(entries).

**Constraints**: Zero writes outside staging before commit (FR-001/002); zero CPU while awaiting a decision (FR-017); each FFI crossing bounded (conflict pages ≤ 1024 records).

**Scale/Scope**: Archives up to 1,000,000 entries; 1,000,000 conflicts is a supported case (re-extracting into the same folder).

## Constitution Check

*GATE: evaluated before research and re-evaluated after design.*

| Constitution clause | Status | How |
|---|---|---|
| §2 hot-path floors (no allocation in codec inner loops; ZIP decompression ≥ 4500 MB/s) | PASS | Decoders untouched; they keep writing into a fresh directory. Commit is a metadata walk. SC-004 benchmark gate added (T040) |
| §3A frozen files | PASS | No frozen file is touched (the list names Swift/C files of the pre-Rust engine; none are in scope) |
| §4.II defense in depth (no string-only path security, `O_NOFOLLOW` verification, deferred fixups) | PASS | dirfd walk with `O_DIRECTORY \| O_NOFOLLOW`, `RENAME_NOFOLLOW_ANY`, `fstatat(AT_SYMLINK_NOFOLLOW)`; existing secure-extract flags unchanged |
| §4.III bounds (overflow-checked arithmetic, narrowing clamps, credential zeroization) | PASS | Conflict ids `u32` with checked conversion; Keep Both counter checked; password stays `ZeroizingString` |
| §4.IV oracle-first (golden corpus, differential tests, crash-first fuzzing) | PASS | ≥ 200-scenario collision corpus generated at test time from real archives (ZIP, 7z, tar.gz) built by system `zip`/`tar` and the engine; fault-injection matrix; fuzz target for `ResolutionSet` validation and Keep Both naming |
| §4.C logging (TTLogger / tracing only) | PASS | No `print`; Rust uses existing `tracing` macros |
| §5 gates, no `--no-verify` | PASS | |

Constitution drift noted (not a violation of this feature): §1 still describes a Swift + C11 engine with static C bindings; the shipping engine is Rust behind UniFFI. Tracked for a constitution amendment outside this feature.

Post-design re-check: PASS. No Complexity Tracking entries.

## Design

### D1. Session lifecycle (Rust core)

Typestate `Session<S>` in `crates/ttzip-archive/src/fs/collision/session.rs`:

```
Session<Created> --preflight()--> Preflighted { conflicts: ConflictTable }   (ZIP/7z)
Session<Created> --stage()------> Session<Staged>                              (all formats)
Session<Staged>  --post_stage_conflicts()--> ConflictTable                    (non-listable formats)
Session<Staged>  --commit(&ResolutionSet)--> CommitStep::{Finished(Outcome), NeedsDecision(Summary)}
any non-terminal --abort()--> Outcome { status: Stopped }
```

The UniFFI object (`ttzip-engine/src/uniffi_api/extraction_session.rs`) holds `Mutex<SessionSlot>` where `SessionSlot` is an enum of these typestates; transitions take the value out, call the typed method, and put the successor back. Wrong-state calls return `InvalidSessionState` instead of compiling into undefined behavior.

### D2. Preflight (FR-002, FR-003, SC-005)

Input: the entry list from the inspect fast path (sorted by path; directories flagged). Algorithm:

1. Open `destination_root` as a dirfd. Walk entries in path order with a stack of `(prefix, dirfd | Absent)`.
2. When an entry's parent directory is `Absent` at the destination, every descendant is free: skip the whole subtree without syscalls (prefix pruning). For a typical extraction into an empty or unrelated folder this means one `fstatat` per top-level entry.
3. Otherwise `fstatat(parent_fd, name, AT_SYMLINK_NOFOLLOW)`: `ENOENT` → free; directory onto directory → push `openat(O_DIRECTORY|O_NOFOLLOW)` and continue; anything else → `Conflict` (kind from `ItemKind` pair), existing name via `getattrlistat(ATTR_CMN_NAME)`.

Cost: at most one `fstatat` per entry whose parent exists at the destination, plus one `getattrlistat` per conflict. 100k overlapping entries ≈ 100k `fstatat` (≈ 0.1–0.2 s on APFS, to be measured in T041) — inside SC-005.

Formats without a cheap listing (tar.*, rar via libarchive, …) skip preflight; the same probe runs over the staged tree after `stage`.

### D3. Staging (FR-001, FR-012)

`crates/ttzip-archive/src/fs/collision/staging.rs` replaces `fs/staging_sandbox.rs`:

- Creates `<destination_root>/.ttzip_staging_<pid>_<session_id>/{payload,displaced}` with mode `0700`; creates `destination_root` itself if missing and remembers that.
- Exposes the payload directory only as a `StagingRoot` newtype. The orchestrator's extraction entry points (`unified/extract.rs`, `unified/extract_single.rs`) take `&StagingRoot` instead of `&Path`, so passing a user destination to a decoder no longer type-checks.
- `sweep_stale(destination_root)`: removes top-level `.ttzip_staging_<pid>_*` whose pid is dead.
- `Drop` without commit: `remove_dir_all(staging)`; remove `destination_root` if created by this session and now empty.

### D4. Commit walker (FR-001, FR-006, FR-007, FR-009, FR-011, FR-016)

`crates/ttzip-archive/src/fs/collision/commit.rs`, generic over `F: FsOps` (real syscalls in production, a fault-injecting implementation in tests; static dispatch, no vtable).

Walk the staged payload depth-first with paired dirfds `(staged_dir_fd, dest_dir_fd)`, in sorted name order (deterministic outcome and resumable cursor):

| Staged item vs. destination (`fstatat` NOFOLLOW) | Action |
|---|---|
| Destination absent | `renameatx_np(RENAME_EXCL \| RENAME_NOFOLLOW_ANY)`. A whole staged directory moves in one call. `EEXIST` → treat as new conflict (below) |
| Dir onto dir | Recurse with `openat(O_DIRECTORY \| O_NOFOLLOW)` on both sides |
| Conflict covered by `Replace`, same type | `renameatx_np(RENAME_NOFOLLOW_ANY)` (atomic replace) |
| Conflict covered by `Replace`, type mismatch | `renameatx_np(RENAME_SWAP \| RENAME_NOFOLLOW_ANY)`; original now in staging, moved to `displaced/` |
| Conflict covered by `KeepBoth` | Loop N = 2, 3, …: `renameatx_np(RENAME_EXCL \| RENAME_NOFOLLOW_ANY)` to `<stem> N<.ext>` until success (bounded at N ≤ 10,000 → `FailedItem`) |
| Conflict covered by `Skip` | Leave in staging |
| Conflict not covered (destination changed since detection) | Apply `default_action` if present; otherwise append to the conflict table as `CommitRecheck`, stop the walk, return `NeedsDecision` with the cursor saved |
| Per-item syscall error (`EACCES`, `ENAMETOOLONG`, `ENOTSUP`, …) | Record `FailedItem`, continue with the next sibling |

Free-name placement (rows "Destination absent", "KeepBoth", and `CommitRecheck`) goes through `fs_ops::place_exclusive` (FR-022): `RENAME_EXCL` when the volume reports it; otherwise, or on runtime `ENOTSUP`/`EOPNOTSUPP`/`ENOSYS`, `linkat` + unlink staged name for regular files. When no atomic exclusive primitive is available for the item kind (directories and symlinks without `RENAME_EXCL`, or regular files without both `RENAME_EXCL` and hard links), `place_exclusive` returns `ENOTSUP` (`FailureReason::NotSupportedByVolume`), preserving both destination (including existing empty directories) and staged source without creating any reservation placeholder or falling back to plain `rename`. No path uses an existence lookup as the exclusivity guarantee. A failed capability probe yields `VolCaps::default()` (all unsupported). `ENOTSUP` for `RENAME_SWAP` or `place_exclusive` fails only that item.

C-ABI extraction entry points (`ffi/archive_ffi/{extract,unified}.rs`) drive `ExtractionSession` through one helper (`ffi/archive_ffi/staged_commit.rs`) that applies the policy, maps `OutcomeStatus` to `TTZipStatus`, reports the committed item count, and populates `TTZipErrorInfo` with ABI v3 layout parity across Rust and `sdk/include/ttzip.h` (FR-023).

After the walk: `remove_dir_all(staging)` (skipped items and displaced originals go with it), unregister, return `ExtractionOutcome`.

Crash safety argument (FR-011): every destination name changes only through one `renameatx_np` call, which the filesystem applies atomically; a crash between calls leaves already-moved items new and the rest original. The only non-destination state is the staging directory, removed by the next session in that folder or at next app launch. No journal (data-model §3.1).

### D5. Resolution semantics

`ResolutionSet` is validated once per `commit` (sorted unique in-range overrides). Lookup during the walk: conflict id by binary search of the sorted conflict table on `relative_path`, then override by binary search on id, then `default_action`. Policy sessions build `ResolutionSet { default_action: policy, overrides: [] }`; `Stop` policy aborts on the first conflict detected (preflight, post-stage, or recheck).

### D6. Swift facade and UI

- **TTZipCore** `Facades/TTZipEngineFacade+Extraction.swift`: `extract(_:handling:progress:)` drives the loop: `new → preflight → [decide] → stage (NativeComputeDispatcher extraction queue) → [decide] → commit → while NeedsDecision { decide; commit }`. Resolutions accumulate in one `ResolutionSet` value. Cancellation via the existing token maps to `abort()`.
- **TTZipCore** `ExtractionStagingRegistry` (`UserDefaults`), launch sweep invoked from `AppDelegate`.
- **TTKitUI** `ForensicCollisionPanel`: actions = Replace, Keep Both (default, Return), Skip, Stop; "apply to all remaining" toggle; shows name, relative path, size, mtime of both sides, conflict kind, `index / total`. Removed: `keepNewer`, `keepLarger`, `mergeDirectory`, the ` (1)` documentation.
- **TTZipApp** `CollisionPromptPresenter` (implements `CollisionDecisionProvider`): pages conflicts lazily, one panel per conflict until "apply to all" or the end; `OverlayState.collisionPrompt: CollisionPrompt?` replaces the four Phase 10 fields.
- **Settings** General section: "When extracted items already exist": Ask (default) / Keep Both / Replace / Skip → `ttzip_collision_default`.
- **Callers**: interactive — `quickExtractArchive`, `extractArchive`, `extractSingleEntry` (with `root_at_common_parent: true`); policy `.keepBoth` — Finder Extract Here (`AppIntentDispatcher`), batch (`ArchiveBatchFacade`), drag-out, FileProvider, plugin host.

### D7. Performance budget (SC-004)

No-conflict path adds: one `fstatat` per top-level staged item, one `renameatx_np` per top-level item (today: one `rename` of the whole staging dir when the destination did not exist, else a full recursive promotion with per-file `remove_file` + `rename`). For destinations that already exist, the new walker performs fewer syscalls than today's `promote_dir_contents`, which visits every file. For fresh destinations the extra cost is O(top-level items) metadata calls. Verified by T040 against the standard benchmark corpus.

### D8. Three-phase metadata plan & intra-archive duplicate isolation (FR-024)

1. **Phase 1 (Decode & Stage)**: Decoders write into `staging/payload` with directories at `0o700` and regular files at `0o600`. When `SafeExtractEngine::apply_deferred_metadata` runs on a transactional staging payload root (`.ttzip_staging_*/payload`), it first captures each staged item's `(st_dev, st_ino)` identity (`expected_identity`) via `populate_staged_identities` (failing explicitly if any staged entry's identity cannot be resolved) and stores a `StagedMetadataPlan` in `fs::staged_metadata` instead of calling `chmod` inside `staging/payload`. Both full and selective extraction (`extract_selected_entries`) populate `SafeExtractEngine`. Intra-archive duplicate regular files always stage with `InFlightJailFile::open(..., true)` using an exclusive sibling temporary file (`.ttzip_inflight_*` with `O_CREAT | O_EXCL | O_NOFOLLOW`, never `O_TRUNC`) followed by atomic `renameat` on commit, decoupling Stage 1 from Stage 2 `CollisionPolicy` and protecting hardlinked siblings.
2. **Phase 2 (Transactional Commit)**: `DFSCommitWalker::commit_with_mapping` records `CommittedPathEntry` items in `CommitPathMapping`, mapping each committed `staged_rel_path` to its `committed_rel_path` (accounting for `KeepBoth` renames on files or ancestor directories) and tagging directories with `CommittedEntryDisposition::MergedExistingDir` when merged into a pre-existing destination directory.
3. **Phase 3 (Final Metadata Application)**: `apply_committed_metadata` resolves each committed item step-by-step from `DirJail::root_fd()` with `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`, requires `expected_identity: Some((exp_dev, exp_ino))` and verifies `fstatat` + `fstat(fd)` match `(exp_dev, exp_ino)`, applies `futimens(fd)` and `fchmod(fd)` on the held FD (and `O_SYMLINK` / `AT_SYMLINK_NOFOLLOW` for symlinks), applies directory `mode` and `mtime` bottom-up (deepest first) only to newly placed directories while leaving `MergedExistingDir` directories untouched, and merges any metadata failure into `ExtractionOutcome` (`CompletedWithFailures` / C-ABI `ErrExtractionFailed`) without deleting committed file contents.

## Project Structure

### Documentation (this feature)

```text
products/ttzip/core/specs/229-extraction-collision-protocol/
├── spec.md
├── plan.md                       # this file
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── extraction-session.schema.json
│   └── uniffi-api.md
├── checklists/requirements.md
└── tasks.md                      # /speckit-tasks output
```

### Source Code

```text
products/ttzip/core/rust/
├── crates/ttzip-types/src/options/mod.rs              # collision_policy: u8, ABI v3
├── crates/ttzip-archive/src/fs/
│   ├── collision/                                     # NEW (pure core + syscall shell)
│   │   ├── mod.rs                                     # types, ConflictTable arena, ResolutionSet validation
│   │   ├── naming.rs                                  # Keep Both name generation (pure)
│   │   ├── probe.rs                                   # dirfd walk, fstatat, getattrlistat
│   │   ├── preflight.rs                               # entry list -> ConflictTable with prefix pruning
│   │   ├── staging.rs                                 # StagingRoot, sweep_stale, Drop cleanup
│   │   ├── fs_ops.rs                                  # FsOps trait, LibcFsOps
│   │   ├── commit.rs                                  # commit walker
│   │   └── session.rs                                 # typestate Session<S>
│   └── staging_sandbox.rs                             # DELETED
├── crates/ttzip-archive/src/archive/unified/
│   ├── extract.rs                                     # takes &StagingRoot
│   └── extract_single.rs                              # takes &StagingRoot; no direct destination writes
├── crates/ttzip-archive/tests/collision_*.rs          # corpus, fault injection, property tests
├── ttzip-engine/src/uniffi_api/
│   ├── extraction_session.rs                          # NEW UniFFI object + records
│   ├── extraction.rs                                  # one-shot functions -> session; 2 exports deleted
│   ├── types.rs                                       # config record: policy replaces overwrite_existing
│   └── integrity.rs                                   # drop removed field
├── ttzip-engine/src/ffi/archive_ffi/{extract,unified}.rs
├── ttzip-python/src/lib.rs
└── ttzip-tui/src/cli/handlers/extract.rs              # --on-conflict

products/ttzip/core/Sources/TTZipCore/
├── Generated/ttzip_engine.swift                       # regenerated
├── Facades/TTZipEngineFacade+Extraction.swift         # NEW session loop
├── Facades/ExtractionStagingRegistry.swift            # NEW
├── ArchiveExtractor.swift                             # rewired
└── Facades/ArchiveBatchFacade.swift                   # policy .keepBoth

infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionPanel.swift   # actions reduced

products/ttzip/apple/Sources/TTZipApp/
├── ViewModels/AppSubStates.swift                      # OverlayState.collisionPrompt
├── ViewModels/AppViewState+ArchiveOperations.swift    # interactive handling
├── Services/CollisionPromptPresenter.swift            # NEW
├── ViewModels/Dispatchers/AppIntentDispatcher.swift   # Extract Here -> .keepBoth
├── Views/Main/MainRootOverlaysView.swift              # renders collisionPrompt
├── Views/SettingsView+Tabs.swift                      # default behavior picker
└── Services/AppDelegate.swift                         # launch sweep

products/ttzip/bindings/{cpp,csharp}/, products/ttzip/core/sdk/python/ttzip/   # regenerated + SDK wrapper policy param
```

**Structure Decision**: The collision protocol is a self-contained module (`fs/collision/`) inside the existing archive crate, with a pure core (types, naming, resolution validation, planning) and a thin syscall shell (`fs_ops.rs`, `probe.rs`). Removing the feature would mean deleting that directory and restoring the old sandbox, with no other crate depending on its internals.

## Risks

| Risk | Mitigation |
|---|---|
| `RENAME_NOFOLLOW_ANY` / `RENAME_EXCL` unsupported on non-APFS volumes | Detect `ENOTSUP`/`ENOSYS` once per session, switch to documented fallback; corpus runs on an APFS case-sensitive disk image and an exFAT disk image created with `hdiutil` in tests |
| Generated bindings drift across Swift / Python / C++ / C# | Regenerate all in one task (T020) with the existing scripts; Swift build + Python SDK tests in the same commit |
| Staging inside the destination is visible in Finder during extraction | Name starts with `.`; hidden by default in Finder; removed on completion |
| Existing tests encode silent overwrite | Rewritten in T019/T035 to assert the new semantics (list in research R5) |
| 1M-conflict re-extraction | Paging + "apply to all" (SC-002); conflict table is columnar (data-model §1.4) |
