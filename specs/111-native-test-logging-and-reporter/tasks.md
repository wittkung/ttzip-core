# Tasks: Native High-Aesthetic Test Logging, Harness & Reporter (111-native-test-logging-and-reporter)

## Phase 1: Setup & Foundational Infrastructure

- [x] T001 [P] Create `Sources/TTZipCore/Testing/TestLogger.swift` with `TestLogLevel`, TaskLocal session buffer, and thread-safe atomic output in `Sources/TTZipCore/Testing/TestLogger.swift`
- [x] T002 [P] Upgrade ANSI color definitions, badge formatting, and box-drawing utilities in `Sources/TTZipCore/Testing/TestTerminalRenderer.swift`

## Phase 2: User Story 1 - Clean & Aesthetic Terminal Test Execution Stream (Priority: P1)

- [x] T003 [US1] Implement aligned single-line stream table renderer (`[ %3d/%3d ] [ BADGE ] [%-12s] %-42s (%s)`) in `Sources/TTZipCore/Testing/TestTerminalRenderer.swift`
- [x] T004 [US1] Implement ANSI/Unicode dual-mode executive summary dashboard card in `Sources/TTZipCore/Testing/TestTerminalRenderer.swift`
- [x] T005 [US1] Wire the new terminal stream renderer into `Sources/TTZipCLI/TestCommand.swift`
- [x] T006 [US1] Upgrade `./scripts/run_local_ci_gate.sh` and `./scripts/run_all_tests.sh` with zlib-ng/libarchive stage table formatting in `scripts/run_local_ci_gate.sh`

## Phase 3: User Story 2 - High-Fidelity Failure Diagnostic & Diff Presentation (Priority: P2)

- [x] T007 [P] [US2] Implement silent-on-success and deferred atomic failure card formatting in `Sources/TTZipCore/Testing/TestLogger.swift`
- [x] T008 [P] [US2] Integrate `UnicodeDiagnosticFormatter` and `FastHexDiffEngine` into failure card rendering in `Sources/TTZipCore/Testing/TestTerminalRenderer.swift`

## Phase 4: User Story 3 - Unified Native Test Logger & Telemetry Pipeline (Priority: P3)

- [x] T009 [P] [US3] Implement NDJSON telemetry serialization and contract compliance in `Sources/TTZipCore/Testing/TestTelemetryStream.swift`
- [x] T010 [P] [US3] Add comprehensive unit tests for `TestLogger`, TaskLocal isolation, and terminal rendering in `Tests/TTZipTests/TestTelemetryAndRendererTests.swift`

## Phase 5: Polish & Convergence

- [x] T011 Run `./scripts/lint_codebase_standards.sh` to assert 0 warnings and 100% SPDX header coverage
- [x] T012 Run full test suite regression `swift test` and `swift run ttzip-cli test` to verify end-to-end aesthetic output
