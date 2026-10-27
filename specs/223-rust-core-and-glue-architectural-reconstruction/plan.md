# Implementation Plan: Rust Core & Glue Layer Architectural Reconstruction

**Branch**: `223-rust-core-and-glue-architectural-reconstruction` | **Date**: 2026-08-24 | **Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/core/specs/223-rust-core-and-glue-architectural-reconstruction/spec.md)

---

## 1. Summary

Reconstruct the Safe Rust engine, C-ABI bridge, and Swift glue layer of TTZip to resolve 8 systemic architectural contradictions: establish a thread-local error diagnostics pipeline, implement a fault-tolerant unified `ArchiveSource` abstraction, build a high-throughput streaming parallel ZIP writer, preserve format integrity during in-place mutations, eliminate 25 dead C-ABI exports and worker pool runtime, streamline VFS session search with zero heap allocations, and connect the high-precision CSM+Bigram charset detection pipeline.

---

## 2. Technical Context

- **Languages/Toolchains**: Swift 6.0 (Strict Concurrency), Rust 1.80+ (2021 Edition, `no_std` SIMD friendly), C11
- **Key Crates & Codecs**: `memmap2`, `crossbeam-utils`, `rayon`, `libdeflate`, `zstd`, `fast-lzma2`, `snap`, `brotli`, `cc`
- **Platform Features**: Apple Silicon NEON & PMULL intrinsics, APFS `clonefile`/`fstore_t`, POSIX `statfs`, CryptoKit
- **Testing Suites**: `cargo test`, `swift test`, AddressSanitizer/ThreadSanitizer, `verify_cabi_symbols.sh`
- **Target OS**: macOS 14.0+ (Apple Silicon arm64 & Intel x86_64)

---

## 3. Constitution & Gate Invariants

1. **LOC Defense Invariant**: Every source file must remain $\le 800$ LOC (target $< 350$ LOC).
2. **C-ABI Export Parity Invariant**: 100% of declared symbols in `ttzip_rust_glue.h` must match exported symbols in `libTTZipVendor.a`.
3. **Bounded Memory Invariant**: Single entry extraction peak RSS $\le \text{uncompressed\_size} + 64\text{MB}$.
4. **No-Dead-Code Invariant**: 0 unreferenced worker pool or ring buffer C-ABI exports.
5. **Compile Warning Zero Tolerance**: Zero warnings under `-warnings-as-errors` across Swift and Rust targets.

---

## 4. Architectural Stages & File Touch Matrix

```text
Stage 0: Foundation & Subtractive Cleanup
├── core/rust/ttzip-engine/src/types.rs                          # [MODIFY] DiagnosticErrorContext & thread-local
├── core/rust/ttzip-engine/src/runtime/worker_pool/mod.rs        # [DELETE] Remove dead worker pool
├── core/rust/ttzip-engine/src/runtime/worker_pool/pool.rs       # [DELETE] Remove dead worker pool
├── core/rust/ttzip-engine/src/runtime/ring_buffer/spsc.rs       # [MODIFY] Type-safe split(), remove push/pop on &self
├── core/rust/ttzip-engine/src/ffi/runtime_ffi/                  # [DELETE] Remove dead worker pool/ring buffer FFI
├── core/Sources/CTTZipBridge/include/ttzip_rust_glue.h          # [MODIFY] Remove 25 dead exports, add error diagnostics
└── core/rust/ttzip-engine/src/crypto/vault.rs                   # [MODIFY] Remove custom GCM/GHASH, delegate to CryptoKit

Stage 1: Unified ArchiveSource & Zero-Copy I/O
├── core/rust/ttzip-engine/src/archive/source/                   # [NEW] ArchiveSource trait, MmapSource, StreamSource
├── core/rust/ttzip-engine/src/archive/unified/extract_single.rs # [MODIFY] Replace fs::read with ArchiveSource
├── core/rust/ttzip-engine/src/archive/unified/extract.rs        # [MODIFY] Stream split volume without /tmp staging
└── core/rust/ttzip-engine/src/archive/in_place_edit.rs          # [MODIFY] Safe shadow in-place edit with ArchiveSource

Stage 2: Streaming Parallel ZIP Writer
├── core/rust/ttzip-engine/src/zip/writer/streaming_parallel.rs  # [NEW] Multi-core streaming pwrite writer
├── core/rust/ttzip-engine/src/archive/unified/create.rs         # [MODIFY] Route ZIP creation to streaming parallel engine
└── core/rust/ttzip-engine/src/zip/writer/mod.rs                 # [MODIFY] Export streaming parallel writer

Stage 3: VFS Session Lifecycle & Zero-Allocation Search
├── core/rust/ttzip-engine/src/fs/vfs/search.rs                  # [MODIFY] Zero-allocation UTF-8 fuzzy search
├── core/rust/ttzip-engine/src/ffi/fs_ffi.rs                     # [MODIFY] Return node id array without CString alloc
├── core/Sources/TTZipCore/Bridge/RustVfsBridge.swift            # [MODIFY] Route search to RustVfsSession
└── core/Sources/TTZipCore/Bridge/RustVfsSession.swift           # [MODIFY] Manage lifecycle of VFS tree handle

Stage 4: End-to-End Charset & Build System Modernization
├── core/rust/ttzip-engine/src/ffi/archive_ffi/inspect.rs        # [MODIFY] Forward CSM+Bigram encoding to callback
├── core/Sources/TTZipCore/ArchiveReader.swift                   # [MODIFY] Consume Rust-provided encoding
├── core/Sources/TTZipCore/SystemServices.swift                  # [MODIFY] Deprecate fragile GB18030 Swift detector
└── core/rust/ttzip-engine/build.rs                              # [MODIFY] Migrate from raw clang/libtool to cc crate
```

---

## 5. Verification Plan

1. `cargo test --workspace`: 100% pass across format and crypto unit tests.
2. `swift test`: 100% pass across facade integration and VFS search benchmarks.
3. `./scripts/verify_cabi_symbols.sh`: 0 missing or orphaned C-ABI exports.
4. `./scripts/run_local_ci_gate.sh`: All formatting, line-count, invariant, and memory tests pass.
