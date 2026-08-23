# Tasks: 147-full-122-files-c-migration-and-swift-slimming

## Phase 1: Cluster 1 - Split Volume & In-Place Mutation Sinking (US1)

- [x] T001 [US1] Create ttzip_split.h and ttzip_split.c for zero-copy multi-volume splitting in Sources/CTTZipBridge/
- [x] T002 [US1] Create ttzip_inplace.h and ttzip_inplace.c for in-place archive mutation and entry appending in Sources/CTTZipBridge/

## Phase 2: Cluster 2 - Security, FEC & Credential Scrubbing (US2)

- [x] T003 [US2] Create ttzip_security.h and ttzip_security.c providing DSE memory zeroing and Reed-Solomon FEC in Sources/CTTZipBridge/

## Phase 3: Cluster 3 & 4 - Swift Thin-Binding & Decoupling (US3 & US4)

- [x] T004 [US3] Create Sources/TTZipCore/Adapters/NativeMicrokernelBridge.swift unifying Swift thin-calls to ttzip_archive_create, ttzip_archive_extract, and ttzip_archive_extract_entry_mem
- [x] T005 [US4] Update Sources/CTTZipBridge/include/ttzip_api.h with ttzip_split.h, ttzip_inplace.h, ttzip_security.h

## Phase 4: Full Local CI Verification & Audit (US1 - US4)

- [x] T006 [US4] Update cli/main.c with security & split-stream benchmarks
- [x] T007 [US1] Run scripts/local-ci.sh and ensure CMake build + 76 Swift tests pass 100% green
