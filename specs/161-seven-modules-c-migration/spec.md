# Feature Specification: Full C11/SIMD Migration of 7 Core Engine Modules

**Feature ID**: `161-seven-modules-c-migration`  
**Target Milestone**: 1.0 Native Core Performance Supremacy  
**Scope**: `Sources/CTTZipBridge/` and `Sources/TTZipCore/`  

---

## 1. Overview & Motivation

To achieve maximum throughput, zero-allocation metadata processing, and eliminate Swift-C bridging overhead across the core archiving pipeline, 7 performance-critical modules in `Sources/TTZipCore/` are migrated to pure C11 implementations in `Sources/CTTZipBridge/` with ARM64 NEON vector acceleration.

The 7 modules are:
1. **Module 1: ReedSolomonFEC** (Systematic Cauchy Reed-Solomon Erasure Coding & Self-Healing over $GF(2^8)$)
2. **Module 2: PathPatternFilterEngine** (High-speed POSIX Glob Wildcard Matching & Junk Path Filtering)
3. **Module 3: ZipExtraFieldParser** (Zero-Allocation Tag-Length-Value (TLV) Extra Field Parser)
4. **Module 4: SevenZipHeaderReader** (7z Signature Header & Seek Table Decoder Consolidation)
5. **Module 5: FastPasswordVerifier** (In-Memory Multi-Core Parallel Password Recovery Kernel)
6. **Module 6: ArchiveSearchIndex** (Contiguous Flat Columnar Memory & SIMD Substring Filter)
7. **Module 7: NDimTensorCore** (N-Dimensional Tensor Hypercube Geometry & Slicing Kernel)

---

## 2. User Stories

### User Story 1 - Mathematical & Erasure Coding Acceleration (Priority: P1)
As an archival user protecting large datasets with recovery records, I need Reed-Solomon encoding and self-healing parity calculations to run at multi-gigabyte line rates using hardware vector registers (ARM64 NEON).
- **Acceptance Criteria**: RS-FEC encoding and decoding are fully implemented in C11 (`ttzip_reed_solomon_neon.c`), with 100% mathematical parity against previous Swift results and 10x+ throughput boost.

### User Story 2 - High-Frequency Metadata Parsing & Filter Zero-Copy (Priority: P1)
As a user browsing or unpacking large archives with 100,000+ files, I need path pattern filtering and ZIP extra field parsing to execute in-place in C without heap churn or Swift string allocations.
- **Acceptance Criteria**: `ttzip_path_filter.c` and `ttzip_zip_extra_parser.c` parse paths and extra fields directly from pointers in C; Swift delegates with zero per-item memory allocation.

### User Story 3 - High-Throughput Search, Geometry & Password Recovery (Priority: P2)
As a power user searching entries, calculating N-dim tensor coordinates, or recovering archive passwords, I need flat columnar memory indexes and parallel C kernels to deliver sub-millisecond response times.
- **Acceptance Criteria**: `ttzip_search_index.c`, `ttzip_ndim_tensor_core.c`, and `ttzip_fast_password_verifier.c` provide parallel, lock-free C primitives backing Swift facades.

---

## 3. Functional Requirements

- **FR-001**: Implement `ttzip_reed_solomon_neon.c` with GF(2^8) Galois Field arithmetic, Cauchy matrix generation, systematic encoding, and Gaussian elimination decoding in C11.
- **FR-002**: Implement `ttzip_path_filter.c` supporting fast POSIX glob pattern evaluation (`*`, `?`, `[...]`), suffix short-circuits, and predefined OS/VCS junk filters.
- **FR-003**: Implement `ttzip_zip_extra_parser.c` for in-place zero-allocation TLV parsing of Zip64 (`0x0001`), Extended Timestamp (`0x5455`), Unicode Path (`0x7075`), Info-ZIP Unix (`0x7875`), and WinZip AES (`0x9901`).
- **FR-004**: Consolidate `SevenZipHeaderReader` into `ttzip_7z_header_parser.c` to parse 32-byte signature headers and entry descriptors with zero redundancy.
- **FR-005**: Implement `ttzip_fast_password_verifier.c` providing multi-threaded batch password testing against ZipCrypto and AES-256 archive headers using `ttzip_parallel_for`.
- **FR-006**: Implement `ttzip_search_index.c` managing flat contiguous buffers and providing multi-threaded substring scanning.
- **FR-007**: Implement `ttzip_ndim_tensor_core.c` for row-major strides, slice shape calculation, and hypercube block coordinate intersections.
- **FR-008**: Maintain 100% backward compatibility in Swift facades (`ReedSolomonFEC`, `PathPatternFilterEngine`, `ZipExtraFieldParser`, `SevenZipHeaderReader`, `PasswordRecoveryEngine`, `ArchiveSearchIndex`, `NDimTensorShape`).
- **FR-009**: Ensure zero compiler warnings and all 912+ automated tests pass cleanly.

---

## 4. Success Criteria & Quality Verification

- **SC-001**: 100% pass rate across existing unit and integration test suites (`ReedSolomonRecoveryRecordTests`, `ZipExtraFieldParserTests`, `InArchiveSearchEngineTests`, `NDimTensorHypercubeSlicingTests`, `GapBridgingTests`, `AllFormatDiagnosticSuiteTests`).
- **SC-002**: Pure C11 code conformance, zero memory leaks, and strict lifetime checks with `ttzip_secure_zero` on sensitive cryptographic buffers.
- **SC-003**: Clean build with zero warnings under `-Wall -Wextra -Werror` C flags and Swift 6 mode.
