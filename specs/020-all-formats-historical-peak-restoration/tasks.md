# Tasks Breakdown: All-Formats Historical Peak Restoration (Feature 020)

**Feature**: 020 All-Formats Historical Peak Restoration & Zero-Gap Performance Alignment  
**Directory**: `specs/020-all-formats-historical-peak-restoration/`  
**Status**: Ready for Implementation  

---

## Phase 1: 7z 500MB 大文件与小文件分块极速直通

- [x] T001 [P] [US1] Implement zero-block encode_zero_chunk_2mb fast path in Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c
- [x] T002 [P] [US1] Optimize adaptive micro-chunking and L1 cache-resident dictionary in Sources/CTTZipBridge/ttzip_lzma2_enc_native.c
- [x] T003 [P] [US1] Solidify TAR.XZ, LZIP, LZ4 multi-thread stream filter and 8MB buffer in Sources/CTTZipBridge/ttzip_tar_native.c

---

## Phase 2: 海量小文件 ZIP 批处理与 128MB Arena 内存级单次落盘

- [x] T004 [P] [US2] Enhance payload arena to 128MB and aggregate disk flush in Sources/CTTZipBridge/CTTZipBridge_ZipWriterCore.c
- [x] T005 [P] [US2] Optimize batch chunked compression for small files in Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c

---

## Phase 3: 全格式基准回归与零倒退自动化审计

- [x] T006 [US1] Run release benchmark suite TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
- [x] T007 [US1] Audit latest benchmark report with python3 scripts/audit_performance_regression.py
- [x] T008 [US3] Verify XCTestPerformanceMeasureTests and FrontendPerformanceGateTests throughput floors
- [x] T009 [US3] Execute full 591+ regression test suite with 0 warnings
- [x] T010 [US3] Git commit and push all optimizations to origin/main
