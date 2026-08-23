# Feature Specification: 095-comprehensive-systems-engineering-evolution

## Overview & Context

TTZip is a state-of-the-art native compression and archiving engine for macOS (Apple Silicon & Intel). Following an in-depth systems survey of world-class open-source projects (**SQLite, Linux Kernel, OpenSSL/BoringSSL, Meta Zstandard, ClickHouse, simdjson, libdeflate**), this specification defines a comprehensive, end-to-end engineering evolution across four core pillars:
1. **Testing & Verification Engineering**: N-way consensus differential testing (`bsdtar`, `7z`, `ditto`, `zipinfo`), property-based randomized tree generation, structured dual-stream mutation fuzzing, and continuous sanitizer gates (ASan, UBSan, TSan, leaks CLI).
2. **Defensive Systems Code Style & Memory Safety**: Struct magic sentinels with free-poisoning (`0xDEADBEEF`), dead-store elimination (DSE) immune memory wiping (`ttzip_secure_zero`), integer overflow checking macros, strict compiler flags (`-fvisibility=hidden`, `-Wmissing-prototypes`, `-Wvla`, `-Wshadow`), and cacheline alignment (128B) to eliminate multicore false sharing.
3. **Documentation & Formal Mathematical Invariants**: Literate mathematical proofs embedded directly in source code for all critical constants ($N_{\max} = 5552$, SWAR offsets, LZMA range intervals, Galois field Barrett reduction), standardized Design-by-Contract annotations (`@pre`, `@post`, `@invariant`, `@complexity`, `@threadsafe`), and automated Clang `-Wdocumentation` verification.
4. **Microarchitectural & Vectorization Frontiers**: 16-byte SIMD candidate filtering for search and path sanitization, branchless primitives (Eytzinger binary search), software prefetching in LZMA match finders, and non-temporal streaming stores (`stnp`) for direct write sinks.

---

## User Scenarios & Personas

### Scenario 1: Systems Engineer & Core Engine Contributor (Zero-Regression & Code Quality)
- **Goal**: Read, modify, and audit complex C/Swift low-level code with absolute confidence in correctness, memory safety, and thread safety.
- **Experience**: Mathematical equations and loop invariants are proven directly above the code; compiler warnings enforce zero unprototyped functions; struct magic sentinels catch any UAF or cross-thread race immediately.

### Scenario 2: Security Auditor & Fuzzing Engineer (Robustness against Hostile Archives)
- **Goal**: Verify that TTZip is completely immune to malformed inputs, zipbombs, Zip Slip symlink traversal attacks, and integer overflow exploits.
- **Experience**: The N-way differential test suite and structure-aware mutation fuzzer continuously hammer the engine with hostile payloads, verifying zero crashes, zero memory leaks, and clean error returns under all sanitizers.

### Scenario 3: End User & Application UI (Blazing Speed & Zero Configuration Overhead)
- **Goal**: Compress, decompress, and search massive archives (100,000+ files) with instantaneous responsiveness and zero manual tuning switches.
- **Experience**: Directory scanning and search queries execute up to 10x faster with zero heap allocation spikes; memory footprint remains minimal and predictable.

---

## Functional Requirements

### Pillar 1: Testing & Verification Architecture
- **FR-001**: Implement `MultiWayOracleConsensusTest` in `Tests/TTZipTests/` discovering and cross-verifying outputs against macOS system tools (`/usr/bin/tar`, `/usr/bin/unzip`, `/usr/bin/ditto`, `/usr/bin/zipinfo`) and external CLI engines (`7z`, `bsdtar`).
- **FR-002**: Implement `ArchivePropertyBasedTreeGenerator` generating randomized file trees with configurable depth ($\ge 20$), APFS Unicode NFC/NFD variations, sparse files, mixed POSIX permissions (`000`..`777`), and symlink/hardlink graphs.
- **FR-003**: Implement structure-aware mutation fuzzing verifying 6 hostile vectors: magic corruption, bitstream truncation, overlapping zipbomb entries, out-of-bounds varints, symlink TOCTOU escapes, and 0-byte boundaries.
- **FR-004**: Integrate UndefinedBehaviorSanitizer (`-fsanitize=undefined`) and macOS native `leaks --atExit` automation into local and CI test execution scripts.

