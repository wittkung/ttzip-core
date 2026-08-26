# Feature Specification: Systemic Quality, FFI Hardening, Steady-State VFS Concurrency, and CI Governance

- **Feature ID**: `003-systemic-quality-and-architecture-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `DRAFT`
- **Created**: 2026-08-24
- **Target Subsystems**: `ttzip-engine` (Rust Core), `CTTZipBridge` (C-ABI), `TTZipCore` (Swift SDK), `CI/CD & Gate Suite`

---

## 1. Executive Summary & Business Context

Recent architectural audits and A/B benchmarking identified systemic vulnerabilities across four critical operational dimensions:
1. **Cross-Language FFI & Memory Safety**: TLS error pointers susceptible to thread-hopping dangling references and Swift raw pointer uninitialized write UB.
2. **I/O & Concurrency Architecture**: Legacy post-extraction disk scans (`calculateDirectorySize`) creating multi-second I/O bottlenecks, and cooperative thread pool starvation risks.
3. **Memory & Cache Steady-State**: Unbounded VFS node growth without slot recycling and solid decompression memory spikes ($O(N)$ RAM blowups).
4. **Defensive Testing & CI Gate Governance**: Absence of formalized sanitizers, constant-time crypto verifiers, and mandatory zero-regression A/B performance gates before release.

This specification formalizes the systemic hardening, architecture restructuring, and CI governance across the entire TTZip engine.

---

## 2. User Stories & Acceptance Scenarios

### User Story 1: Zero-Trust Cross-Language FFI & Type-Safe Error Handling (Priority: P1)
**As a** macOS native developer integrating TTZip into Swift 6 async workflows,  
**I want** all FFI operations to return structured, stack-allocated error details without thread-local pointer escapes,  
**So that** multi-threaded background extractions never crash from dangling pointers or produce corrupted error messages across Swift `Task` suspensions.

- **Scenario 1.1 (Stack-Allocated Error Reporting)**: When an FFI operation fails, the Rust kernel populates the caller-allocated `TTZipErrorInfo` struct with the exact error code, 512-byte UTF-8 diagnostic message, failing entry path, and byte offset.
- **Scenario 1.2 (Thread-Hopping Resilience)**: When a Swift async `Task` initiates an extraction on Thread A, suspends, and resumes on Thread B, error extraction succeeds with 100% fidelity without reading invalid TLS state.
- **Scenario 1.3 (Bi-directional Symbol Parity)**: The CI gate enforces 100% bijection between `ttzip_rust_glue.h` declarations and `libTTZipVendor.a` Mach-O exported global text symbols.

### User Story 2: True Non-Blocking Concurrency & Zero-I/O Direct Metrics (Priority: P1)
**As an** end-user extracting a 100,000-file archive on Apple Silicon,  
**I want** extraction progress to update smoothly at 60Hz and complete instantly without secondary disk rescans,  
**So that** the UI remains responsive and extraction duration is bounded strictly by compression throughput and disk I/O bandwidth.

- **Scenario 2.1 (Direct Metrics Extraction)**: `ttzip_rust_archive_extract_unified_v2` directly returns the exact total uncompressed bytes via `out_extracted_bytes`, eliminating `calculateDirectorySize` recursive disk scans.
- **Scenario 2.2 (60Hz Monotonic UI Throttling)**: The C-ABI progress bridge throttles Swift progress updates to $\le 60\text{Hz}$ via `clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)` while guaranteeing keyframe delivery (0% and 100%).
- **Scenario 2.3 (Cooperative Rapid Cancellation)**: Canceling a Swift `Task` signals the Rust kernel via atomic tokens and cooperative callbacks, interrupting Rayon loops and file extraction in $\le 10\text{ms}$.

### User Story 3: Bounded Memory Streaming & Steady-State VFS Cache Arena (Priority: P2)
**As a** user previewing large solid 7z or ZIP archives under constrained memory,  
**I want** decompression and caching to operate within fixed memory bounds ($O(1)$ RAM),  
**So that** memory spikes never trigger system OOM or force swap thrashing.

- **Scenario 3.1 (7z Solid Sliding Decompression)**: Decompressing a single entry from a multi-gigabyte 7z solid stream utilizes a sliding window and terminates immediately upon reading the target entry without decompressing subsequent entries.
- **Scenario 3.2 (VFS Freelist Slot Recycling)**: The 16-way sharded VFS cache pool recycles inactive node indices via a freelist, maintaining a constant node vector size under continuous cache eviction cycles.
- **Scenario 3.3 (Three-Phase Lock-Free Cache I/O)**: VFS `get` and `put` operations perform LZ4 decompression and disk spill file I/O outside of shard locks, minimizing write-lock hold times to $\le 100\text{ns}$.

### User Story 4: Continuous Sanitizer, Constant-Time Crypto & A/B Performance CI Governance (Priority: P2)
**As a** systems maintainer and release engineer,  
**I want** automated CI gates enforcing AddressSanitizer, ThreadSanitizer, constant-time crypto execution, and statistical A/B benchmarking,  
**So that** regressions in memory safety, race conditions, side-channels, or throughput are caught before merge.

- **Scenario 4.1 (Sanitizer Cleanliness)**: All Swift and Rust tests pass with zero errors under ASan, TSan, and UBSan configurations.
- **Scenario 4.2 (Constant-Time Verification)**: Cryptographic verification routines for AES-GCM tags, WinZip PVV/MAC, and password recovery execute without data-dependent branch instructions.
- **Scenario 4.3 (Automated A/B Performance Gate)**: Pre-release verification executes 5 interleaved rounds in an isolated git worktree; throughput regressions $>3\%$ ($p < 0.05$) fail the build.

---

## 3. Functional Requirements (FR-01 to FR-16)

### Functional Requirements: Cross-Language FFI & Memory Safety
- **FR-01**: The C-ABI layer MUST provide `TTZipErrorInfo` (784 bytes, 8-byte aligned) for structured stack error propagation across all fallible FFI entrypoints.
- **FR-02**: The Rust engine MUST completely eliminate `thread_local! static LAST_ERROR` and associated retrieval/clearing functions (`ttzip_rust_last_error_message`, `ttzip_rust_clear_last_error`).
- **FR-03**: The Swift bridge (`CUnsafeBufferAdapter`) MUST utilize typed memory allocation (`UnsafeMutablePointer<UInt8>.allocate`) with guaranteed `deinitialize` and `deallocate` lifecycle pairing.
- **FR-04**: The CI pipeline MUST include `verify_cabi_symbols.sh` to enforce complete bidirectional mapping between `ttzip_rust_glue.h` and `libTTZipVendor.a` Mach-O symbols.

### Functional Requirements: I/O & Concurrency Architecture
- **FR-05**: `ttzip_rust_archive_extract_unified_v2` MUST return total extracted uncompressed bytes directly through `out_extracted_bytes: *mut u64`.
- **FR-06**: Swift engine implementors (`ArchiveEngineBridge`) MUST NOT execute secondary file system directory rescans (`calculateDirectorySize`) after FFI extraction.
- **FR-07**: `NativeComputeDispatcher` MUST execute all blocking FFI computations outside the Swift 6 cooperative thread pool.
- **FR-08**: `ProgressBridgeContext` MUST throttle progress emissions to 60Hz using nanosecond monotonic clock checks while guaranteeing delivery of keyframes ($0\%$ and $100\%$).
- **FR-09**: Cooperative cancellation MUST propagate across FFI within $\le 10\text{ms}$ upon Swift `Task.cancel()` or handle cancellation.

### Functional Requirements: Bounded Memory & VFS Steady-State
- **FR-10**: The parallel ZIP writer (`streaming_parallel.rs`) MUST process entries in bounded batches ($\le 64$ entries / batch) with immediate positional `pwrite` flush.
- **FR-11**: The 7z decompression engine MUST support streaming solid block decompression with early termination upon extracting targeted entries.
- **FR-12**: `VFSLz4CachePool` MUST implement intrusive freelist slot reuse (`free_indices.pop()`) in `allocate_node` to prevent unbounded memory growth.
- **FR-13**: `VFSLz4CachePool` MUST execute disk spill file I/O and LZ4 decompression outside of shard read/write locks.
- **FR-14**: Single-entry in-memory extraction (`extractSingleEntryData`) MUST execute a two-stage probe to allocate exact uncompressed byte buffers without fixed 32MB waste.

### Functional Requirements: Defensive Testing & CI Governance
- **FR-15**: Cryptographic routines in `vault.rs`, `winzip.rs`, and `password_recovery.rs` MUST execute in constant-time using bitwise masks and volatile memory wiping.
- **FR-16**: The release gate MUST execute `run_comprehensive_ab_benchmark.py` and enforce a zero-regression tolerance ($\le -3\%$ throughput delta, $p < 0.05$).

---

## 4. Success Criteria

- **SC-01 (Zero TLS Leakage)**: 0 occurrences of TLS-backed error retrieval in the entire codebase.
- **SC-02 (Zero Post-Extract I/O)**: 0ms spent on post-extraction disk size calculation across all archive formats.
- **SC-03 (UI Frame Rate Stability)**: 0 frame drops on `@MainActor` during 100,000-file archive extractions, maintaining steady 60/120 FPS.
- **SC-04 (VFS Steady-State RAM)**: `VFSLz4CachePool` memory consumption remains bounded at `max_ram_bytes` $\pm 5\%$ over $10^6$ continuous insert/evict operations.
- **SC-05 (Zero-Regression Performance)**: Automated A/B benchmark verification confirms $\ge 0\%$ throughput delta or statistically insignificant variance across all supported codecs.
- **SC-06 (Sanitizer Compliance)**: 100% clean test execution under ASan, TSan, and UBSan.

---

## 5. Non-Functional & Systemic Governance Constraints

- **Language Standard**: Rust 2021 Edition, Swift 6 Strict Concurrency (`-strict-concurrency=complete`).
- **Memory Safety**: No uninitialized pointer dereferencing, no data-dependent branches on secret keys, explicit memory zeroization on drop.
- **Single-File Limit**: All source files MUST adhere to the single-file size threshold of $\le 800$ LOC.
- **Architecture Support**: Native Apple Silicon ARM64 and Intel x86_64 Universal Binary (`macos-arm64_x86_64`).
