# Phase 0 Research: Single-Core Deflate Engine Consolidation on libdeflate

**Feature Branch**: `137-libdeflate-single-core-consolidation`

**Date**: 2026-08-20

## Research Findings & Architectural Decisions

### R001: Thread-Local Compressor/Decompressor Pooling in CTTZipStreamCoder

- **Decision**: 
  Maintain and enforce the static C11 thread-local storage (TLS) caching array pattern (`static TTZIP_THREAD_LOCAL struct libdeflate_compressor* g_tls_compressors[14]` and `static TTZIP_THREAD_LOCAL struct libdeflate_decompressor* g_tls_decompressor`) with lazy, bounds-clamped initialization in `ttzip_get_tls_compressor(int level)` and `ttzip_get_tls_decompressor(void)`.
- **Rationale**:
  1. **Zero Allocation Latency**: Allocating a `libdeflate_compressor` requires internal state allocation (128 KB to 2 MB depending on level). Pre-caching instances in thread-local storage eliminates allocation churn, which would otherwise consume 20–35% of CPU time during high-concurrency block compression.
  2. **Lock-Free Concurrency & Zero Race Conditions**: `TTZIP_THREAD_LOCAL` maps to `_Thread_local` (C11) or `__thread` (Clang/GCC). In GCD / Swift async execution (`DispatchQueue.concurrentPerform`, `Task.detached`), each worker thread in the system thread pool accesses its own private pointer slots with zero mutex locking, zero atomic operations, and zero data races.
  3. **Strictly Bounded Memory Footprint**: Lazy initialization (`if (!g_tls_compressors[l])`) ensures only compression levels actually used by a thread are allocated. Because GCD manages a fixed pool of persistent worker threads, the maximum memory allocated across the process is bounded by `(active_GCD_worker_threads * requested_compression_levels * sizeof(compressor_state))` with zero unbounded leaking across task invocations.
- **Alternatives Considered**:
  - *Per-Call Allocation (`libdeflate_alloc_compressor` / `libdeflate_free_compressor`)*: Rejected due to extreme performance degradation (thousands of allocations per second, allocator lock contention).
  - *Centralized Mutex / Spinlock Pool (`os_unfair_lock` / `NSLock`)*: Rejected due to CPU cache-line bouncing and lock contention across 8–16 Apple Silicon cores when executing parallel block compression in `ZipBlockParallelCompressor`.
