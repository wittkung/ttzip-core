# Implementation Plan: Comprehensive Physical Benchmarking Suite, Detailed Comparison Documentation, and Top-Tier Professional README Reconstruction

**Branch**: `073-performance-benchmarking-and` | **Date**: 2026-08-18 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/073-performance-benchmarking-and/spec.md)

**Input**: Feature specification from `/specs/073-performance-benchmarking-and/spec.md`

---

## Summary

Deliver a premier, world-class benchmarking, documentation, and legal clarity overhaul for TTZip:
1. **Empirical Benchmarking & Whitepaper**: Run monotonic hardware benchmark suites across all 16 supported formats against industry competitors (Apple `ditto`, 7-Zip `7zz`, GNU/BSD `tar`, `pigz`, `zstd`, `xz`, `lz4`, `brotli`) and author `docs/PERFORMANCE.md`.
2. **Top-Tier Open-Source README Reconstruction**: Author an international-standard `README.md` showcasing the 16-format matrix, 9-command CLI suite, UNIX pipe streaming, macOS GUI capabilities, performance cards, and upstream open-source contributions.
3. **Legal & Licensing Alignment**: Standardize all license declarations (`README.md`, `LICENSE`, `ACKNOWLEDGEMENTS.md`, `Formula/ttzip-cli.rb`) to the Source-Available / Anti-Copycat License model with Enterprise Commercial Licensing channels.

---

## Technical Context

**Language/Version**: Swift 6.0 (Strict Concurrency) + C11 (Clang / GCC, POSIX, ARM NEON SIMD)
**Primary Dependencies**: Static C libraries (`libarchive.a`, `libdeflate.a`, `liblzma.a`, `libzstd.a`, `liblz4.a`, `libb2.a`, `uchardet.a`), AppKit/SwiftUI
**Storage**: APFS direct I/O, POSIX memory-mapped buffers (`mmap`), in-memory streams
**Testing**: SwiftPM XCTest (`swift test`), `XCTestPerformanceMeasureTests`, `ZipBenchPkTests`, `AllFormatsPkSuiteTests`, CLI test suites
**Target Platform**: macOS 14.0+ Sonoma & macOS 15.0+ Sequoia (Apple Silicon M1/M2/M3/M4 prioritized, Intel x86_64 compatible)
**Project Type**: CLI tool (`ttzip-cli`) + Native macOS GUI App (`TTZipApp`) + Shared Core Engine (`TTZipCore` / `CTTZipBridge`)
**Performance Goals**: Monotonic throughput >= 1,500 MB/s (ZIP L1), >= 7,500 MB/s (ZIP Extract), >= 15,000 MB/s (TAR.ZST), PMULL CRC64 >= 48 GB/s
**Constraints**: 100% In-process C static binding (zero `posix_spawn` in hot paths), zero kernel zeroing in loops, zero broken documentation links
**Scale/Scope**: Full 16 archive formats, 4 industrial workloads, 10 competitor tool comparisons, 9 CLI subcommands

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant | Status | Verification Detail |
| :--- | :---: | :--- |
| **Zero-Cost Abstraction on Hot Paths** | ✅ PASSED | Benchmarking suites use direct in-process static C bindings and monotonic hardware timers without heap overhead. |
| **Fast-Path Bypass Preservation** | ✅ PASSED | Native C ZIP extraction and ARM NEON / PMULL paths remain unaltered and explicitly benchmarked. |
| **Hard Performance Floors** | ✅ PASSED | All throughput floors verified via `XCTestPerformanceMeasureTests`. |
| **The Four Systemic Invariants** | ✅ PASSED | Stream-First microbuffering, Invariant-First POSIX security, Bounds-First magic/clamp, Oracle-First diff testing. |
| **Logging Discipline** | ✅ PASSED | No bare `printf`/`print` in production code; all CLI output uses structured formatters and `TTLogger`. |

---

## Phase 0: Research Items & Findings

- R001 [SUBAGENT:research] 《物理性能展示与基准架构设计》: Researched and established nanosecond monotonic clock measurement, 4 industrial workload definitions, 10+ competitor multithreaded parity setup, and hardware vector acceleration breakdowns. Output in `specs/073-performance-benchmarking-and/research.md`.
- R002 [SUBAGENT:research] 《顶级开源 README 架构与内容重构》: Researched international top-tier system projects (ripgrep, uv, zstd) and designed the 10-chapter "CLI-First, GUI-Enhanced, Engine-Hardened" structure. Output in `specs/073-performance-benchmarking-and/research.md`.
- R003 [SUBAGENT:research] 《开源协议与商业授权体系严格确界》: Audited licensing inconsistencies (README badge vs root LICENSE vs Formula vs SPDX) and codified the Source-Available Anti-Copycat License + Enterprise Commercial model. Output in `specs/073-performance-benchmarking-and/research.md`.

---

## Phase 1: Design Artifacts & Contracts

- **Data Model**: `specs/073-performance-benchmarking-and/data-model.md` defining `BenchmarkMeasurementRecord`, `FormatSupportCapability`, `DocumentationSection`, and `LicensePolicyDefinition`.
- **Contracts**:
  - `contracts/benchmark-metrics.json` [SUBAGENT:research]: JSON schema for monotonic benchmark records and environment metadata.
  - `contracts/readme-structure.json` [SUBAGENT:research]: JSON schema governing README section presence, format counts, and subcommand coverage.
  - `contracts/licensing-policy.json` [SUBAGENT:research]: JSON schema for legal terms, prohibitions, and enterprise licensing.
- **Quickstart Guide**: `specs/073-performance-benchmarking-and/quickstart.md` with runnable validation scenarios and diagnostics.

---

## Project Structure & Planned Changes

```text
TTZip/
├── README.md                                    # [MODIFY] Comprehensive overhaul to Tier-1 standard
├── LICENSE                                      # [MODIFY] Standardize naming and legal clarity
├── ACKNOWLEDGEMENTS.md                          # [MODIFY] Update license notices & upstream giving-back
├── Formula/
│   └── ttzip-cli.rb                             # [MODIFY] Synchronize license metadata
├── docs/
│   ├── PERFORMANCE.md                           # [NEW] Comprehensive physical benchmark whitepaper
│   └── competitor_benchmark_report.md           # [MODIFY] Update with exhaustive 1v1 data
├── Sources/
│   ├── TTZipCore/CLI/CLIPackageManifest.swift   # [MODIFY] Align license metadata in formula generator
│   └── TTZipCLI/                                # [VERIFY] Verify all 9 subcommands & help text
└── Tests/TTZipTests/
    └── CLIPackagingTests.swift                  # [MODIFY] Update test assertions for license sync
```

---

## Complexity Tracking

| Invariant / Check | Justification |
| :--- | :--- |
| None | No architectural complexity violations introduced. |
