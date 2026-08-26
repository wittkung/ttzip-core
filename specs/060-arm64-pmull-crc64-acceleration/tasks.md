# Tasks: 060-arm64-pmull-crc64-acceleration

**Feature Name**: ARM64 PMULL 硬件级 CRC64 (ECMA-182) 加速引擎接入  
**Status**: Completed  
**Created**: 2026-08-17  
**Parent Plan**: [plan.md](./plan.md)

---

## Dependencies & Execution Strategy
- **Strategy**: MVP first (T001 -> T002 -> T003 -> T004 -> T005 -> T006 -> T007 -> T008)
- **Dependency Flow**:
  - Foundational C Header & Modulemap -> Core PMULL C Implementation -> Swift Wrapper -> Test Suite & Performance Floor Verification

---

## Phase 1: Setup & Foundational Infrastructure

- [x] T001 [P] Create C native header `Sources/CTTZipBridge/include/ttzip_crc64.h` with `ttzip_crc64` and `ttzip_crc64_pmull` declarations
- [x] T002 [P] Register `ttzip_crc64.h` in `Sources/CTTZipBridge/include/module.modulemap` and include in `Sources/CTTZipBridge/include/CTTZipBridge.h`

---

## Phase 2: User Story 1 (US1) - ARM64 PMULL CRC64 4-Way Hardware Folding Engine

- [x] T003 [US1] Implement 4-way 64-byte vector folding, 16-byte convergence folding, and Barrett reduction in `Sources/CTTZipBridge/ttzip_crc64.c`
- [x] T004 [US1] Verify Barrett reduction constants `fold512`, `fold128`, `mu_p` and tail masks `vmasks_64` in `Sources/CTTZipBridge/ttzip_crc64.c`

---

## Phase 3: User Story 2 (US2) - Swift Zero-Copy Wrapper & Public API

- [x] T005 [US2] Implement `@frozen public enum CRC64Checksum` with zero-copy `Data` and buffer methods in `Sources/TTZipCore/Crypto/CRC64Checksum.swift`

---

## Phase 4: User Story 3 (US3) - Scalar Fallback & Extreme Boundary Hardening

- [x] T006 [US3] Implement Slicing-by-8 scalar fallback table and 0-byte/NULL boundary handling in `Sources/CTTZipBridge/ttzip_crc64.c`

---

## Phase 5: Polish & Quality Gates

- [x] T007 Create comprehensive test suite `Tests/TTZipTests/CRC64HardwareTests.swift` covering Golden Vector, 0-256 byte differential testing, unaligned slices, and >= 30,000 MB/s throughput gate
- [x] T008 Run `swift test --filter CRC64HardwareTests` and verify all-green test passes
