# Implementation Plan: Streamlining Redundant Swift C-Wrapper Tests

**Branch**: `155-155-streamline-redundant` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/155-155-streamline-redundant/spec.md`

---

## Summary

Audit and prune pure C-wrapper redundant Swift test files from `Tests/TTZipTests/` whose invariant coverage is 100% verified by the operational C11 test suite (`tests/c/test_*.c`), while preserving all Swift architectural pattern tests, ConcurrencyBridge tests, and AppKit UI tests. This accelerates `swift test` and solidifies the dual-engine testing architecture.

---

## Technical Context

- **Language/Version**: Swift 6.0 & ANSI C11
- **Primary Dependencies**: None (Zero third-party test dependencies)
- **Storage**: In-memory test fixtures and synthetic buffers
- **Testing**: CTest (`tests/c/`) + Swift XCTest (`Tests/TTZipTests/`, `Tests/TTZipAppTests/`)
- **Target Platform**: macOS (Apple Silicon arm64 & Intel x86_64)
- **Project Type**: Native High-Performance Archiving Engine & Desktop App
- **Performance Goals**: Reduce Swift test compilation/execution overhead while maintaining 100% invariant coverage
- **Constraints**: 0 compiler warnings across all targets, 0 CI cloud quota usage, 100% local verification

---

## Constitution Check

*GATE: Passed. Complies with zero cloud actions quota, zero external dependency, and sub-second local test invariants.*

- [x] Principle 1: Zero Cloud CI Quota — All tests execute purely locally.
- [x] Principle 2: Zero Dependency Bloat — Pure Swift & C native code.
- [x] Principle 3: Performance First — Eliminates redundant Swift FFI overhead in test runs.
- [x] Principle 4: High Reliability & Invariant Preservation — 100% microkernel coverage maintained in `tests/c/`.

---

## Phase 0: Outline & Research

The following research items were formally investigated:
- R001 [SUBAGENT:research] 《Swift-to-C Test Redundancy & Decoupling Audit》：Systematic line-by-line classification of all 127 Swift test files into Candidates for Pruning vs MUST Retain.

*Artifact*: [research.md](./research.md)

---

## Phase 1: Design & Contracts

The following design artifacts establish the data structures, schema contracts, and verification procedures:
- Data Model: [data-model.md](./data-model.md)
- Contracts:
  - `contracts/test-suite-inventory.json` [SUBAGENT:research] — Formal JSON Schema for test suite inventory and audit records.
- Quickstart & Verification: [quickstart.md](./quickstart.md)

---

## Project Structure & Planned Changes

```text
TTZip/
├── Tests/
│   └── TTZipTests/
│       ├── ZipSlipDefenseTests.swift                     # [DELETE] (Covered by tests/c/test_security_zipslip.c)
│       ├── SingleCoreDeflateOracleTests.swift            # [DELETE] (Covered by tests/c/test_deflate_zopfli.c)
│       ├── SevenZipHeaderParserTests.swift               # [DELETE] (Covered by tests/c/test_7z_lzma2.c)
│       ├── BranchlessDecompTests.swift                   # [DELETE] (Covered by tests/c/test_deflate_zopfli.c)
│       ├── StreamingDecompressorDualSymbolLutTests.swift  # [DELETE] (Covered by tests/c/test_deflate_zopfli.c)
│       ├── SwarOptimizationBenchmarkTests.swift         # [DELETE] (Covered by tests/c/test_magic_sniff.c & strnatcmp.c)
│       └── CRC32PmullDifferentialTests.swift             # [DELETE] (Covered by tests/c/test_crc_neon.c)
└── specs/155-155-streamline-redundant/
    ├── spec.md
    ├── checklists/requirements.md
    ├── plan.md
    ├── research.md
    ├── data-model.md
    ├── contracts/test-suite-inventory.json
    └── quickstart.md
```

---

## Complexity Tracking

| Aspect | Justification | Alternatives Considered |
| :--- | :--- | :--- |
| **Pruning 7 Pure C-Wrapper Files** | Eliminates duplicate test maintenance and speeds up SwiftPM test compile cycles without losing a single invariant assertion. | Retaining duplicates was rejected due to doubled CI runtime and maintenance debt. |
