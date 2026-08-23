# Tasks: 工业级极端边界、安全漏洞与元数据测试体系 (Feature 163)

**Feature ID**: `163-industrial-testing-and-fuzzing-suite`  
**Created**: 2026-08-20  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Foundational Fixtures

- [x] T001 Populate `tests/fixtures/cve/` with malformed test payloads and boundary test cases
- [x] T002 Populate `tests/fixtures/compat/` with legacy format test archives (PKZIP, GNU Tar longlink)
- [x] T003 Create `tests/fuzz/ttzip_archive.dict` container token dictionary

---

## Phase 2: User Story 1 (P1) - CVE Defense & Malformed Bitstream Test Suite

- [x] T004 [P] [US1] Implement programmatic Huffman/window corruption tests in `tests/c/test_cve_regressions.c`
- [x] T005 [P] [US1] Implement CVE fixture rejection tests in `tests/c/test_cve_regressions.c`

---

## Phase 3: User Story 2 (P2) - Historical & Non-Standard Archive Backward Compatibility

- [x] T006 [P] [US2] Implement legacy ZIP format extraction tests in `tests/c/test_compat_archives.c`
- [x] T007 [P] [US2] Implement GNU Tar longlink and Base256 UID tests in `tests/c/test_compat_archives.c`

---

## Phase 4: User Story 3 (P3) - macOS APFS Extended Attributes & Sparse Files

- [x] T008 [P] [US3] Implement `com.apple.quarantine` and custom `xattr` roundtrip in `tests/c/test_fs_metadata.c`
- [x] T009 [P] [US3] Implement 1GB APFS sparse file hole preservation test in `tests/c/test_fs_metadata.c`

---

## Phase 5: User Story 4 (P4) - Clang LibFuzzer In-Memory Harness & Build Integration

- [x] T010 [US4] Implement zero-leak `LLVMFuzzerTestOneInput` in `tests/fuzz/fuzz_extract_engine.c`
- [x] T011 [US4] Register `ttzip_fuzzer` target and new test suites in `CMakeLists.txt` and `tests/c/test_main.c`

---

## Phase 6: Integration, Verification & 5-Gate Gatekeeping

- [x] T012 [US1] Run `./build/ttzip_c_test_runner all` verifying all 24 suites pass
- [x] T013 [US4] Run `./build/ttzip_fuzzer -max_total_time=3` fuzzing trial
- [x] T014 [US1] Run `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json` verifying <5s all green
