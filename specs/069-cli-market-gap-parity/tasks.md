# Tasks: Comprehensive Market Gap Parity and Advanced Ergonomics for `ttzip-cli`

**Feature Branch**: `069-cli-market-gap-parity`  
**Specification**: [`specs/069-cli-market-gap-parity/spec.md`](spec.md)  
**Implementation Plan**: [`specs/069-cli-market-gap-parity/plan.md`](plan.md)  

---

## Phase 1: Setup (Infrastructure & Option Specifications)

**Purpose**: Expand CLI models, command specifications, and schemas.

- [x] T001 [P] Extend `CLIOptions` in `Sources/TTZipCore/CLI/CLIOptions.swift` with filter options, password sources, collision policy, and tree depth.
- [x] T002 [P] Extend `CLICommandSpec` in `Sources/TTZipCore/CLI/CLICommandSpec.swift` with `cat`, `tree`, `hash`, `delete`, and `update` command metadata and options.
- [x] T003 [P] Extend `POSIXCLIArgumentParser` in `Sources/TTZipCore/CLI/POSIXCLIArgumentParser.swift` with `--exclude`, `--include`, `--strip-components`, `--exclude-vcs`, `--no-mac-metadata`, `--files-from`, `--password-file`, and `--overwrite` flags.

---

## Phase 2: Foundational (C Bridge Extensions & Core Filter Engine)

**Purpose**: Foundational C and Swift primitives required by all user stories.

- [x] T004 [P] Implement `PathPatternFilterEngine` in `Sources/TTZipCore/Security/PathPatternFilterEngine.swift` providing Darwin `fnmatch(3)` pattern evaluation and path component stripping.
- [x] T005 [P] Implement binary stream detection `ttzip_is_buffer_binary` and string utilities in `Sources/CTTZipBridge/CTTZipUtils.c` and `Sources/CTTZipBridge/include/CTTZipUtils.h`.
- [x] T006 [P] Implement non-echo passphrase reader `ttzip_read_passphrase` via Darwin `readpassphrase(3)` in `Sources/CTTZipBridge/CTTZipUtils.c` and `Sources/CTTZipBridge/include/CTTZipUtils.h`.

---

## Phase 3: User Story 1 (P1) - Advanced Pattern Filtering & Path Transformations 🎯 MVP

**Goal**: Support `--exclude`, `--include`, `--strip-components`, `--exclude-vcs`, `--no-mac-metadata`, and `--files-from`.

**Independent Test**: `ttzip-cli archive bundle.tar.zst src/ --exclude "*.git/*" --exclude ".DS_Store" --exclude-vcs` creates an archive with zero excluded items.

- [x] T007 [P] [US1] Integrate `PathPatternFilterEngine` into `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift` for fast directory exclusion during archive creation.
- [x] T008 [P] [US1] Integrate component stripping and glob filtering into `Sources/TTZipCore/ArchiveExtractor.swift` during extraction.
- [x] T009 [US1] Implement `FileFilterListLoader` in `Sources/TTZipCore/CLI/FileFilterListLoader.swift` for asynchronous `--files-from` manifest ingestion.
- [x] T010 [US1] Wire filter options in `Sources/TTZipCLI/CLICommandRouter.swift` for `archive` and `extract` subcommands.

---

## Phase 4: User Story 2 (P1) - UNIX Stream Piping & Single-Entry Extraction (`cat`)

**Goal**: Stream decompressed entry bytes directly to `STDOUT_FILENO` without temporary disk files, with TTY binary protection.

**Independent Test**: `ttzip-cli cat bundle.zip config.json | jq .version` outputs JSON directly to stdout without disk I/O.

- [x] T011 [P] [US2] Implement `ttzip_stream_archive_entries_to_fd` in `Sources/CTTZipBridge/CTTZipBridge_Archive.c` and declare in `Sources/CTTZipBridge/include/CTTZipBridge_Archive.h`.
- [x] T012 [P] [US2] Implement `handleCatArchive` and stdout redirection (`-o -`) in `Sources/TTZipCLI/CLICommandRouter.swift` with TTY binary guard checking.

---

## Phase 5: User Story 3 (P1) - Process-Safe Credential & Password Management

**Goal**: Prevent password leaks in `ps aux` by adding non-echo TTY prompt, `--password-file`, and `TTZIP_PASSWORD` with volatile zeroing.

**Independent Test**: Extract an AES-256 encrypted archive without `-p`, verifying the secure terminal prompt appears and `ps aux` shows no secret.

