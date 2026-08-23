# Tasks: Single-Core DEFLATE Engine Surpassing libdeflate

**Feature Branch**: `113-single-core-surpass-libdeflate`
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish C & Swift bridge headers and benchmark harness.

- [x] T001 [P] Create unified C header declarations in `Sources/CTTZipBridge/include/CTTZipDeflateEngine.h`
- [x] T002 [P] Create SIMD match finder definitions in `Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h`
- [x] T003 Configure test harness and dataset fixtures in `Tests/TTZipTests/SingleCoreDeflatePkTests.swift`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core hardware-level algorithms and data structures needed by all user stories.

- [x] T004 Implement 4-way NEON parallel hash vector swizzler (`ttzip_neon_hash4_probe`) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`
- [x] T005 [P] Implement 2-Tier SWAR match length evaluator (`ttzip_hybrid_match_len_neon`) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`
- [x] T006 [P] Implement 12-bit dual-symbol direct Huffman table structures in `Sources/CTTZipBridge/native_inflate/ttzip_inflate_dual_lut.h`
- [x] T007 [P] Implement NEON in-register $D < 16$ match replicator with static permutation table in `Sources/CTTZipBridge/native_inflate/ttzip_inflate_neon_replicate.h`

---

## Phase 3: User Story 1 - Single-Core Ultra-High-Throughput Compression (Priority: P1) 🎯 MVP

**Goal**: Single-core DEFLATE compression exceeding libdeflate Level 1 by $\ge 10.0\%$ and Level 6 by $\ge 5.0\%$.

**Independent Test**: `swift test --filter SingleCoreDeflatePkTests` executes and passes differential throughput against libdeflate.

### Implementation for User Story 1

- [x] T008 [P] [US1] Implement Dual/Quad-Token tree-parallel bitstream packer in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_bitstream.h`
- [x] T009 [P] [US1] Implement 8-Archetype pre-compiled dynamic Huffman codebook cluster and NEON dot-product classifier in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c`
- [x] T010 [US1] Implement single-core Level 1 fast-greedy compression pipeline in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`
- [x] T011 [US1] Implement adaptive entropy skip bypass in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`
- [x] T012 [US1] Implement single-core compression dispatcher in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [x] T013 [US1] Add Swift benchmark wrapper and differential tests in `Tests/TTZipTests/SingleCoreDeflatePkTests.swift`

---

## Phase 4: User Story 2 - Single-Core Ultra-Fast Decompression (Priority: P2)

**Goal**: Single-core DEFLATE decompression exceeding libdeflate by $\ge 10.0\%$.

**Independent Test**: `swift test --filter SingleCoreDecompressPkTests` verifies extraction throughput and correctness.

### Implementation for User Story 2

- [x] T014 [P] [US2] Implement 12-bit dual-symbol direct Huffman table builder in `Sources/CTTZipBridge/native_inflate/ttzip_inflate_dual_lut.h`
- [x] T015 [US2] Implement dual-symbol fast-loop decompression stream consumer in `Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c`
- [x] T016 [US2] Integrate NEON in-register small-distance match replicator into decompression loop in `Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c`
- [x] T017 [US2] Implement bounded error checking and buffer safety guards in `Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c`
- [x] T018 [US2] Add Swift decompression benchmark tests in `Tests/TTZipTests/SingleCoreDecompressPkTests.swift`

---

## Phase 5: User Story 3 - Deterministic Bit-Stream Verification & Cross-Ecosystem Oracle Compatibility (Priority: P3)

**Goal**: 100% round-trip verification and bit-exact oracle validation with `/usr/bin/unzip`, `/usr/bin/gzip`, and `zlib`.

**Independent Test**: `swift test --filter SingleCoreDeflateOracleTests` verifies 1,000+ randomized payloads with 0 errors.

### Implementation for User Story 3

- [x] T019 [P] [US3] Implement cross-tool round-trip testing harness in `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`
- [x] T020 [US3] Add randomized edge-case corpus generator (0B, micro, all-0xFF, random, repetitive) in `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`
- [x] T021 [US3] Validate SHA-256 byte-exact parity between engine-generated outputs and system unzip/gzip in `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Performance hardening, memory safety audit, and quickstart validation.

- [x] T022 [P] Verify zero intermediate heap allocations (`malloc`, `Data(count:)`) across all compression/decompression hot paths in `Sources/CTTZipBridge/`
- [x] T023 Run end-to-end `quickstart.md` validation suite via `swift test`