### Pillar 2: Systems Code Style, Memory Safety & Invariants
- **FR-005**: Define unified overflow-checked arithmetic macros (`ttzip_add_overflow`, `ttzip_mul_overflow`, `ttzip_sub_overflow`) and integer clamp wrappers in `CTTZipCommon.h`.
- **FR-006**: Embed 32-bit `magic` canaries in C bridge structures and enforce `0xDEADBEEFU` poisoning on destruction/free.
- **FR-007**: Implement `ttzip_secure_zero` using `memset_s` / `explicit_bzero` with volatile assembly barriers, applying it across all cryptographic key expansions and sensitive memory buffers.
- **FR-008**: Define `TTZIP_CACHELINE_ALIGNED` (128 bytes for Apple Silicon ARM64, 64 bytes for x86_64) and apply to multithreaded worker slot arrays to eliminate false sharing.
- **FR-009**: Configure strict compiler warning flags (`-fvisibility=hidden`, `-Wall`, `-Wextra`, `-Wmissing-prototypes`, `-Wstrict-prototypes`, `-Wvla`, `-Wshadow`, `-Wformat=2`) in `Package.swift`.

### Pillar 3: Documentation & Mathematical Invariants
- **FR-010**: Embed formal mathematical derivations and invariant proofs in source code for Adler-32 ($N_{\max} = 5552$ quadratic root), SWAR bit differences ($\text{ctz64}(D) \gg 3$), LZMA range coder probability bounds ($P \in [1, 2047]$), and CRC64 Barrett reduction.
- **FR-011**: Standardize Doxygen/HeaderDoc/SwiftDoc tags across all C and Swift interfaces: `@brief`, `@param[in,out]`, `@return`, `@pre`, `@post`, `@invariant`, `@complexity`, `@threadsafe`.
- **FR-012**: Enforce Bi-directional Comment-Code Semantic Invariant, verifying that comment verbs and parameter identifiers match physical instruction opcodes 100%.

### Pillar 4: Microarchitectural & Algorithmic Optimization
- **FR-013**: Implement 16-byte ARM NEON candidate vector filtering (`vceqq_u8` + `vmaxvq_u8`) in `ArchiveSearchIndex.swift` to bypass scalar string searches for >95% of candidate entries.
- **FR-014**: Integrate software prefetch hints (`__builtin_prefetch`) in LZMA2 HC4/BT4 match finders 2 iterations ahead of hash chain lookups to hide DRAM latency.
- **FR-015**: Ensure all optimizations preserve the Zero Configuration Creep invariant with 100% transparent default execution.

---

## Measurable Success Criteria

| Metric | Target Baseline | Verification Method |
| :--- | :--- | :--- |
| **Compiler Warnings** | 0 warnings, 0 missing prototypes under strict flags | `swift build -Xswiftc -warnings-as-errors` |
| **Sanitizer Verification** | 0 ASan errors, 0 TSan data races, 0 UBSan undefined behaviors | `swift test --sanitize=address` & `--sanitize=thread` |
| **Memory Leak Gate** | 0 UnsafeMutablePointer leaks | macOS `leaks --atExit` on test binaries |
| **Differential Consensus** | 100% identity between TTZip and system oracles | `swift test --filter DifferentialOracleTests` |
| **Hard Performance Floors** | All 13 constitution performance gates pass with 0 regressions | `swift test --filter XCTestPerformanceMeasureTests` |
| **Test Suite Pass Rate** | 100% pass across all 525+ unit and integration tests | `swift test` |

---

## Clarifications & Edge Cases

### ## Clarifications
- **C11 / POSIX / Swift 6.0 Compatibility**: All C code strictly complies with C11 and POSIX.1-2008 without non-standard GNU extensions; all Swift code maintains Swift 6.0 concurrency (`Sendable`, `@MainActor`) compliance.
- **Zero Public Flag Exposure**: No new user-facing options or CLI flags shall be added; all memory optimizations and SIMD accelerations run transparently based on hardware and file size heuristics.
