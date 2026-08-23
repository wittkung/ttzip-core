# Implementation Plan: C Test Harness Migration & Dual-Engine Testing Architecture

**Branch**: `154-c-test-harness-migration` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/154-c-test-harness-migration/spec.md`

---

## Summary

Migrate all core microkernel, compression, and security tests into a zero-dependency header-only C11 test framework (`tests/c/ttzip_test_harness.h`), register isolated CTest suite targets in CMake, and provide a unified native runner binary (`ttzip_c_test_runner`). This establishes a decoupled dual-engine test architecture where native C tests execute in < 50ms and Swift tests focus exclusively on AppKit UI/ViewModels.

---

## Technical Context

- **Language/Version**: ISO/IEC 9899:2011 (ANSI C11) & Swift 6.0
- **Primary Dependencies**: None (Zero third-party test dependencies; standard libc headers only)
- **Storage**: In-memory test fixtures, synthetic buffers, temporary test directories
- **Testing**: Native C11 Test Harness (`ttzip_test_harness.h`) + CMake CTest + Swift XCTest (AppKit UI)
- **Target Platform**: macOS (Apple Silicon arm64 & Intel x86_64) and standard POSIX Linux/FreeBSD
- **Project Type**: Native High-Performance Compression Microkernel & macOS GUI App
- **Performance Goals**: Complete C test runner execution in **< 50 milliseconds** (< 100ms under ASan)
- **Constraints**: Zero dynamic heap allocation in test harness assertions, 0 compiler warnings, 0 ASan memory leaks
- **Scale/Scope**: 8+ comprehensive C test suites covering 20+ microkernel subsystems and all 16 compression formats

---

## Constitution Check

*GATE: Passed. Complies with zero cloud actions quota, zero external dependency, and sub-second local test invariants.*

- [x] Principle 1: Zero Cloud CI Quota — All tests execute purely on the local machine via `ctest` and `scripts/local-ci.sh`.
- [x] Principle 2: Zero Dependency Bloat — The C test framework is a single header file using ANSI C11 standard library only.
- [x] Principle 3: Performance First — Eliminates 2–4s Swift runtime startup overhead for algorithmic tests; achieves < 15ms in-process run.
- [x] Principle 4: High Reliability & Memory Safety — Compatible with AddressSanitizer and UndefinedBehaviorSanitizer.

---

## Phase 0: Outline & Research

The following research items were formally dispatched and investigated via dedicated research subagents:
- R001 [SUBAGENT:research] 《C11 Zero-Dependency Test Harness Architecture》：Zero-heap allocation, platform monotonic timing, ANSI color progression, and rich fail-fast assert macros.
- R002 [SUBAGENT:research] 《CMake & CTest Integration Architecture》：Unified test runner binary (`ttzip_c_test_runner`) with granular CTest target registration and ASan/UBSan build flags.
- R003 [SUBAGENT:research] 《Swift-to-C Microkernel Test Mapping》：Complete mapping of algorithmic, container, and security invariants from `Tests/TTZipTests/` to `tests/c/test_*.c`.

*Artifact*: [research.md](./research.md)

---

## Phase 1: Design & Contracts

The following design artifacts establish the data structures, JSON schema contracts, and verification procedures:
- Data Model: [data-model.md](./data-model.md)
- Contracts:
  - `contracts/c-test-runner-schema.json` [SUBAGENT:research] — Formal JSON Schema for test runner execution telemetry.
- Quickstart & Verification: [quickstart.md](./quickstart.md)

---

## Project Structure & Planned Changes

```text
TTZip/
├── CMakeLists.txt                              # [MODIFY] Add enable_testing(), C test targets & CTest suites
├── scripts/local-ci.sh                         # [MODIFY] Prepend CTest execution before Swift test stages
├── tests/
│   └── c/
│       ├── ttzip_test_harness.h                # [NEW] Header-only C11 test framework
│       ├── test_main.c                         # [NEW] Unified test runner & sub-command dispatcher
│       ├── test_crc_neon.c                     # [NEW] CRC32/CRC64 hardware & software parity tests
│       ├── test_magic_sniff.c                  # [NEW] 16-format sub-nanosecond magic sniffing tests
│       ├── test_strnatcmp.c                    # [NEW] C11 natural numeric sort tests
│       ├── test_deflate_zopfli.c               # [NEW] Deflate & Zopfli roundtrip & dictionary tests
│       ├── test_7z_lzma2.c                     # [NEW] 7z headers, Varint & LZMA2 block tests
│       ├── test_tar_container.c                # [NEW] Tar/Pax headers, SWAR octal & tree tests
│       ├── test_security_zipslip.c             # [NEW] Defensive path traversal interception tests
│       └── test_concurrency_threadpool.c       # [NEW] Thread pool, counting semaphore & memory budget tests
└── specs/154-c-test-harness-migration/
    ├── spec.md
    ├── checklists/requirements.md
    ├── plan.md
    ├── research.md
    ├── data-model.md
    ├── contracts/c-test-runner-schema.json
    └── quickstart.md
```

---

## Complexity Tracking

| Aspect | Justification | Alternatives Considered |
| :--- | :--- | :--- |
| **Unified Runner Binary** | Compiling a single `ttzip_c_test_runner` avoids multi-binary link overhead while allowing granular CTest execution via sub-command dispatch. | Separate binaries rejected due to 8x linker latency. |
