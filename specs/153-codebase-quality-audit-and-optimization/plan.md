# Implementation Plan: Codebase Quality Audit and Optimization

**Branch**: `153-codebase-quality-audit-and-optimization` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/153-codebase-quality-audit-and-optimization/spec.md`

---

## Summary

This feature executes a comprehensive codebase quality audit and optimization across the TTZip workspace. It establishes a 100% clean compilation baseline under Swift 6.0 and C11, resolves localization catalog and error description inconsistencies, enforces strict logging hygiene (eliminating residual bare `print` statements in favor of `TTLogger`), hardens C bridge dynamic memory allocations with overflow-safe arithmetic, and validates the entire 525+ test suite against constitutional performance floors.

---

## Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), C11 (`-O3 -Wall -Wextra -Wvla -Wformat=2`).
- **Primary Dependencies**: `Sparkle` (v2.6.0 for direct distribution), static vendor libraries (`libTTZipVendor.a`, `TTZipVendor.xcframework`).
- **Storage**: In-memory streams, file-backed stream buffers, POSIX file descriptors.
- **Testing**: SPM `swift test` (XCTest, 525+ test cases covering unit, integration, crypto, fuzzing, and benchmark suites).
- **Target Platform**: macOS 14.0+ (ARM64 Apple Silicon NEON/PMULL and x86_64).
- **Project Type**: Native macOS Application + High-Performance Compression Engine Library (`TTZipCore`) + Command-Line Tools (`ttzip-cli`, `ttzip-bench`).
- **Performance Goals**: ZIP L1 compression >= 1500 MB/s, ZIP L6 >= 800 MB/s, ZIP decompression >= 4500 MB/s, ZIP AES-256 decompression >= 1800 MB/s, Small file scan >= 2000 MB/s.
- **Constraints**: Zero bare `print(...)`/`printf(...)` in production code; Zero unhandled memory allocations; Zero heap allocations on hot compression loops.
- **Scale/Scope**: ~120+ Swift/C source files across 5 SPM targets (`TTZipCore`, `TTZipCLI`, `TTZipApp`, `CTTZipBridge`, `TTZipBench`).

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant / Gate | Requirement | Status | Verification Method |
| :--- | :--- | :--- | :--- |
| **1. Stream-First** | Zero unconstrained heap allocations; micro-buffering pull pipeline ($\le 128\text{MB}$ resident memory). | PASS | Audit `ZipExtremeBlockWriter.swift` and C bridge allocators. |
| **2. Invariant-First** | Secure POSIX extraction flags, overflow-safe arithmetic (`__builtin_mul_overflow`), TOCTOU mitigation. | PASS | Harden array allocation calculations in `CTTZipExtract.c`, `CTTZipBridge_7zSolid.c`, `CTTZipBridge_Crypto.c`. |
| **3. Bounds-First** | Magic header lifecycles (`TTZIP_STRUCT_MAGIC` / `TTZIP_POISON_FREE`), secure key wiping (`ttzip_secure_zero`). | PASS | Verified active in `CTTZipStreamCoder.c`, `CTTZipBridge_Crypto.c`. |
| **4. Oracle-First** | UU-decoded golden corpus, bidirectional differential tests with `/usr/bin/tar` & `/usr/bin/unzip`. | PASS | Full regression suite executed via `swift test`. |
| **5. Logging Discipline** | Zero bare `print`, `printf`, `puts`, `fprintf`, or `NSLog` in production code; all logging via `TTLogger`. | PASS | Migrate bare `print` in `ZipExtremeBlockWriter.swift:139` to `TTLogger`. |
| **6. Frozen Files Rule** | Subsystem freeze on core ZIP parallel files preserved. | PASS | Zero modifications required to frozen files; changes confined to non-frozen wrappers and bridge allocators. |

---

## Phase 0: Research Items

- R001 [SUBAGENT:research] 《Localization Catalog Key Structure & Error Mapping Alignment》: Investigate `Sources/TTZipCore/Localization/` files, align `ArchiveError+L10n.swift` key mappings, forward `ArchiveError.errorDescription` to `localizedDescription()`, and verify 100% key parity across all 7 catalogs.
- R002 [SUBAGENT:research] 《Logging Hygiene & Four Systemic Engineering Invariants Audit》: Scan `Sources/` for bare logging calls, audit `Sources/CTTZipBridge/` for sensitive memory wiping (`explicit_bzero`), magic lifecycle sentinels, and integer overflow checks on dynamic allocations.

---

## Phase 1: Artifacts & Contracts

- **Data Model**: `specs/153-codebase-quality-audit-and-optimization/data-model.md`
- **Contracts**:
  - `specs/153-codebase-quality-audit-and-optimization/contracts/localization-catalog-contract.json` [SUBAGENT:research]
  - `specs/153-codebase-quality-audit-and-optimization/contracts/logging-event-contract.json` [SUBAGENT:research]
  - `specs/153-codebase-quality-audit-and-optimization/contracts/archive-error-contract.json` [SUBAGENT:research]
- **Quickstart Guide**: `specs/153-codebase-quality-audit-and-optimization/quickstart.md`

---

## Project Structure & Planned Modifications

### Documentation
```text
specs/153-codebase-quality-audit-and-optimization/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── localization-catalog-contract.json
│   ├── logging-event-contract.json
│   └── archive-error-contract.json
├── checklists/
│   └── requirements.md
└── tasks.md
```

### Source Code Modifications by Component

#### 1. Localization & Error Subsystem (`Sources/TTZipCore/Localization/` & `Sources/TTZipCore/`)
- `Sources/TTZipCore/Localization/Extensions/ArchiveError+L10n.swift`:
  - Align error key mapping for `.readFailed(code:)` to `L10n.Errors.readError`.
- `Sources/TTZipCore/ArchiveReader.swift`:
  - Delegate `ArchiveError.errorDescription` to `localizedDescription()` for unified localized descriptions.
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+*.swift`:
  - Ensure all 7 catalogs (`En`, `ZhHans`, `ZhHant`, `Ja`, `De`, `Es`, `Fr`) have complete key coverage for all `L10n.Errors`.

#### 2. Core Logging Hygiene (`Sources/TTZipCore/Zip/`)
- `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`:
  - Replace bare `print(...)` on fallback compression failure with `TTLogger.shared.warning(...)`.

#### 3. C Bridge Defensive Hardening (`Sources/CTTZipBridge/`)
- `Sources/CTTZipBridge/CTTZipExtract.c`:
  - Add `ttzip_mul_overflow` check on `total_entries * sizeof(ttzip_parsed_entry_t)` before `malloc`.
- `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`:
  - Add `ttzip_mul_overflow` check on `num_files * sizeof(uint64_t)` before `malloc`.
- `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`:
  - Add `ttzip_mul_overflow` check on crypto buffer dynamic allocations.

#### 4. Automated Test Suite Validation (`Tests/TTZipTests/`)
- Validate all test targets and verify zero regressions across unit and performance tests.

---

## Complexity Tracking

*No constitutional violations identified. No complexity tracking justifications required.*
