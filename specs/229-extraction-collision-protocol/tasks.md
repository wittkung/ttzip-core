---
description: "Task list for the extraction name-collision protocol"
---

# Tasks: Extraction Name-Collision Protocol

**Input**: `products/ttzip/core/specs/229-extraction-collision-protocol/` (plan.md, spec.md, research.md, data-model.md, contracts/)

**Tests**: Required — the spec defines measurable outcomes SC-001…SC-006 and the constitution requires oracle-first testing.

**Format**: `- [ ] Txxx [P?] [Story?] Description (files)`; `[P]` = no shared files with other open `[P]` tasks of the same wave. Paths are relative to the repository root; `R/` = `products/ttzip/core/rust/`, `C/` = `crates/ttzip-archive/src/fs/collision/` under `R/`.

**Dispatch waves** (each wave's tasks have disjoint write sets):

| Wave | Tasks | Depends on |
|---|---|---|
| A | T001, T002–T006 (one agent, Rust core), T007 (TTKit panel) | — |
| B | T008–T011 | A |
| C | T012–T019 | B |
| D | T020–T024 (TTZipCore), T025–T029 (TTZipApp), T030–T031 (SDK/CLI) | C |
| E | T032–T040 | D |

---

## Phase 1: Setup

- [ ] T001 [P] Test fixture builder generating real archives at test time (ZIP via `/usr/bin/zip` and the engine writer, 7z via the engine writer, tar.gz via `/usr/bin/tar`) and destination trees (files, dirs, symlinks, case and NFD variants, read-only items); helpers to snapshot a destination tree (path, kind, size, mtime, content hash) and diff two snapshots. (`R/crates/ttzip-archive/tests/support/collision_fixtures.rs`, `R/crates/ttzip-archive/tests/support/mod.rs`)

## Phase 2: Foundational (blocks all stories)

- [ ] T002 Core types per data-model §1: `ItemKind`, `ItemMeta`, `ConflictKind`, `DetectionPhase`, columnar `ConflictTable` (string arena + offsets, sorted by relative path, binary-search lookup), `CollisionAction`, `CollisionPolicy`, `ResolutionSet` with `validate(&ConflictTable)` (sorted, unique, in range) and `action_for(id)`; `ExtractionOutcome`, `FailedItem`, `FailureReason`. Unit tests + a property test for validation. (`R/C/mod.rs`)
- [ ] T003 Keep Both naming per research R3: pure `fn candidate(name: &OsStr, kind: ItemKind, n: u32) -> OsString` (`<stem> N<.ext>`, dotfiles have no extension, directories never split), checked counter. Table-driven tests incl. `name 2.txt → name 2 2.txt`, `.bashrc → .bashrc 2`, `archive.tar.gz → archive.tar 2.gz`. (`R/C/naming.rs`)
- [ ] T004 `FsOps` trait (static dispatch) with `LibcFsOps`: `openat_dir_nofollow`, `fstatat_nofollow`, `getattrlistat_name`, `renameatx(from_fd, from, to_fd, to, flags)`, `unlinkat`, `remove_tree`; errno mapping to `FailureReason`; volume capability probe (`RENAME_EXCL`/`RENAME_SWAP` support, cached per session). All `unsafe` blocks carry `// SAFETY:` comments. (`R/C/fs_ops.rs`)
- [ ] T005 Destination probe: dirfd-based lookup of a relative path without following symlinks, returning `Absent | Dir(fd) | Item(ItemMeta)`; on-disk name via `ATTR_CMN_NAME` for conflicts only. Tests on the default APFS volume and on a case-sensitive APFS disk image created with `hdiutil` (skipped with a logged reason if `hdiutil` is unavailable). (`R/C/probe.rs`)
- [ ] T006 Staging per plan D3: `StagingArea::create(destination_root)` (creates root if missing and records it), `StagingRoot` newtype for the payload dir, `displaced_dir()`, `sweep_stale(destination_root) -> u32` (dead-pid check via `kill(pid, 0)` → `ESRCH`), `Drop` cleanup incl. removing a root this session created. (`R/C/staging.rs`)
- [ ] T007 [P] `ForensicCollisionPanel`: actions reduced to `replace`, `keepBoth`, `skip`, `stop`; Return = Keep Both; Escape = Stop; shows name, relative path, size, mtime, conflict kind and `index / total`; remove `keepNewer`, `keepLarger`, `mergeDirectory` and the ` (1)` doc comment; update TTKit tests and any TTKit consumer. (`infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionPanel.swift`, `infra/ttkit/Tests/**`)

**Checkpoint**: `cargo test -p ttzip-archive collision` green for T002–T006; `bazel test //infra/ttkit:all` green for T007.

## Phase 3: User Story 1 — Never destroy existing files without consent (P1) 🎯 MVP

- [ ] T008 [US1] Preflight per plan D2: from a sorted entry list (inspect fast path output) build a `ConflictTable` with prefix pruning; one `fstatat` per entry whose parent exists at the destination. Tests: empty destination (zero conflicts, syscall count ≤ top-level entries via a counting `FsOps`), overlap, dir-onto-dir merge, type mismatch both ways, symlink at destination, case/NFD variants. (`R/C/preflight.rs`)
- [ ] T009 [US1] Post-stage detection: same probe over the staged payload tree for formats without a cheap listing. (`R/C/preflight.rs`)
- [ ] T010 [US1] Commit walker per plan D4, generic over `F: FsOps`: paired dirfd DFS in sorted order, exclusive moves, same-type replace, type-mismatch swap into `displaced/`, Keep Both loop (N ≤ 10,000), Skip, `CommitRecheck` with saved cursor, per-item failures, `ENOTSUP` fallback switch, final staging removal. (`R/C/commit.rs`)
- [ ] T011 [US1] Typestate session per plan D1 (`Session<Created>`, `Session<Staged>`, terminal states), `abort()`, policy-to-resolution mapping, `Stop` policy aborts on first conflict. (`R/C/session.rs`)
- [ ] T012 [US1] Orchestrator: `unified/extract.rs` takes `&StagingRoot` and no longer creates or commits a sandbox; delete `R/crates/ttzip-archive/src/fs/staging_sandbox.rs` and its module declaration; fix `R/crates/ttzip-archive/src/archive/builder.rs` (`ExtractBuilder::extract_to` runs a policy session). (`R/crates/ttzip-archive/src/archive/unified/extract.rs`, `.../unified/mod.rs`, `.../archive/builder.rs`, `.../fs/mod.rs`)
- [ ] T013 [US1] C-ABI: `TTZipExtractOptions.overwrite_existing: bool` → `collision_policy: u8`, `TTZIP_ABI_VERSION_3`, reject v2 with `ErrAbiMismatch`, invalid values with `ErrInvalidParam`; update every construction site from the inventory (research R5) incl. `zip/mod.rs`, `tar/tests.rs`, `unified/tests_lifecycle.rs`, `ffi/archive_ffi/{extract,unified}.rs`, `uniffi_api/integrity.rs`, benches and `ttzip-engine/tests/*`. (`R/crates/ttzip-types/src/options/mod.rs` + listed sites)
- [ ] T014 [US1] UniFFI object `ExtractionSession` + records/enums exactly as `contracts/extraction-session.schema.json` and `contracts/uniffi-api.md` §1, §3; `sweep_stale_staging`. (`R/ttzip-engine/src/uniffi_api/extraction_session.rs`, `R/ttzip-engine/src/uniffi_api/mod.rs`, `R/ttzip-engine/src/uniffi_api/error_mapping.rs`)
- [ ] T015 [US1] One-shot functions per `contracts/uniffi-api.md` §2: `extract_archive_stream(+policy)` via session; delete `extract_archive_stream_advanced`, `extract_single_entry_to_path`, `UniFFIExtractArchiveConfig.overwrite_existing`. (`R/ttzip-engine/src/uniffi_api/extraction.rs`, `R/ttzip-engine/src/uniffi_api/types.rs`)
- [ ] T016 [US1] Regenerate all bindings with the existing scripts (`products/ttzip/core/scripts/build_rust.sh`, `generate_crossplatform_bindings.sh`, `postprocess_uniffi_swift.py`): Swift `Generated/ttzip_engine.swift`, C headers, C++ / C# bindings, Python `ttzip_engine.py`. (generated files only)
- [ ] T020 [US1] Swift facade loop per plan D6 and `contracts/uniffi-api.md` §4: `CollisionHandling`, `CollisionDecisionProvider`, `TTZipEngineFacade.extract(_:handling:progress:)`; stage on the existing extraction queue; cancellation → `abort()`. (`products/ttzip/core/Sources/TTZipCore/Facades/TTZipEngineFacade+Extraction.swift`)
- [ ] T021 [US1] Rewire `ArchiveExtractor` full-extraction paths and `TTZipEngineFacade.quickExtract` / `extractWithCommand` to the facade loop; remove now-unused wrappers. (`products/ttzip/core/Sources/TTZipCore/ArchiveExtractor.swift`, `.../Facades/TTZipEngineFacade.swift`, `.../Commands/ExtractCommand.swift`)
- [ ] T025 [US1] `OverlayState.collisionPrompt: CollisionPrompt?` replaces `showForensicCollision`, `collisionSource`, `collisionTarget`, `collisionResolver`. (`products/ttzip/apple/Sources/TTZipApp/ViewModels/AppSubStates.swift`)
- [ ] T026 [US1] `CollisionPromptPresenter: CollisionDecisionProvider` — lazy paging (1024 per page), one panel per conflict until "apply to all", returns `ResolutionSet` or `nil` (Stop). (`products/ttzip/apple/Sources/TTZipApp/Services/CollisionPromptPresenter.swift`)
- [ ] T027 [US1] Render `collisionPrompt` with `ForensicCollisionPanel`; interactive handling for `quickExtractArchive` and `extractArchive` honoring the Settings default (`Ask` → presenter, else policy). (`products/ttzip/apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift`, `.../ViewModels/AppViewState+ArchiveOperations.swift`)

**Checkpoint**: US1 acceptance scenarios 1–6 pass as Rust integration tests (T032) and the quickstart §3 steps 1–5 pass in the app.

## Phase 4: User Story 2 — Partial and single-entry extraction obey the same rules (P1)

- [x] T017 [US2] `unified/extract_single.rs::extract_selected_entries` takes `&StagingRoot`; remove every direct destination write; implement `root_at_common_parent` path remapping (pure function + tests). (`R/crates/ttzip-archive/src/archive/unified/extract_single.rs`)
- [x] T018 [US2] UniFFI `extract_selected_entries(+policy, +root_at_common_parent)` via session; `ExtractionRequest.entry_filter` support in `ExtractionSession`. (`R/ttzip-engine/src/uniffi_api/extraction.rs`, `.../extraction_session.rs`) — Deviation: the one-shot `extract_selected_entries` export was deleted instead of re-signed; after T022 it has zero callers and `ExtractionSession` + `entry_filter` covers it (see contracts/uniffi-api.md).
- [x] T022 [US2] Rewire `ArchiveExtractor.extractSingleFile`, `ArchiveSelectiveExtractor.extractSelected`, `TTZipEngineFacade.extractSingleEntry` to the facade loop with `root_at_common_parent: true` for user destinations. (`products/ttzip/core/Sources/TTZipCore/ArchiveExtractor.swift`, `.../Facades/TTZipEngineFacade.swift`)
- [x] T028 [US2] `AppViewState.extractSingleEntry` and the archive browser's extract actions use interactive handling. (`products/ttzip/apple/Sources/TTZipApp/ViewModels/AppViewState+ArchiveOperations.swift`)

**Checkpoint**: US2 scenarios pass (T032); quickstart §3 step 6.

## Phase 5: User Story 3 — Non-interactive extraction uses a policy (P2)

- [x] T019 [US3] One-shot extraction functions reject `Ask` with InvalidArgument; `Stop` policy returns early with zero writes; C-ABI whole-archive extraction routed through staging (FR-001 root-cause fix). (`R/ttzip-engine/src/uniffi_api/extraction.rs`, `.../ffi/archive_ffi/extract.rs`)
- [x] T023 [US3] Policy `.keepBoth` for batch (`ArchiveBatchFacade`), drag-out (`ArchiveFilePromiseProvider.swift`), FileProvider (`TTZipFileProviderExtension.swift`), and app private caches (`.replace`). (`products/ttzip/core/Sources/TTZipCore/Facades/ArchiveBatchFacade.swift`, `products/ttzip/apple/Sources/TTZipApp/Services/ArchiveFilePromiseProvider.swift`, `products/ttzip/apple/Sources/TTZipFileProvider/TTZipFileProviderExtension.swift`)
- [x] T029 [US3] Settings General section picker "When extracted items already exist" (Ask / Keep Both / Replace / Skip) bound to `@AppStorage("ttzip_collision_default")`; Finder Extract Here in `AppIntentDispatcher` enforces `.policy(.keepBoth)`. (`products/ttzip/apple/Sources/TTZipApp/Views/SettingsView+Tabs.swift`, `.../ViewModels/Dispatchers/AppIntentDispatcher.swift`)
- [ ] T030 [P] [US3] Python: `ttzip-python` C-ABI caller sets `collision_policy`; SDK `extract(...)` gains `on_conflict="keep_both"`; SDK tests. (`R/ttzip-python/src/lib.rs`, `products/ttzip/core/sdk/python/ttzip/__init__.py`, SDK tests)
- [x] T031 [P] [US3] TUI/CLI `--on-conflict keep-both|replace|skip|stop` (default `keep-both`). (`R/ttzip-tui/src/cli/handlers/extract.rs`, CLI arg definitions)

**Checkpoint**: US3 scenarios pass; quickstart §3 steps 7–8; SC-006 verified by T036.

## Phase 6: User Story 4 — Interruption never leaves a half-replaced destination (P2)

- [x] T024 [US4] `ExtractionStagingRegistry` (`UserDefaults` key `TTZip_ActiveStagingRoots`) used by the facade loop; launch sweep. (`products/ttzip/core/Sources/TTZipCore/Facades/ExtractionStagingRegistry.swift`, `products/ttzip/apple/Sources/TTZipApp/Services/AppDelegate.swift`)
- [x] T033 [US4] Fault-injection matrix: a `FaultFsOps` failing the k-th call for every k across Replace / Keep Both / type-mismatch commits of 100 items; after each run assert every destination path is original-or-new and complete (snapshot diff) and the outcome lists exactly the uncompleted items. Kill-test: child process killed with SIGKILL mid-commit, parent asserts the same invariant and that `sweep_stale` removes staging. (`R/crates/ttzip-archive/tests/collision_fault_injection.rs`)

**Checkpoint**: SC-003 satisfied.

## Phase 7: Polish & Cross-Cutting

- [x] T032 Collision corpus ≥ 200 scenarios = {zip, 7z, tar.gz} × {Replace, Keep Both, Skip, Stop} × {file/file, dir/file, file/dir, dir merge, symlink at destination, case variant, NFD variant, Keep Both name taken, read-only item, destination changed during decision (recheck), 10,000 conflicts with apply-to-all, nested conflict below merged dir, empty archive, selected entries, single entry, root_at_common_parent, missing destination root}; assert zero unchanged-without-decision violations and exact expected trees. (`R/crates/ttzip-archive/tests/collision_corpus.rs`)
- [x] T034 Rewrite tests that encoded silent overwrite to the new semantics: `staging_sandbox.rs:157` (deleted with the file — port its symlink-immunity cases to T032), `secure_extract_tests.rs:195`, `safe_extract.rs:625`, `tests_lifecycle.rs`, `UniFFIBindingsTests.swift:122`, `AppViewStateArchiveOperationsTests.swift:114`.
- [x] T035 Swift tests: facade loop with a fake provider (preflight → decide → stage → recheck → decide → commit), presenter paging and apply-to-all, Stop → destination unchanged. (`products/ttzip/core/Tests/TTZipTests/ExtractionSessionFacadeTests.swift`, `products/ttzip/apple/Tests/TTZipAppTests/CollisionPromptPresenterTests.swift`, BUILD test srcs)
- [x] T036 SC-006 test: batch, drag-out and Finder-intent paths with a provider that fails the test if called. (`products/ttzip/apple/Tests/TTZipAppTests/NonInteractiveExtractionTests.swift`)
- [ ] T037 Fuzz targets: `ResolutionSet` validation and `naming::candidate` (crash-first: samples written to disk before execution). (`R/fuzz/` existing layout)
- [x] T038 Static gates in CI: `git grep` checks from quickstart §2 added to `tools/ci/lint_code_quality.py` (forbid `overwrite_existing`, `promote_dir_contents`, `StagingSandbox` under `products/ttzip`); orphan gate passes with the new types reachable.
- [x] T039 Run quickstart.md §1–§4 and record outputs in the PR description.
- [x] T040 Benchmark `bench_extract_collision`: no-conflict extraction vs. `main` on the standard corpus (SC-004 ≤ 3 %), preflight at 10k / 100k overlapping entries (SC-005 ≤ 0.5 s), commit at 1M conflicts with apply-to-all. (`R/ttzip-engine/benches/bench_extract_collision.rs`, `R/ttzip-engine/Cargo.toml` bench entry)

## Dependencies

- T002 blocks T003–T011; T004 blocks T005, T006, T010; T006 blocks T010–T012.
- T008–T011 block T012–T019.
- T014–T016 block every Swift task (T020–T029).
- T020 blocks T021–T024, T027, T028; T025 + T026 block T027.
- T012–T019 block T030–T031 (C-ABI field and engine API).
- Phase 7 runs after Phases 3–6; T032 and T033 can start once T011 is done (Rust-level).

## Parallel Opportunities

- Wave A: Rust core (T002–T006) ∥ TTKit panel (T007) ∥ fixtures (T001).
- Wave D: TTZipCore (T020–T024) ∥ TTZipApp (T025–T029, after T020's protocol file exists — dispatch T020 first, then T025–T029) ∥ SDK/CLI (T030, T031).
- Wave E: T032 ∥ T033 ∥ T035/T036 ∥ T037 ∥ T040.

## Implementation Strategy

MVP = Phases 1–3 (US1) behind no flag: the old silent-overwrite path is removed in the same change, never kept alongside. US2 lands in the same release because it closes the same data-loss defect on the selective path. US3/US4 complete the policy and recovery guarantees before the release is cut.

## Phase 8: Data Integrity Regression & Outcome Hardening (Group D & Group G1)

- [x] T041 [Group D] Document terminal outcome states (`Completed`, `CompletedWithFailures`, `Stopped`, `Cancelled`, `Corrupted`), item commit failure vs. staging cleanup failure semantics, single-file atomic replacement vs. multi-volume creation backup/rollback/preservation protocol, pre-commit dual cancellation point of no return, and Swift Facade caller inventory. (`products/ttzip/docs/twin/05_archive_transfer.md`, `products/ttzip/core/specs/229-extraction-collision-protocol/{spec.md,contracts/uniffi-api.md,tasks.md}`)
- [ ] T042 [Group D] Stage archive creation (`create_archive`, `create_archive_explicit`, `create_archive_stream`, `create_archive_advanced`) in a sibling `.ttzip_create_txn_*` directory with explicit caller-supplied `CreateCommitMode`: use direct POSIX atomic `rename` for `CreateCommitMode::SingleFile` (zero missing-target window, zero neighbor scanning even when target filename ends with `.001`), and use a two-phase `.ttzip_create_bak_*` backup & verified rollback transaction for `CreateCommitMode::SplitVolume(VolumeNamingScheme)` (even when output fits in volume 1 alone, matching only confirmed contiguous volume chains, preserving `.ttzip_create_bak_*` and returning a locatable/recoverable error if any rollback restore fails). (`R/crates/ttzip-archive/src/archive/unified/create_staging.rs`, `R/crates/ttzip-archive/src/archive/unified/create.rs`, `R/crates/ttzip-archive/src/archive/split/writer.rs`, `R/ttzip-engine/src/uniffi_api/archive.rs`)
  - Status: `[Impl: done (explicit CreateCommitMode)] [Rust Tests: passed (28/28 Bazel & Cargo)] [Swift Tests: N/A] [Integration Acceptance: pending integration]`
- [x] T043 [Group D] Fix `fs/collision/commit.rs` (`walk_dir`, `commit_directory`, `commit_file`, `replace_via_swap`) to distinguish item commit failures (recorded in `outcome.failed` without aborting sibling traversal via `?`, incrementing `extracted`/`replaced`/`kept_both`/`skipped`/`merged_dirs` only on `Ok(())`) from post-commit staging cleanup failures (`displaced_dirfd` parking / drained `payload_fd` `rmdir`). (`R/crates/ttzip-archive/src/fs/collision/commit.rs`, `R/crates/ttzip-archive/tests/collision_fault_injection.rs`)
  - Status: `[Impl: done (accepted)] [Rust Tests: passed (6/6 collision_fault_injection_test + 12/12 data_integrity_regression_tests)] [Swift Tests: passed (10/10 ExtractionSessionFacadeTests)] [Integration Gate: pending coordinator full gate]`
- [x] T044 [Group D] Enforce pre-commit dual cancellation checks (`Task.isCancelled || token?.isCancelled() == true || cancelToken.isCancelled()`) before every `session.commit` call (including after interactive `decide`) and propagate `ExtractionOutcome` through `ExtractResult`, `TTZipEngineFacade.quickExtract` / `executePipelineExtract`, and `AppViewState+ArchiveOperations.swift`. (`products/ttzip/core/Sources/TTZipCore/Facades/TTZipEngineFacade+Extraction.swift`, `products/ttzip/core/Sources/TTZipCore/Facades/TTZipEngineFacade.swift`, `products/ttzip/apple/Sources/TTZipApp/ViewModels/AppViewState+ArchiveOperations.swift`)
  - Status: `[Impl: done (accepted)] [Rust Tests: N/A] [Swift Tests: passed (10/10 ExtractionSessionFacadeTests + 7/7 AppViewStateArchiveOperationsTests)] [Integration Gate: pending coordinator full gate]`
- [ ] T045 [Group D] Maintain the 6 reproduction tests (`data_integrity_repro_tests.rs`) and expand Rust & Swift regression suites covering 2nd-volume commit failure rollback, rollback restore failure preserving `.ttzip_create_bak_*` with recovery, single-file & multi-volume neighbor protection (`report.zip.1`, `report.zip.2026`), `.001`-named single-file archive vs. single-volume split archive creation (`test_dot_001_single_file_vs_single_volume_split_mode_distinction`), fewer-volume replacement cleanup, directory-merge item commit failure vs. post-swap staging cleanup failure, cancel-during-decide before commit, and end-to-end Swift `.stopped` / `.completedWithFailures` UI propagation. (`R/ttzip-engine/tests/data_integrity_repro_tests.rs`, `R/ttzip-engine/tests/data_integrity_regression_tests.rs`, `R/crates/ttzip-archive/tests/collision_fault_injection.rs`, `products/ttzip/core/Tests/TTZipTests/ExtractionSessionFacadeTests.swift`, `products/ttzip/apple/Tests/TTZipAppTests/AppViewStateArchiveOperationsTests.swift`)
  - Status: `[Impl: done] [Rust Tests: passed (28/28 across 3 suites in Bazel & Cargo)] [Swift Tests: passed (17/17 across 2 suites in Bazel)] [Integration Acceptance: pending integration]`
- [ ] T046 [Group G1] Unify CRC32 (`crc_defined` including `crc == 0`), truncated EOF, libarchive error codes (`ARCHIVE_WARN = -20`, `ARCHIVE_RETRY = -10`), missing split volume tail verification, pre-auth target file protection, and `ArchiveExtractor.mapQuarantineWorkerError(.fileNotFound)`. (`R/crates/ttzip-archive/src/{sevenz,zip,archive/unified,archive/split,fs/dir_jail.rs,fs/safe_extract.rs}`, `products/ttzip/core/Sources/TTZipCore/ArchiveExtractor.swift`)


## Phase 9: Audit Follow-up (Antigravity, 2026-10-17)

- [x] T047 [Antigravity] Implement FR-022 atomic exclusive placement for volumes without `RENAME_EXCL` and for failed capability probes: `fs_ops::place_exclusive` (`RENAME_EXCL` → `linkat` + unlink staged name for regular files → `ENOTSUP` / `FailureReason::NotSupportedByVolume` when no atomic exclusive primitive is available, with zero reservation fallback), used by `commit_directory` (destination absent), `commit_file` (non-conflicting / `CommitRecheck`), and `commit_keep_both`. Tests: Keep Both candidate already taken (file and directory, including empty directory candidates), unsupported volume capabilities (`NotSupportedByVolume` preserving destination and staged source with zero reservation artifacts), same-name file/directory appearing concurrently before placement, and capability probe failure. (`R/crates/ttzip-archive/src/fs/collision/{fs_ops.rs,commit.rs,commit_exclusive_tests.rs}`)
- [x] T048 [Antigravity] Implement FR-023 for the extraction C-ABI and synchronize `TTZipErrorInfo` / `TTZipExtractOptions` ABI v3 in `sdk/include/ttzip.h`: shared `staged_commit.rs` driver used by `ttzip_rust_extract_archive`, `ttzip_rust_archive_extract_unified{,_v2}`, `ttzip_rust_archive_extract_selected`; Stop → `Cancelled`, `CompletedWithFailures` → `ErrExtractionFailed`, only `Completed` → `Ok`; `out_extracted_count` from actual commit outcome; C header `sizeof` / `alignof` / `offsetof` parity and real `out_error` verification against Rust. (`R/ttzip-engine/src/ffi/archive_ffi/{mod.rs,staged_commit.rs,extract.rs,unified.rs}`, `products/ttzip/core/sdk/include/{ttzip.h,ttzip.hpp}`, `products/ttzip/core/sdk/c/{test_c_sdk.c,interop_cli.c,asan_stress_test.c}`, `R/ttzip-engine/tests/cabi_extract_outcome_contract_tests.rs`)
- [x] T049 [Antigravity / Group G] Implement FR-024 three-phase extraction metadata ordering, `DirJail` FD-bound metadata application with `(st_dev, st_ino)` identity verification (failing explicitly if Stage 1 identity capture fails or `expected_identity` is missing), intra-archive duplicate safe sibling replacement (`InFlightJailFile`), and metadata failure propagation (`OutcomeStatus::CompletedWithFailures` / C-ABI `TTZipStatus::ErrExtractionFailed`) across Rust `ExtractionSession`, UniFFI `ExtractionSession`, and C-ABI while preserving committed file contents. (`R/crates/ttzip-archive/src/fs/{staged_metadata.rs,safe_extract.rs,dir_jail.rs,tests_safe_extract.rs}`, `R/crates/ttzip-archive/src/fs/collision/{mod.rs,commit.rs,session.rs}`, `R/ttzip-engine/src/uniffi_api/extraction_session.rs`, `R/ttzip-engine/tests/{cabi_extract_outcome_contract_tests.rs,secure_extract_tests.rs}`)
