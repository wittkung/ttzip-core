# Research & Technical Decisions: 172-purge-legacy-c-bridge

## R001: 93 Legacy C Source Files Audit & Purge Categorization

- **Decision**: Physically delete 92 `.c` files across 3 progressive batches, leaving `Sources/CTTZipBridge/CTTZipBridge.c` as the sole lean C file (<200 LOC) containing minimal FFI glue.
- **Rationale**: Original C codebase is `~50,000+` lines of C11 with manual pointer math, thread-local allocations, and duplicate implementations of 7z/ZIP/Crypto/SIMD/Codecs. All core functionalities are now powered by `rust/ttzip-glue` with memory safety, zero-copy, and SIMD hardware acceleration.
- **Alternatives Considered**:
  - *Keep all C files as fallback*: Rejected due to dead code bloat, slow build times, duplicate symbol hazards, and maintenance debt.
  - *Completely delete CTTZipBridge target*: Rejected because `ReedSolomonFEC` (Reed-Solomon recovery), `ZipExtremeBlockWriter` (Zopfli C library), and `posix_spawn` need a minimal C bridge until migrated in future iterations.
- **Source**: Subagent Audit Report; `Sources/CTTZipBridge/*.c`; `rust/ttzip-glue/src/ffi/`; `Package.swift:36-75`.

## R002: Swift Adapter Call Sites Redirection to Rust C-ABI

- **Decision**: Update `HardwareChecksumAdapter`, `ZipCryptoEngine`, `SevenZipCAdapter`, `ZstdCAdapter`, `LzfseCAdapter`, `LibdeflateCAdapter` to invoke `ttzip_rust_*` functions exported by `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- **Rationale**: `rust/ttzip-glue` exports 100% C-ABI compatible functions for PMULL CRC32, UDOT Adler32, AES-256 CTR/CBC, SHA-256 KDF, Deflate, Zstd, FL2 LZMA2, LZFSE, Snappy, and Archive inspect/create/extract.
- **Alternatives Considered**:
  - *Keep C wrapper intermediate layer*: Rejected to eliminate FFI hops and C translation unit overhead.
- **Source**: `Sources/CTTZipBridge/include/ttzip_rust_glue.h`; `Sources/TTZipCore/Adapters/*.swift`.

## R003: Utility Functions Migration to Swift 6 Native

- **Decision**: Replace `ttzip_strnatcmp` with `String.localizedStandardCompare`, `ttzip_core_aligned_alloc_16k` with `UnsafeMutableRawPointer.allocate(alignment: 16384)`, `ttzip_mem_budget_*` / `ttzip_thread_budget_*` with `ProcessInfo`, and `ttzip_platform_monotonic_nanos` with `ContinuousClock`.
- **Rationale**: Swift 6 standard library and Foundation provide direct, platform-optimized, and concurrency-safe APIs that eliminate the need for custom C wrappers.
- **Alternatives Considered**:
  - *Port utility functions to Rust*: Rejected because these are trivial 1-liner Swift standard library calls with zero overhead.
- **Source**: `Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift`; `Sources/TTZipCore/ConcurrencyBridge.swift`; `Sources/TTZipCore/DiskItemSorter.swift`.
