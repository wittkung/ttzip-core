# Tasks: zlib-ng Streaming Fallback Engine & Cross-Platform Hardware Acceleration

**Feature**: `076-zlib-ng-streaming-fallback`  
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)  
**Status**: Completed  

---

## Phase 1: Setup & Dependency Verification

- [x] T001 Verify zlib-ng CMake build configuration (`-DZLIB_COMPAT=ON`, `-DWITH_NATIVE_INSTRUCTIONS=ON`, `-DDYNAMIC_CPU_DISPATCH=ON`) in `scripts/build_zlib_ng.sh` and `CMakeLists.txt`
- [x] T002 Verify Universal 2 static library bundling in `Vendor/TTZipVendor.xcframework` and `Vendor/libTTZipVendor.a`

---

## Phase 2: Foundational Infrastructure

- [x] T003 [P] Update dynamic hardware SIMD capability detection (ARM CRC32, AVX-512, PCLMUL) in `Sources/CTTZipBridge/ttzip_platform_detect.c` and `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`
- [x] T004 [P] Harden single-session zero-lock state machine lifecycle and magic invariant (`TTZIP_DEFLATE_STREAM_MAGIC`) in `Sources/CTTZipBridge/CTTZipStreamCoder.c`

---

## Phase 3: User Story 1 (Priority: P1) - State-Machine Streaming Deflate Acceleration

**Goal**: Deliver high-throughput incremental chunked Deflate compression and decompression (RFC 1950, RFC 1951, RFC 1952) via hardware-accelerated `zlib-ng` fallback.  
**Independent Test**: Execute `swift test --filter DeflateStreamCoderTests` and `swift test --filter DeflateStreamingPipelineTests`.

- [x] T005 [P] [US1] Implement multi-format RFC 1950/1951/1952 and flush mode unit tests in `Tests/TTZipTests/DeflateStreamCoderTests.swift`
- [x] T006 [P] [US1] Implement async stream pipeline and concurrent TaskGroup tests in `Tests/TTZipTests/DeflateStreamingPipelineTests.swift`
- [x] T007 [US1] Implement `DeflateStreamCompressor`, `DeflateStreamDecompressor`, and `DeflateStreamEngine` async sequences in `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift`
- [x] T008 [US1] Implement C streaming bridge routines (`ttzip_deflate_stream_init`, `ttzip_deflate_stream_process`, `ttzip_inflate_stream_init`, `ttzip_inflate_stream_process`, `ttzip_deflate_stream_free`) in `Sources/CTTZipBridge/CTTZipStreamCoder.c`

---

## Phase 4: User Story 2 (Priority: P2) - Libarchive & Global Stream Filter Modernization

**Goal**: Ensure `libarchive` Deflate and GZIP archive filters automatically bind to `zlib-ng` with hardware SIMD.  
**Independent Test**: Verify `Package.swift` builds cleanly without `.linkedLibrary("z")` and archive extraction tests pass.

- [x] T009 [P] [US2] Verify `Package.swift` linker settings and static binding to `TTZipVendor` in `Package.swift`
- [x] T010 [US2] Validate `ArchiveExtractor` GZIP and TAR.GZ stream filter integration in `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift`

---

## Phase 5: User Story 3 (Priority: P3) - Strict Tier Isolation & Zero Regression for Fast-Path

**Goal**: Guarantee Tier 1 `libdeflate` whole-buffer operations remain 100% untouched and all historical peak throughput floors are strictly preserved.  
**Independent Test**: Execute `swift test --filter XCTestPerformanceMeasureTests`.

- [x] T011 [P] [US3] Execute whole-buffer performance measure tests in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift` and assert zero throughput regression ($\Delta \ge 0.0\%$)
- [x] T012 [US3] Verify `LibdeflateAccelerator.swift` and `Sources/TTZipCore/Zip/ZipParallelWriter.swift` Tier 1 fast-path bypass retention

---

## Phase 6: Polish & Standards Compliance

- [x] T013 Verify contract consistency against `specs/076-zlib-ng-streaming-fallback/contracts/deflate-stream-coder-contract.json` and `specs/076-zlib-ng-streaming-fallback/contracts/hardware-capabilities-contract.json`
- [x] T014 Run full regression suite via `swift test` and confirm all 525+ tests pass