- [x] T013 [P] [US3] Implement `SecureCredentialResolver` in `Sources/TTZipCore/Security/SecureCredentialResolver.swift` supporting TTY prompt, file, and env variable retrieval with volatile zeroing.
- [x] T014 [US3] Integrate `SecureCredentialResolver` into `Sources/TTZipCLI/CLICommandRouter.swift` for archive inspection, extraction, and verification.

---

## Phase 6: User Story 4 (P2) - Visual Tree Hierarchy & Auto-Paging Navigation

**Goal**: Render Unicode hierarchical tree view (`tree`) and auto-pipe large listings through `$PAGER` / `less -RFX`.

**Independent Test**: `ttzip-cli tree bundle.zip --depth 2` renders formatted tree glyphs; large lists automatically open in pager.

- [x] T015 [P] [US4] Implement `TerminalPagerEngine` in `Sources/TTZipCore/CLI/TerminalPagerEngine.swift` supporting `isatty` detection, window row checks, and `less -RFX` spawning.
- [x] T016 [P] [US4] Implement `ArchiveVisualTreeRenderer` in `Sources/TTZipCore/CLI/ArchiveVisualTreeRenderer.swift` with Unicode glyphs (`├──`, `└──`) and depth limiting.
- [x] T017 [US4] Wire `tree` command handling and auto-pager in `Sources/TTZipCLI/CLICommandRouter.swift`.

---

## Phase 7: User Story 5 (P2) - In-Archive Manipulation & Checksums (`delete`, `update`, `hash`)

**Goal**: In-place archive entry deletion, modification syncing (`update`), and standalone entry checksum calculation (`hash`).

**Independent Test**: `ttzip-cli hash bundle.zip` outputs CRC32 and SHA256 digests for all archive entries without extraction.

- [x] T018 [P] [US5] Implement `handleHashArchive` in `Sources/TTZipCLI/CLICommandRouter.swift` calculating CRC32/SHA256 digests.
- [x] T019 [US5] Implement `handleDeleteArchive` and `handleUpdateArchive` in `Sources/TTZipCLI/CLICommandRouter.swift`.

---

## Phase 8: User Story 6 (P2) - Granular Overwrite Policies & File Collision Resolution

**Goal**: Handle extraction collisions gracefully via `--overwrite [always|never|newer|backup|prompt]` and `/dev/tty` interactive dialogs.

**Independent Test**: Extracting conflicting files under `--overwrite backup` generates `.bak` copies without overwriting original files.

- [x] T020 [P] [US6] Implement `FileCollisionResolver` in `Sources/TTZipCore/CLI/FileCollisionResolver.swift` supporting `/dev/tty` interactive prompts and `.bak` backups.
- [x] T021 [US6] Integrate `FileCollisionResolver` into `Sources/TTZipCore/ArchiveExtractor.swift` and `CLICommandRouter.swift`.

---

## Phase 9: User Story 7 (P3) - Shell Auto-Completions & UNIX Man Page

**Goal**: Generate production auto-completions for Zsh, Bash, Fish, and NuShell, and render UNIX groff mdoc man page.

**Independent Test**: `ttzip-cli completion fish` and `ttzip-cli completion nushell` output valid shell syntax.

- [x] T022 [P] [US7] Implement `generateFishCompletion` and `generateNushellCompletion` in `Sources/TTZipCore/CLI/CLICommandSpec.swift`.
- [x] T023 [US7] Update `Sources/TTZipCLI/CLICommandRouter.swift` to route `completion fish` and `completion nushell`.

---

## Phase 10: Polish & E2E Validation

**Purpose**: End-to-end integration tests, regression checks, and performance gate verification.

- [x] T024 [P] Create comprehensive E2E tests in `Tests/TTZipTests/CLISubcommandsEndToEndTests.swift` covering all 7 user stories.
- [x] T025 Run full unit and CLI test suite (`swift test --filter CLICommandE2ETests`) and verify zero regression.
- [x] T026 Execute `swift test --filter XCTestPerformanceMeasureTests` to assert cold startup and throughput invariants.

---

## Dependencies & Execution Order

- **Setup (Phase 1)**: Independent (T001, T002, T003 can run in parallel `[P]`).
- **Foundational (Phase 2)**: Depends on Phase 1 (T004, T005, T006 can run in parallel `[P]`).
- **User Stories (Phase 3..9)**: Depend on Phase 2.
  - Phase 3 (US1), Phase 4 (US2), Phase 5 (US3) are P1 MVP blockers.
  - Phase 6 (US4), Phase 7 (US5), Phase 8 (US6) are P2 enhancements.
  - Phase 9 (US7) is P3 polish.
- **Polish (Phase 10)**: Depends on all user stories being implemented.
