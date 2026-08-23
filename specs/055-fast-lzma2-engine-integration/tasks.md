# Tasks: Fast-LZMA2 Multi-Threaded Engine Integration

**Feature Directory**: `specs/055-fast-lzma2-engine-integration`

**Created**: 2026-08-17

**Status**: Ready for Implementation

---

## Phase 1: Setup & In-Tree Engine Source Integration

- [x] T001 [US3] Ingest and place fast-lzma2 core C sources in `Sources/CTTZipBridge/fast-lzma2/`
- [x] T002 [US3] Update `Package.swift` to add `.headerSearchPath("fast-lzma2")` under `CTTZipBridge` target in `Package.swift`

---

## Phase 2: C Bridge & Hybrid Dispatcher

- [x] T003 [P] [US1] Declare `ttzip_fl2_compress_block` and `ttzip_fl2_stream_*` C API in `Sources/CTTZipBridge/include/ttzip_fl2_lzma2.h`
- [x] T004 [P] [US1] Implement C bridge adapter, 16KB page alignment, and Magic lifecycle in `Sources/CTTZipBridge/ttzip_fl2_bridge.c`
- [x] T005 [P] [US1] Export `ttzip_fl2_lzma2.h` in Clang module definition `Sources/CTTZipBridge/include/module.modulemap`

---

## Phase 3: Swift Core Engine & Hybrid Routing

- [x] T006 [P] [US2] Implement `SevenZipLZMA2HybridStrategy` in `Sources/TTZipCore/SevenZip/SevenZipLZMA2HybridStrategy.swift`
- [x] T007 [P] [US2] Update `SevenZipCAdapter` to invoke Fast-LZMA2 with L1 NEON fallback in `Sources/TTZipCore/Adapters/SevenZipCAdapter.swift`
- [x] T008 [P] [US1] Update 7Z encoding pipeline to consume multithreaded FL2 engine in `Sources/TTZipCore/SevenZip/NativeSevenZipEngine.swift`

---

## Phase 4: Tests & Performance Regression Gates

- [x] T009 [P] [US4] Implement unit tests for FL2 block/stream compression, memory bounds, and differential decompression in `Tests/TTZipTests/FastLZMA2Tests.swift`
- [x] T010 [P] [US1] Add Level 5 high-compression throughput gate test in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- [x] T011 [US4] Execute full matrix regression benchmark `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and audit with `python3 scripts/audit_performance_regression.py`

---

## Dependencies & Parallel Execution Constraints

```
Phase 1 (T001, T002)
       │
       ▼
Phase 2 (T003, T004, T005) [P]
       │
       ▼
Phase 3 (T006, T007, T008) [P]
       │
       ▼
Phase 4 (T009, T010) [P] ──→ T011 (Final Audit)
```
