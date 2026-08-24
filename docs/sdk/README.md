# 🚀 TTZip Multi-Language SDK Ecosystem

[![License: BSD-3-Clause](https://img.shields.io/badge/License-BSD--3--Clause-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/LICENSE-BSD)
[![Architecture: Safe Rust Core](https://img.shields.io/badge/Core-Safe%20Rust%20Microkernel-orange.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/ARCHITECTURE.md)
[![C-ABI: Version 2.0](https://img.shields.io/badge/ABI-C--ABI%202.0%20Standard-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/Sources/CTTZipBridge/include/ttzip.h)
[![Hardware: ARM NEON & AVX-512](https://img.shields.io/badge/Hardware-ARM%20NEON%20%2F%20PMULL%20%2F%20AVX512-purple.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/BENCHMARK_MATRIX.md)

Welcome to the official developer documentation for the **TTZip Multi-Language SDK Ecosystem**. TTZip is an enterprise-grade, microkernel-powered native archiving and compression engine engineered in **Safe Rust** and distributed across modern language runtimes via zero-overhead, memory-safe bindings.

---

## 1. Master SDK Navigation Matrix

TTZip provides official, first-class native bindings for all major systems and application programming languages:

| Language / Runtime | Official Package / Crate | Integration Mechanism | Concurrency Model | Zero-Copy Support | Developer Guide |
| :--- | :--- | :--- | :--- | :---: | :---: |
| **Rust** | `ttzip-engine` | Native Safe Rust Microkernel | Rayon Work-Stealing Pool | ✅ Direct (`&[u8]`, mmap) | [Rust Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/RUST_GUIDE.md) |
| **Swift 6** | `TTZipCore` (SPM) | Direct C-ABI Modulemap | Strict Concurrency (`Actor`, `Sendable`) | ✅ Pointer Pinning (`UnsafeBufferPointer`) | [Swift Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/SWIFT_GUIDE.md) |
| **Python** | `ttzip` (PyPI / Maturin) | Native PyO3 Native Extension | GIL-Free Background Worker Threads | ✅ `PyBuffer` / Buffer Protocol | [Python Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/PYTHON_GUIDE.md) |
| **Java 22+ / Kotlin** | `com.ttzip:ttzip-core` | Panama FFM (`Arena`, `MethodHandle`) | Virtual Threads & Kotlin Coroutine `Flow` | ✅ Panama `MemorySegment` | [JVM & Kotlin Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/JVM_KOTLIN_GUIDE.md) |
| **C++20 / C11** | `ttzip` (CMake / Header) | Modern C++20 RAII & C11 ABI 2.0 | Standard `std::jthread` / POSIX Threads | ✅ `std::span<const uint8_t>` | [C++20 & C11 Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/CPP_C_GUIDE.md) |
| **Go** | `github.com/ttzip/ttzip-go` | CGO Zero-Alloc + `io/fs.FS` | Goroutines + `context.Context` | ✅ 1MB Chunk Amortization | [Go Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/GO_GUIDE.md) |
| **Dart / Flutter** | `ttzip` (pub.dev) | `dart:ffi` Dynamic Library | Background `Isolate.run` & `Stream` | ✅ `TypedData` (`Uint8List`) | [Dart & Flutter Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/DART_FLUTTER_GUIDE.md) |
| **C# / .NET 8+** | `TTZip` (NuGet) | P/Invoke + `SafeHandle` | `IAsyncEnumerable<T>` + `Channel<T>` | ✅ `ReadOnlySpan<byte>` | [.NET Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/DOTNET_GUIDE.md) |
| **Node.js / TypeScript** | `@ttzip/core` (npm) | N-API / Native Node Addon | Libuv Thread Pool & Event Loop | ✅ Node.js `Buffer` | [Node.js & TS Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/NODE_TYPESCRIPT_GUIDE.md) |

For specialized enterprise recipes (AES-256 encryption, Reed-Solomon ECC, Solid tuning, VFS cache, cancellation tokens), see the [Advanced Settings Recipes](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/ADVANCED_SETTINGS_RECIPES.md).

---

## 2. Core Architectural Invariants

TTZip is built upon six uncompromised engineering pillars that govern all language bindings:

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                            Application / Client Layer                            │
│      (Swift 6 · Python · Java 22+ · C++20 · Go · Dart/Flutter · .NET 8 · Node)   │
└────────────────────────────────────────┬─────────────────────────────────────────┘
                                         │ In-Process Direct Invocation (No Fork)
┌────────────────────────────────────────▼─────────────────────────────────────────┐
│                        TTZip C-ABI 2.0 Contract Boundary                         │
│   - Struct Size & ABI Version Handshake (ttzip_rust_glue.h)                      │
│   - Thread-Local Diagnostic Context (ttzip_rust_last_error_message)              │
│   - Cooperative Non-Blocking Cancellation Tokens (< 5ms abort latency)           │
└────────────────────────────────────────┬─────────────────────────────────────────┘
                                         │ Zero-Copy Pointer Exchange
┌────────────────────────────────────────▼─────────────────────────────────────────┐
│                      Safe Rust Microkernel Engine Layer                          │
│   - Storage Medium Prober: statfs(2) NVMe APFS Mmap vs Remote Stream pread       │
│   - Multi-Core Work-Stealing Scheduler: Rayon parallel chunk compressor          │
│   - Streaming Parallel ZIP Writer with APFS Extent Preallocation (fstore_t)      │
│   - Hardware Accelerators: ARM NEON, PMULL, ARMv8 Crypto, x86 AVX2/AVX-512       │
│   - Reed-Solomon GF(2^8) Erasure Coding & Self-Healing Repair Engine             │
│   - Zip-Slip & Path Traversal Canonical Directory Sanitizer                      │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Invariant 1: Zero-Subprocess Architecture
Traditional archiving wrappers invoke external command-line binaries (e.g. `tar`, `7z`, `zip`) via `subprocess.Popen` or `ProcessBuilder`. This introduces severe drawbacks:
- **Process Fork Latency**: 35ms–90ms OS process spawn overhead per invocation.
- **IPC Buffer Throttling**: Memory serialization penalties and pipe backpressure bottlenecks.
- **OOM & Zombie Leaks**: Uncontrolled memory growth outside container cgroup constraints.

TTZip links the native engine **directly into the host process**, operating through in-memory pointers and zero-copy buffers.

### Invariant 2: Project Panama FFM (Zero-JNI)
On the JVM (Java 22+ / Kotlin), TTZip completely replaces legacy JNI wrappers with the OpenJDK Foreign Function & Memory API (`java.lang.foreign`). Off-heap memory is managed through confined `Arena` allocators, ensuring deterministic deallocation and hardware-level throughput (>4.47 GB/s decompression) without garbage collection pauses.

### Invariant 3: Standardized C-ABI 2.0 Contract
All cross-language bridges communicate through [ttzip.h](file:///Users/kevintung/Documents/dev/products/ttzip/core/Sources/CTTZipBridge/include/ttzip.h) and [ttzip_rust_glue.h](file:///Users/kevintung/Documents/dev/products/ttzip/core/Sources/CTTZipBridge/include/ttzip_rust_glue.h). Every options struct enforces:
```c
uint32_t struct_size; // sizeof(TTZipCreateOptions)
uint32_t abi_version; // Always 2
```
This bidirectional handshake guarantees ABI forward/backward compatibility across dynamic library updates.

### Invariant 4: Thread-Local Error Diagnostics
When an operation encounters an error, the Rust microkernel stores rich diagnostic details (`TTZipErrorInfo`: status, message, faulty entry path, and file offset) in thread-local storage without heap allocation. Calling languages query `ttzip_rust_last_error_message()` for actionable root causes.

### Invariant 5: Dynamic Storage Medium Probing
The engine probes the target filesystem using `statfs(2)`:
- **Local NVMe APFS**: Activates `MmapSource` (`libc::mmap` + `MADV_SEQUENTIAL`) for hardware-speed random access.
- **Remote / Virtual Mounts (SMB, NFS, Cloud)**: Dynamically falls back to `StreamSource` (`pread` with 64KB buffers), preventing kernel `SIGBUS` panics.

### Invariant 6: Zip-Slip & Path Traversal Defense
All extracted entry paths are sanitized and validated against canonical destination directories before opening file descriptors. Absolute path escapes (`/etc/passwd`) and relative traversals (`../../`) are neutralized at the kernel boundary.

---

## 3. Full 16-Format Compatibility Matrix

TTZip supports comprehensive archive creation, extraction, streaming, and encryption across 16 container and codec formats:

| Format Identifier | File Extensions | Create | Extract | Streaming | AES-256 Encryption | Solid Blocks | RS-ECC Recovery | In-Place Mutation | Engine / Codec |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| **ZIP / ZIP64** | `.zip` | ✅ | ✅ | ✅ | ✅ (AES-256 / ZipCrypto) | — | ✅ (Appended) | ✅ | Parallel `libdeflate` + Rayon |
| **7-Zip (7z)** | `.7z` | ✅ | ✅ | ✅ | ✅ (AES-256 + SHA-256 KDF) | ✅ (1–256MB) | ✅ | ✅ | Pure Rust 7z + Fast-LZMA2 |
| **POSIX TAR** | `.tar` | ✅ | ✅ | ✅ | — | — | ✅ | ✅ | Pure Rust Streaming TAR |
| **Gzip Compressed TAR** | `.tar.gz`, `.tgz` | ✅ | ✅ | ✅ | — | — | ✅ | — | `libdeflate` GZ + Streaming TAR |
| **Bzip2 Compressed TAR** | `.tar.bz2`, `.tbz2` | ✅ | ✅ | ✅ | — | — | ✅ | — | Multi-threaded Bzip2 + TAR |
| **XZ Compressed TAR** | `.tar.xz`, `.txz` | ✅ | ✅ | ✅ | — | ✅ (XZ Streams) | ✅ | — | Multi-threaded LZMA2 + TAR |
| **Zstandard TAR** | `.tar.zst`, `.tar.zstd` | ✅ | ✅ | ✅ | — | — | ✅ | — | Parallel `zstd` (LDM enabled) |
| **Raw Gzip Stream** | `.gz` | ✅ | ✅ | ✅ | — | — | — | — | Single-stream `libdeflate` |
| **Raw Bzip2 Stream** | `.bz2` | ✅ | ✅ | ✅ | — | — | — | — | Single-stream `bzip2` |
| **Raw XZ Stream** | `.xz` | ✅ | ✅ | ✅ | — | — | — | — | Single-stream `liblzma` |
| **Raw Zstandard Stream** | `.zst`, `.zstd` | ✅ | ✅ | ✅ | — | — | — | — | Single-stream `zstd` |
| **Apple LZFSE** | `.lzfse` | ✅ | ✅ | ✅ | — | — | — | — | Apple LZFSE hardware codec |
| **Google Snappy** | `.sz`, `.snappy` | ✅ | ✅ | ✅ | — | — | — | — | Framed & Raw Snappy codec |
| **Brotli** | `.br` | ✅ | ✅ | ✅ | — | — | — | — | Google Brotli (q=1..11) |
| **Apple Disk Image (DMG)** | `.dmg` | — | ✅ | ✅ | ✅ (Encrypted DMG) | — | — | — | Read-only UDIF/HFS+/APFS parser |
| **Self-Extracting SFX** | `.exe`, `.sfx` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | Zip/7z SFX offset detector |

---

## 4. Benchmark Throughput Quick Reference

Performance measured on **Apple Silicon (M-Series, ARM NEON & PMULL Active)** using the **Silesia Compression Corpus** (201.04 MB uncompressed):

### 4.1 In-Process Warm SDK Throughput (Production Backend Server)
*Simulates in-process execution in Long-Running Services (Spring Boot, FastAPI, Tokio, ASP.NET Core, Go HTTP):*

| Language SDK | Binding Mechanism | Compression (Normal) | Decompression (Raw) | vs. Native Rust | Memory Peak (RSS) |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **Rust** | Native Microkernel (`rayon`/`neon`) | **260.4 MB/s** | **4,844.8 MB/s** | 100.0% (Baseline) | 7.0 MB |
| **C++20** | Modern RAII (`std::span` zero-copy) | **261.4 MB/s** | **4,862.3 MB/s** | 100.3% | 7.0 MB |
| **Swift 6** | Strict Actor Concurrency (Pinned Ptrs) | **265.8 MB/s** | **4,847.8 MB/s** | 100.0% | 21.2 MB |
| **Go** | CGO Zero-Alloc + `io/fs.FS` | **253.0 MB/s** | **4,711.7 MB/s** | 97.3% | 10.0 MB |
| **Java 22+** | Project Panama FFM (`Arena` Downcall) | **251.2 MB/s** | **4,470.8 MB/s** | 92.3% | 66.6 MB |
| **Python** | PyO3 `PyBuffer` + `allow_threads` | **248.0 MB/s** | **4,029.5 MB/s** | 83.2% | 21.2 MB |

### 4.2 Cold Subprocess CLI Invocation Penalty
*Demonstrates why external subprocess calls (`subprocess.run(["tar", ...])`) degrade throughput on short tasks:*

| Invocation Mode | Process Spawn Overhead | Task Duration (40ms work) | Measured Effective Speed | Root Cause of Degradation |
| :--- | :---: | :---: | :---: | :--- |
| **TTZip Native SDK** | **0.0 ms** (In-Process) | **40.8 ms** | **4,844.8 MB/s** | Zero process spawn penalty |
| **Rust / C++ CLI** | ~0.8 ms | 41.6 ms | 4,751.4 MB/s | Native Mach-O instant load |
| **Python 3.14 Subprocess** | ~35.0 ms | 75.8 ms | 2,835.7 MB/s | Python interpreter & dynamic import tax |
| **Java 22+ Subprocess** | ~90.0 ms | 130.8 ms | 1,614.0 MB/s | JVM startup, ClassLoader & JIT cold start |

---

## 5. Quick Verification & Installation

To test and verify all SDKs locally in your workspace:

```bash
# 1. Compile the native Rust microkernel
cd core/rust && cargo build --release

# 2. Run cross-language test matrix
cd core && ./scripts/run_all_sdk_tests.sh

# 3. Run microbenchmarks
cd core && ./scripts/run_sdk_benchmarks.sh
```

---

## 6. Language Documentation Directory

Navigate directly to your target language guide:

- 🦀 [Rust Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/RUST_GUIDE.md)
- 🐦 [Swift 6 Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/SWIFT_GUIDE.md)
- 🐍 [Python Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/PYTHON_GUIDE.md)
- ☕️ [Java 22+ & Kotlin Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/JVM_KOTLIN_GUIDE.md)
- ⚡️ [C++20 & C11 Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/CPP_C_GUIDE.md)
- 🦫 [Go Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/GO_GUIDE.md)
- 🎯 [Dart & Flutter Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/DART_FLUTTER_GUIDE.md)
- 🔷 [.NET 8+ (C#) Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/DOTNET_GUIDE.md)
- 🟢 [Node.js & TypeScript Developer Guide](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/NODE_TYPESCRIPT_GUIDE.md)
- 🛠 [Advanced Settings Recipes](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/ADVANCED_SETTINGS_RECIPES.md)
