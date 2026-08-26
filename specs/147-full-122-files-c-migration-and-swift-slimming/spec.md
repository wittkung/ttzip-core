# Feature Specification: 147-full-122-files-c-migration-and-swift-slimming

## 1. Executive Summary & User Scenarios

### User Scenario 1 (US1): Full Container Format Sinking to Pure C Microkernel
As an archiving engine developer, I want all ZIP, 7Z, TAR, Split-volume, and In-Place mutation operations across 37 files in `TTZipCore/Zip`, `TTZipCore/SevenZip`, `TTZipCore/Tar`, `TTZipCore/Split`, and `TTZipCore/InPlaceEdit` migrated directly to C11 (`ttzip_zip_container.c`, `ttzip_7z_container.c`, `ttzip_tar_container.c`, `ttzip_split.c`, `ttzip_inplace.c`), so that Swift does zero manual byte packing, zero chunk management, and zero manual filesystem walking.

### User Scenario 2 (US2): Security, Error Correction & Search Microkernels in C
As a security-conscious user, I want password clearing (DSE defense), PBKDF2/Argon2 key derivation, Reed-Solomon forward error correction (FEC), recovery records, and search indexing across 14 files in `TTZipCore/Security`, `TTZipCore/Crypto`, `TTZipCore/Search`, and `TTZipCore/VFS` executed in C memory, ensuring cryptographic memory safety and zero sensitive data leakage in Swift ARC heap.

### User Scenario 3 (US3): Frontend Heavy Computing Decoupling & Instant Previews
As a GUI user, I want archive tree construction, sub-nanosecond magic sniffing, natural sorting, and in-memory direct entry preview extraction across 11 files in `TTZipApp/Services` and `TTZipApp/ViewModels` offloaded to C (`ttzip_archive_tree.c`, `ttzip_magic_sniff.c`, `ttzip_strnatcmp.c`, `ttzip_archive_extract_entry_mem`), achieving sub-millisecond responsiveness with 80% lower UI memory footprint.

### User Scenario 4 (US4): Standalone Pure C CLI & Benchmark Sovereignty
As a DevOps engineer, I want the CLI and benchmark subsystems (60 files in `TTZipCore/CLI` and `TTZipCore/Benchmark`) unified and superseded by the standalone `ttzip-cli` binary, running full multi-format benchmarks with zero Swift runtime dependency across macOS, Linux, and Windows.

---

## 2. Functional Requirements

- **FR-001**: `ttzip_archive.c` and container modules must completely absorb all container packing, parsing, Direct I/O, APFS preallocation, and chunked writing logic from `Sources/TTZipCore/Zip/` (18 files), `Sources/TTZipCore/SevenZip/` (14 files), and `Sources/TTZipCore/Tar/` (1 file).
- **FR-002**: `ttzip_split.c` and `ttzip_inplace.c` must absorb multi-volume split/combine (`Sources/TTZipCore/Split/`, 2 files) and in-place archive mutation/appending (`Sources/TTZipCore/InPlaceEdit/`, 2 files).
- **FR-003**: `ttzip_security.c` and `ttzip_fec.c` must absorb Reed-Solomon FEC, recovery records, credential memory scrubbing, and path fuzzing (`Sources/TTZipCore/Security/` & `Crypto/`, 11 files).
- **FR-004**: `ttzip_archive_tree.c`, `ttzip_strnatcmp.c`, and `ttzip_magic_sniff.c` must serve all tree building, searching, sorting, and preview extraction for `TTZipApp/Services/` and `ViewModels/` (11 files).
- **FR-005**: `cli/main.c` must provide multi-format benchmark runners and CLI commands superseding Swift `CLI/` (18 files) and `Benchmark/` (42 files).
- **FR-006**: Swift `TTZipCore` must be refactored into a thin binding layer (`<8,000` total lines) routing directly to `ttzip_api.h` C functions via `UnsafeBufferPointer` and `@_silgen_name` / C headers.
- **FR-007**: 100% of existing Swift unit tests, matrix tests, and observer tests must pass green with zero regression.

---

## 3. Success Criteria

1. **Codebase Slimming**: Swift `TTZipCore` reduced by >20,000 lines of redundant byte-packing code, transformed into a pure thin binding shell.
2. **Performance Scaling**:
   - In-memory magic sniffing throughput >= 400 Million ops/s.
   - Natural sorting throughput >= 30 Million ops/s.
   - Archive tree search latency <= 1 millisecond for 10,000 entries.
   - Single-entry preview extraction latency <= 5 milliseconds without creating disk files.
3. **Zero Regression**: 100% of Swift core and concurrency test suites pass green in `./scripts/local-ci.sh`.
4. **Cross-Platform Parity**: Standalone `ttzip-cli` builds and runs all commands without Swift dependencies.
