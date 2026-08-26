# Feature Specification: TTZip CLI & Full Multi-Language SDK Architectural Evolution

- **Feature ID**: `005-cli-and-multi-language-sdk-architecture-evolution`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `SPECIFIED`
- **Created**: 2026-08-24
- **Target Subsystems**:
  - `ttzip-tui` (Standalone Native CLI & Ratatui Interactive TUI)
  - `ttzip-engine` (Rust Core Engine & Pure Rust Public SDK)
  - `ttzip-cabi` / `CTTZipBridge` (Canonical C-ABI 2.0 & Stable Foreign Function Interface)
  - `TTZipCore` (Swift 6 Strict Concurrency Actor-First SDK)
  - `ttzip-python` (PyO3 Zero-Copy Buffer Protocol & GIL-Free Python SDK)
  - **Multi-Language Tier-1 SDKs**: C++20 (`ttzip-cpp`), Go (`ttzip-go`), C#/.NET 8+ (`ttzip-dotnet`), Java 22+/Kotlin (`ttzip-jvm`), Dart/Flutter (`ttzip-dart`)

---

## 1. Executive Summary & Concrete Audit Findings

A comprehensive, full-stack architectural and Developer Experience (DX) audit across `core/rust/ttzip-tui`, `core/rust/ttzip-engine`, `core/Sources/TTZipCore`, `core/rust/ttzip-python`, `core/sdk/`, and C-ABI export headers revealed critical memory/concurrency defects, abstraction proliferation, non-idiomatic wrapper implementations, and developer usability bottlenecks.

### 1.1 CLI & TUI Subsystem (`core/rust/ttzip-tui`)

1. **Unbounded Heap Buffering on Ingestion ($O(N)$ RAM OOM)**:
   - *Defect*: `read_archive_data_auto` (`format.rs:77`), `execute_extract` (`extract.rs:40`), `execute_hash` (`hash.rs:26`), and `AppState::new` (`state.rs:59`) load entire archive files into contiguous heap memory via `fs::read(path)`.
   - *Impact*: Ingesting multi-gigabyte archives (e.g. 20GB–100GB backups) immediately triggers out-of-memory crashes or kernel swap thrashing.
   - *Paradigm Shift*: **Streaming I/O & Memory-Mapped File Access**. Replace `fs::read` with streaming readers (`std::io::Read + Seek`) and zero-allocation memory mapping (`memmap2`) for files exceeding 16MB.

2. **Multibyte Character Slice Panic**:
   - *Defect*: `execute_list` (`list.rs:95-99`) truncates strings using raw byte slicing `&path_display[path_display.len() - 33..]`.
   - *Impact*: Any archive entry containing multibyte UTF-8 characters (CJK characters, accented letters, emojis) at the truncation boundary triggers a thread panic (`byte index is not a char boundary`).
   - *Fix*: Use char-boundary aware slicing via `unicode-segmentation` or `chars().rev().take().collect()`.

3. **Shallow Container Verification & Stubbed Implementations**:
   - *Defects*:
     - `execute_check` (`check.rs:33-45`): Only verifies container header parsing without decompressing payload blocks or validating CRC32/Adler32 digests. Corrupted payloads with intact headers falsely pass as valid.
     - `execute_comment` (`comment.rs:21-25`) & `execute_lock` (`lock.rs:21-22`): Contain stubbed/mock implementations returning hardcoded strings without modifying archive bits.
     - `execute_doctor` (`doctor.rs:39-56`): Misleadingly lists 16 unsupported formats (DMG, ISO9660, CPIO, PAX, CAB, AR/DEB) that have no engine implementation.
     - `execute_bench` (`braille_plotter.rs:195-211`): Employs hardcoded static coordinates rather than live benchmark measurements.
   - *Paradigm Shift*: Full cryptographic integrity verification passes (`--deep`), honest diagnostic capabilities, and dynamic Pareto computation.

4. **Broken ASCII Tree Traversal**:
   - *Defect*: `execute_tree` (`tree.rs:56-72`) uses a flat linear loop where only the final entry in the archive receives `└── `; all other entries receive `├── ` with missing directory vertical trunks (`│   `).
   - *Fix*: Depth-first recursive hierarchy traversal tracking ancestor branch states (`ancestor_is_last: Vec<bool>`).

