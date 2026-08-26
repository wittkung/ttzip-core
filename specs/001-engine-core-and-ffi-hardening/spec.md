# Feature Specification: TTZip Engine Core & FFI Layer Hardening

- **Feature ID**: `001-engine-core-and-ffi-hardening`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `DRAFT`
- **Author**: Antigravity / CTO Persona
- **Target Subsystems**: `ttzip-engine` (Rust), `CTTZipBridge` (C-ABI), `TTZipCore` (Swift)

---

## 1. Problem Statement & Motivation

During a systematic architectural, concurrency, and security audit of TTZip, critical defects, memory safety vulnerabilities, and performance bottlenecks were discovered across the Rust microkernel, C-ABI bridge, and Swift glue layers:
1. **Archive Formats & Solid Safety**:
   - Non-boundary string slicing panic in TAR writer.
   - Missing `0x17` `EncodedHeader` parsing in 7z.
   - Intermediate symlink traversal vulnerability during extraction.
   - 7z Solid decompression and compression全量物化 in memory (`Vec<u8>`), causing severe OOM on large archives.
   - 7z writer hardcoding `dict_prop = 20u8` (4MB), corrupting Maximum/Ultra level archives for standard unpackers.
   - ZIP `streaming_parallel` accumulating all compressed payload blocks in memory before writing to disk.
   - ZIP multi-threaded extraction silently dropping progress callbacks and rendering cancellation flags non-functional.
   - ZIP extraction hardcoding WinZip AES-256 (28-byte salt/tag assumption), breaking ZipCrypto, AES-128, and AES-192 extraction.
2. **Cryptography & Resilience**:
   - Broken AES-256-CBC chaining and incorrect inverse round keys on non-AArch64 platforms.
   - 12.8MB hard failure on Reed-Solomon recovery records.
   - Timing side-channels in WinZip MAC comparisons and hand-rolled GHASH in `vault.rs` due to secret-dependent bit branching.
   - 7z password recovery target hardcoding `salt: vec![]` and `num_cycles_power: 19`, failing on standard salted 7z files.
   - Password Vault auto-unlock performing destructive trial extraction directly to disk.
3. **Cross-Language FFI & Swift Concurrency**:
   - `thread_local!` error store escaping raw pointers from dropped borrow guards (UB/UAF and lost error context across Swift `Task` thread hops).
   - Swift `ArchiveEngineBridge` `compressStream` / `extractStream` marked `async` but executing synchronous blocking C-ABI calls on caller/MainActor threads (Fake Async).
   - `ProgressBridgeContext` calling `Task.isCancelled` inside POSIX C callbacks where Swift Task-local context is absent.
   - Post-extraction disk re-scanning (`calculateDirectorySize`) creating catastrophic I/O lag on 100,000 small files.
   - `CUnsafeBufferAdapter` performing `.pointee = 0` assignment on uninitialized memory (Swift UB).
   - `extractSingleEntryData` blindly allocating fixed 32MB buffers, causing memory churn on small files and truncation failures on >32MB files.
4. **VFS, I/O & Toolchain Architecture**:
   - VFS Cache Pool Arena LRU `free_indices` never being popped or reused, causing unbounded memory array leakage.
   - VFS cache shard write lock held during synchronous `fs::read` and `lz4_decompress`.
   - Build script `scripts/build_rust.sh` invoking `lipo -extract arm64` and stripping `x86_64` architecture slices from Universal binaries.

---

## 2. Requirements & User Stories

### 2.1 Archive & Decompression Security
- **REQ-01 (TAR UTF-8 Safety)**: The TAR writer must truncate strings to the 100-byte `ustar` header limit strictly on valid UTF-8 character boundaries without triggering panic. Full paths must continue to be stored in PAX extended headers.
- **REQ-02 (7z EncodedHeader Support)**: The 7z metadata parser must detect `K_ENCODED_HEADER` (`0x17`), decompress the packed header stream via `decode_7z_solid_payload`, and recursively parse the inner metadata tags.
- **REQ-03 (Anti-Traversal Symlink Barrier)**: Safe extract must verify that no intermediate directory component in an extraction path is a symlink. Symlink target paths must be strictly validated within destination directory root.
- **REQ-04 (Streaming 7z Decompression & Dynamic Dictionary Mapping)**:
  - Replace full-memory solid stream materialization with a zero-materialization streaming state machine (`Streaming7zExtractor` backed by `Fl2DStream`).
  - 7z writer must dynamically query `ctx.dict_property()` and map compression levels to standard dictionary sizes (Fastest: 256KB, Normal: 16MB, Maximum: 32MB, Ultra: 64MB).
