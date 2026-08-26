# Tasks: Unified SOTA Codec Engine & Multi-Core Architecture

**Input**: Design documents from `/specs/142-unified-sota-codec-architecture/` (`spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`).  
**Prerequisites**: All Phase 0 & Phase 1 design artifacts complete and verified.  
**Organization**: Tasks are grouped by user story with explicit `[P]` parallelism markers.

---

## Phase 1: Setup & Codec VTable ABI Definition

- [ ] T001 Define unified C ABI function pointer table `ttzip_codec_ops_t` in `Sources/CTTZipBridge/include/ttzip_codec.h`
- [ ] T002 [P] Implement `libdeflate` single-core SOTA wrapper in `Sources/CTTZipBridge/codecs/ttzip_codec_deflate.c`
- [ ] T003 [P] Implement `fast-lzma2` single-core SOTA wrapper in `Sources/CTTZipBridge/codecs/ttzip_codec_lzma2.c`
- [ ] T004 [P] Implement `libzstd` single-core SOTA wrapper in `Sources/CTTZipBridge/codecs/ttzip_codec_zstd.c`

---

## Phase 2: Foundational Multi-Core Parallel Engine & Dictionary Overlap

- [ ] T005 Implement universal multi-core parallel scheduler in `Sources/CTTZipBridge/parallel/ttzip_parallel_engine.c`
- [ ] T006 [P] Implement zero-copy sliding ring dictionary buffer in `Sources/CTTZipBridge/parallel/ttzip_dict_overlap.c`
- [ ] T007 [P] Implement format-aware bitstream sequencer (BFINAL management) in `Sources/CTTZipBridge/parallel/ttzip_bitstream_seq.c`
- [ ] T008 [P] Integrate memory-page flyweight pooling in `Sources/CTTZipBridge/CTTZipSysAlloc.c`

---

## Phase 3: User Story 1 - SOTA Single-Core & Multi-Core Integration (Priority: P1) 🎯 MVP

- [ ] T009 [P] [US1] Wire `ttzip_codec_deflate` to multi-core scheduler in `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c`
- [ ] T010 [P] [US1] Wire `ttzip_codec_lzma2` to multi-core scheduler in `Sources/CTTZipBridge/CTTZipBridge_7zParallel.c`
- [ ] T011 [P] [US1] Wire `ttzip_codec_zstd` to multi-core scheduler in `Sources/CTTZipBridge/CTTZipBridge_Zstd.c`
- [ ] T012 [US1] Validate single-core speedup and multi-core scaling in `Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift`

---

## Phase 4: User Story 2 - Dual-Track Adaptive Scheduling (Priority: P2)

- [ ] T013 [P] [US2] Implement small-file vs large-file dual-track routing in `Sources/TTZipCore/Engines/ArchiveWriter.swift`
- [ ] T014 [P] [US2] Implement P-core vs E-core asymmetric chunk sizing in `Sources/TTZipCore/AppleSiliconTuner.swift`
- [ ] T015 [US2] Validate memory envelope invariant under 50GB payload in `Tests/TTZipTests/BatchSmallFileMemoryTests.swift`

---

## Phase 5: User Story 3 - Decoupled Container Framing & Standard Compliance (Priority: P3)

- [ ] T016 [P] [US3] Decouple ZIP container header builder from codecs in `Sources/TTZipCore/Zip/ZipHeaderBuilder.swift`
- [ ] T017 [P] [US3] Decouple 7Z solid folder builder from codecs in `Sources/TTZipCore/SevenZip/SevenZipWriter.swift`
- [ ] T018 [P] [US3] Decouple TAR PAX stream pipeline from codecs in `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`
- [ ] T019 [US3] Validate standard oracle compatibility via external `/usr/bin/unzip` and `/usr/bin/tar` in `Tests/TTZipTests/ArchiveStandardsComplianceTests.swift`

---

## Phase 6: Polish & Architecture Convergence

- [ ] T020 [P] Update software architecture documentation in `ARCHITECTURE.md`
- [ ] T021 [P] Update format matrix documentation in `docs/formats/format-support-matrix.md`
- [ ] T022 Execute end-to-end full test suite via `swift test`
