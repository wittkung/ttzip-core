# Tasks: 145-pure-c-container-framing-and-cli-engine

## Phase 1: Top-Level Public C Archive Orchestrator (US1 & US2)

- [x] T001 [US2] Create ttzip_archive.h declaring top-level archive operations in Sources/CTTZipBridge/include/ttzip_archive.h
- [x] T002 [US2] Implement ttzip_archive.c with pure C11 file iteration, compression, and extraction in Sources/CTTZipBridge/ttzip_archive.c
- [x] T003 [US1] Update ttzip_api.h to include ttzip_archive.h in Sources/CTTZipBridge/include/ttzip_api.h

## Phase 2: Standalone Pure C CLI Tool (US3)

- [x] T004 [US3] Implement cli/main.c with subcommands (-c, -x, -l, -t, -b, --version) in cli/main.c
- [x] T005 [US3] Update CMakeLists.txt to add ttzip-cli executable target in CMakeLists.txt

## Phase 3: x86_64 SIMD & CPUID Runtime Probing (US4)

- [x] T006 [US4] Update ttzip_platform_detect.c to probe and export x86_64 SSE4.2, AVX2, AVX-512, PCLMULQDQ, and AES-NI in Sources/CTTZipBridge/ttzip_platform_detect.c

## Phase 4: Local CI & End-to-End Verification (US1 - US4)

- [x] T007 [US3] Update scripts/local-ci.sh to compile and execute ttzip-cli --benchmark
- [x] T008 [US1] Run full local CI test matrix and verify 0 errors
