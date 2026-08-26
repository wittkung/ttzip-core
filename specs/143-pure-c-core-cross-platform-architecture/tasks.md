# Tasks: Pure C11 Core Engine (`libttzip`) & Cross-Platform Architecture

**Input**: Design documents from `/specs/143-pure-c-core-cross-platform-architecture/` (`spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`).  
**Prerequisites**: All Phase 0 & Phase 1 design artifacts complete and verified.  
**Organization**: Tasks are grouped by user story with explicit `[P]` parallelism markers.

---

## Phase 1: Portable Thread Pool & File System Foundation

- [x] T001 Define cross-platform thread pool API `ttzip_threadpool.h` in `Sources/CTTZipBridge/include/ttzip_threadpool.h`
- [x] T002 Implement POSIX `pthread` and Win32 `ThreadPool` backends in `Sources/CTTZipBridge/ttzip_threadpool.c`
- [x] T003 [P] Define cross-platform file system abstraction `ttzip_fs.h` in `Sources/CTTZipBridge/include/ttzip_fs.h`
- [x] T004 [P] Implement POSIX (`opendir`/`lstat`/`mmap`) backend in `Sources/CTTZipBridge/ttzip_fs.c`
- [x] T005 [P] Implement Win32 (`FindFirstFileW`/`MapViewOfFile`/`\\?\`) backend in `Sources/CTTZipBridge/ttzip_fs.c`

---

## Phase 2: Dual-ISA SIMD Acceleration & Hardware Vector Parity

- [x] T006 [P] Implement x86_64 PCLMULQDQ CRC64 kernel in `Sources/CTTZipBridge/ttzip_crc64.c`
- [ ] T007 [P] Implement x86_64 SSE4.2 CRC32 kernel in `Sources/CTTZipBridge/hardware/ttzip_crc32_x86_sse42.c`
- [ ] T008 [P] Implement x86_64 AVX2 Adler-32 kernel in `Sources/CTTZipBridge/hardware/ttzip_adler32_x86_avx2.c`
- [x] T009 Wire Dual-ISA runtime dispatch in `Sources/CTTZipBridge/ttzip_crc64.c`

---

## Phase 3: User Story 1 - GCD Elimination & Pure C Parallel Sinking (Priority: P1) 🎯 MVP

- [ ] T010 [P] [US1] Replace GCD in `Sources/CTTZipBridge/CTTZipBridge_ZipChunkedStream.c` with `ttzip_threadpool`
- [ ] T011 [P] [US1] Replace GCD in `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c` with `ttzip_threadpool`
- [ ] T012 [P] [US1] Replace GCD in `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c` with `ttzip_threadpool`
- [ ] T013 [P] [US1] Replace GCD in `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c` with `ttzip_threadpool`
- [ ] T014 [P] [US1] Replace GCD in `Sources/CTTZipBridge/CTTZipExtract.c` with `ttzip_threadpool`
- [ ] T015 [US1] Validate full C bridge build without `<dispatch/dispatch.h>` via `CMakeLists.txt`

---

## Phase 4: User Story 2 - Container Sinking & Public C API (Priority: P2)

- [ ] T016 [P] [US2] Implement standalone ZIP/Zip64 writer in `Sources/CTTZipBridge/containers/ttzip_zip.c`
- [ ] T017 [P] [US2] Implement standalone 7Z solid writer in `Sources/CTTZipBridge/containers/ttzip_7z.c`
- [ ] T018 [P] [US2] Implement standalone TAR PAX stream writer in `Sources/CTTZipBridge/containers/ttzip_tar.c`
- [ ] T019 [US2] Define and export versioned public C ABI `include/ttzip_api.h` and implement `core/ttzip_archive.c`

---

## Phase 5: User Story 3 - Swift Thin Shell & Full Verification (Priority: P3)

- [ ] T020 [P] [US3] Refactor Swift `ArchiveWriter.swift` to delegate directly to `ttzip_archive_create()`
- [ ] T021 [P] [US3] Refactor Swift `ArchiveExtractor.swift` to delegate directly to `ttzip_archive_extract()`
- [ ] T022 [US3] Execute full test matrix via `swift test --filter AllFormatsAndAdvancedParametersMatrixTests`
- [ ] T023 [US3] Update documentation in `docs/architecture/libttzip_pure_c_cross_platform_blueprint.md`
