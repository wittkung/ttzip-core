# Implementation Research: TTZip CLI & Full Multi-Language SDK Architectural Evolution

- **Feature ID**: `005-cli-and-multi-language-sdk-architecture-evolution`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `RESEARCH_COMPLETE`
- **Created**: 2026-08-24

---

## 1. Research Decision Matrix

### 1.1 Canonical C-ABI 2.0 & Unified Foreign Function Interface
- **Decision**: Introduce Canonical C-ABI 2.0 with:
  1. Universal Deallocator: `void ttzip_free(void *ptr, TTZipMemoryKind kind)` handling strings, buffers, aligned memory, and descriptors uniformly.
  2. Thread-Safe Error Delivery: Out-pointer descriptor `TTZipError **out_error` passing error code, OS errno, byte offset, entry path, and diagnostic message.
  3. Binary Stability Envelope: Every exported struct incorporates `uint32_t struct_size` and `uint32_t abi_version`.
  4. Zero-Copy Descriptors: `TTZipBufferRef { const uint8_t *data; size_t len; }` and `TTZipBufferMut { uint8_t *data; size_t len; size_t capacity; }`.
- **Rationale**: The previous design exported 8 fragmented free functions and stored error messages in thread-local storage (`RefCell` in TLS). In asynchronous multi-threaded environments (Swift Concurrency, Tokio, Rayon, GCD), worker threads terminate or switch context before error querying, yielding dangling pointers and segfaults.
- **Alternatives Considered**:
  - *Keep TLS errors with mutex protection*: Rejected because it introduces cross-thread lock contention and does not solve thread termination UAF.
  - *Return error strings directly as return values*: Rejected because it prevents returning rich diagnostic context (offsets, paths, HTTP/OS status codes) alongside numeric error enums.

### 1.2 CLI 2.0 Streaming Architecture & Memory-Mapped File Ingestion
- **Decision**: Replace `fs::read(path)` across `ttzip-tui` and `ttzip-engine` single-entry extractors with memory-mapped I/O (`memmap2`) for files $>16\text{MB}$ and fixed 64KB/128KB `BufReader` streams for sequential processing.
- **Rationale**: Ingesting 20GB–50GB archives via `fs::read` attempted multi-gigabyte contiguous heap allocations, triggering immediate OOM kills. Virtual memory mapping provides kernel page-cache-backed zero-copy slices, bounding peak memory to $\le 64\text{MB}$ regardless of archive size.
- **Alternatives Considered**:
  - *Dynamic chunked heap buffers*: Rejected because memory-mapped access allows random-access Central Directory and 7z header parsing at zero allocation cost.
  - *Temporary disk unpack caches*: Rejected due to SSD write amplification and security risks with sensitive unencrypted files.

### 1.3 Swift 6 SDK 2.0: Actor Hierarchy & Noncopyable (`~Copyable`) PageBuffers
- **Decision**: Re-architect `TTZipCore` into a 2-tier Swift 6 model:
  1. `public actor TTZipEngine`: Unified public actor owning asynchronous archive operations, password vault coordination, and task lifecycle.
  2. `public struct TTZipArchive: Sendable`: Immutable, lock-free archive representation with `AsyncStream<ArchiveProgress>` and async entry iterators.
  3. `~Copyable struct PageBuffer`: Single-ownership compile-time noncopyable memory buffer with automatic deterministic C-ABI deallocation on scope exit.
- **Rationale**: Resolves over 40 `@unchecked Sendable` bypasses, eliminates lock allocation explosion in `ArchiveTreeNode` (10,000+ mutexes), and cures the immortal `Task` chained memory leak in `CommandHistoryManager`.
- **Alternatives Considered**:
  - *Retain 4-tier Facade/Pipeline/Command/Implementor hierarchy with mutexes*: Rejected due to architectural over-engineering, circular call stacks (up to 6 hops per operation), and persistent Swift 6 strict concurrency warnings.
  - *Combine framework reactive streams*: Rejected in favor of native Swift Concurrency `AsyncStream` and `AsyncSequence` for zero-overhead, multiplatform Apple/Linux compatibility.

### 1.4 Python SDK 2.0: PyO3 GIL-Free Execution & Buffer Protocol
- **Decision**:
  1. Wrap all CPU-intensive compression/decompression kernels with `py.allow_threads(...)`.
  2. Implement the Python Buffer Protocol (`PyBuffer<u8>`), supporting zero-copy `memoryview`, `bytearray`, and NumPy array decompression.
  3. Provide `with ttzip.open(...) as arc:` context managers and standard `zipfile.ZipFile` drop-in replacement classes.
  4. Fix Zstandard frame decompression with a streaming decoder fallback when frame uncompressed size is omitted, and adopt the standard RFC 8878 / LZ4 Frame format.
- **Rationale**: Resolves GIL serialization where 16 Python threads were bottlenecked on a single core, and fixes decompression errors on high-compression streams.
- **Alternatives Considered**:
  - *Python multiprocessing fallback*: Rejected due to heavy IPC serialization overhead compared to in-process GIL-free native threads.

### 1.5 Multi-Language SDK Ecosystem: True Native FFI vs Pseudo Subprocesses
- **Decision**:
  - **Java 22+ / Kotlin**: Adopt `java.lang.foreign.*` (Foreign Function & Memory API - FFM) with `Arena.ofConfined()` and `MemorySegment` downcalls. Eliminate all `ProcessBuilder` shell spawns and scalar Java CRC loops.
  - **Dart / Flutter**: Adopt real `dart:ffi` dynamic library binding with background Flutter `Isolate` dispatch and `Stream<ArchiveProgress>`. Eliminate all `Process.run('ttzip')` invocations.
  - **C# / .NET 8+**: Adopt `ReadOnlySpan<byte>`, `SafeHandleZeroAlloc`, and `IAsyncEnumerable` with Native AOT support.
  - **C++20**: Implement official header-only RAII wrapper `ttzip.hpp` utilizing `std::span`, `std::expected`, and `std::filesystem::path`.
  - **Go**: Implement `ttzip-go` exposing standard library `io/fs.FS` virtual filesystem and `context.Context` cancellation.
- **Rationale**: Pseudo-implementations (spawning CLI binaries via shell subprocesses) completely fail in mobile app sandboxes (iOS/Android), containerized cloud functions without Rust toolchains, and high-throughput server workloads.
- **Alternatives Considered**:
  - *Legacy JNI (Java Native Interface)*: Rejected in favor of modern standard Java 22+ FFM which requires zero C JNI glue code and avoids JNI JNIEnv synchronization overhead.
  - *CLI daemon with JSON-RPC IPC*: Rejected due to IPC serialization overhead, socket lifecycle management complexity, and lack of mobile OS support.

---

## 2. Best Practices & Standard References

1. **SEI CERT C / C-ABI Standards**: Fixed-size structs, aligned layouts, opaque pointers, explicit error status returns.
2. **Swift 6 Strict Concurrency Guideline**: Zero data races, Actor isolation, `@Sendable` closure boundaries, typed throws.
3. **Java FFM API (JEP 454)**: Structured lifetime arenas, zero-copy native memory segments, downcall handles.
4. **Dart FFI Specification**: Native function bindings, finalizers for GC-managed native handles, isolate compute isolation.
5. **Zstandard RFC 8878 & LZ4 Frame Specification**: Interoperable frame headers, magic numbers, streaming block decoding.
