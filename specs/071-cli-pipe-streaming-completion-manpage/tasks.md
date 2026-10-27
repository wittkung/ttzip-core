# Tasks: CLI UNIX Pipe Streaming, Shell Auto-Completion, BSD Man Page, and Local CI/CD Gate

**Feature Branch**: `071-cli-pipe-streaming-completion-manpage`  
**Created**: 2026-08-17  
**Status**: Ready for Implementation  
**Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/071-cli-pipe-streaming-completion-manpage/spec.md) | **Plan**: [`plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/071-cli-pipe-streaming-completion-manpage/plan.md)

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: Core stream descriptors, signal handlers, and exit code foundations.

- [x] T001 Implement `StreamExecutionMode`, `StreamPipelineConfig`, and `StreamProgressRouting` in `Sources/TTZipCore/CLI/StreamPipeAdapter.swift`.
- [x] T002 Add `CLIExitCode.sigpipe = 141` in `Sources/TTZipCore/CLI/CLIExitCode.swift`.
- [x] T003 [P] Add signal handler installing `signal(SIGPIPE, SIG_IGN)` in `Sources/CTTZipBridge/CTTZipDiagnostics.c` and `Sources/CTTZipBridge/include/CTTZipDiagnostics.h`.

---

## Phase 2: User Story 1 - UNIX Pipe & Standard I/O Streaming (Priority: P1) 🎯 MVP

**Goal**: Support zero-disk-footprint stdout stream creation (`-o -`), stdin stream extraction (`-i -`), single-entry stdout cat (`-O -`), and safe SIGPIPE exits.

**Independent Test**: `swift test --filter PipeStreamingTests` and `ttzip-cli create -f tar.zst -o - <dir> | ttzip-cli extract -i - -d <target>` asserting bit-exact SHA-256 hash identity.

- [x] T004 [P] [US1] Support `-o -` stdout streaming in `Sources/CTTZipBridge/ttzip_tar_native.c` and `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`.
- [x] T005 [P] [US1] Implement stderr progress redirection and binary stdout stream protection in `Sources/TTZipCore/CLI/TerminalRenderEngine.swift`.
- [x] T006 [US1] Enhance `StreamPipeAdapter.swift` with `isatty` detection, binary safety probing (`ttzip_is_buffer_binary`), and stdin spooling in `Sources/TTZipCore/CLI/StreamPipeAdapter.swift`.
- [x] T007 [US1] Wire `-o -`, `-i -`, `-O -` / `--to-stdout` in `Sources/TTZipCLI/CLICommandRouter.swift` and `Sources/TTZipCore/CLI/POSIXCLIArgumentParser.swift`.
- [x] T008 [P] [US1] Create unit and E2E test suite `Tests/TTZipTests/PipeStreamingTests.swift` validating roundtrip stdout -> stdin, `cat`, and `SIGPIPE` exit code 141.

---

## Phase 3: User Story 2 - Dynamic Shell Auto-Completion Generation System (Priority: P2)

**Goal**: Dynamically generate compliant Zsh, Bash, Fish, and Nushell tab completion scripts derived directly from `CLICommandSpec.allSpecs`.

**Independent Test**: `swift test --filter ShellCompletionTests` and verifying `ttzip-cli completion zsh` syntax.

- [x] T009 [P] [US2] Enhance `CLIOptionSpec` with `valueChoices` and `isFilePath` in `Sources/TTZipCore/CLI/CLICommandSpec.swift`.
- [x] T010 [US2] Implement `ShellCompletionGenerator.swift` in `Sources/TTZipCore/CLI/ShellCompletionGenerator.swift` rendering Zsh, Bash, Fish, and Nushell scripts.
- [x] T011 [US2] Wire `completion` subcommand in `Sources/TTZipCLI/CLICommandRouter.swift`.
- [x] T012 [P] [US2] Create unit test suite `Tests/TTZipTests/ShellCompletionTests.swift` asserting valid script generation across all 4 shells.

---

## Phase 4: User Story 3 - BSD Man Page (mdoc) Manual Generation (Priority: P3)

**Goal**: Generate standard BSD mdoc troff manual pages for `ttzip-cli(1)` and `ttzip(1)` with 100% `mandoc -Tlint` cleanliness.

**Independent Test**: `swift test --filter ManPageGeneratorTests` and `ttzip-cli man | mandoc -Tlint`.

- [x] T013 [US3] Implement `ManPageGenerator.swift` in `Sources/TTZipCore/CLI/ManPageGenerator.swift` with 12 BSD mdoc sections.
- [x] T014 [US3] Wire `man` subcommand in `Sources/TTZipCLI/CLICommandRouter.swift` and delegate `CLICommandSpec.generateManPage()` to `ManPageGenerator`.
- [x] T015 [P] [US3] Create unit test suite `Tests/TTZipTests/ManPageGeneratorTests.swift` validating mdoc section structure and `mandoc -Tlint` formatting.

---

## Phase 5: User Story 4 - Local CI/CD Automated Test Gate & Regression Pipeline (Priority: P4)

**Goal**: Deliver a 1-command local industrial test gate runner (`scripts/run_local_ci_gate.sh`) verifying build, standards, differential oracles, fuzzing, performance floors, and pipe streaming.

**Independent Test**: Executing `./scripts/run_local_ci_gate.sh` and asserting 100% passing scorecard and exit code 0.

- [x] T016 [US4] Implement executable shell script `scripts/run_local_ci_gate.sh` driving the 6-stage regression gate with ANSI summary table.
- [x] T017 [US4] Integrate local CI gate check into `scripts/run_all_tests.sh`.
- [x] T018 [P] [US4] Create unit test `Tests/TTZipTests/LocalCIGateTests.swift` validating gate execution and report model.

---

## Phase 6: Polish & Verification

**Purpose**: Full regression pass, performance floor verification, and consistency convergence.

- [x] T019 Run full test suite (`swift test`) and performance gates (`swift test --filter XCTestPerformanceMeasureTests`).
- [x] T020 Run local CI/CD automated gate (`./scripts/run_local_ci_gate.sh`) to assert all 6 stages pass.
- [x] T021 Execute `speckit-converge` and `speckit-analyze` to assert 100% specification and implementation convergence.

---

## Dependencies & Execution Order

```
[Phase 1: Setup & Foundational (T001..T003)]
         │
         ├───▶ [Phase 2: US1 Pipe Streaming (T004..T008)] 🎯 MVP
         │
         ├───▶ [Phase 3: US2 Shell Completion (T009..T012)]
         │
         ├───▶ [Phase 4: US3 BSD Man Page (T013..T015)]
         │
         └───▶ [Phase 5: US4 Local CI Gate (T016..T018)]
                   │
                   ▼
         [Phase 6: Polish & Full Verification (T019..T021)]
```
