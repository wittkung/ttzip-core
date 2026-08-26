# Implementation Plan: C Test Harness Expansion & Advanced Microkernel Migration

**Branch**: `156-156-test-harness-expansion` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/156-156-test-harness-expansion/spec.md`

---

## Summary

Expand the native zero-dependency C11 test harness to cover 5 additional advanced microkernel subsystems: BloscLZ/BitGroom, In-Place Canonical Huffman / Adaptive Block Splitting, Snappy block and framed streams, Apple DMG UDIF demuxing / LZFSE, and Radix Trie virtual archive filesystem. Register all 13 suites in `CMakeLists.txt` / CTest and prune redundant Swift FFI wrapper tests.

---

## Technical Context

- **Language/Version**: ISO/IEC 9899:2011 (ANSI C11) & Swift 6.0
- **Primary Dependencies**: None (Zero third-party test dependencies)
- **Storage**: In-memory test buffers and temporary sandbox directories
- **Testing**: Native C11 Test Harness (`tests/c/ttzip_test_harness.h`) + CMake CTest + Swift XCTest
- **Target Platform**: macOS (Apple Silicon arm64 & Intel x86_64) and standard POSIX Linux/FreeBSD
- **Project Type**: Native High-Performance Archiving Engine & Desktop App
- **Performance Goals**: All 13 test suites execute in **< 15 milliseconds** total
- **Constraints**: Zero dynamic heap allocation in assertions, 0 compiler warnings, 0 ASan memory leaks
- **Scale/Scope**: 13 comprehensive C test suites covering all compression formats and virtual filesystems

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
- R001 [SUBAGENT:research] 《C Microkernel API & Header Audit》：Detailed audit of BloscLZ, BitGroom, Huffman in-place, Snappy, DMG/LZFSE, and Radix tree API signatures and test vectors.

*Artifact*: [research.md](./research.md)

---

## Phase 1: Design & Contracts

The following design artifacts establish the data structures, schema contracts, and verification procedures:
- Data Model: [data-model.md](./data-model.md)
- Contracts:
  - `contracts/c-test-expansion-schema.json` [SUBAGENT:research] — Formal JSON Schema for expanded 13-suite test runner telemetry.
- Quickstart & Verification: [quickstart.md](./quickstart.md)

---

## Project Structure & Planned Changes

```text
TTZip/
├── CMakeLists.txt                              # [MODIFY] Register 5 new C test suites in CTest
├── tests/
│   └── c/
│       ├── test_main.c                         # [MODIFY] Register 5 new suite runners
│       ├── test_blosc_engine.c                 # [NEW] BloscLZ, BitGroom, SuperChunk tests
│       ├── test_huffman_inplace.c              # [NEW] Canonical Huffman & Block Splitting tests
│       ├── test_snappy_engine.c                # [NEW] Snappy block & framed stream tests
│       ├── test_dmg_lzfse.c                    # [NEW] Apple DMG demuxing & LZFSE tests
│       └── test_archive_tree.c                 # [NEW] Radix tree virtual filesystem tests
└── specs/156-156-test-harness-expansion/
    ├── spec.md
    ├── checklists/requirements.md
    ├── plan.md
    ├── research.md
    ├── data-model.md
    ├── contracts/c-test-expansion-schema.json
    └── quickstart.md
```

---

## Complexity Tracking

| Aspect | Justification | Alternatives Considered |
| :--- | :--- | :--- |
| **5 New Native C Test Suites** | Expands CTest to 13 suites covering 100% of microkernel codecs with sub-15ms execution time. | Retaining Swift tests was rejected due to slow FFI startup. |