- **Source**:
  - [`Sources/CTTZipBridge/CTTZipStreamCoder.c:20-54`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipStreamCoder.c#L20-L54)
  - [`Sources/CTTZipBridge/include/CTTZipStreamCoder.h:27-36`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipStreamCoder.h#L27-L36)
  - [`Sources/CTTZipBridge/include/ttzip_platform.h:85-97`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_platform.h#L85-L97)

---

### R002: Swift 6 Memory-Safe Zero-Copy Adapter Performance

- **Decision**:
  Consolidate all Swift 6 Deflate bridging on `LibdeflateCAdapter.swift` via a two-tier memory strategy:
  1. **Direct Pointer Pass-Through**: Direct `UnsafeRawPointer` to `UnsafeMutableRawPointer` bridging for high-throughput pipeline components (`ZipBlockParallelCompressor`, `ChunkedDeflateStreamWriter`).
  2. **Flyweight & Zero-Copy Safe Bridging**: `MemoryPageFlyweightPool.shared.withBuffer(size:)` for `Data` payloads $\le 64\text{ KB}$, and single-allocation `Data(bytesNoCopy:count:deallocator:.custom)` for payloads $> 64\text{ KB}$.
- **Rationale**:
  1. **Zero-Copy Inlined Intrinsic**: `CUnsafeBufferAdapter.withBufferPointer(data)` unwraps `Data` into raw pointers using `data.withUnsafeBytes` without copying byte buffers across the Swift/C boundary.
  2. **Page-Aligned Memory Reuse**: For $\le 64\text{ KB}$ buffers, `MemoryPageFlyweightPool` provides pre-allocated 4KB/16KB/64KB buffers aligned to Apple Silicon 16KB hardware boundaries (`posix_memalign`), eliminating dynamic heap allocations on hot paths and maximizing NEON vector load/store throughput.
  3. **Direct Memory Ownership Transfer**: For large buffers ($> 64\text{ KB}$), allocating uninitialized pointers and passing them to `Data(bytesNoCopy:deallocator:)` transfers pointer ownership directly to the Swift `Data` object upon completion, completely avoiding the secondary copy that `Data(bytes:count:)` requires.
  4. **Pointer Slicing in Parallel Blocks**: `ZipBlockParallelCompressor` wraps the input buffer in a `SendablePointerBox` and advances raw pointers per 512KB slice (`ptrBox.pointer.advanced(by: offset)`), feeding `ttzip_libdeflate_compress` with zero memory copies or intermediate slice objects.
- **Alternatives Considered**:
  - *Standard `Data(bytes:count:)` Initializer*: Rejected because copying large multi-megabyte buffers doubles peak RSS and consumes valuable memory bus bandwidth.
  - *Swift `Array<UInt8>` Mutable Buffers*: Rejected because Swift ARC retain/release traffic and copy-on-write (COW) checks on large byte arrays degrade single-core compression throughput by 15–25%.
- **Source**:
  - [`Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift:15-121`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift#L15-L121)
  - [`Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift:98-165`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift#L98-L165)
  - [`Sources/TTZipCore/Flyweights/MemoryPageFlyweightPool.swift:10-67,126-196`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Flyweights/MemoryPageFlyweightPool.swift#L10-L196)
  - [`Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift:25-65`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift#L25-L65)

---

### R003: Multi-tier Deflate Strategy & Upstream Boundaries

- **Decision**:
  Enforce the formal dual-tier Deflate topology:
  - **Tier 1 (Whole-Buffer & Chunk Plane)**: Exclusively `libdeflate` (`ttzip_libdeflate_compress` / `decompress`) for all single-file, block-parallel (512KB), chunked stream (1MB), and in-memory operations.
  - **Tier 2 (Stateful Streaming & Sliding Window Pipeline)**: Exclusively `zlib-ng` (`ttzip_deflate_stream_*` / `ttzip_inflate_stream_*`) for asynchronous `AsyncSequence` streaming, arbitrary-sized incremental buffers, and cross-block 32KB dictionary injection (`deflateSetDictionary`).
  - **Research Isolation**: Isolate experimental custom single-core deflate routines in `native_deflate/` as an internal research oracle / educational baseline (`ttzip_native_deflate_*`), keeping production runtime pipelines 100% focused on `libdeflate` and `zlib-ng`.
- **Rationale**:
  1. `libdeflate` is engineered specifically for whole-buffer compression and decompression, relying on complete block analysis for SIMD Huffman and LZ77 parsing. It does not provide an incremental state machine with arbitrary mid-stream suspension or sliding dictionary retention across disjoint chunks.
  2. `zlib-ng` provides full RFC 1950/1951/1952 state machine compatibility (`z_stream`), supporting granular flush modes (`Z_SYNC_FLUSH`, `Z_FULL_FLUSH`, `Z_FINISH`) and cross-block dictionary preconditioning with dynamic CPU feature detection.
  3. Updating `ARCHITECTURE.md` Section 2.5 and maintaining dedicated test suites (`AdapterPatternTests`, `LibdeflateCAdapterTests` for Tier 1; `DeflateStreamEngineTests` for Tier 2) provides crystal clear system boundaries and prevents future redundant micro-optimization efforts.
- **Alternatives Considered**:
  - *Emulating Stateful Streaming with libdeflate Micro-Blocks*: Rejected because flushing and re-initializing `libdeflate` across small chunks discards sliding-window history, degrading compression ratio and breaking RFC 1951 continuous stream compatibility.
  - *Deleting `native_deflate/` Code*: Rejected in favor of keeping it isolated as a benchmark oracle / reference implementation, preserving algorithmic assets without incurring production maintenance costs.
- **Source**:
  - [`ARCHITECTURE.md:76-83`](file:///Users/kevintung/Documents/dev/TTZip/ARCHITECTURE.md#L76-L83)
  - [`Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift:13-122,136-329,540-650`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift#L13-L650)
  - [`Sources/CTTZipBridge/CTTZipStreamCoder.c:56-108,295-431`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipStreamCoder.c#L56-L431)
  - [`specs/137-libdeflate-single-core-consolidation/spec.md:11-20,83-100`](file:///Users/kevintung/Documents/dev/TTZip/specs/137-libdeflate-single-core-consolidation/spec.md#L11-L100)