5. **TUI 60 FPS Busy-Looping & Memory Churn**:
   - *Defects*:
     - `EventHandler` (`event.rs:101-129`) unconditionally emits 16ms tick events, forcing constant terminal redrawing even when idle.
     - `render_explorer` (`ui/explorer.rs:37`, `vfs/view.rs:71-108`) recursively flattens the entire tree via `vfs.flatten_visible()` on every single frame, generating thousands of heap allocations per second on 50k+ entry archives.
   - *Paradigm Shift*: Event-driven terminal blocking (`crossterm::event::poll(None)`) during static views; viewport windowing with a cached visible index array updated only on navigation.

6. **Missing UNIX Filter/Pipe Composability & Monolithic Exit Codes**:
   - *Defects*:
     - Stdin/stdout archiving (`ttzip create -` and `ttzip extract -`) is unsupported.
     - Wordlist dictionary piping for recovery (`ttzip recover secret.zip -d -`) is unsupported.
     - Unhandled `SIGPIPE` causes broken pipe errors on downstream `head` / `grep`.
     - `main.rs:171-174` maps all failure states to generic exit code `1`.
   - *Paradigm Shift*: Full standard UNIX pipeline support with standard exit code taxonomy (`EX_OK=0`, `EX_USAGE=2`, `EX_DATAERR=3`, `EX_NOPERM=4`, `EX_IOERR=5`, `EX_INTERRUPT=130`).

---

### 1.2 Swift SDK Subsystem (`core/Sources/TTZipCore`)

1. **Infinite Recursion in Default Protocol Extension**:
   - *Defect*: `ArchiveProtocols.swift:503-549` (`ArchiveWriting.createArchive` and `createArchiveSync`) default protocol implementation calls itself recursively with identical arguments without delegating to an underlying engine instance.
   - *Impact*: Calling the protocol method without an overriding implementation instantly triggers a stack overflow crash.

2. **Critical Telemetry Semantic Defect (Throughput Returned as Duration)**:
   - *Defect*: `Facades/TTZipEngineFacade.swift:609-611` (`executePipelineExtract`) returns `res.throughputMBs` (e.g. `450.0` MB/s) which is assigned directly to `ExtractResult.durationSeconds` (`L513`), falsely reporting a 450-second duration for a sub-second extraction.

3. **Broken Cancellation in GCD-to-Task Bridge**:
   - *Defect*: `Concurrency/NativeComputeDispatcher.swift:58`: `Task.isCancelled` is evaluated inside a GCD `computeQueue.async` closure where it **always returns `false`** because GCD execution runs outside the calling Swift async Task context.

4. **Unbounded Immortal Task Retention (Memory Leak)**:
   - *Defect*: `Commands/ArchiveCommandProtocol.swift:150-165` (`CommandHistoryManager.chainTask`) chains every new task to `currentOperationTask` by capturing `previousTask?.result`. This creates an immortal singly-linked list of `Task` objects retaining all execution closures and stack traces for the entire app lifetime.

5. **Pervasive `@unchecked Sendable` & Mutex Explosion**:
   - *Defect*: Over 40 classes suppress Swift 6 data-race checks using `@unchecked Sendable` and manual `NSLock` / `os_unfair_lock` instances (including `ArchiveCompositeDirectory.swift:191-201`, which instantiates a separate `NSLock` per folder, creating 10,000+ lock objects on large archives).
   - *Defect*: `Types/ArchiveEntryMetadata.swift:114` passes struct stored property `os_unfair_lock_s` inout (`&unfairLock`), violating Swift memory stability rules.

6. **Use-After-Free & Unsafe Buffer Over-read in C-ABI Bridge**:
   - *Defect*: `Bridge/RustVfsSession.swift:180-204` (`getChildren`) exposes raw `UnsafePointer<CChar>?` from internal Rust nodes inside public structs, causing a Use-After-Free if `RustVfsSession` is deallocated.
   - *Defect*: `Bridge/TTZipErrorInfo+Extensions.swift:73-84` rebinds fixed-size 512-byte tuples to `CChar` and invokes `String(cString:)` without length bounding, causing out-of-bounds reads if the string is non-null-terminated.
   - *Defect*: `Bridge/RustVfsBridge.swift:16-21` performs $O(N)$ separate `strdup` and `free` heap allocations per entry.

---

### 1.3 Rust Engine Core & C-ABI Subsystem (`core/rust/ttzip-engine`)

1. **Thread-Local Storage (TLS) Dangling Pointer Hazard**:
   - *Defect*: `types.rs:318-327` (`ttzip_rust_last_error_message`) returns `err.message.as_ptr() as *const c_char` pointing into a dropped `RefCell` borrow in TLS. Concurrent calls or cross-thread dispatch return dangling pointers, `NULL`, or corrupted memory.

