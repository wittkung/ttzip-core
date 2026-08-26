# Tasks: Comprehensive CPI & Microarchitectural Optimization Audit

**Feature ID**: `160-cpi-microarchitecture-optimization-audit`  
**Created**: 2026-08-20  
**Status**: In Progress  

---

## Phase 1: Setup & Foundational Telemetry

- [x] T001 Enhance C benchmark harness with frequency detection and CPB / IPC calculation helpers in `tests/c/ttzip_benchmark_harness.h`
- [x] T002 Integrate CPB, GB/s, and IPC output into checksum benchmark suite in `tests/c/bench_checksums.c`
- [x] T003 Integrate CPB and IPC telemetry into codec benchmark suite in `tests/c/bench_codecs.c`

---

## Phase 2: User Story 1 (P1) - Hardware Disassembly Audit & Assembly Proof

- [x] T004 [P] [US1] Extract machine disassembly of `CTTZipCRC32Neon.c` (PMULL 12-way/4-way folding) and verify zero stack spills and 12-way independent accumulator allocation using `otool -tv` / `clang -S`
- [x] T005 [P] [US1] Extract machine disassembly of `CTTZipAdler32Neon.c` and verify NEON 64B unrolling and dot product scheduling using `otool -tv` / `clang -S`
- [x] T006 [P] [US1] Extract machine disassembly of `Sources/CTTZipBridge/native_deflate/` and `fast-lzma2/` to inspect inner loop instruction count and register allocation

---

## Phase 3: User Story 2 (P2) - Zlib-ng Upstream PR Scheme Alignment & Memory Hardening

- [x] T007 [US2] Apply verified upstream zlib-ng PR scheme (2x-Unrolled NEON + VORR + Single-UMAXV) to `Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h` strictly per Spec 110/118
- [x] T008 [US2] Disassemble `CTTZipNEONMatchFinder.h` with `otool -tv` to verify removal of intermediate `fmov`/`umov` in 32B loop and 0 stack spills
- [x] T009 [US2] Add 64-byte L1D cache line alignment `__attribute__((aligned(64)))` to `ttzip_prefetch_slot_t` in `Sources/CTTZipBridge/include/CTTZipPrefetchPipeline.h` to prevent multi-core false sharing

---

## Phase 4: User Story 3 (P3) - Comprehensive Verification, CI & Audit Report

- [x] T010 [US3] Build and run C benchmark runner `./build/tests/c/ttzip_benchmark_runner --checksums --codecs` to collect full CPB / IPC telemetry
- [x] T011 [US3] Run C unit test runner `./build/tests/c/ttzip_test_runner` under AddressSanitizer and verify 100% pass rate with 0 memory leaks
- [x] T012 [US3] Run full 5-stage local CI pipeline `./scripts/local-ci.sh` ensuring 0 compiler warnings and 100% green status
- [x] T013 [US3] Generate comprehensive CPI optimization & disassembly audit report in `specs/160-cpi-microarchitecture-optimization-audit/cpi_audit_report.md`
