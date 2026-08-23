# Implementation Plan: 095-comprehensive-systems-engineering-evolution

## Technical Context & Constraints
- **Target OS**: macOS 14.0+ (Sonoma) on Apple Silicon (ARM64 / ARMv8.2-A+crypto) and Intel (x86_64).
- **Standards Compliance**: C11, POSIX.1-2008, Swift 6.0 (`Sendable`, `@MainActor`).
- **Performance Invariants**: Hot paths strictly zero dynamic allocation, zero false sharing, branchless kernels.
- **Architectural Rules**: Frozen ZIP engines respected; multi-agent isolation via `SPECIFY_FEATURE_DIRECTORY`.

---

## Constitution Check
- [x] Zero-Cost Abstraction on hot paths preserved.
- [x] Fast-Path bypass retained.
- [x] All 13 performance throughput floors enforced.
- [x] Zero configuration creep guaranteed.

---

## Phase 0: Research Index
- `- R001 [SUBAGENT:research] Multi-Way Differential Consensus Oracles` (Completed)
- `- R002 [SUBAGENT:research] Property-Based Generative Tree Fuzzing` (Completed)
- `- R003 [SUBAGENT:research] Struct Magic Sentinel Embedding & Free-Poisoning` (Completed)
- `- R004 [SUBAGENT:research] Dead-Store Elimination (DSE) Immune Memory Zeroing` (Completed)
- `- R005 [SUBAGENT:research] Integer Overflow Checking & Clamping Invariants` (Completed)
- `- R006 [SUBAGENT:research] Multicore False Sharing Elimination via 128-Byte Cacheline Alignment` (Completed)
- `- R007 [SUBAGENT:research] Literate Mathematical Invariant Proofs Embedded in Source Code` (Completed)
- `- R008 [SUBAGENT:research] Standardized Design-by-Contract Annotation Taxonomy` (Completed)
- `- R009 [SUBAGENT:research] 16-Byte SIMD Vector Candidate Filtering` (Completed)
- `- R010 [SUBAGENT:research] Software Prefetching in LZMA2 Match Finders` (Completed)

---

## Phase 1: Contracts & Data Model
- `data-model.md`: Data models for consensus reports, property trees, and C defensive structs.
- `contracts/systems-evolution-schema.json`: JSON Schema Draft-07 specification.
- `quickstart.md`: 4 validation scenarios.

---

## Phase 2: Implementation Breakdown by Component

### Component 1: Defensive C Core (`Sources/CTTZipBridge/`)
1. [`CTTZipCommon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipCommon.h):
   - Add `ttzip_add_overflow`, `ttzip_mul_overflow`, `ttzip_sub_overflow`.
   - Add `TTZIP_STRUCT_MAGIC` (`0x545A4354`) and `TTZIP_POISON_FREE` (`0xDEADBEEFU`).
   - Add `TTZIP_CACHELINE_ALIGNED` (128 bytes on Apple Silicon ARM64, 64 bytes on x86_64).
2. [`CTTZipBridge_Crypto.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_Crypto.c):
   - Add `ttzip_secure_zero` on sensitive key schedule buffers before function exit.
3. [`Package.swift`](file:///Users/kevintung/Documents/dev/TTZip/Package.swift):
   - Add strict compiler flags: `-fvisibility=hidden`, `-Wall`, `-Wextra`, `-Wmissing-prototypes`, `-Wstrict-prototypes`, `-Wvla`, `-Wshadow`, `-Wformat=2`.

### Component 2: Documentation & Mathematical Invariants
1. Embed formal mathematical proofs in headers and implementation files:
   - `CTTZipAdler32Neon.c`: $N_{\max} = 5552$ quadratic solution and DotProd matrix.
   - `ttzip_crc64.c`: Barrett polynomial reduction $\mu(x)$ derivation.
   - `ttzip_bcj_arm64_neon.c`: ARM64 branch offset sign-extension arithmetic.

### Component 3: Testing & Verification Harness (`Tests/TTZipTests/`)
1. [`DifferentialOracleTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/DifferentialOracleTests.swift):
   - Expand oracle discovery to include `ditto`, `bsdtar`, `7z`, and `zipinfo`.
2. [`ArchiveMutationFuzzTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ArchiveMutationFuzzTests.swift):
   - Verify hostile archive defense against truncated streams, out-of-bounds varints, and overlapping headers.

### Component 4: Microarchitectural Search Acceleration (`Sources/TTZipCore/`)
1. [`ArchiveSearchIndex.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Search/ArchiveSearchIndex.swift):
   - Implement fast candidate prefix/suffix filtering before calling scalar string matchers.
