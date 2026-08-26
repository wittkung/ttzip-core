# Tasks: Full-Matrix libdeflate Architecture

**Feature Directory**: `specs/053-chunked-deflate-compressor`
**Created**: 2026-08-17
**Status**: Completed

---

## Phase 1: Setup & Preconditions

- [x] T001 [P] Create PAL header with cross-platform definitions in `Sources/CTTZipBridge/include/CTTZipPlatform.h`
- [x] T002 [P] Create chunked DEFLATE streaming header in `Sources/CTTZipBridge/include/CTTZipBridge_ZipChunkedStream.h`
- [x] T003 [P] Update `Sources/CTTZipBridge/CTTZipStreamCoder.c` to use `TTZIP_THREAD_LOCAL` from `CTTZipPlatform.h`

---

## Phase 2: User Story 1 (P0) - Large File Chunked Streaming Core

- [x] T004 [US1] Implement 1MB chunked multi-threaded DEFLATE streaming compressor with bounded memory pool in `Sources/CTTZipBridge/CTTZipBridge_ZipChunkedStream.c`
- [x] T005 [US1] Implement RFC 1951 empty stored block synchronization (`0x0000FFFF`) and BFINAL bit management in `Sources/CTTZipBridge/CTTZipBridge_ZipChunkedStream.c`
- [x] T006 [US1] Implement stream backpressure control and incremental hardware CRC-32 tracking in `Sources/CTTZipBridge/CTTZipBridge_ZipChunkedStream.c`
- [x] T007 [US1] Implement Swift wrapper `ChunkedDeflateStreamWriter` with adaptive routing in `Sources/TTZipCore/Zip/ChunkedDeflateStreamWriter.swift`
- [x] T008 [US1] Update `LibdeflateCAdapter` to bridge chunked streaming methods in `Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift`

---

## Phase 3: User Story 2 (P1) - libdeflate Upstream Upgrade & Build Automation

- [x] T009 [P] [US2] Create automated build script `scripts/build_libdeflate.sh` for Universal 2 (`arm64` + `x86_64`) static library compilation
- [x] T010 [US2] Upgrade `Vendor/include/libdeflate.h` and sync with `Vendor/TTZipVendor.xcframework`
- [x] T011 [US2] Update `ACKNOWLEDGEMENTS.md` to reflect `libdeflate v1.22` and MIT license compliance

---

## Phase 4: User Story 3 (P2) - Windows MSVC PAL & CMake Cross-Platform Matrix

- [x] T012 [P] [US3] Create root `CMakeLists.txt` supporting Windows MSVC and macOS compilation of `libdeflate` and `CTTZipBridge`
- [x] T013 [P] [US3] Add cross-platform Windows MSVC compatibility to `Sources/CTTZipBridge/include/ttzip_platform.h`
- [x] T014 [US3] Verify C bridge files compile with zero warnings under MSVC strict flags (`/W4 /O2 /MD`)
- [x] T015 [US3] Create schema verification test for `PlatformBuildManifest` in `Tests/TTZipTests/CrossPlatformManifestTests.swift`

---

## Phase 5: User Story 4 (P2) - Oracle Differential & Performance Gate

- [x] T016 [P] [US4] Implement unit and memory RSS verification test suite in `Tests/TTZipTests/ChunkedDeflateStreamingTests.swift`
- [x] T017 [US4] Execute differential oracle test comparing output with `/usr/bin/unzip` and 7-Zip
- [x] T018 [US4] Run performance regression suite `swift test --filter XCTestPerformanceMeasureTests` to assert zero regression on standard paths

---

## Verification Summary

- **Unit & Chunked Tests**: Executed 3 tests in `ChunkedDeflateStreamingTests`, 0 failures.
- **Cross-Platform Schema Tests**: Executed 2 tests in `CrossPlatformManifestTests`, 0 failures.
- **Performance Floors**: 13/13 performance floors passed with zero regression.
