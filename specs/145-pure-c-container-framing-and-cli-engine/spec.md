# Feature Specification: 145-pure-c-container-framing-and-cli-engine

## 1. Executive Summary & User Scenarios

### User Scenario 1 (US1): Pure C High-Throughput Container Metadata Framing
As an archiving engine developer, I want all ZIP, TAR, and 7Z container framing logic (Local Headers, Central Directory, EOCD, PAX headers, 7Z folder DAGs) executed directly in C11 (`ttzip_zip_container.c`, `ttzip_tar_native.c`, `ttzip_7z_header_writer.c`), so that Swift does zero manual byte packing and memory allocation during file header serialization.

### User Scenario 2 (US2): Unified Public C Archive Orchestrator (`ttzip_archive.c`)
As a multi-platform developer, I want a single top-level C API (`ttzip_archive_create`, `ttzip_archive_extract`, `ttzip_archive_list`, `ttzip_archive_test`) that orchestrates the entire compression/decompression pipeline across all 16 formats using `ttzip_threadpool`, `ttzip_fs`, and SOTA codecs with 0 Swift dependencies.

### User Scenario 3 (US3): Standalone Pure C Cross-Platform CLI Tool (`ttzip-cli`)
As a DevOps engineer or Linux/Windows user, I want a single-binary CLI tool `ttzip-cli` compiled via CMake that supports fast archive creation (`-c`), extraction (`-x`), listing (`-l`), testing (`-t`), and multi-core benchmarking (`-b`) without needing any Swift runtime or Apple-exclusive libraries.

### User Scenario 4 (US4): x86_64 SIMD Vector Acceleration & Runtime CPUID Dispatch
As a user running on Intel Mac, Windows x86_64, or Linux x86_64, I want the engine to dynamically detect SSE4.2, AVX2, AVX-512, PCLMULQDQ, and AES-NI at runtime and dispatch to optimized vector microkernels, achieving hardware parity with Apple Silicon NEON.

---

## 2. Functional Requirements

- **FR-001**: `ttzip_zip_container.c` must provide in-place, zero-allocation serialization for PKZip Local Headers, Central Directory Headers, Zip64 Extended Information, and EOCD records.
- **FR-002**: `ttzip_archive.h` and `ttzip_archive.c` must implement top-level archive operations (`ttzip_archive_create`, `ttzip_archive_extract`, `ttzip_archive_list`, `ttzip_archive_test`) utilizing `ttzip_fs` and `ttzip_threadpool`.
- **FR-003**: `cli/main.c` must implement a complete command-line interface `ttzip-cli` compiled via CMake into `build/ttzip-cli`.
- **FR-004**: `ttzip_platform_detect.c` must perform runtime CPUID feature probing for x86_64 (SSE4.2, AVX2, AVX-512, PCLMULQDQ, AES-NI) and ARM64 (NEON, PMULL, CRC32, AES, SHA3).
- **FR-005**: All CMake targets (`ttzip`, `ttzip-cli`) must compile cleanly with 0 errors and pass `./scripts/local-ci.sh`.

---

## 3. Success Criteria

1. `CMakeLists.txt` builds both `libttzip.a` and `ttzip-cli` executable successfully in Release mode.
2. `./build/ttzip-cli --version` and `./build/ttzip-cli --benchmark` execute cleanly with exit code 0.
3. 100% of Swift core and matrix test suites pass green (76+ tests).
