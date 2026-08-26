# Comprehensive Research & Architecture Decisions: TTZip Systemic Governance & Resilience

- **Feature ID**: `002-systemic-architecture-and-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `RESOLVED`

---

## 1. Research Decision 1: C-ABI Lifecycle Management & Reference Counting (`US1`)

- **Context**: Swift 6 structured concurrency executes across a cooperative thread pool, hopping across POSIX worker threads. When Swift `Task` cancellation or out-of-scope deinitialization occurs, calling `free()` on raw C pointers while background Rust threads (e.g. Rayon / POSIX decompression loops) dereference `(*token).is_cancelled()` causes fatal Use-After-Free (UAF) / SIGSEGV hazards.
- **Decision**: All long-lived Rust opaque handles (`CancellationToken`, `FilterExpr`, `VfsSession`) must encapsulate their inner state in `Arc<T>` and expose symmetric C-ABI retain/release primitives (`ttzip_rust_<handle>_retain`, `ttzip_rust_<handle>_release`). Swift wrapper types (`TaskExecutionHandle`, `ProgressBridgeContext`, `ClosureBox<T: Sendable>`) must retain the token upon creation/dispatch and release upon final deinitialization.
- **Source**: `core/rust/ttzip-engine/src/ffi/runtime_ffi/cancellation_ffi.rs:14-65`, `core/Sources/CTTZipBridge/include/ttzip_rust_glue.h:310-330`, `core/Sources/TTZipCore/Concurrency/TaskExecutionHandle.swift:11-86`.
- **Alternatives Considered**:
  - Raw unmanaged pointers: Vulnerable to premature deallocation during async cancellation.
  - Swift-only actor locks: Cannot coordinate with foreign POSIX C callbacks or Rayon worker threads.

---

## 2. Research Decision 2: Structured Error Propagation & Task Hopping (`US1`)

- **Context**: Thread-local storage (`LAST_ERROR`) fails in Swift structured concurrency because an `async` function may start on Thread $T_1$, execute a C-ABI call on worker Thread $T_2$, and resume on Thread $T_3$, silently losing the TLS error context and returning interior pointers with undefined lifetimes.
- **Decision**: Define a stack-allocated C-ABI structured error envelope `TTZipErrorInfo` passed as an explicit out-parameter (`out_error: *mut TTZipErrorInfo`). All core Rust FFI functions write structured failure diagnostics (status code, subsystem domain, UTF-8 message, file, and line) directly into caller-provided stack memory before returning negative status codes.
- **Source**: `core/rust/ttzip-engine/src/types.rs:218-272`, `core/Sources/CTTZipBridge/include/ttzip_rust_glue.h:123-129`, `core/Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift:426-490`.
- **Alternatives Considered**:
  - String pointer returns (`*mut c_char`): Requires explicit deallocation and causes memory leaks if uncaught.
  - Global error queues: Introduces global lock contention across concurrent extraction tasks.

---

## 3. Research Decision 3: Zero-Fragment Contiguous Buffer Marshaling (`US1`)

- **Context**: Passing 100,000 entry paths across FFI previously performed 100,000 individual `strdup` and `free` heap allocations ($1.6\text{MB}-3.2\text{MB}$ chunk header overhead, $45\text{ms}-120\text{ms}$ latency, and severe memory fragmentation).
- **Decision**: Adopt `TTZipPackedStringArray` memory layout: a single contiguous byte buffer with a header storing string count, total payload length, and packed uint32 offset and length tables ($4.4\text{MB}$ total, $<1.2\text{ms}$ marshaling latency).
- **Source**: `core/Sources/TTZipCore/Bridge/CUnsafeBufferAdapter.swift:37-94`, `core/Sources/CTTZipBridge/include/ttzip_rust_glue.h:131-136`, `core/rust/ttzip-engine/src/types.rs`.
- **Alternatives Considered**:
  - JSON serialization: High CPU parsing overhead for 100k items.
  - Null-delimited strings: Scanning $O(N)$ length on every element lookup.

---

## 4. Research Decision 4: POSIX Intermediate Symlink Traversal Verification (`US2`)

- **Context**: POSIX `O_NOFOLLOW` only protects the terminal leaf node of a path. If an intermediate directory (e.g. `archive_root/sub_dir`) is a symlink pointing to `/etc`, opening `archive_root/sub_dir/passwd` will escape the extraction root and compromise the host filesystem.
- **Decision**: Implement `validate_no_intermediate_symlinks` to traverse every parent component iteratively, verifying via `libc::lstat` that no intermediate component is a symlink before allowing directory creation or file writes. Combine with two-stage bottom-up deferred metadata restoration (files first, deepest directories first).
- **Source**: `core/rust/ttzip-engine/src/fs/safe_extract.rs:18-39, 224-306`, `core/rust/ttzip-engine/src/archive/unified/extract.rs:167-192`.
- **Alternatives Considered**:
  - Relying solely on `canonicalize()`: Fails on non-existent target files during archive creation.
  - Post-extraction scanning: Vulnerable to time-of-check to time-of-use (TOCTOU) race attacks.

---

## 5. Research Decision 5: Unicode Multi-Byte Boundary Safety (`US2`)

- **Context**: Direct slicing on Rust strings (e.g. `short_name = &path[..100]`) panics when the 100-byte boundary lands in the middle of a 3-byte CJK or 4-byte Emoji codepoint.
- **Decision**: Implement `truncate_to_char_boundary` using backward scanning to find the largest index $\le 100$ where `s.is_char_boundary(end)` is true. Emit standard POSIX.1-2001 PAX extended headers (`typeflag = b'x'`) to preserve lossless full-length UTF-8 paths for modern extractors.
- **Source**: `core/rust/ttzip-engine/src/archive/tar/writer.rs:19-30, 90-174`, `core/rust/ttzip-engine/src/archive/tar/pax.rs:136-160`.

---

## 6. Research Decision 6: Constant-Time Cryptographic Verification (`US2`)

- **Context**: Standard equality comparisons (`!=`) and early-exiting loops in WinZip AES MAC, PVV verification, and Password Vault GCM authentication tags introduce timing side-channel attack vectors.
- **Decision**: Enforce constant-time comparison primitives (`subtle::ConstantTimeEq`, `subtle::Choice`) across all MAC and authentication tag verifications, compiling to branchless bitwise arithmetic masks with `core::hint::black_box` optimization barriers.
- **Source**: `core/rust/ttzip-engine/src/crypto/sha1/winzip.rs:17-26, 81-87`, `core/rust/ttzip-engine/src/crypto/vault.rs:164-171, 299`.

---

## 7. Research Decision 7: Dynamic Slice Scaling for Erasure Coding in $\text{GF}(2^8)$ (`US2`)

- **Context**: In Systematic Cauchy Reed-Solomon coding over Galois Field $\text{GF}(2^8)$, the Cauchy matrix non-singularity invariant requires $X \cap Y = \emptyset$, imposing a strict mathematical ceiling $K + M \le 256$. A fixed 64KB slice size forces $K > 200$ for files $>12.8\text{MB}$ and $K > 256$ for files $>16.78\text{MB}$, causing fatal matrix singularity.
- **Decision**: Implement Dynamic Slice Scaling with 4KB page alignment:
  $$S_{\min} = \left\lceil \frac{L}{K_{\max}} \right\rceil = \left\lceil \frac{L}{200} \right\rceil, \quad S_{\text{eff}} = \left\lceil \frac{\max(S_{\text{base}}, S_{\min})}{4096} \right\rceil \times 4096$$
  Guarantees $K \le 200$ and $K+M \le 256$ for arbitrary file sizes from $1\text{KB}$ to $>100\text{GB}$ with $<4\text{MB}$ memory overhead.
- **Source**: `core/rust/ttzip-engine/src/crypto/rs_fec/record_format.rs:73-93`, `core/rust/ttzip-engine/src/crypto/rs_fec/cauchy.rs:20`.

---

## 8. Research Decision 8: Two-Phase Lock Splitting for Concurrent Cache Pools (`US3`)

- **Context**: Holding shard write locks during synchronous disk I/O (`fs::write`) or decompression (`lz4_decompress`) triggers lock convoys and thread pool head-of-line blocking, producing $30\text{s}+$ tail latency spikes.
- **Decision**: Enforce two-phase lock splitting (`plan_evictions`):
  1. *Phase 1 (Critical Section, Nanoseconds)*: Acquire shard write lock, update LRU pointers, detach evicted RAM buffers into local variables, drop lock immediately.
  2. *Phase 2 (Out-of-Lock Physical I/O)*: Perform physical file writes (`fs::write`) and deletions (`fs::remove_file`) without holding any lock.
- **Source**: `core/rust/ttzip-engine/src/vfs/cache_pool.rs:125-147, 231-308`, `core/Sources/TTZipCore/VFS/VFSLz4CachePool.swift:40-146`.

---

## 9. Research Decision 9: Zero-Allocation Unicode String Comparisons (`US3`)

- **Context**: In an archive with 100,000 entries, sorting and searching via `s.to_lowercase()` generated $>3.4 \times 10^6$ heap allocations and tens of megabytes of memory churn.
- **Decision**: Implement `cmp_case_insensitive` and `starts_with_ignore_case` using streaming lazy character iterators (`chars().flat_map(|c| c.to_lowercase())`), comparing Unicode codepoints on the stack with **0 bytes heap allocation**.
- **Source**: `core/rust/ttzip-engine/src/fs/vfs/node.rs:90-105`, `core/rust/ttzip-engine/src/fs/vfs/search.rs:72-84`.

---

## 10. Research Decision 10: Linear-Time VFS Tree Construction ($O(N)$) (`US3`)

- **Context**: Inserting flat metadata entries into `VfsTree` via linear child scans (`children.iter().position(...)`) degraded to $O(N^2)$ algorithmic complexity ($\approx 5 \times 10^9$ comparisons for 100k files).
- **Decision**: Introduce `VfsTreeBuilder` hash pre-indexing (`HashMap<String, usize>`) for parent directory resolution, guaranteeing $O(N)$ linear construction time.
- **Source**: `core/rust/ttzip-engine/src/fs/vfs/tree.rs:15-40`.

---

## 11. Research Decision 11: Single-Pass Streamed Split I/O (`US3`)

- **Context**: Two-pass split archiving (compressing to `/tmp/intermediate.zip` then slicing) doubled disk writes, increased SSD wear, and required $\ge 2\times$ free disk space.
- **Decision**: Implement direct zero-buffer split archiving using libarchive `archive_write_open2` custom callbacks (`split_write_cb`, `split_close_cb`, `split_free_cb`) driving `SplitVolumeWriter`.
- **Source**: `core/rust/ttzip-engine/src/archive/unified/create.rs:54-94, 154-186`, `core/rust/ttzip-engine/src/archive/split/writer.rs:42-225`.

---

## 12. Research Decision 12: Multi-Architecture CI Matrix & Statistical Regression Gates (`US4`)

- **Context**: Hardware NEON crypto instructions on Apple Silicon hid software fallback bugs on non-ARM64 architectures (e.g. missing AES inverse round keys).
- **Decision**:
  1. Build and test against a 4-tier matrix: `aarch64-apple-darwin` (ARM64), `x86_64-apple-darwin` (Intel/Rosetta), `x86_64-unknown-linux-gnu` (Linux), and forced software fallback mode (`--cfg force_software_crypto`).
  2. Implement mathematical statistical A/B benchmark auditing (`ab_performance_audit.py` using Welch's unequal-variance $t$-test and incomplete beta function $p$-values) enforcing zero throughput regression ($\Delta\% \ge 0\%$).
  3. Enforce pre-push gates (`run_local_ci_gate.sh`, LOC $\le 800$, 100% C-ABI symbol alignment, JSON schema contract validation) with zero `--no-verify` bypass.
- **Source**: `core/scripts/ab_performance_audit.py:1-191`, `core/scripts/statistical_delta.py:1-250`, `core/rust/ttzip-engine/tests/property_tests.rs:1-633`.
