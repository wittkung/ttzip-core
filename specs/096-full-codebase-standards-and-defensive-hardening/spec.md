# Feature Specification: 096-full-codebase-standards-and-defensive-hardening

## Overview & Context

This feature implements a comprehensive, codebase-wide code standards standardization and defensive systems hardening across all source files in TTZip (`Sources/CTTZipBridge/`, `Sources/TTZipCore/`, `Sources/TTZipApp/`, `Sources/TTZipCLI/`, `Tests/TTZipTests/`, `scripts/`).

Drawing directly from world-class systems engineering practices (**Linux Kernel, SQLite, OpenSSL/BoringSSL, Meta Zstandard, ClickHouse, simdjson**), this initiative resolves all latent compiler warnings, enforces strict Hoare Triple Design-by-Contract documentation across all public and internal C/Swift interfaces, embeds struct magic sentinels with free-poisoning (`0xDEADBEEFU`), mandates Dead-Store Elimination (DSE) immune sensitive memory wiping, applies arithmetic overflow built-ins (`__builtin_add_overflow`), and ensures 100% SPDX copyright consistency.

---

## User Scenarios & Personas

### Scenario 1: Systems Engineer & Code Reviewer (Clarity, Predictability & Auditability)
- **Goal**: Review any C or Swift file in the codebase and immediately understand parameter invariants, memory ownership, pre/postconditions, asymptotic complexity, and mathematical proofs without ambiguity.
- **Experience**: Every function header features standardized Doxygen/DocC tags; all magic constants have embedded proofs; compiler warning output is strictly clean (0 warnings).

### Scenario 2: Security & Concurrency Auditor (Deterministic Memory Safety)
- **Goal**: Verify that the codebase has zero memory corruption vulnerabilities (UAF, double free, heap/stack overflow, sensitive memory residue).
- **Experience**: Structs are verified on entry via `magic` canaries; all freed structs are poisoned with `0xDEADBEEFU`; all sensitive password and crypto key buffers are erased via `ttzip_secure_zero` with compiler assembly barriers; worker slots are aligned to 128-byte cachelines to eliminate false sharing.

### Scenario 3: Continuous Integration & Release Engineering (Strict Quality Gates)
- **Goal**: Prevent any code violating standards or introducing subtle bugs from entering `main`.
- **Experience**: Local CI gates and build scripts enforce strict compiler flags (`-fvisibility=hidden`, `-Wall`, `-Wextra`, `-Wmissing-prototypes`, `-Wvla`, `-Wformat=2`, `-Wdocumentation`), failing immediately on unprototyped functions or missing SPDX headers.

---

## Functional Requirements

### Track 1: C Bridge Headers & Export Contract Standardization (`Sources/CTTZipBridge/include/`)
- **FR-001**: Standardize all 20+ `.h` header files with Doxygen Hoare Triple annotations: `@brief`, `@param[in,out]`, `@return`, `@pre`, `@post`, `@invariant`, `@complexity`, `@threadsafe`.
- **FR-002**: Ensure all public C API entry points are explicitly annotated with `TTZIP_API`, and internal/private functions are marked `static` or non-exported.
- **FR-003**: Embed 32-bit `magic` canaries (`uint32_t magic;`) in all C handle structs and verify them upon function entry.

### Track 2: C Source Defensive Hardening & Warning Elimination (`Sources/CTTZipBridge/`)
- **FR-004**: Fix all compiler warnings across C source files under strict flags (`-Wall -Wextra -Wmissing-prototypes -Wstrict-prototypes -Wvla -Wshadow -Wformat=2`), eliminating unused variables, uninitialized variables, and signed/unsigned comparison mismatches.
- **FR-005**: Replace raw arithmetic operations on buffer sizes and chunk offsets with `ttzip_add_overflow` and `ttzip_mul_overflow`.
- **FR-006**: Apply `ttzip_secure_zero` to all cryptographic key expansion buffers, KDF contexts, and temporary sensitive arrays prior to function epilogues.
- **FR-007**: Apply `TTZIP_POISON_FREE (0xDEADBEEFU)` to struct magic fields before invoking `free()` or recycling into memory pools.

### Track 3: Swift Core & App Layer DocC Standardization (`Sources/TTZipCore/`, `Sources/TTZipApp/`)
- **FR-008**: Standardize DocC Design-by-Contract annotations across public/internal Swift protocols, structs, and classes (`- Precondition:`, `- Postcondition:`, `- Invariant:`, `- Complexity:`, `- Note: Thread Safety:`).
- **FR-009**: Verify all C pointer conversions (`CUnsafeBufferAdapter`, `withUnsafeBytes`) are safe, non-escaping, and bounds-checked.
- **FR-010**: Ensure 100% Swift 6.0 concurrency compliance (`Sendable`, `@MainActor`, actor isolation).

### Track 4: SPDX License & Header Consistency Across the Repository
- **FR-011**: Verify that 100% of source files (`.c`, `.h`, `.swift`, `.sh`, `.py`) start on line 1 with the official SPDX copyright and license header.
- **FR-012**: Retain absolute zero performance regression across all 13 constitutional performance gates and 100% pass rate across 525+ unit tests.

---

## Measurable Success Criteria

| Metric | Target Baseline | Verification Method |
| :--- | :--- | :--- |
| **Compiler Warning Cleanliness** | 0 warnings under strict flags | `swift build -Xswiftc -warnings-as-errors` |
| **SPDX Header Compliance** | 100% files compliant | Full codebase regex scan script |
| **Defensive Memory Safety** | 0 UAF / 0 uninitialized variables / 0 VLA stack frames | `swift test --sanitize=address` |
| **Constitutional Performance Floors** | All 13 performance gates pass with $\ge$ baseline throughput | `swift test --filter XCTestPerformanceMeasureTests` |
| **Test Suite Pass Rate** | 100% pass across all 525+ tests | `swift test` |

---

## Clarifications & Non-Goals

### ## Clarifications
- **Frozen File Compliance**: The frozen ZIP core engine files (`ZipParallelExtractor.swift`, `ZipParallelWriter.swift`, `ZipCryptoEngine.swift`, `CTTZipBridge_Crypto.c`, `CTTZipExtract.c`) remain structurally protected; only non-logic documentation/SPDX enhancements are permitted without thawing.
- **Zero Configuration Creep**: No public flags or configuration toggles shall be added; all defensive checks run by default.
