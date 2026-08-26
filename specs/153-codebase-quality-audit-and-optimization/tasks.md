# Tasks: Codebase Quality Audit and Optimization

**Feature Branch**: `153-codebase-quality-audit-and-optimization` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Dependencies & Execution Graph

```
[Phase 1: Setup] ──► [Phase 2: Foundational]
                             │
                             ▼
              [Phase 3: User Story 1 (P1: Compilation Baseline)]
                             │
                             ▼
              [Phase 4: User Story 2 (P2: Invariants & Logging)]
                             │
                             ▼
              [Phase 5: User Story 3 (P3: Regression & Performance)]
                             │
                             ▼
              [Phase 6: Polish & Quality Convergence]
```

---

## Phase 1: Setup & Environment Baseline

- [x] T001 Inspect workspace environment and verify compiler flags in `Package.swift`

---

## Phase 2: Foundational Prerequisites

- [x] T002 Verify `LocaleKey.swift` enum definitions and group registrations in `Sources/TTZipCore/Localization/LocaleKey.swift`

---

## Phase 3: User Story 1 - Clean Zero-Error Compilation Baseline (Priority: P1)

**Goal**: Restore 100% clean compilation baseline across all SPM targets (`TTZipCore`, `TTZipCLI`, `TTZipApp`, `CTTZipBridge`, `TTZipBench`, and test suites).

**Independent Test**: Execute `swift build --build-tests` and verify clean build with exit code 0.

- [x] T003 [P] [US1] Align `ArchiveError` mapping (`readFailed` to `L10n.Errors.readError`) in `Sources/TTZipCore/Localization/Extensions/ArchiveError+L10n.swift`
- [x] T004 [P] [US1] Delegate `ArchiveError.errorDescription` to `localizedDescription()` in `Sources/TTZipCore/ArchiveReader.swift`
- [x] T005 [P] [US1] Verify error key parity across all 7 language catalogs in `Sources/TTZipCore/Localization/Catalogs/`
- [x] T006 [US1] Execute full target build verification via `swift build --build-tests`

---

## Phase 4: User Story 2 - Comprehensive Invariant and Logging Hygiene Audit (Priority: P2)

**Goal**: Eliminate bare logging calls in core engines and harden C bridge dynamic allocations with arithmetic overflow protection.

**Independent Test**: Grep codebase for bare `print` in core modules and verify overflow guards in C bridge dynamic allocations.

- [x] T007 [P] [US2] Replace bare `print` with `TTLogger.shared.warning` in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`
- [x] T008 [P] [US2] Harden dynamic entry array allocation with `ttzip_mul_overflow` in `Sources/CTTZipBridge/CTTZipExtract.c`
- [x] T009 [P] [US2] Harden solid file array allocation with `ttzip_mul_overflow` in `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`
- [x] T010 [P] [US2] Harden crypto buffer allocations with `ttzip_mul_overflow` in `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`

---

## Phase 5: User Story 3 - Full Regression and Performance Floor Verification (Priority: P3)

**Goal**: Validate full automated test suite (525+ tests) and confirm constitutional throughput floors.

**Independent Test**: Execute `swift test` and confirm 100% pass rate.

- [x] T011 [US3] Run localization integrity tests via `swift test --filter LocalizationIntegrityTests`
- [x] T012 [US3] Run full core regression test suite via `swift test --filter TTZipTests`
- [x] T013 [US3] Run performance throughput gate tests via `swift test --filter XCTestPerformanceMeasureTests`

---

## Phase 6: Polish & Quality Convergence

**Goal**: Final static analysis pass, code review, and Spec Kit convergence analysis.

- [x] T014 Run static analysis and verify zero compiler warnings across all targets in `Sources/`
- [x] T015 Perform cross-artifact consistency verification and converge status
