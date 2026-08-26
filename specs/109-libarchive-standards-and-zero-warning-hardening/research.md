# Phase 0 Research: Libarchive Standards & Zero-Warning Codebase Hardening

## R001: Libarchive C Error Semantics, State Machine & Header Standards

### Decision
1. **Error Status Code Tri-State Model**:
   Adopt libarchive's standardized 6 return codes (`ARCHIVE_OK = 0`, `ARCHIVE_EOF = 1`, `ARCHIVE_RETRY = -10`, `ARCHIVE_WARN = -20`, `ARCHIVE_FAILED = -25`, `ARCHIVE_FATAL = -30`).
   - `ARCHIVE_FAILED` indicates an isolated entry-level failure (e.g. corrupt header in a single file). The engine transitions to `ARCHIVE_STATE_DATA_RECOVERY`, skips the corrupt block, and successfully advances to the next entry.
   - `ARCHIVE_FATAL` indicates container-level structural corruption. The engine immediately terminates and frees handles.
2. **POSIX Header Type Safety**:
   Use `uint32_t magic` guards on all C handle structs and adhere strictly to `la_int64_t`/`la_ssize_t` types.
3. **Doxygen Documentation Standard**:
   Annotate every C/Swift function with explicit `@ownership`, `@threadsafe`, `@pre`, `@post`, and `@invariant` tags in 100% professional English.

### Rationale
- Maximizes data recovery when decompressing archives with occasional bad entries.
- Guarantees zero-lock reentrancy across GCD/Task worker threads.
- Eliminates undefined behavior and memory leaks across language boundaries.

### Alternatives Considered
- *Fail-Fast on all errors*: Rejected because single-entry corruption would terminate extraction of thousands of healthy files.
- *Mutex-locked shared archive handles*: Rejected due to severe multi-core contention violating the 10,000+ MB/s throughput requirement.

### Source
- `Vendor/libarchive-upstream/libarchive/archive.h` (Lines 37, 232-239, 263-305)
- `Vendor/libarchive-upstream/libarchive/archive_read.c` (Lines 640-725, 970-1052)
- `Vendor/libarchive-upstream/libarchive/archive_platform.h` (Lines 28-62, 122-176)

---

## R002: Swift 6.0 Zero-Warning Gate & In-Process Test Observation

### Decision
1. **Zero-Warning Gate**:
   Enforce `-Xswiftc -warnings-as-errors` in `scripts/run_local_ci_gate.sh` and `scripts/lint_codebase_standards.sh`.
2. **In-Process Lifecycle Interception**:
   Use Swift native `XCTestObservation` (`TTZipTestObserver`) registered idempotently with `XCTestObservationCenter.shared`.
   - Normal execution: `TTLogger` captures debug/trace messages to a 2,000-entry ring buffer in memory, maintaining silent console output.
   - Test failure: `testCase(_:didRecord:)` flags failure and dumps the ring buffer for immediate diagnostic visibility.
3. **Benchmark Tiering**:
   Decouple heavy PK suites (`*PkTests.swift`) using `TTZIP_RUN_BENCHMARKS=1`.

### Rationale
- Eliminates brittle external Python pipe wrappers (`swift test 2>&1 | python script.py`), ensuring pure exit-code propagation.
- Suppresses noise during high-throughput testing while providing complete failure trace reconstruction.
- Reduces full test suite duration from ~600s to < 40s.

### Alternatives Considered
- *External Python stdout streaming parser*: Rejected due to pipe buffering delays, exit code masking, and external interpreter dependencies.
- *Hardcoding unsafeFlags in Package.swift*: Rejected because SPM flags `.unsafeFlags(["-warnings-as-errors"])` as an unsafe dependency in downstream environments.

### Source
- `Tests/TTZipTests/TTZipTestObserver.swift` (Lines 13-59)
- `Sources/TTZipCore/Utilities/Logger.swift` (Lines 35-147)
- `Package.swift` (Lines 1-85)
- Apple Developer Docs: `XCTestObservationCenter` & `XCTestObservation`
