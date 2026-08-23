# Implementation Plan: 096-full-codebase-standards-and-defensive-hardening

## Technical Context & Constraints
- **Target OS**: macOS 14.0+ (Sonoma) on Apple Silicon (ARM64 / ARMv8.2-A+crypto) and Intel (x86_64).
- **Standards Compliance**: C11, POSIX.1-2008, Swift 6.0 (`Sendable`, `@MainActor`).
- **Performance Invariants**: Hot paths strictly zero dynamic allocation, zero false sharing.
- **Frozen File Rules**: ZIP core engines preserved.

---

## Constitution Check
- [x] Zero-Cost Abstraction on hot paths preserved.
- [x] Fast-Path bypass retained.
- [x] All 13 performance throughput floors enforced.
- [x] Zero configuration creep guaranteed.

---

## Phase 0: Research Index
- `- R001 [SUBAGENT:research] Header Standardization & Hoare Triple Contracts` (Completed)
- `- R002 [SUBAGENT:research] Compiler Warning Flags Zero-Tolerance` (Completed)
- `- R003 [SUBAGENT:research] Struct Magic Sentinel & Free-Poisoning Architecture` (Completed)
- `- R004 [SUBAGENT:research] DSE-Immune Memory Eradication` (Completed)

---

## Phase 1: Contracts & Data Model
- `data-model.md`: Defensive type models and Hoare Triple contract templates.
- `contracts/standards-hardening-schema.json`: JSON Schema Draft-07 specification.
- `quickstart.md`: 4 validation scenarios.

---

## Phase 2: Implementation Breakdown by Component

### Track 1: C Bridge Warning Clean-Up & Defensive Poisoning (`Sources/CTTZipBridge/`)
1. Fix compiler warnings across C bridge files (`CTTZipSpawnPipelines.c`, `CTTZipSliceProfiler.c`, `CTTZipFilterPipeline.c`, `CTTZipCommon.c`, `CTTZipExtract.c`).
2. Add struct magic sentinel checks and `TTZIP_POISON_FREE` poisoning in stream decoders and context lifecycles.

### Track 2: C Bridge Headers Hoare Triple Standardization (`Sources/CTTZipBridge/include/`)
1. Standardize headers with `@brief`, `@param[in,out]`, `@return`, `@pre`, `@post`, `@complexity`, `@threadsafe`.
2. Ensure all public functions are exported via `TTZIP_API`.

### Track 3: Swift Core & App Layer DocC Standardization (`Sources/TTZipCore/`)
1. Standardize DocC Design-by-Contract annotations.
2. Verify Swift 6.0 concurrency safety.

### Track 4: SPDX License Compliance Scan & Verification
1. Run repository-wide scan to assert 100% SPDX header compliance.
2. Execute full regression and performance gate verification.
