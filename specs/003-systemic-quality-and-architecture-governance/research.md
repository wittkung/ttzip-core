# Technical Research & Architecture Decisions: Systemic Quality and Governance

- **Feature ID**: `003-systemic-quality-and-architecture-governance`
- **Created**: 2026-08-24
- **Target Subsystems**: `ttzip-engine` (Rust), `CTTZipBridge` (C-ABI), `TTZipCore` (Swift), `CI/CD Gates`

---

## 1. Dimension 1: Cross-Language FFI & Safe Typed Memory

### Decision: Stack-Allocated `TTZipErrorInfo` C-ABI Protocol
- **Decision**: Completely deprecate `thread_local! static LAST_ERROR` and all TLS error accessors. Standardize on passing `out_error: *mut TTZipErrorInfo` (784 bytes, 8-byte aligned) allocated on the caller's stack frame.
- **Rationale**: Swift 6 Strict Concurrency dispatches continuations across arbitrary worker threads in the cooperative thread pool. Thread-local storage guarantees data corruption or dangling pointers upon `Task` suspension. Stack out-parameters provide zero heap allocation, deterministic lifetimes, and thread-safety.
- **Alternatives Considered**:
  - *Heap-allocated `*mut TTZipError` with explicit free function*: Introduces memory leak risks if Swift callers fail to pair with `ttzip_rust_free_error`.
  - *Keep TLS with thread-id hashing table*: High synchronization contention on global lock tables, non-deterministic memory bloat.

### Decision: 4-Layer C-ABI Symbol & Layout Verification Suite
- **Decision**: Automate `verify_cabi_symbols.sh` to perform bidirectional bijection validation via `nm -gU libTTZipVendor.a` against `ttzip_rust_glue.h`, paired with compile-time `offset_of!` static assertions in `cabi_layout_tests`.
- **Rationale**: Guarantees zero silent struct field misalignment or missing symbol linkage between Swift clang modules and Rust static libraries.
- **Alternatives Considered**: Manual code review (error-prone, failed to catch previous struct layout padding drift).

---

## 2. Dimension 2: Concurrency & Zero-I/O Architecture

### Decision: Direct Return of Extracted Metrics (`out_extracted_bytes`)
- **Decision**: Export `ttzip_rust_archive_extract_unified_v2` with `out_extracted_bytes: *mut u64` to return precise uncompressed byte counts directly from the decompression stream.
- **Rationale**: Eliminates post-extraction `calculateDirectorySize` recursive disk scans, reducing latency for 100,000-file extractions by over 88% (from ~7s to <800ms).
- **Alternatives Considered**:
  - *Async file system event monitoring (FSEvents)*: High system overhead, non-deterministic event coalescing.
  - *Swift-side file tracker during extraction*: Fails to capture engine-managed symlinks and directory metadata efficiently.

### Decision: 60Hz Monotonic Clock Throttling with Keyframe Guarantee
- **Decision**: Use `clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)` in `ProgressBridgeContext` with $16.6\text{ms}$ gating and forced keyframe delivery ($0\%$ and $100\%$).
- **Rationale**: Prevents `@MainActor` event queue saturation during high-throughput small file extraction while guaranteeing progress bar completeness.

---

## 3. Dimension 3: Bounded Memory Streaming & Steady-State VFS Cache Arena

### Decision: Sliding-Window Solid 7z Extraction with Early Termination
- **Decision**: Implement `Streaming7zExtractor` to decompress 7z solid blocks in bounded chunks (1MB~8MB) and break out of decompression immediately upon reaching the target file end.
- **Rationale**: Eliminates the catastrophic $O(N)$ allocation `vec![0u8; total_uncompressed]` that caused OOM crashes on multi-gigabyte 7z archives.
- **Alternatives Considered**: Full decompression to temporary disk files (causes severe SSD write wear and 10x latency penalty).

### Decision: 16-Way Sharded Arena Freelist Slot Recycling & 3-Phase Lock Splitting
- **Decision**: Implement `allocate_node` popping from `free_indices` in `VFSLz4CachePool`, combined with `Arc<[u8]>` snapshots, keeping write-lock durations under 100ns and moving LZ4 compression/decompression completely outside locks.
- **Rationale**: Ensures $O(1)$ memory consumption matching configured `max_ram_bytes` and zero lock contention across 16 Rayon worker threads.

---

## 4. Dimension 4: Defensive Testing & Continuous CI Gate Governance

### Decision: Multi-Language Sanitizer & Constant-Time Crypto Integration
- **Decision**: Standardize `run_sanitizers.sh` for ASan, TSan, and UBSan across mixed SwiftPM/Cargo builds, and enforce constant-time bitwise mask arithmetic for GHash and WinZip AES.
- **Rationale**: Prevents memory corruption, thread races, and timing side-channel attacks before code reaches release branches.

### Decision: Automated Git Worktree A/B Performance Benchmark Gate
- **Decision**: Embed `run_comprehensive_ab_benchmark.py` and `statistical_delta.py` into release verification to block any change introducing $>3\%$ throughput regression ($p < 0.05$).
- **Rationale**: Guarantees zero performance regression across compiler upgrades, architecture refactoring, and feature additions.
