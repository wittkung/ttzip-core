# Tasks: Libdeflate Architecture Integration & Performance Exploitation

**Feature**: [062-libdeflate-architecture-integration](spec.md) | **Plan**: [plan.md](plan.md)

---

## Phase 1: Core C Bridge Hardening

- [x] T001 [P] [US1] Modernize streaming state and fix CRC-32 / Adler-32 incremental updates in `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- [x] T002 [P] [US1] Add 7Z Method ID 0x040108 direct `ttzip_libdeflate_decompress` routing in `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`

## Phase 2: Swift Engine & Test Suite Integration

- [x] T003 [US2] Expand multi-level roundtrip and chunked streaming test matrix in `Tests/TTZipTests/LibdeflateAcceleratorTests.swift`
- [x] T004 [US3] Execute full test suite (`swift test`) and performance gate (`swift test --filter LibdeflateAcceleratorTests`) to assert zero regression
