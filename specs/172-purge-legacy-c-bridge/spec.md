# Feature Specification: 172-purge-legacy-c-bridge

## 1. Overview & Context

With the full migration of TTZip's core microkernel, SIMD intrinsics, hardware cryptographic pipelines, codecs, and container formats to Safe Rust (`rust/ttzip-glue`) in Features 168-171, the 93 legacy C source files (`~50,000+` lines of code) in `Sources/CTTZipBridge/` are no longer needed.

This feature physically purges 92 obsolete C source files and their nested headers, redirects any remaining Swift callers to `ttzip_rust_*` C-ABI or pure Swift 6 implementations, and converges `Sources/CTTZipBridge/` into a lean single-file glue skeleton (`CTTZipBridge.c` < 200 lines) with zero duplicate symbols, 0 warnings, and 100% test passing rate across all 859 Swift tests and 7 CI gates.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Single-File C Bridge Skeleton (Developer & SPM Build Experience)
- **Given** a developer clones or builds the TTZip repository with Xcode / `swift build`.
- **When** the build pipeline compiles `Sources/CTTZipBridge/`.
- **Then** SPM compiles only one minimal C source file (`CTTZipBridge.c`) without compiling 92 redundant C files, cutting C compilation time to `< 0.2s`.

### User Scenario 2: 100% Rust-Backed Core Functionality (No Dead Legacy C Code)
- **Given** TTZip performs checksums, AES-256 encryption, DEFLATE/Zstandard/LZMA2/LZFSE/Snappy compression, ZIP/7z archive creation and extraction.
- **When** Swift code calls `HardwareChecksumAdapter`, `ZipCryptoEngine`, `SevenZipCAdapter`, `ZstdCAdapter`, `LzfseCAdapter`, `LibdeflateCAdapter`, `ArchiveExtractor`, `ArchiveWriter`.
- **Then** execution is dispatched directly to `ttzip_rust_*` C-ABI functions exported by `Vendor/TTZipVendor.xcframework` with zero reliance on legacy C implementations.

### User Scenario 3: Swift 6 Native Replacement for Utility Code
- **Given** callers requiring natural string sorting, 16KB page-aligned allocations, system memory/CPU budget querying, or platform timers.
- **When** calling `DiskItemSorter`, `CUnsafeBufferAdapter`, `ConcurrencyBridge`, `PlatformMonotonicTimer`.
- **Then** these utilities execute purely via Swift 6 Foundation (`String.localizedStandardCompare`), `UnsafeMutableRawPointer.allocate(byteCount:alignment: 16384)`, `ProcessInfo`, and `ContinuousClock` with 0 C dependencies.

---

## 3. Functional Requirements & Technical Boundaries

1. **FR-1**: Physically remove the 44 obsolete/dead experimental C files in Batch 1.
2. **FR-2**: Redirect all Swift Adapter call sites from legacy C symbols to `ttzip_rust_*` C-ABI exports.
3. **FR-3**: Physically remove the 33 superseded C files in Batch 2.
4. **FR-4**: Migrate utility functions in Batch 3 to Swift 6 native implementations.
5. **FR-5**: Converge the remaining necessary C wrappers (Reed-Solomon, Zopfli, POSIX spawn, CRC64, Magic sniff) into a single `Sources/CTTZipBridge/CTTZipBridge.c`.
6. **FR-6**: Clean up `Sources/CTTZipBridge/include/` to remove unused headers while retaining essential public headers and `ttzip_rust_glue.h`.
7. **FR-7**: Ensure `swift test` (859 tests) and `./scripts/run_local_ci_gate.sh` (7 stages) pass 100% with 0 warnings.

---

## 4. Clarifications

- **Q1**: What happens to subdirectories `Sources/CTTZipBridge/{fast-lzma2, lzfse, snappy}`?
  - **Decision**: They are compiled into `libttzip_native_codecs.a` by `rust/ttzip-glue/build.rs` and bundled into `TTZipVendor.xcframework`. They remain excluded from SPM in `Package.swift`.
- **Q2**: What happens to `native_inflate/` and `zopfli/`?
  - **Decision**: `native_inflate/` is dead experimental code and will be deleted. `zopfli/` is kept for the Zopfli ultra-compression engine used by `ZipExtremeBlockWriter.swift` and compiled via `CTTZipBridge.c`.
