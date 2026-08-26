# Tasks: 146-full-c-engine-sinking-and-frontend-decoupling

## Phase 1: Pure C TAR & 7Z Container Sinking (US1)

- [x] T001 [US1] Create ttzip_tar_container.h and ttzip_tar_container.c in Sources/CTTZipBridge/
- [x] T002 [US1] Create ttzip_7z_container.h and ttzip_7z_container.c in Sources/CTTZipBridge/
- [x] T003 [US1] Integrate TAR & 7Z creation routing into ttzip_archive_create in Sources/CTTZipBridge/ttzip_archive.c

## Phase 2: Magic Sniffing & Natural Numeric Sorting (US3)

- [x] T004 [US3] Create ttzip_magic_sniff.h and ttzip_magic_sniff.c in Sources/CTTZipBridge/
- [x] T005 [US3] Create ttzip_strnatcmp.h and ttzip_strnatcmp.c in Sources/CTTZipBridge/

## Phase 3: In-Memory Entry Extraction & Fast Tree (US2 & US4)

- [x] T006 [US2] Implement ttzip_archive_extract_entry_mem in Sources/CTTZipBridge/ttzip_archive.c
- [x] T007 [US4] Create ttzip_archive_tree.h and ttzip_archive_tree.c in Sources/CTTZipBridge/
- [x] T008 [US4] Implement ttzip_archive_inspect in Sources/CTTZipBridge/ttzip_archive.c

## Phase 4: Public ABI & CLI Integration (US1 - US4)

- [x] T009 [US1] Update ttzip_api.h to include all new subsystems in Sources/CTTZipBridge/include/ttzip_api.h
- [x] T010 [US3] Update cli/main.c to add benchmarks for Magic Sniffing, Natural Sorting, and Tree search
- [x] T011 [US1] Run scripts/local-ci.sh to ensure CMake and Swift tests pass 100% green
