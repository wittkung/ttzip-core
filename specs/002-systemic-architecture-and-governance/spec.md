# Feature Specification: TTZip Systemic Architecture & Engineering Governance

- **Feature ID**: `002-systemic-architecture-and-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `DRAFT`
- **Author**: Antigravity / CTO Persona
- **Target Subsystems**: `ttzip-engine` (Rust), `CTTZipBridge` (C-ABI), `TTZipCore` (Swift), `CI/CD & Testing` (Scripts, Proptest, XCTest)

---

## 1. Problem Statement & Motivation

During our architectural reflection and deep audit of TTZip, four systemic vulnerabilities and engineering blind spots were identified:
1. **Cross-Language Lifecycle & FFI Semantic Gaps**: Swift and Rust are individually memory-safe, but the C-ABI glue layer lacked formal interface contracts, leading to silent concurrency degradation (`thread_budget = 0`), format corruption (`format = 1`), and asynchronous lifetime use-after-free (UAF) hazards (`CancellationToken`, `FilterExpr`).
2. **Defensive Systems Architecture Vulnerabilities**: Incomplete security assumptions (e.g. `O_NOFOLLOW` not protecting intermediate directory traversal, non-boundary Unicode string truncation panics, missing constant-time cryptography checks).
3. **Concurrency Contention & Memory Churn**: Coarse-grained locking patterns (holding shard write locks during synchronous disk I/O in cache pools) and hot-path heap allocations (`to_lowercase()` in VFS sorting and search loops).
4. **Testing Architecture & Matrix Blind Spots**: Test suites previously verified happy-path functionality on local ARM64 machines, while missing cross-architecture CI matrices (x86_64 software fallback correctness), property-based boundary testing, and high-contention tail-latency regression monitoring.

---

## 2. Requirements & User Stories

### 2.1 User Story 1 - Cross-Language FFI Contract & Lifecycle Governance
*As a native macOS app developer, I want a contract-driven, strictly reference-counted FFI layer so that memory lifecycles, error context, and multi-core thread budgets are deterministically preserved across asynchronous Swift Tasks and Rust background threads.*

- **FR-001 (Contract-Driven ABI Validation)**: All C-ABI interfaces, struct layouts, and packed data envelopes must be formally declared in verifiable JSON Schemas and validated via automated contract linters.
- **FR-002 (Unified Handle Reference Counting)**: All long-lived Rust opaque handles (`CancellationToken`, `FilterExpr`, `ArchiveContext`, `VfsSession`) exposed to Swift must use explicit atomic reference counting (`Arc<T>` with `retain`/`release` C-ABI endpoints), eliminating any possibility of premature deallocation.
- **FR-003 (Structured Error Channel Model)**: Deprecate thread-local storage (TLS) `LAST_ERROR` as the primary error propagation mechanism. Core FFI endpoints must accept explicit `out_error: *mut TTZipErrorInfo` out-parameters to guarantee diagnostics are preserved across Swift Task hopping.
- **FR-004 (Contiguous Buffer Marshaling)**: All multi-string and batch metadata exchanges between Swift and Rust must use single-allocation packed buffers (`TTZipPackedStringArray`), eliminating per-element `strdup`/`free` heap fragmentation.

### 2.2 User Story 2 - Defensive Systems Architecture & Anti-Traversal Verification
*As a security-conscious user, I want the archive engine to defend against malicious path traversal, multi-byte boundary corruption, and cryptographic side-channels so that archive operations cannot compromise the host filesystem or leak sensitive data.*

- **FR-005 (Intermediate Symlink Traversal Barrier)**: Path extraction and safe directory creation must recursively inspect every ancestor path segment using POSIX `lstat` / `realpath` to guarantee that no intermediate component resolves to a symlink escaping the target extraction sandbox.
- **FR-006 (Unicode-Aware Boundary Truncation)**: Header generation for legacy formats (TAR USTAR 100-byte name field, ZIP entry extra fields) must strictly truncate at UTF-8 character boundaries (`floor_char_boundary`), preventing multi-byte Unicode panics.
- **FR-007 (Side-Channel Resistant Cryptography)**: All cryptographic verification routines (WinZip AES MAC, PVV header verification, Password Vault authentication tags) must execute in constant time (`subtle::ConstantTimeEq`), eliminating timing side-channels.
- **FR-008 (Dynamic Slice Scaling for Erasure Coding)**: Reed-Solomon recovery record generation must adaptively scale slice sizes based on total archive size to guarantee Cauchy matrix invertibility in $\text{GF}(2^8)$ for archives of arbitrary gigabyte/terabyte scale.

### 2.3 User Story 3 - Lock-Free Concurrency & Zero-Allocation Hotpaths
*As a performance-focused user, I want archive indexing, VFS searching, and cache operations to execute with zero heap allocation churn and non-blocking I/O so that UI rendering remains smooth at 60/120 FPS.*

- **FR-009 (Two-Phase Lock Splitting Pattern)**: Shared concurrent data structures (e.g. `VFSLz4CachePool`, `VfsTree`) must strictly decouple eviction/mutation decision-making from physical disk I/O. Shard write locks must only protect in-memory pointer swaps; physical file reads/writes must occur outside the lock.
- **FR-010 (Zero-Allocation Case-Insensitive Matching)**: VFS node sorting, prefix filtering, and fuzzy string matching must operate over Unicode character iterators without allocating intermediate `String` instances on the heap.
- **FR-011 (O(N) Pre-Indexed VFS Construction)**: VFS tree construction from flat entry metadata must use hash-indexed parent resolution (`VfsTreeBuilder`), maintaining linear $O(N)$ complexity even for archives containing $>100,000$ files.
- **FR-012 (Direct Streamed Split Archive Writing)**: Multi-volume archive creation must write directly to segmented file chunks using `archive_write_open2` custom I/O callbacks, eliminating intermediate full-file temporary disk writes.

### 2.4 User Story 4 - Multi-Matrix Testing, Property Fuzzing & Performance Gate
*As a quality engineering maintainer, I want automated cross-architecture CI, property-based boundary testing, and P99 latency regression gates so that no regression escapes into production.*

- **FR-013 (Cross-Architecture CI Matrix)**: CI pipelines must build and execute unit and differential tests across both `aarch64-apple-darwin` (ARM64) and `x86_64-apple-darwin` / `x86_64-unknown-linux-gnu` targets to ensure software fallback algorithm parity.
- **FR-014 (Property-Based & Fuzzing Suite)**: Property tests (`proptest`) must test mathematical codec invariants, roundtrip lossless fidelity, and cryptographic key expansion across randomly generated byte sequences and Unicode permutations.
- **FR-015 (Statistical A/B Benchmark Gate)**: The release verification gate must enforce multi-round interleaved A/B benchmark comparisons against the baseline commit, verifying zero throughput degradation ($\Delta\% \ge 0\%$) and tail latency bounds.
- **FR-016 (Bounded Resource LRU Protections)**: All in-memory metadata caches in Swift and Rust must enforce bounded capacities (e.g. 2048-entry LRU for VFS raw chunk sizes) to prevent memory leakage under long-running daemon workloads.

---

## 3. Success Criteria

- **SC-001 (Zero UAF & Lifetime Invariants)**: 100% of C-ABI handle interactions pass AddressSanitizer (ASan) and ThreadSanitizer (TSan) without use-after-free or data races.
- **SC-002 (100% Contract Compliance)**: All C-ABI data structures validate cleanly against contract schemas via `lint-contracts.sh` with 0 warnings.
- **SC-003 (Zero Regression Throughput Parity)**: All 57 hardware Codec test points in `ttzip-bench gate` maintain $\Delta\% \ge 0\%$ throughput vs baseline.
- **SC-004 (Sub-Second 10K Node VFS Search)**: 10,000-node VFS tree search executes in $<100\text{ms}$ with zero intermediate heap allocations.
- **SC-005 (Anti-Traversal Sandbox Guarantee)**: 100% of malicious path traversal test vectors (including symlink jumping, relative escaping, and intermediate symlinks) are blocked with `ErrSecurityViolation`.
- **SC-006 (Arbitrary File Size RS-FEC Recovery)**: Recovery record creation succeeds and repairs corrupted streams across files from $1\text{KB}$ to $>100\text{GB}$ without arithmetic overflow in $\text{GF}(2^8)$.
- **SC-007 (Tail Latency Consistency)**: End-to-end multi-file compression and extraction roundtrip standard deviation stays under $\pm 500\text{ms}$, eliminating $10\text{s}+$ lock contention spikes.
- **SC-008 (Dual Architecture Parity)**: All crypto, codec, and VFS tests pass identically on ARM64 hardware and x86_64 software targets.

---

## 4. Key Entities & Data Models

- **`TTZipErrorInfo`**: C-ABI structured error envelope containing status code, error domain, human-readable UTF-8 message buffer, and file/line telemetry.
- **`TTZipPackedStringArray`**: Contiguous memory buffer holding $N$ packed UTF-8 strings with uint32 offset tables for zero-fragment FFI transfers.
- **`VfsTreeBuilder`**: Hash-pre-indexed linear-time builder for hierarchical directory representation.
- **`TwoPhaseEvictionPlan`**: Shard-level eviction metadata decoupled from physical I/O writes.
