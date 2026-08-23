# Task Breakdown: libarchive POSIX / Darwin Disk Space Pre-allocation (`ARCHIVE_EXTRACT_PREALLOCATE`)

## Phase 2: Setup & Build System Probes

- [x] T001 [P] [US1] Define `ARCHIVE_EXTRACT_PREALLOCATE (0x80000)` in `Vendor/libarchive-upstream/libarchive/archive.h`
- [x] T002 [P] [US1] Add `HAVE_POSIX_FALLOCATE` and `HAVE_F_PREALLOCATE` probes in `Vendor/libarchive-upstream/CMakeLists.txt` and `Vendor/libarchive-upstream/configure.ac`

## Phase 3: Core Implementation (User Story 1 & 2)

- [x] T003 [US1] Implement `preallocate_file()` helper and invocation in `Vendor/libarchive-upstream/libarchive/archive_write_disk_posix.c`
- [x] T004 [US2] Implement Darwin two-tier cascade and POSIX error classification in `Vendor/libarchive-upstream/libarchive/archive_write_disk_posix.c`

## Phase 4: Unit Testing & Registration (User Story 3)

- [x] T005 [P] [US3] Create `test_write_disk_preallocate.c` with 4 test scenarios in `Vendor/libarchive-upstream/libarchive/test/test_write_disk_preallocate.c`
- [x] T006 [P] [US3] Register `test_write_disk_preallocate.c` in `Vendor/libarchive-upstream/libarchive/test/CMakeLists.txt` and `Vendor/libarchive-upstream/Makefile.am`

## Phase 5: Verification & Quality Gates

- [x] T007 [P] [US1] Run `swift build` and `swift test` in TTZip root to verify build and zero regression
- [x] T008 [P] [US1] Run `git diff --check` and BSD KNF formatting verification in `Vendor/libarchive-upstream/`
