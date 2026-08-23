# Feature Specification: Full 22-File Swift Microkernel Test Migration to Native C11

**Feature ID**: `158-157-test-migration`  
**Created**: 2026-08-20  
**Status**: Ready for Plan  

---

## 1. Problem Statement & Motivation

Following the successful establishment of the C11 native test platform (currently 14 CTest suites executing in < 5ms), 22 remaining low-level Swift test files in `Tests/TTZipTests/` still perform pure C pointer FFI calls, SWAR bit calculations, SIMD/NEON mathematical invariants, and entropy evaluations.

These 22 files incur heavy SwiftPM compilation and ARC runtime overhead while duplicating logic that can be tested with bit-level precision in C11. This feature systematically migrates all 22 files to 5 dedicated C11 test suites under `tests/c/`, registers them in CTest, prunes the redundant Swift files, and validates zero memory leaks via AddressSanitizer and 100% green local CI.

---

## 2. User Scenarios & Priorities

### User Story 1 (Priority: P1) - SIMD Checksums & Entropy Models (Clusters 1 & 2)
As a systems engineer, I want Adler-32 NEON, CRC64-XZ scalar/vector oracles, and SWAR Shannon entropy evaluators tested directly in C11, so that SIMD register operations and math bounds are verified with zero heap allocations.

- **Acceptance Scenario 1.1**: Implement `tests/c/test_adler_crc64.c` covering Adler-32 RFC 1950 vectors, 5552B modulo boundaries, and CRC64-XZ alignment matrices.
- **Acceptance Scenario 1.2**: Implement `tests/c/test_entropy_evaluator.c` covering SWAR 8.0-scale Shannon entropy calculations, extreme routing thresholds, and tiered chunking.

### User Story 2 (Priority: P2) - Match Finders, Bitstreams & Blosc2 Slicing (Clusters 3 & 4)
As a compression engineer, I want LZ77 hash chains, canonical Huffman bitstreams, 32KB cross-block dictionaries, and Blosc2 tensor/sparse micro-slicing tested natively in C11.

- **Acceptance Scenario 2.1**: Implement `tests/c/test_matchfinder_advanced.c` covering hash chain probing, bitstream truncation, and ring dictionary chaining.
- **Acceptance Scenario 2.2**: Implement `tests/c/test_blosc_slicing.c` covering micro-block lazy slicing, constant chunk MSB tagging `(1ULL << 63)`, superchunk headers, and tensor pipeline filters.

### User Story 3 (Priority: P3) - 7z ARMv8 KDF Crypto, LZ4 VFS, Snappy Fuzzing & Pruning (Cluster 5 & Pruning)
As a security and CI engineer, I want 7z ARMv8 SHA-256 KDF key derivation, LZ4 block/frame decompression, and Snappy varint fuzzing tested in C11, with all 22 redundant Swift test files cleanly deleted and CI green.

- **Acceptance Scenario 3.1**: Implement `tests/c/test_crypto_lz4_snappy.c` covering SHA-256 KDF loops, AES-256 session init, LZ4 raw blocks, and malformed Snappy buffer resilience.
- **Acceptance Scenario 3.2**: Register all 5 new suites in `tests/c/test_main.c` and `CMakeLists.txt` (total 19 CTest targets).
- **Acceptance Scenario 3.3**: Prune all 22 redundant Swift test files from `Tests/TTZipTests/`.
- **Acceptance Scenario 3.4**: Validate AddressSanitizer/UBSan and execute the 5-stage local CI pipeline with 0 warnings.

---

## 3. Functional Requirements

- **FR-001**: The system MUST implement 5 new C11 test suites in `tests/c/` covering all invariants from the 22 target Swift test files.
- **FR-002**: All new C tests MUST utilize `tests/c/ttzip_test_harness.h` with nanosecond monotonic hardware timing and zero dynamic heap allocation in assertions.
- **FR-003**: The test runner `ttzip_c_test_runner` and `CMakeLists.txt` MUST register and dispatch all 19 test suites via sub-command filtering and CTest.
- **FR-004**: All 22 redundant Swift test files MUST be physically deleted from `Tests/TTZipTests/` without breaking any retained Swift design pattern or AppKit UI tests.
- **FR-005**: All targets (`swift build`, `swift build --build-tests`, `cmake --build build`) MUST compile with **0 compiler warnings and 0 linker warnings**.
- **FR-006**: The full C test runner MUST pass under AddressSanitizer and UndefinedBehaviorSanitizer with **0 memory leaks and 0 undefined behavior**.

---

## 4. Success Criteria

- **SC-001 (Microsecond Performance)**: All 19 C test suites execute in **< 15 milliseconds** total.
- **SC-002 (100% CTest Pass Rate)**: 19/19 CTest targets pass green (`100% tests passed, 0 tests failed`).
- **SC-003 (Zero ASan Leaks)**: 0 memory leaks and 0 UB detected in `build_asan`.
- **SC-004 (22 Swift Files Pruned)**: Exactly 22 redundant files deleted from `Tests/TTZipTests/`, reducing Swift test build time.
- **SC-005 (Local CI Green)**: All 5 stages of `scripts/local-ci.sh` execute cleanly with 0 cloud quota.
