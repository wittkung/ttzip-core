# Tasks: CLI Test System, Full Coverage, and Compression/Decompression Standards Professionalization

**Feature Branch**: `070-cli-test-system-standards-professionalization`  
**Input**: `specs/070-cli-test-system-standards-professionalization/` (`spec.md`, `plan.md`, `data-model.md`, `contracts/`)

---

## Phase 1: Setup & Core Data Models

**Purpose**: Establish core data models and interfaces for format standards, oracle reports, and fuzzing configurations.

- [x] T001 [P] Implement `ArchiveFormatStandardSpec`, `StandardCitation`, and `ArchiveMagicSignature` in `Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift`.
- [x] T002 [P] Implement `FileTreeManifest` and `DifferentialTestReport` data models in `Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift`.
- [x] T003 [P] Implement `FuzzMutationConfig` and `DeterministicPRNG` in `Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift`.

---

## Phase 2: Foundational Standards & Diagnostic Infrastructure

**Purpose**: Core scanning primitives, ZIP Extra Field parsing, and hex diff formatting.

- [x] T004 [P] Implement multi-anchor signature scanner `ArchiveMagicSignatureScanner` supporting `.head`, `.tail`, `.sector`, and `.tarOffset` in `Sources/TTZipCore/Standards/ArchiveMagicSignatureScanner.swift`.
- [x] T005 [P] Implement zero-allocation `ZipExtraFieldParser` supporting tags `0x5455`, `0x7075`, `0x7875`, `0x0001`, and `0x9901` in `Sources/TTZipCore/Standards/ZipExtraFieldParser.swift`.
- [x] T006 [P] Enhance `FastHexDiffEngine` with 64-byte SIMD chunk hopping and 16-byte aligned ANSI visual diffs in `Sources/TTZipCore/Testing/FastHexDiffEngine.swift`.
- [x] T007 [P] Extend `CTTZipDiagnostics.c` with monotonic negative error severity ordering helper `ttzip_err_combine` in `Sources/CTTZipBridge/CTTZipDiagnostics.c`.

---

## Phase 3: User Story 1 - Standards Conformance & Format Registry (Priority: P1)

**Goal**: Complete registration and validation for all 16 supported formats against official RFC, ISO, and POSIX specifications.

**Independent Test**: `swift test --filter ArchiveStandardsComplianceTests` passes 100%.

- [x] T008 [P] [US1] Build `ArchiveFormatStandardRegistry.shared` with full citations and magic signatures for all 16 formats in `Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift`.
- [x] T009 [US1] Implement `StandardsComplianceChecker` validating ZIP, TAR (POSIX Pax), GZIP (RFC 1952), and Zstandard (RFC 8878) headers in `Sources/TTZipCore/Standards/StandardsComplianceChecker.swift`.
- [x] T010 [P] [US1] Create unit test suite `ArchiveStandardsComplianceTests` in `Tests/TTZipTests/ArchiveStandardsComplianceTests.swift`.
- [x] T011 [US1] Connect `--standard <format>` option in `Sources/TTZipCore/CLI/CLIOptions.swift` and `Sources/TTZipCLI/TestCommand.swift`.

---

## Phase 4: User Story 2 - Differential Oracle Comparison Testing (Priority: P1)

**Goal**: Implement bidirectional 3-way differential testing against native reference tools (`/usr/bin/tar`, `bsdtar`, `/usr/bin/unzip`, `7zz`).

**Independent Test**: `swift test --filter DifferentialOracleTests` passes with 0 failures.

- [x] T012 [P] [US2] Implement `DifferentialOracleRegistry` and `OracleBinaryResolver` for dynamic oracle discovery in `Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift`.
- [x] T013 [US2] Implement `DifferentialManifestVerifier` performing 5-dimension manifest comparisons (SHA-256, APFS paths, symlinks, modes, hex diffs) in `Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift`.
- [x] T014 [P] [US2] Create differential test suite `DifferentialOracleTests` in `Tests/TTZipTests/DifferentialOracleTests.swift`.

---

## Phase 5: User Story 3 - Crash-First Malformed Stream & Security Fuzzing (Priority: P2)

**Goal**: Harden C and Swift parsers against corrupted headers, truncated streams, bad CRCs, and Zip Slip path traversal.

**Independent Test**: `swift test --filter ArchiveMutationFuzzTests` executes 50+ iterations with 0 crashes.

- [x] T015 [P] [US3] Implement composable mutation operators (`.corruptMagic`, `.corruptCRC`, `.truncateStream`, `.injectZipSlipPath`, `.oversizeHeader`, `.invalidDictSize`) in `Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift`.
- [x] T016 [US3] Implement crash-first sandbox persistence and negative `ttzip_error_t` status code verification in `Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift`.
- [x] T017 [P] [US3] Create comprehensive fuzzing test suite in `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`.

---

## Phase 6: User Story 4 - Diagnostic Test Harness & Hex Diff UX (Priority: P2)

**Goal**: CLI test sub-command with colored status badges, `--json` NDJSON telemetry, and diagnostic reports.

**Independent Test**: `swift run ttzip-cli test --standard zip --json` outputs valid NDJSON stream.

- [x] T018 [P] [US4] Extend `CLIOptions`, `CLICommandSpec`, and `POSIXCLIArgumentParser` with `--standard`, `--differential`, `--fuzz`, `--tier` in `Sources/TTZipCore/CLI/`.
- [x] T019 [US4] Update `TestCommand.swift` to execute standards suites, differential oracle suites, and mutation fuzzing with NDJSON events in `Sources/TTZipCLI/TestCommand.swift`.
- [x] T020 [P] [US4] Create CLI test diagnostic verification tests in `Tests/TTZipTests/CLITestCommandDiagnosticTests.swift`.

---

## Phase 7: Libarchive Golden Corpus Integration

**Purpose**: Load and decompress libarchive's historical 90+ `.uu` golden archive fixtures to guarantee legacy compatibility.

- [x] T021 [P] Implement `LibarchiveUUDecoder` in `Sources/TTZipCore/Testing/LibarchiveUUDecoder.swift` to decode all format fixtures.
- [x] T022 [P] Create comprehensive regression test suite in `Tests/TTZipTests/LibarchiveGoldenCorpusTests.swift`.

---

## Phase 8: Polish & E2E Verification

**Purpose**: Final quality checks, full regression pass, and performance floor verification.

- [x] T023 Run full test suite (`swift test --filter CLI`) to assert zero regression across all CLI features.
- [x] T024 Run performance regression tests (`swift test --filter XCTestPerformanceMeasureTests`).
- [x] T025 Execute `speckit-converge` and `speckit-analyze` to assert 100% specification and implementation convergence.

---

## Dependencies & Execution Order

- **Phase 1 (Setup)**: Independent (T001, T002, T003 can run in parallel `[P]`).
- **Phase 2 (Foundational)**: Depends on Phase 1 (T004, T005, T006, T007 can run in parallel `[P]`).
- **Phase 3 (US1) & Phase 4 (US2)**: Depend on Phase 2 (P1 MVP core).
- **Phase 5 (US3) & Phase 6 (US4)**: Depend on Phase 2 (P2 robustness & UX).
- **Phase 7 (Libarchive Corpus)**: Independent test suite.
- **Phase 8 (Polish)**: Depends on all implementation tasks.
