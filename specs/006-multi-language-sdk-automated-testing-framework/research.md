# Research & Architectural Decision Matrix: TTZip Full Multilingual SDK Testing System

- **Feature ID**: `006-multi-language-sdk-automated-testing-framework`
- **Created**: 2026-08-24
- **Coverage**: 9 SDK Ecosystems, Cross-Language Interop Matrix, Security Fuzzing, ASan/TSan Automation, Performance Regression Harness

---

## 1. Unified Test Runner Architecture (Native vs Subprocess Wrapping)

- **Context**: How should the unified multi-language test coordinator invoke and inspect test results from 9 disparate language runtimes (Cargo, SwiftPM, PyTest, JUnit 5, Dart Test, xUnit, CTest/Clang, Go Test, Vitest)?
- **Decision**: **Hierarchical Native Runner with Structured JSON Interceptor Envelope**.
- **Rationale**:
  1. Each language ecosystem must run using its canonical idiomatic test framework (`cargo test`, `swift test`, `python3 -m unittest`, `mvn test` / standalone JUnit launcher, `dart test`, `dotnet test`, `go test`, compiled C/C++ native test runner).
  2. The root orchestrator `run_sdk_test_matrix.sh` queries toolchain presence (`command -v`), dynamically invokes native test runners with JSON/JUnit output flags, parses stdout/stderr, and aggregates execution metrics into a unified `sdk-test-report.json`.
- **Alternatives Considered**:
  - *Single C/Rust Binary Running Everything via C-ABI*: Fails to validate language-specific high-level ergonomics (e.g. Kotlin Coroutines `Flow`, Python context managers, Swift Actor concurrency, C# `IAsyncEnumerable`).
  - *Pure CI-only multi-job matrix*: Slower feedback loop during local development; cannot be executed offline by contributors with single command.

---

## 2. Cross-Language Interoperability Matrix ($N \times N$ Round-Trip)

- **Context**: How to systematically test cross-language compatibility across 9 languages without writing $9 \times 9 = 81$ separate bidirectional integration programs?
- **Decision**: **Shared Canonical Fixture Corpus + Bidirectional CLI/FFI Assertion Matrix**.
- **Rationale**:
  1. A deterministic fixture generator creates 4 canonical data scenarios (ASCII Text, Deeply Nested Directory Tree, Multibyte CJK/Emoji, Sparse Large File).
  2. Each SDK exposes a standard headless test interface:
     - `create_archive(source_dir, dest_archive, format, password)`
     - `extract_archive(archive_path, output_dir, password)`
     - `list_entries(archive_path)`
  3. The matrix test orchestrator triggers `SDK_A.create` $\to$ `SDK_B..I.extract` and verifies SHA-256 recursive checksum match against the original source directory.
- **Alternatives Considered**:
  - *Static pre-generated golden files only*: Tests decoding only; misses bugs in language-specific archive encoders.

---

## 3. Security, Fuzzing & Malicious Fixture Suite

- **Context**: How to ensure all SDKs safely reject or neutralize malicious archives without crashing or writing out of bounds?
- **Decision**: **Embedded Security Fixture Suite + Conformance Assertions**.
- **Test Scenarios**:
  1. **Zip Slip (Path Traversal)**: Archive containing entries with `../../../../tmp/evil.sh` or absolute `/etc/passwd`.
     - *Assertion*: All SDKs must either sanitize target path to stay strictly within destination root or throw `SecurityException` / `ErrPathTraversal`.
  2. **Zip Bomb (Decompression Ratio Overflow)**: 42.zip or recursive multi-gigabyte sparse streams.
     - *Assertion*: All SDKs must enforce default ratio limits (e.g. 1000:1) and bounded memory usage ($\le 64\text{MB}$ RSS).
  3. **Truncated / Corrupted Streams**: Files with chopped EOCD or flipped central directory bits.
     - *Assertion*: All SDKs must return clean error codes without memory faults (SIGSEGV, SIGBUS) or panic aborts.

---

## 4. Sanitizers & Concurrency Memory Leak Gates (ASan / TSan)

- **Context**: Native FFI bridges in Python (PyO3), Swift (UnsafePointer), Java (Panama MemorySegment), Go (CGO), Dart (dart:ffi), C# (P/Invoke), C++20 and C11 can introduce subtle memory leaks and data races.
- **Decision**: **Clang / Rust Clang-compatible ASan/LSan/TSan Harness**.
- **Execution Strategy**:
  - Rust engine & C11 / C++20 test binaries compiled with `-Zsanitizer=address` and `-fsanitize=address,undefined`.
  - Go CGO tests executed with `go test -race`.
  - Automated leak check asserting 0 bytes leaked across 10,000 iterations of archive open/close/extract cycles.
