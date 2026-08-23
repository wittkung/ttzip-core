# Feature Specification: C Test Harness Expansion & Advanced Microkernel Migration

**Feature ID**: `156-156-test-harness-expansion`  
**Created**: 2026-08-20  
**Status**: Ready for Plan  

---

## 1. Problem Statement & Motivation

Phase 1 established the zero-dependency C11 test framework (`tests/c/ttzip_test_harness.h`) and migrated 8 core subsystems (CRC, magic sniffing, natural sorting, Deflate/Zopfli, 7z/LZMA2, Tar, security, concurrency).

Phase 2 will complete the migration of all remaining advanced compression, entropy calculation, and virtual filesystem microkernels into native C11 test suites:
1. **Blosc2 & BloscLZ Engine & Slicing (`test_blosc_engine.c`)**: Fast byte-level compression, bit-grooming quantization, superchunk serialization, and tensor/sparse slicing.
2. **In-Place Huffman & Adaptive Block Splitting (`test_huffman_inplace.c`)**: Kraft-McMillan bounds, Canonical code generation, RBIT bit reversal, entropy-driven block split evaluations.
3. **Snappy Block & Framing Engine (`test_snappy_engine.c`)**: Raw Snappy block compression/decompression roundtrip, framing chunking, and CRC32c validation.
4. **Apple DMG Demuxer & LZFSE Decoder (`test_dmg_lzfse.c`)**: Apple UDIF koly trailer demuxing, sector block tables, LZFSE lossless decompression.
5. **Radix Trie & Virtual Archive File Tree (`test_archive_tree.c`)**: Radix tree insertion, prefix lookup, natural sort tree traversal, and multi-million node memory bounds.

---

## 2. User Scenarios & Priorities

### User Story 1 (Priority: P1) - High-Throughput Microkernel Engines (Blosc2, In-Place Huffman, Snappy)
As a TTZip developer, I want Blosc2 chunking, in-place Huffman bitstream calculations, and Snappy framed streams tested directly in native C11 with zero Swift FFI overhead, so that sub-microsecond vector operations can be verified in < 5ms.

- **Acceptance Scenario 1.1**: Implement `tests/c/test_blosc_engine.c` verifying BloscLZ roundtrip, BitGroom mantissa quantization, and SuperChunk indexing.
- **Acceptance Scenario 1.2**: Implement `tests/c/test_huffman_inplace.c` verifying Kraft-McMillan limits, ARM64 RBIT bit reversal, and adaptive block split cost evaluations.
- **Acceptance Scenario 1.3**: Implement `tests/c/test_snappy_engine.c` verifying raw Snappy block and framed streaming roundtrip.

### User Story 2 (Priority: P2) - Apple DMG Demuxing, LZFSE, and Radix Virtual Archive Tree
As an archiver engineer, I want Apple UDIF DMG trailers, LZFSE decoders, and Radix virtual archive trees tested natively in C, so that large disk images and deeply nested file hierarchies are verified with zero memory leaks.

- **Acceptance Scenario 2.1**: Implement `tests/c/test_dmg_lzfse.c` verifying UDIF koly trailers, sector block decoding, and LZFSE decompression.
- **Acceptance Scenario 2.2**: Implement `tests/c/test_archive_tree.c` verifying Radix trie node insertions, prefix search, and natural ordering.

### User Story 3 (Priority: P3) - CTest Registration & Swift Pruning
As a CI engineer, I want all new test suites registered in `CMakeLists.txt` and `test_main.c`, and corresponding redundant Swift wrappers in `Tests/TTZipTests/` safely pruned.

- **Acceptance Scenario 3.1**: Register all 5 new suites in `tests/c/test_main.c` and `CMakeLists.txt` with CTest targets.
- **Acceptance Scenario 3.2**: Prune corresponding pure C-wrapper tests from `Tests/TTZipTests/` (`BloscLZNativeEngineTests.swift`, `Blosc2BitGroomingTests.swift`, `InPlaceHuffmanTests.swift`, `SnappyBlockEngineTests.swift`, `SnappyFramingStreamTests.swift`).
- **Acceptance Scenario 3.3**: Validate 100% green pass in `scripts/local-ci.sh` with 0 warnings.

---

## 3. Functional Requirements

- **FR-001**: The system MUST implement native C11 test suites for Blosc2, Huffman in-place, Snappy, DMG/LZFSE, and Radix tree.
- **FR-002**: All new test suites MUST use `tests/c/ttzip_test_harness.h` with zero heap allocation and high-precision monotonic timing.
- **FR-003**: The test runner `ttzip_c_test_runner` MUST support granular execution of all 13+ test suites via sub-command dispatch and CTest.
- **FR-004**: The project MUST maintain 0 compiler and 0 linker warnings across Swift, Clang C11, and CMake targets.
- **FR-005**: All C tests MUST pass under AddressSanitizer and UndefinedBehaviorSanitizer with zero memory leaks.
- **FR-006**: Redundant Swift C-wrapper test files MUST be cleanly pruned while retaining all Swift architecture and AppKit UI tests.

---

## 4. Success Criteria

- **SC-001 (Microsecond Execution)**: All 13 C test suites execute in **< 15 milliseconds** total.
- **SC-002 (100% CTest Pass Rate)**: `ctest --test-dir build --output-on-failure` achieves 100% pass across all suites.
- **SC-003 (Zero ASan Leaks)**: Zero memory leaks and zero undefined behavior detected under AddressSanitizer/UBSan.
- **SC-004 (Zero Compiler Warnings)**: 0 warnings in `swift build`, `swift build --build-tests`, and `cmake --build build`.
- **SC-005 (Local CI Green)**: All 5 stages of `scripts/local-ci.sh` pass cleanly with 0 cloud quota.
