# Implementation Plan: Full 22-File Swift Test Migration to C11

**Branch**: `158-157-test-migration` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/158-157-test-migration/spec.md`

---

## Summary

Migrate all remaining 22 microkernel, SIMD, entropy, and bitstream Swift test files from `Tests/TTZipTests/` into 5 dedicated C11 native test suites in `tests/c/`. Register all 19 suites in `CMakeLists.txt` and `test_main.c`, physically delete the 22 redundant Swift test files, and verify zero warnings and 100% green local CI.

---

## Technical Context

- **Language/Version**: ISO/IEC 9899:2011 (ANSI C11) & Swift 6.0
- **Primary Dependencies**: None (Zero third-party test dependencies)
- **Storage**: In-memory synthetic buffers and deterministic bitstreams
- **Testing**: Native C11 Test Harness (`tests/c/ttzip_test_harness.h`) + CMake CTest + Swift XCTest
- **Target Platform**: macOS (Apple Silicon arm64 & Intel x86_64) and standard POSIX Linux/FreeBSD
- **Project Type**: Native High-Performance Archiving Engine & Desktop App
- **Performance Goals**: All 19 C test suites execute in **< 15 milliseconds** total
- **Constraints**: 0 compiler warnings, 0 ASan leaks, 0 cloud quota usage

---

## Constitution Check

*GATE: Passed. Complies with zero cloud actions quota, zero external dependency, and sub-second local test invariants.*

- [x] Principle 1: Zero Cloud CI Quota — 100% local verification.
- [x] Principle 2: Zero Dependency Bloat — Pure ANSI C11 standard library only.
- [x] Principle 3: Performance First — Sub-15ms total test cycle.
- [x] Principle 4: High Reliability & Memory Safety — Validated with AddressSanitizer and UndefinedBehaviorSanitizer.

---

## Phase 0: Outline & Research

The following research items were formally dispatched and investigated:
- R001 [SUBAGENT:research] 《Full 22-File Swift Microkernel Decoupling & Cluster Mapping》：Systematic mapping of all 22 candidate Swift test files into 5 high-speed C11 test suites.

*Artifact*: [research.md](./research.md)

---

## Phase 1: Design & Contracts

The following design artifacts establish the data structures, schema contracts, and verification procedures:
- Data Model: [data-model.md](./data-model.md)
- Contracts:
  - `contracts/c-migration-22-schema.json` [SUBAGENT:research] — Formal JSON Schema for full 22-file migration telemetry.
- Quickstart & Verification: [quickstart.md](./quickstart.md)

---

## Project Structure & Planned Changes

```text
TTZip/
├── CMakeLists.txt                              # [MODIFY] Register 5 new C test suites in CTest (total 19)
├── tests/
│   └── c/
│       ├── test_main.c                         # [MODIFY] Register 5 new suite runners
│       ├── test_adler_crc64.c                  # [NEW] Cluster 1: Adler-32 NEON & CRC64-XZ tests
│       ├── test_entropy_evaluator.c            # [NEW] Cluster 2: Shannon Entropy & Routing tests
│       ├── test_matchfinder_advanced.c         # [NEW] Cluster 3: Fast Match Finder & Dictionary tests
│       ├── test_blosc_slicing.c                # [NEW] Cluster 4: Blosc2 Slicing & SuperChunk tests
│       └── test_crypto_lz4_snappy.c            # [NEW] Cluster 5: 7z KDF Crypto & LZ4/Snappy tests
└── specs/158-157-test-migration/
    ├── spec.md
    ├── checklists/requirements.md
    ├── plan.md
    ├── research.md
    ├── data-model.md
    ├── contracts/c-migration-22-schema.json
    └── quickstart.md
```

---

## Complexity Tracking

| Aspect | Justification | Alternatives Considered |
| :--- | :--- | :--- |
| **5 New C Suites replacing 22 Swift Files** | Slashes Swift test compilation time, eliminates FFI bridging overhead, and enables AddressSanitizer bit-level verification. | Keeping dual test files was rejected due to doubled CI runtime and maintenance debt. |
