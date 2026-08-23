# Tasks: Full C11/SIMD Migration of 7 Core Engine Modules

**Feature Branch**: `161-seven-modules-c-migration` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Dependencies & Execution Graph

```
[Phase 1: Setup & Headers]
          │
          ▼
[Phase 2: User Story 1 (P1: Mathematical & SIMD Acceleration - Reed-Solomon)]
          │
          ▼
[Phase 3: User Story 2 (P1: Metadata & Zero-Copy - PathFilter & ZipExtraField)]
          │
          ▼
[Phase 4: User Story 3 (P2: High-Throughput Search, Geometry & Password Recovery)]
          │
          ▼
[Phase 5: Polish & Full Regression Verification]
```

---

## Phase 1: Setup & Bridge Header Declarations

- [x] T001 [P] Declare C API headers in `Sources/CTTZipBridge/include/ttzip_reed_solomon.h`, `Sources/CTTZipBridge/include/ttzip_path_filter.h`, `Sources/CTTZipBridge/include/ttzip_zip_extra_field.h`, `Sources/CTTZipBridge/include/ttzip_password_verifier.h`, `Sources/CTTZipBridge/include/ttzip_search_index.h`
- [x] T002 Verify umbrella header inclusion in `Sources/CTTZipBridge/include/CTTZipBridge.h`

---

## Phase 2: User Story 1 - Mathematical & Erasure Coding Acceleration (Priority: P1)

**Goal**: Implement Galois Field GF(2^8) and ARM64 NEON Cauchy Reed-Solomon erasure coding in C and connect to Swift facade.

- [x] T003 [P] [US1] Implement `Sources/CTTZipBridge/ttzip_reed_solomon.c` with GF(2^8) arithmetic, Cauchy matrix generation, Gauss-Jordan matrix inversion, and NEON `vqtbl1q_u8` vector encoding/decoding
- [x] T004 [US1] Bridge `Sources/TTZipCore/Security/ReedSolomonFEC.swift` to call `ttzip_rs_create_cauchy_matrix`, `ttzip_rs_encode_neon`, and `ttzip_rs_decode_neon`
- [x] T005 [US1] Run `ReedSolomonRecoveryRecordTests` to verify 100% mathematical fidelity and self-healing parity

---

## Phase 3: User Story 2 - High-Frequency Metadata Parsing & Filter Zero-Copy (Priority: P1)

**Goal**: Implement zero-allocation path glob matching and in-place ZIP Extra Field TLV parsing in C.

- [x] T006 [P] [US2] Implement `Sources/CTTZipBridge/ttzip_path_filter.c` with POSIX glob pattern evaluation, suffix fast-path, and predefined VCS/Mac junk filters
- [x] T007 [P] [US2] Implement `Sources/CTTZipBridge/ttzip_zip_extra_field.c` with unaligned load TLV parser for 0x0001, 0x5455, 0x7075, 0x7875, and 0x9901
- [x] T008 [P] [US2] Consolidate 7z signature and descriptor parsing in `Sources/CTTZipBridge/ttzip_7z_header_parser.c`
- [x] T009 [US2] Bridge `Sources/TTZipCore/Security/PathPatternFilterEngine.swift` to `ttzip_path_filter.c`
- [x] T010 [US2] Bridge `Sources/TTZipCore/Standards/ZipExtraFieldParser.swift` to `ttzip_zip_extra_field.c`
- [x] T011 [US2] Bridge `Sources/TTZipCore/SevenZip/SevenZipHeaderReader.swift` to `ttzip_7z_header_parser.c`
- [x] T012 [US2] Run `ZipExtraFieldParserTests` and `RecursiveDirectoryEdgeCaseTests` to verify metadata parsing

---

## Phase 4: User Story 3 - High-Throughput Search, Geometry & Password Recovery (Priority: P2)

**Goal**: Implement flat columnar memory search, multi-core in-memory password verification, and unrolled N-dim hypercube block solving.

- [x] T013 [P] [US3] Implement `Sources/CTTZipBridge/ttzip_password_verifier.c` with in-memory PVV and ZipCrypto header verification using `ttzip_parallel_for`
- [x] T014 [P] [US3] Implement `Sources/CTTZipBridge/ttzip_search_index.c` with contiguous buffer storage and NEON substring filtering
- [x] T015 [P] [US3] Extend `Sources/CTTZipBridge/CTTZipTensorSlicing.c` with `ttzip_tensor_find_intersecting_blocks` unrolled coordinate hypercube block solver
- [x] T016 [US3] Bridge `Sources/TTZipCore/PasswordRecoveryEngine.swift` to `ttzip_password_verifier.c`
- [x] T017 [US3] Bridge `Sources/TTZipCore/Search/ArchiveSearchIndex.swift` to `ttzip_search_index.c`
- [x] T018 [US3] Bridge `Sources/TTZipCore/NDim/NDimTensorLayout.swift` to `CTTZipTensorSlicing.c`
- [x] T019 [US3] Run `InArchiveSearchEngineTests` and `NDimTensorHypercubeSlicingTests` to verify search and tensor slicing

---

## Phase 5: Polish & Full Regression Verification

**Goal**: Verify all targets compile cleanly with zero warnings and pass 100% of the test suite.

- [x] T020 Run full compilation (`swift build --build-tests`) with 0 warnings
- [x] T021 Execute entire 912-test suite across TTZipPackageTests to guarantee zero regressions
- [x] T022 Execute `@speckit-converge` and `@speckit-analyze` consistency verification