- **REQ-05 (Bounded Channel ZIP Parallel Pipeline)**:
  - Parallel ZIP compression must use a bounded MPSC channel (bounded at 16 chunks / 64MB) where worker threads push compressed chunks and an I/O thread flushes them via POSIX `pwrite`.
  - Multi-threaded ZIP extraction must correctly invoke throttled progress callbacks and check atomic cancellation flags.
  - ZIP reader must support ZipCrypto, WinZip AES-128, AES-192, and AES-256.

### 2.2 Cryptography & Recovery Records
- **REQ-06 (Cross-Platform AES-256-CBC Correctness)**: AES-256-CBC encryption/decryption on non-AArch64 platforms must maintain continuous cipher state across blocks and generate correct inverse round keys with `InvMixColumns`.
- **REQ-07 (Dynamic Slice Scaling for RS-FEC)**: Reed-Solomon recovery record creation must dynamically scale `slice_size` based on archive length to guarantee $K \le 200$ and $K+M \le 256$ in $\text{GF}(2^8)$, supporting arbitrary archive sizes with 4096-byte alignment.
- **REQ-08 (Constant-Time Operations & System Crypto Integration)**:
  - Replace hand-rolled GHASH in `vault.rs` with standard `aes-gcm` or Swift `CryptoKit.AES.GCM` to eliminate timing side-channels and exploit hardware `PMULL` / `AES-NI`.
  - WinZip MAC and PVV verification must execute constant-time comparisons.
- **REQ-09 (Full 7z Recovery Target Parsing & Non-Destructive Vault Probing)**:
  - 7z recovery target inspector must extract real Salt and NumCyclesPower from 7z Coder Properties.
  - Password Vault auto-unlock in `quickExtract` must probe passwords with in-memory non-destructive inspection before writing to disk.

### 2.3 FFI, Lifecycle & Concurrency
- **REQ-10 (Deterministic Error Envelope Out-Parameter)**: Replace `thread_local!` error store with a structured value-passed `TTZipErrorInfo` C-ABI contract (`status`, `error_code`, `message[512]`, `entry_path[256]`, `offset`).
- **REQ-11 (True Swift Concurrency & Cancellation Context Binding)**:
  - Swift `ArchiveEngineBridge` implementors must wrap blocking C-ABI calls in `Task.detached(priority:)` or background dispatchers to never block caller or MainActor threads.
  - `ProgressBridgeContext` must bind Swift `withTaskCancellationHandler` cancellation checks rather than evaluating bare `Task.isCancelled` in C callbacks.
- **REQ-12 (Direct FFI Byte Accounting)**: `ttzip_rust_archive_extract_unified_v2` must return extracted byte counts directly via `out_extracted_bytes: *mut u64`, eliminating post-extraction `calculateDirectorySize` recursive disk scans.
- **REQ-13 (Memory Safe Buffers & Precise Allocation)**:
  - `CUnsafeBufferAdapter` must use `UnsafeMutablePointer<UInt8>.allocate` with explicit `initialize(to: 0)` and `deinitialize(count:)`.
  - `extractSingleEntryData` must use two-stage exact allocation (probe size first, then allocate exact `Data(count: probedLen)`).

### 2.4 VFS, Cache & Toolchain
- **REQ-14 (VFS Cache Arena Slot Reuse & Lock-Free I/O)**:
  - `VFSLz4CachePool` must pop and reuse `free_indices` on node insertions.
  - Node reads and writes must execute disk I/O and LZ4 decompression outside shard write locks.
- **REQ-15 (Universal Binary Build Script Fix)**:
  - `scripts/build_rust.sh` must retain both `arm64` and `x86_64` slices in `libTTZipVendor.a` and update `TTZipVendor.xcframework` identifiers to `macos-arm64_x86_64`.

---

## 3. Success Metrics
1. **Zero Memory Leaks & Zero UB**: Valgrind/ASan clean execution on 1,000,000 VFS cache insertions, string conversions, and error reporting.
2. **Bounded Memory Footprint**: Peak RAM during 100GB ZIP/7z extraction and compression capped at $\le 128\text{MB}$.
3. **Responsive Concurrency**: UI frame rate maintains 60/120 FPS on `@MainActor` during heavy archiving operations with $<10\text{ms}$ cancellation abort latency.
4. **I/O Overhead Elimination**: Post-extraction processing latency for 100,000 small files reduced from $>5\text{s}$ to $0\text{s}$.
5. **Universal Multi-Arch**: 100% build and test pass on Apple Silicon (arm64) and Intel (x86_64 / Rosetta).