2. **Deallocator Fragmentation (8 Disparate Free Functions)**:
   - *Defect*: The C-ABI exports 8 distinct free routines (`ttzip_rust_free_string`, `ttzip_rust_rs_free_buffer`, `ttzip_rust_free_compliance_report`, `ttzip_rust_bench_free_string`, `ttzip_rust_vfs_free_string`, `ttzip_rust_free_hex_diff`, `ttzip_rust_free_differential_string`, `ttzip_rust_free_aligned`).
   - *Impact*: Inability for foreign language bindings (C++, Go, C#, Java, Dart) to manage memory uniformly, leading to mismatched free calls and heap corruption.

3. **C-ABI Leakage into Pure Rust Public APIs**:
   - *Defect*: Core Rust orchestrators (`UnifiedArchiveOrchestrator`) take C-ABI option structs (`TTZipCreateOptions`, `TTZipExtractOptions`) with raw pointers and `extern "C"` callbacks rather than idiomatic Rust types (`&Path`, closures, traits, builders).
   - *Defect*: Global flat re-exports with `#![allow(ambiguous_glob_reexports)]` in `lib.rs:15`.

4. **Heap Bloat in Single-Entry Extraction**:
   - *Defect*: `ffi/archive_ffi/extract.rs:322` executes `let data = fs::read(p)` to extract a single entry from a 7z archive, allocating multi-gigabyte memory for a single file lookup.

---

### 1.4 Python SDK Subsystem (`core/rust/ttzip-python`)

1. **GIL Contention in Buffer Codecs**:
   - *Defect*: `compress_buffer` and `decompress_buffer` (`lib.rs:318-386`) do not release the GIL via `py.allow_threads(...)`, blocking Python multithreading during CPU-intensive operations.

2. **Zstandard Buffer Sizing Truncation**:
   - *Defect*: `lib.rs:326-336` allocates `data.len() * 4 + 4096` when Zstd headers lack uncompressed size descriptors. Compression ratios exceeding 4:1 fail with buffer exhaustion.

3. **Lack of Python Buffer Protocol (`PyBuffer`)**:
   - *Defect*: Lacks zero-copy `memoryview`, `bytearray`, and NumPy buffer writing, forcing intermediate heap `Vec<u8>` and duplicate `PyBytes` allocations.
   - *Defect*: Missing context managers (`with ttzip.open(...) as arc:`) and lazy entry generators.

---

### 1.5 Developer Experience (DX), Usability & Professional Ergonomics Audit

A specialized audit was conducted to assess the real-world developer experience, API ergonomics, ease of onboarding, and professional tooling standards across the CLI and all language SDKs:

#### A. CLI Developer Experience Bottlenecks
1. **Interactive TTY vs Script Ambiguity**:
   - *Issue*: Invoking `ttzip my_archive.zip` automatically launches the interactive Ratatui TUI. When executed in non-interactive CI/CD runners, Docker containers, or SSH scripts without a TTY, it attempts to initialize raw terminal mode and panics or hangs.
   - *Standard*: The CLI must detect whether standard input and output are attached to an interactive TTY (`std::io::stdout().is_terminal()`). If not a TTY and no subcommand is passed, it should default to `list` (or print helpful usage) rather than attempting TUI rendering.
2. **Missing Flag Conventions & Discovery**:
   - *Issue*: Standard CLI tools (`tar`, `zip`, `7z`, `zstd`) support `--quiet` / `-q`, `--dry-run`, `--include <glob>`, `--exclude <glob>`, `--strip-components <N>`, and `--overwrite [always|never|prompt]`. TTZip hardcodes overwrite behavior and lacks filtering flags in headless commands.
   - *Issue*: No shell auto-completion generator for Bash, Zsh, and Fish (`ttzip completions zsh`).
3. **Machine-Readable Telemetry & Scripting**:
   - *Issue*: Mixing banner text logs with JSON output makes programmatic JSON parsing fragile in Bash scripts.
   - *Standard*: A strict `--format [table|json|ndjson|plain]` flag, with `--format json` guaranteeing pure JSON on stdout and diagnostic logs strictly isolated to stderr. Support for `NO_COLOR` standard (https://no-color.org).

#### B. Multi-Language SDK Ergonomics & Pseudo-Wrapper Deficiencies
1. **Java / Kotlin SDK Pseudo-Implementation**:
   - *Issue*: `core/sdk/jvm/src/main/java/com/ttzip/TTZip.java` claims to use Java FFM, but actually executes `ProcessBuilder("rust/target/release/ttzip")` via shell subprocesses, while calculating CRC32 using a naive slow Java for-loop!
   - *Standard*: True zero-overhead Java 22+ Foreign Function & Memory (FFM) binding using `java.lang.foreign.Arena` and `MemorySegment`, enabling in-memory stream compression without disk or subprocess overhead.
2. **Dart / Flutter SDK Pseudo-Implementation**:
   - *Issue*: `core/sdk/dart/lib/ttzip.dart` imports `dart:ffi` but invokes `Process.run('ttzip', ...)` subprocesses.
   - *Standard*: Real `dart:ffi` binding that dispatches decompression tasks onto Flutter background `Isolate`s and streams progress via `Stream<ArchiveProgress>`.
3. **Swift SDK API Proliferation**:
   - *Issue*: To compress a file, a Swift developer must choose between 4 competing abstractions (`TTZipEngineFacade`, `CompressCommand`, `ArchivePipelineBuilder`, `ArchiveWriter`) and pass 9 parameters.
   - *Standard*: Streamlined Swift 6 API:
     ```swift
     // Single-line High-Level API
     try await TTZip.archive("Sources", to: "backup.7z", level: .maximum)
     
     // Fluent Swift 6 Async Actor API
     let archive = try await TTZipArchive.open("package.zip")
     for await entry in archive.entries {
         print(entry.path, entry.uncompressedSize)
     }
     ```
4. **Rust Engine Standalone Inhospitality**:
   - *Issue*: A Rust developer adding `ttzip-engine` to `Cargo.toml` is forced to work with raw C-ABI pointer structs (`TTZipCreateOptions`, `TTZipExtractOptions`).
   - *Standard*: Native Rust API with `ArchiveBuilder`, `ttzip::Archive::open()`, and `std::io::Read` / `std::io::Write` trait implementations.
5. **Python Drop-In Compatibility**:
   - *Issue*: Missing standard-library `zipfile` drop-in replacement classes (`ttzip.ZipFile`, `ttzip.SevenZipFile`).

---

## 2. Target Architecture & Paradigm Shifts

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                 Universal Client Layer                                  │
│                                                                                         │
│  CLI / TUI   Swift SDK    Python SDK   C++20 SDK   Go SDK   C# SDK   JVM SDK   Dart SDK │
└──────┬───────────┬────────────┬────────────┬──────────┬────────┬────────┬──────────┬────┘
       │           │            │            │          │        │        │          │
       │    (Swift 6 Actor)     │            │          │        │        │          │
       │           ▼            │            │          │        │        │          │
       │      TTZipCore         │            │          │        │        │          │
       │           │            │            │          │        │        │          │
       ▼           ▼            ▼            ▼          ▼        ▼        ▼          ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           Canonical C-ABI 2.0 (Stable ABI v2)                          │
│                                                                                         │
│   • Versioned Envelopes (struct_size + abi_version)                                     │
│   • Universal Memory Contract: ttzip_free(ptr, kind)                                    │
│   • Explicit Out-Pointer Error Descriptors: TTZipError** (Zero TLS dependencies)         │
│   • Zero-Copy Descriptors: TTZipBufferRef & TTZipBufferMut                              │
└────────────────────────────────────────────┬────────────────────────────────────────────┘
                                             │
                                             ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                       ttzip-engine: Pure Safe Rust Microkernel                          │
│                                                                                         │
│   • Idiomatic Rust SDK 2.0 (ArchiveBuilder, ExtractBuilder, Reader/Writer Traits)       │
│   • Streaming I/O Engine (Read + Seek, memmap2, Parallel Extraction DAG)               │
│   • Flat Arena-Allocated VFS Index with Packed UTF-8 Offsets                            │
│   • Hardware Acceleration: NEON / AVX-512 CRC32, AES-256 CTR/GCM, Constant-Time KDF     │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Canonical C-ABI 2.0 (Stable ABI v2)
- **Universal Deallocation**: A single entry point `void ttzip_free(void *ptr, TTZipMemoryKind kind)` replaces all fragmented free routines.
- **Explicit Out-Pointer Error Context**: All fallible C-ABI functions accept `TTZipError **out_error` containing `status_code`, `system_errno`, `byte_offset`, `entry_path`, and `message`, completely eliminating TLS memory hazards.
- **Binary Stability Headers**: Every struct begins with `uint32_t struct_size` and `uint32_t abi_version` to support binary evolution without breaking ABI compatibility.
- **Zero-Copy Descriptors**: `TTZipBufferRef { const uint8_t *data; size_t len; }` and `TTZipBufferMut { uint8_t *data; size_t len; size_t capacity; }`.

### 2.2 CLI & Interactive TUI 2.0
- **Streaming & Mmap Engine**: Replace `fs::read` with streaming readers and `memmap2` for zero heap overhead on arbitrary archive sizes.
- **True UNIX Composability**: Stdin/stdout streaming for `create` and `extract`, wordlist streaming for `recover` (`-d -`), and graceful `SIGPIPE` handling.
- **Virtualized TUI Tree**: Contiguous arena VFS storage with cached visible indices and viewport row windowing, rendering at 120 FPS with 0 allocations on idle frames.
- **TTY Intelligence & Autocompletion**: Auto-detects interactive TTY vs headless scripting mode; generates dynamic shell completion scripts (`ttzip completions [bash|zsh|fish]`).
- **POSIX Exit Code Taxonomy**: Standardized exit codes (`0`, `2`, `3`, `4`, `5`, `130`).

### 2.3 Swift SDK 2.0 (`TTZipCore`)
- **Swift 6 Strict Concurrency Actor Architecture**:
  - `actor ArchiveEngine`: Owns compression, extraction, and cancellation state machines.
  - `actor VfsSession`: Owns the native tree handle, isolating lookups and searches.
  - `@globalActor actor ArchiveActor`: Coordinates background worker execution.
- **Ergonomic Developer Interface**: Unified `TTZip` top-level namespace (`TTZip.archive`, `TTZip.extract`, `TTZipArchive.open`).
- **Reactive Streaming Pipeline**: `AsyncStream<ArchiveProgress>` replaces all closure boxing and unmanaged pointer passing.
- **Noncopyable (`~Copyable`) Page Buffers**: Compile-time single-ownership buffers ensuring automatic deallocation and zero use-after-free bugs.
- **Swift 6 Typed Throws**: `throws(ArchiveError)` across all SDK methods with exhaustive C-ABI diagnostic payload mapping.

### 2.4 Pure Idiomatic Rust Engine SDK 2.0
- **Decoupled Architecture**: Separate `ttzip-engine` (100% safe, idiomatic Rust crate) from `ttzip-cabi` (C-ABI adapter).
- **Fluent Builders & Standard Traits**: `ArchiveBuilder`, `ExtractBuilder`, implementing `std::io::Read`, `std::io::Write`, and `std::io::Seek`.
- **Zero-Copy In-Place Editing**: In-place mutations stream untouched slices directly from memory maps into output writers without intermediate `Vec<u8>` allocations.

### 2.5 Python SDK 2.0 (`ttzip-python`)
- **100% GIL-Free Execution**: All CPU-intensive compression, decompression, and checksums run in `py.allow_threads(...)`.
- **Full Python Buffer Protocol**: Native support for `PyBuffer<u8>`, `memoryview`, and `bytearray` zero-copy in-place decompression.
- **Pythonic Architecture**: Context managers (`with ttzip.open(...) as arc:`), `zipfile.ZipFile` drop-in compatibility, entry iterators, and `.pyi` type stubs.

### 2.6 Full Multi-Language SDK Tier-1 Matrix

| Language | Binding Strategy | Key Language-Idiomatic Features | Memory & Concurrency Model |
| :--- | :--- | :--- | :--- |
| **C++20** (`ttzip-cpp`) | Header-only RAII (`ttzip.hpp`) | `std::span`, `std::expected`, `std::unique_ptr<T, TTZipDeleter>`, `std::filesystem::path` | Deterministic RAII destruction, thread-safe const methods. |
| **Rust** (`ttzip`) | Pure Safe Rust Crate | `ArchiveBuilder`, `ArchiveReader`, `std::io::Write` streaming, Rayon parallel DAG | 100% safe Rust, `Send + Sync`, zero C-ABI dependencies. |
| **Swift 6** (`TTZipCore`) | Native Swift Package | `actor`, `~Copyable` PageBuffer, `AsyncStream<ArchiveProgress>`, `throws(ArchiveError)` | Swift 6 Complete Concurrency, `@MainActor` isolation. |
| **Python** (`ttzip`) | PyO3 C-Extension | `with ttzip.open()`, `PyBuffer` zero-copy `memoryview`, entry iterators | GIL-released multithreading, zero-copy buffer sharing. |
| **Go** (`ttzip-go`) | PureGo / CGO Module | `io.Reader`, `io.Writer`, `io/fs.FS` virtual filesystem, `context.Context` cancellation | Goroutine-safe, finalizer-backed handles, zero memory leaks. |
| **C# / .NET** (`TTZip.NET`) | P/Invoke + Native AOT | `Span<byte>`, `ReadOnlySpan<byte>`, `SafeHandleZeroAlloc`, `IAsyncEnumerable<ArchiveProgress>` | CLR GC-safe pinning, `Task`-based asynchronous operations. |
| **Java / Kotlin** (`ttzip-jvm`) | Java 22+ FFM API (Foreign Function & Memory) | `Arena.ofConfined()`, `MemorySegment`, Kotlin Coroutines & `Flow<ArchiveProgress>` | **Real Native FFM (Zero Subprocess / Zero JNI)**, off-heap deterministic arena memory. |
| **Dart / Flutter** (`ttzip-dart`) | `dart:ffi` + `package:ffi` | `Uint8List` typed data, `Stream<ArchiveProgress>`, Flutter background isolate execution | **Real Native FFI (Zero Subprocess)**, isolate-safe, finalizer-governed native cleanup. |

---

## 3. User Stories & Acceptance Scenarios

### User Story 1: Zero-OOM Streaming CLI & Pipeline Composability (Priority: P1)
**As a** command-line power user or DevOps engineer on macOS / Linux,  
**I want** `ttzip` to process arbitrarily large archives and participate seamlessly in UNIX pipelines with standardized exit codes,  
**So that** multi-gigabyte compressions and extractions execute with bounded memory ($\le 64\text{MB}$) and full scriptability.

- **Scenario 1.1 (Standard In/Out Pipe Archiving)**: `tar cf - /var/log | ttzip create - -f 7z | ssh user@remote "cat > backup.7z"` streams without writing intermediate temporary disk files.
- **Scenario 1.2 (Bounded Memory Hashing & Extraction)**: Computing checksums or extracting a 50GB archive consumes $\le 64\text{MB}$ peak RSS.
- **Scenario 1.3 (Multibyte UTF-8 CLI Safety)**: `ttzip list` and `ttzip tree` on archives containing Japanese, Chinese, or emoji filenames render without string-slicing panics.
- **Scenario 1.4 (Standardized Exit Codes)**: Incorrect passwords exit with `4` (`EX_NOPERM`), corrupt files exit with `3` (`EX_DATAERR`), and normal completions exit with `0` (`EX_OK`).
- **Scenario 1.5 (Non-Interactive TTY Safety)**: Executing `ttzip archive.zip` in CI/CD without an interactive TTY prints entry listing rather than attempting TUI initialization.

### User Story 2: Swift 6 Concurrency & Strict Memory Safety (Priority: P1)
**As an** Apple platform developer integrating `TTZipCore` in macOS apps,  
**I want** a clean, actor-isolated Swift 6 API with compile-time data race safety and deterministic memory management,  
**So that** the UI never experiences main-thread freezes, memory leaks, or use-after-free crashes.

- **Scenario 2.1 (Compile-Time Concurrency Safety)**: `TTZipCore` builds cleanly under `-strict-concurrency=complete` with 0 `@unchecked Sendable` bypasses on domain models.
- **Scenario 2.2 (AsyncStream Progress Tracking)**: SwiftUI views observe extraction progress via `AsyncStream<ArchiveProgress>` without closure boxing or `Unmanaged` pointer passing.
- **Scenario 2.3 (Noncopyable Zero-Copy Buffer Lifecycle)**: `PageBuffer` utilizing Swift 6 `~Copyable` guarantees deterministic deallocation on scope exit with 0 use-after-free risks.
- **Scenario 2.4 (Accurate Performance Telemetry)**: `ExtractResult.durationSeconds` correctly reflects elapsed execution time in seconds.

### User Story 3: Canonical C-ABI 2.0 & Multi-Language SDK Ecosystem (Priority: P1)
**As an** enterprise software architect building applications across Python, Go, C#, C++, Java/Kotlin, and Flutter,  
**I want** official, first-class TTZip SDKs for all major programming languages backed by a unified C-ABI 2.0,  
**So that** high-throughput archiving and hardware-accelerated decompression can be embedded natively in any tech stack with zero subprocess spawning.

- **Scenario 3.1 (Universal Memory Contract)**: All native language SDKs release memory through `ttzip_free(ptr, kind)` with 0 allocator mismatch crashes.
- **Scenario 3.2 (Thread-Safe Out-Pointer Error Reporting)**: Concurrent multi-threaded operations retrieve detailed error diagnostics via `TTZipError**` without TLS desynchronization.
- **Scenario 3.3 (Python Zero-Copy Buffer Ingestion)**: Python developers decompress data directly into pre-allocated NumPy / `bytearray` buffers without intermediate memory copies while releasing the GIL.
- **Scenario 3.4 (Java 22+ Real Native FFM Integration)**: Java / Kotlin applications achieve native throughput using `MemorySegment` and `Arena` without legacy JNI or subprocess spawning.
- **Scenario 3.5 (Dart / Flutter Real Native FFI)**: Flutter mobile/desktop apps perform decompression in background isolates via `dart:ffi` with 0 external process execution.
- **Scenario 3.6 (C# .NET 8 Span Support)**: C# applications process archives using `ReadOnlySpan<byte>` and `SafeHandle` with 0 GC pressure.

---

## 4. Functional Requirements (FR-01 to FR-32)

### CLI & TUI Architecture
- **FR-01**: The CLI engine MUST stream archive data using `Read + Seek` and `memmap2` for files $>16\text{MB}$, bounding peak memory to $\le 64\text{MB}$ RSS.
- **FR-02**: The CLI string formatting routines MUST use Unicode character boundary slicing to prevent multibyte panics.
- **FR-03**: The `tree` subcommand MUST implement a true hierarchical depth-first traversal tracking ancestor branch states.
- **FR-04**: The `check` subcommand MUST support `--deep` full payload decompression and cryptographic CRC32/Adler32 verification.
- **FR-05**: The interactive TUI MUST implement event-driven idle blocking and virtualized row windowing, allocating 0 heap buffers on static frames.
- **FR-06**: The CLI MUST support standard UNIX stdin/stdout streaming (`-`) for archive creation, extraction, and password dictionary recovery.
- **FR-07**: The CLI MUST adhere to the standardized POSIX exit code taxonomy (`EX_OK=0`, `EX_USAGE=2`, `EX_DATAERR=3`, `EX_NOPERM=4`, `EX_IOERR=5`, `EX_INTERRUPT=130`).
- **FR-08**: The CLI MUST detect interactive TTY status via `is_terminal()`, automatically falling back to non-interactive listing when run without a TTY.
- **FR-09**: The CLI MUST provide a shell completion command (`ttzip completions <shell>`) for Bash, Zsh, and Fish.

### Canonical C-ABI 2.0 (Stable ABI v2)
- **FR-10**: The C-ABI layer MUST expose a unified deallocation function `ttzip_free(void *ptr, TTZipMemoryKind kind)`, deprecating all fragmented free functions.
- **FR-11**: All fallible C-ABI functions MUST accept explicit `TTZipError **out_error` descriptors, eliminating thread-local storage error retrieval.
- **FR-12**: All C-ABI structs MUST include `uint32_t struct_size` and `uint32_t abi_version` headers for binary compatibility.
- **FR-13**: The C-ABI layer MUST provide zero-copy buffer descriptors (`TTZipBufferRef`, `TTZipBufferMut`) for cross-language memory sharing.
- **FR-14**: All exported C-ABI functions MUST be protected by `catch_unwind` exception boundaries, translating panics into `TTZIP_STATUS_ERR_PANIC_CAUGHT`.

### Swift SDK 2.0 (`TTZipCore`)
- **FR-15**: The SDK MUST migrate to a unified Swift 6 Actor hierarchy (`actor ArchiveEngine`, `actor VfsSession`, `@globalActor actor ArchiveActor`), eliminating manual mutexes and `@unchecked Sendable` bypasses.
- **FR-16**: Protocol extensions in `ArchiveProtocols.swift` MUST NOT contain self-recursive infinite loops.
- **FR-17**: Operation telemetry in `TTZipEngineFacade.swift` MUST report duration in elapsed seconds and throughput in MB/s accurately.
- **FR-18**: Progress reporting across all operations MUST be exposed as `AsyncStream<ArchiveProgress>`, deprecating closure boxing.
- **FR-19**: Memory page buffers MUST adopt Swift 6 `~Copyable` noncopyable types for compile-time single ownership.
- **FR-20**: All fallible SDK operations MUST adopt Swift 6 Typed Throws (`throws(ArchiveError)`).
- **FR-21**: The SDK MUST expose high-level ergonomic APIs (`TTZip.archive`, `TTZip.extract`, `TTZipArchive.open`).

### Pure Rust Engine SDK 2.0
- **FR-22**: The Rust microkernel MUST decouple into pure safe Rust library `ttzip-engine` and C-ABI adapter `ttzip-cabi`.
- **FR-23**: The Rust SDK MUST provide fluent builders (`ArchiveBuilder`, `ExtractBuilder`) and standard `Read`/`Write`/`Seek` adapters.
- **FR-24**: In-place archive mutations MUST stream untouched entries directly from memory maps without intermediate `Vec<u8>` allocations.

### Python SDK 2.0 (`ttzip-python`)
- **FR-25**: All CPU-intensive compression, decompression, and checksum routines MUST release the GIL via `py.allow_threads`.
- **FR-26**: The Python SDK MUST implement the Python Buffer Protocol (`PyBuffer<u8>`), supporting zero-copy `memoryview` and in-place buffer decompression.
- **FR-27**: The Python SDK MUST provide idiomatic context managers (`with ttzip.open(...)`), `zipfile` compatibility, and `.pyi` type stubs.

### Multi-Language Tier-1 SDKs (True Native Implementations)
- **FR-28**: The project MUST provide a header-only C++20 SDK (`ttzip.hpp`) utilizing `std::span`, `std::expected`, and RAII memory management.
- **FR-29**: The project MUST provide a Go SDK (`ttzip-go`) implementing `io.Reader`, `io.Writer`, `io/fs.FS`, and `context.Context` cancellation.
- **FR-30**: The project MUST provide a C#/.NET 8 SDK (`TTZip.NET`) utilizing `ReadOnlySpan<byte>`, `SafeHandle`, and `IAsyncEnumerable`.
- **FR-31**: The project MUST provide a Java 22+/Kotlin SDK (`ttzip-jvm`) utilizing Java Foreign Function & Memory (FFM) API (`Arena`, `MemorySegment`) with zero subprocess spawning, and Kotlin Coroutines/Flow.
- **FR-32**: The project MUST provide a Dart/Flutter SDK (`ttzip-dart`) utilizing true `dart:ffi` in background isolates with zero subprocess spawning.

---

## 5. Success Criteria

- **SC-01 (Bounded Memory Execution)**: CLI and all SDKs process archives of arbitrary size (tested up to 50GB) with peak process RSS $\le 64\text{MB}$.
- **SC-02 (Zero Concurrency Bypasses in Swift 6)**: `TTZipCore` builds cleanly with `-strict-concurrency=complete` with 0 `@unchecked Sendable` warnings or suppressions.
- **SC-03 (Unified C-ABI Memory Safety)**: 100% of native allocations released via `ttzip_free`, with 0 memory leaks or mismatched allocator panics detected under Valgrind/ASan.
- **SC-04 (Zero TLS Error Hazard)**: 100% of error diagnostics delivered via thread-safe `TTZipError**` descriptors with 0 dangling pointers.
- **SC-05 (100% Python GIL-Free Parallelism)**: Multi-threaded Python benchmarks achieve linear scaling across CPU cores during concurrent compression/decompression.
- **SC-06 (120 FPS Idle TUI Performance)**: TUI CPU utilization drops to $<0.1\%$ on static view frames with 0 heap allocations per second.
- **SC-07 (Multi-Language Parity & Real Native FFI)**: All 8 language SDKs (C++20, Rust, Swift 6, Python, Go, C#/.NET, Java/Kotlin, Dart/Flutter) execute via native memory bindings with 0 shell subprocess spawns and pass uniform cross-language integration test suites.
- **SC-08 (Accurate Telemetry & Zero Recursion)**: 0 stack overflow traps in protocol defaults, 100% accurate duration and throughput metrics.

---

## 6. Governance, Compatibility & Project Principles

- **Architecture Boundary Standard**: Full SDD (`[Full SDD]`) governing CLI command surface, C-ABI 2.0 stable contracts, and language SDK public APIs.
- **File Length Standard**: All newly written or refactored source files MUST strictly adhere to $\le 800$ LOC (target $\le 350$ LOC).
- **Tooling & Licensing**: BSD-3-Clause OR Apache-2.0 for core engine, C-ABI bridge, CLI, and multi-language SDKs; GPL-3.0-or-later for macOS application layer.
