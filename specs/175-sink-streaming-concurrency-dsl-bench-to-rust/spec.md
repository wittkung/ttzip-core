# Feature Specification: 175-sink-streaming-concurrency-dsl-bench-to-rust

## 1. Executive Summary & Strategic Motivation
Following the comprehensive Phase 2 audit of remaining non-Rust code in TTZip, several critical computational, streaming, concurrency, and cross-platform bottlenecks were identified:
1. **7z Solid & Seek Performance**: `SevenZipSeekTable.swift` was fully unpacking the entire archive to a temporary directory to extract a single 1KB thumbnail; `SevenZipBlockParallelDecompressor.swift` used temporary intermediate files on disk instead of pure in-memory streaming.
2. **Zstd OOM Risk**: `ZstdCAdapter.swift` loaded multi-gigabyte files into RAM all at once via `Data(contentsOf:)` and preallocated 1GB output buffers.
3. **CPU Polling in WorkerPool**: `ArchiveWorkerPool.swift` and `BoundedProducerConsumerQueue.swift` relied on `Task.sleep` blind polling (10ms/20ms) and `NSLock` contention.
4. **Platform-Tied APIs**: `MIPSHardwareBenchmarkEngine.swift` tied to `mach_absolute_time()`; `CompetitorBenchmarkRunner.swift` tied to `QuartzCore.CACurrentMediaTime()`; `PathPatternFilterEngine.swift` tied to POSIX `fnmatch` (missing on Windows); `DifferentialManifestScanner.swift` tied to Apple `CryptoKit`.
5. **DSL & Glob Inefficiencies**: `ArchiveFilterDSLLexerParser.swift` created heaps of `Character` arrays and runtime regexes.

This feature systematically sinks these 6 core domains into **pure, self-sufficient, cross-platform Safe Rust (`ttzip-glue` and `ttzip-tui`)** with zero-copy streaming, Rayon work-stealing, and true lock-free ring buffers.

---

## 2. Scope of Down-Sinking

### Domain 1: 7z Solid In-Memory Stream Decoder & Instant SeekTable
- **Pure Memory Folder Decoding**: Direct in-memory streaming of 7z Solid blocks (`SevenZipSolidStreamDecoder`) without intermediate `.tmp` disk files.
- **O(1) Memory-Only Single Item Extract**: Extract single items by decoding only their containing Folder directly in RAM and aborting decoding as soon as target bytes are read.
- **Cross-Platform AES & KDF**: Pure Rust ARM64 Crypto / AES-NI acceleration with zero platform-specific `CommonCrypto` dependencies.

### Domain 2: True Bounded Streaming Zstd Engine (Zero OOM)
- **Bounded Buffer Stream Pipeline**: `ZstdStreamingProcessor` operating over 4MB double-buffered ring buffers via `zstd_safe::CStream` / `DStream`.
- **Constant Memory Guarantee**: Resident set size strictly bounded to <16MB regardless of whether compressing/decompressing 10MB or 1TB archives.

### Domain 3: Lock-Free Ring Buffers & Rayon Work-Stealing Concurrency
- **Lock-Free SPSC/MPMC Ring Buffer**: `LockFreeRingBuffer` replacing `NSLock` and Continuation queue arrays.
- **Condition-Based Work-Stealing Worker Pool**: Elimination of all `Task.sleep` polling loops; thread suspension via OS primitives (`parking_lot` Condvar / Futex).

### Domain 4: Zero-Allocation Archive Filter DSL & Cross-Platform Globset
- **Fast Lexer & Zero-Copy AST**: Stack-allocated token stream with `nom`/`winnow` parser.
- **Aho-Corasick Multi-Pattern Glob Matcher**: High-performance globbing via `globset` supporting both POSIX forward slashes and Windows backslashes uniformly.

### Domain 5: In-Memory Benchmarking, High-Precision Monotonic Clock & Pareto Frontier
- **Cross-Platform Monotonic Clock**: `std::time::Instant` replacing `mach_absolute_time()` and `QuartzCore`.
- **Andrew's Monotone Chain 2D Convex Hull**: Pure Rust Pareto frontier calculation for throughput-vs-ratio trade-offs, enabling real-time terminal charting in `ttzip-tui`.

### Domain 6: Cross-Platform Fuzzing & Differential Oracles
- **In-Place Mutation Fuzz Engine**: `bytes::BytesMut` mutation operators eliminating Swift CoW overhead.
- **Cross-Platform Manifest Scanner**: Native `walkdir` + `sha2` engine runnable on Linux, macOS, and Windows CI.

---

## 3. User Scenarios & Acceptance Criteria

### User Scenario 1: Instant Single-File Preview in Huge 7z Archives
- **Given** a 20GB solid 7z archive containing 50,000 files
- **When** the user previews or extracts a single 10KB text file or image
- **Then** extraction completes in <10ms with 0 intermediate bytes written to disk.

### User Scenario 2: Processing 100GB+ Zstd Streams without OOM
- **Given** an ultra-large 100GB `.tar.zst` stream
- **When** compression or decompression is executed
- **Then** peak RAM usage remains strictly <= 16MB throughout the entire operation.

### User Scenario 3: 100% CPU Efficiency without Polling
- **Given** multi-threaded background compression/decompression tasks
- **When** worker pools are idle or waiting for I/O
- **Then** CPU utilization drops to 0.0% instantly without blind `Task.sleep` loops.

---

## 4. Success Metrics
1. **Memory Bound**: Peak memory during streaming compression/decompression <= 16MB.
2. **7z Single-File Seek Speedup**: >100x latency reduction (from seconds of full archive extraction to <10ms).
3. **Cross-Platform Build**: `cargo test --workspace` and `cargo build --release` succeed cleanly on macOS and Linux targets.
4. **Zero Regression**: 100% of existing 860+ Swift tests and 7/7 local CI stages pass.

---

## 5. Clarifications
- **Q1: How does the new 7z solid stream decoder avoid extracting the whole archive on single file lookups?**
  - **Decision**: In `SevenZipSolidStreamDecoder`, the engine looks up the target file's `Folder` index in the 7z Header database, initializes the decompression stream only for that specific folder (in RAM), streams decompression until the target file's byte range is satisfied, and immediately halts stream decompression without reading subsequent blocks or writing anything to disk.
- **Q2: How are Zstd streaming operations handled across FFI without unbounded buffers?**
  - **Decision**: `ttzip_rust_zstd_compress_stream` and `ttzip_rust_zstd_decompress_stream` accept a standard stream callback / file descriptors, maintaining an internal fixed 4MB input/output double buffer managed by `zstd_safe::CStream`/`DStream`.
- **Q3: What replaces the Swift Concurrency `Task.sleep` polling loops in worker pools?**
  - **Decision**: The Rayon multi-core scheduler and `parking_lot::Condvar` primitives are used in Rust. Workers block on OS-level futexes/event semaphores with zero CPU cycles consumed while waiting for new work.

