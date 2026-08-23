# Implementation Plan: 146-full-c-engine-sinking-and-frontend-decoupling

## Technical Context
- **Language & Runtime**: Pure C11 (Core & Virtual Filesystem) + Swift 6.0 (Thin GUI Wrapper).
- **Target Deliverables**:
  1. `Sources/CTTZipBridge/include/ttzip_tar_container.h` + `Sources/CTTZipBridge/ttzip_tar_container.c`
  2. `Sources/CTTZipBridge/include/ttzip_7z_container.h` + `Sources/CTTZipBridge/ttzip_7z_container.c`
  3. `Sources/CTTZipBridge/include/ttzip_magic_sniff.h` + `Sources/CTTZipBridge/ttzip_magic_sniff.c`
  4. `Sources/CTTZipBridge/include/ttzip_strnatcmp.h` + `Sources/CTTZipBridge/ttzip_strnatcmp.c`
  5. `Sources/CTTZipBridge/include/ttzip_archive_tree.h` + `Sources/CTTZipBridge/ttzip_archive_tree.c`
  6. Update `Sources/CTTZipBridge/ttzip_archive.c` with in-memory extraction and inspection.
  7. Update `CMakeLists.txt` and `Sources/CTTZipBridge/include/ttzip_api.h`.
  8. Update `cli/main.c` and `scripts/local-ci.sh`.

## Constitution Check
- Zero-cost abstractions on hot paths maintained.
- Zero GCD dependencies in C or Swift core engines.
- In-memory operations avoid temporary disk writes.

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《TAR POSIX UStar & PAX Header Specification》: Octal checksumming and 512-byte block alignment rules.
- - R002 [SUBAGENT:research] 《Magic Number Sniffing Table》: Canonical binary signatures for media, archives, and document formats.

## Phase 1: Artifacts & Contracts
- `contracts/ttzip-vfs-contract.json`
- `data-model.md`
- `quickstart.md`
