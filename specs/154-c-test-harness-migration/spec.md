# Feature Specification: C Test Harness Migration & Dual-Engine Testing Architecture

**Feature Branch**: `154-c-test-harness-migration`  
**Created**: 2026-08-20  
**Status**: Draft  
**Input**: User description: "我觉得我们都测试体系也应该全量迁移c，全面搭建并推进 /speckit-specify"

---

## User Scenarios & Testing

### User Story 1 - Instant Native C Test Suite via CTest & CMake (Priority: P1)

As an open-source contributor or core engine developer on macOS, Linux, or BSD, I want to execute the complete suite of native microkernel, compression, and security tests in under 100 milliseconds using standard `ctest` or a single binary run, without requiring Xcode, SwiftPM, or Swift runtime overhead.

**Why this priority**:
The C microkernel comprises over 80% of TTZip's codebase. Removing Swift test startup latency (2–4 seconds per run) and decoupling C tests empowers cross-platform CI, rapid iterative local development, and seamless integration with upstream communities (`libarchive`, `zlib-ng`, `fast-lzma2`).

**Independent Test**:
Can be fully tested by running `ctest --output-on-failure` or `./build/ttzip_c_test_runner` from terminal and observing all microkernel tests pass in < 50ms with 0 failures.

**Acceptance Scenarios**:
1. **Given** a standard POSIX/macOS build directory, **When** running `ctest --test-dir build --output-on-failure`, **Then** all registered C test suites execute in < 100ms total and report 100% pass rate.
2. **Given** hardware ARM64 NEON support, **When** `test_crc_neon` executes, **Then** hardware polynomial folding and standard tables produce identical CRC32/CRC64 digests on arbitrary byte buffers with zero allocation.
3. **Given** an invalid or malicious path with directory traversal (`../../etc/passwd`), **When** `test_security_zipslip` processes the entry, **Then** it is strictly intercepted and blocked before any filesystem access occurs.

---

### User Story 2 - Microsecond Zero-Dependency C Test Harness (Priority: P2)

As a systems developer, I want a lightweight, header-only C11 test framework (`ttzip_test_harness.h`) that provides formatted ANSI color reports, microsecond assertion timing, leak detection hooks, and isolated test case execution without any external third-party dependencies.

**Why this priority**:
Zero third-party test dependencies ensures that TTZip remains 100% self-contained, compile-ready on any C11/C99 compiler, and free from external licensing or header conflicts.

**Independent Test**:
Include `ttzip_test_harness.h` in a standalone C file, define test macros (`TEST_CASE`, `ASSERT_EQ`, `ASSERT_STR_EQ`, `ASSERT_PTR_NOT_NULL`), build with clang/gcc, and run.

**Acceptance Scenarios**:
1. **Given** a suite of 20 test cases, **When** all assertions pass, **Then** the harness prints a structured green summary table with per-test microsecond latency.
2. **Given** an assertion failure, **When** `ASSERT_EQ(actual, expected)` fails, **Then** the harness outputs the exact file, line number, expected vs actual values, and continues or halts according to test configuration.

---

### User Story 3 - Dual-Engine Test Boundary Decoupling (Priority: P3)

As a macOS GUI developer, I want the Swift test suite (`XCTest`) to focus purely on AppKit `@Observable` ViewModels, UI view delegations, and macOS system integrations, while all algorithmic, format parsing, and compression codec tests are owned by the C test suite.

**Why this priority**:
Eliminates test duplication, cuts Swift test run times from 15+ seconds down to < 2 seconds, and prevents console output buffer stalling during heavy CPU compression runs.

**Independent Test**:
Run `./scripts/local-ci.sh` and verify that Stage 1 runs CTest (< 0.1s) and Stage 2 runs Swift AppKit tests (< 1.5s).

**Acceptance Scenarios**:
1. **Given** the local CI script, **When** `./scripts/local-ci.sh` runs, **Then** C tests run first in < 0.1s, followed by Swift GUI/AppKit integration tests.
2. **Given** pure C engine changes, **When** developing in C, **Then** developers only need to run `cmake --build build --target test` without invoking `swift test`.

---

## Edge Cases

- **Memory Alignment & Allocation Bounds**: How does the C test harness verify that memory buffers allocated for SIMD operations maintain strict 64-byte cacheline alignment?
- **AddressSanitizer / UBSan Compatibility**: How do C test targets behave when compiled with `-fsanitize=address,undefined`? (Must pass with 0 leak reports and 0 undefined behavior warnings).
- **Big-Endian / Cross-Architecture Portability**: How do integer serialization tests (e.g. Zip Little-Endian vs Tar Octal) behave across architectures?

---

## Requirements

### Functional Requirements

- **FR-001**: System MUST provide a self-contained header-only C11 test framework `tests/c/ttzip_test_harness.h` with zero external library dependencies.
- **FR-002**: System MUST implement test suites in `tests/c/` covering:
  - `test_crc_neon.c`: Hardware PMULL/NEON CRC32 & CRC64 versus software table parity.
  - `test_magic_sniff.c`: Sub-nanosecond header magic sniffing across 16 archive formats.
  - `test_strnatcmp.c`: C11 natural numeric string sort algorithm with strict weak ordering invariants.
  - `test_deflate_zopfli.c`: Native Deflate, Zopfli greedy/iterative compression, and in-place Huffman validation.
  - `test_7z_lzma2.c`: 7z container headers, LZMA2 encoding/decoding, and block splitting.
  - `test_tar_container.c`: Tar/Pax header parsing, SWAR octal decoders, and directory trees.
  - `test_security_zipslip.c`: Defensive path canonicalization and Zip-Slip attack interception.
  - `test_concurrency_threadpool.c`: Thread pool task dispatch, counting semaphore, and memory budget queries.
- **FR-003**: System MUST integrate all C test targets into `CMakeLists.txt` via `enable_testing()` and `add_test()`.
- **FR-004**: System MUST provide a unified C test executable `ttzip_c_test_runner` that runs all registered test suites in a single process run.
- **FR-005**: System MUST update `./scripts/local-ci.sh` to execute the C test runner prior to Swift tests.
- **FR-006**: Swift `XCTest` test files that solely test C functions without Swift logic MUST be streamlined to delegate to C tests.

---

## Success Criteria

### Measurable Outcomes

- **SC-001**: The complete C test suite (`ttzip_c_test_runner`) MUST execute 100% of microkernel test cases in under **100 milliseconds** total runtime on Apple Silicon.
- **SC-002**: `ctest --test-dir build` MUST pass **100% green with 0 failures** across all test suites.
- **SC-003**: Compilation of all C test sources MUST produce **0 compiler warnings and 0 linker warnings** with `-Wall -Wextra -Wpedantic`.
- **SC-004**: Running C tests under AddressSanitizer (`-fsanitize=address`) MUST report **0 memory leaks and 0 heap buffer overflows**.
- **SC-005**: Local CI runtime (`./scripts/local-ci.sh`) for C test stage MUST execute in **< 0.2 seconds**.

---

## Assumptions

- Target build platforms include macOS (Apple Silicon & Intel) and standard POSIX-compliant Linux systems.
- C tests will be compiled with C11 standard (`-std=c11`).
- Swift test targets (`TTZipAppTests`) will be preserved for macOS AppKit and SwiftUI UI components.
