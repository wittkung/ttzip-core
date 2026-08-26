# Feature Specification: 146-full-c-engine-sinking-and-frontend-decoupling

## 1. Executive Summary & User Scenarios

### User Scenario 1 (US1): Pure C TAR & 7Z Container Framing and Multi-Format Archiving
As an archiving engine developer, I want TAR (POSIX UStar / PAX) and 7Z (Solid Block / Coders DAG) container framing and compression to execute entirely in pure C11 (`ttzip_tar_container.c`, `ttzip_7z_container.c`), so that `ttzip_archive_create` supports ZIP, TAR, and 7Z with 0 Swift dependencies.

### User Scenario 2 (US2): In-Memory Single Entry Extraction for Instant Previews
As a GUI user browsing large archives, I want clicking on an image, video, audio, or document to extract the entry directly into memory (`ttzip_archive_extract_entry_mem`) without creating any temporary files on disk, achieving sub-millisecond preview latency and zero SSD wear.

### User Scenario 3 (US3): C-Level Magic Number Sniffing & Natural Numeric Sorting
As a file manager user, I want file types detected accurately in 1ns via file-header magic numbers (`ttzip_magic_sniff.c`) regardless of file extension, and large file lists sorted instantly via C11 natural string comparison (`ttzip_strnatcmp.c`).

### User Scenario 4 (US4): C-Level Fast Archive Tree & Multicore Diagnostic Inspection
As an archive explorer user, I want archive tree hierarchies built and searched in C memory (`ttzip_archive_tree.c`) and full archive integrity checked across all CPU cores (`ttzip_archive_inspect`), returning comprehensive health reports.

---

## 2. Functional Requirements

- **FR-001**: `ttzip_tar_container.c` and `ttzip_tar_container.h` must provide POSIX UStar 512-byte block serialization, octal checksumming, and PAX header support.
- **FR-002**: `ttzip_7z_container.c` and `ttzip_7z_container.h` must provide 7Z signature, main stream, and folder metadata serialization.
- **FR-003**: `ttzip_archive.c` must implement `ttzip_archive_extract_entry_mem` and `ttzip_archive_inspect`.
- **FR-004**: `ttzip_magic_sniff.c` must detect PNG, JPEG, GIF, WEBP, PDF, MP4, MOV, MP3, ZIP, 7Z, TAR, GZ, XZ, ZST magic numbers.
- **FR-005**: `ttzip_strnatcmp.c` must implement case-insensitive natural numeric string comparison.
- **FR-006**: `ttzip_archive_tree.c` must build a compact Radix tree and support case-insensitive substring search.
- **FR-007**: All new modules must be compiled via CMake into `libttzip.a` and `ttzip-cli`, and verified via `./scripts/local-ci.sh`.

---

## 3. Success Criteria

1. CMake builds `libttzip.a` and `ttzip-cli` cleanly with 0 errors.
2. `ttzip-cli --benchmark` exercises all new C subsystems and reports high throughput.
3. 100% of Swift core and matrix tests pass green (76+ tests).
