# Technical Research: Standalone SDK Packaging & SPM Binary Distribution

**Feature**: `226-core-standalone-sdk-and-apple-isolation`  
**Date**: 2026-08-26  
**Status**: COMPLETE

---

## 1. Industry Benchmark Comparison

We surveyed four production-grade Rust/C++ to Swift SDK architectures:

| Project | Core Language | FFI / Bridge | Distribution Pattern | Checksum Verification | Local Dev Switch |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1Password Bionic Core** | Rust | UniFFI + TypeShare | Prebuilt XCFramework via Internal Registry | Yes (SHA-256) | Path overrides in SPM |
| **Mozilla Application Services** | Rust | Mozilla UniFFI | GitHub Release `MozillaRustComponents.xcframework.zip` | Yes (SPM BinaryTarget) | `megazords/ios-rust` local switch script |
| **Matrix Rust SDK** | Rust | Mozilla UniFFI | Dedicated Distributor Repo (`matrix-rust-components-swift`) | Yes (SPM BinaryTarget) | `.package(path: "...")` |
| **Realm Core** | C++ | C-Bridge + Swift | Precompiled S3/GitHub Release XCFramework | Yes (SPM BinaryTarget) | CMake local build fallback |

---

## 2. Apple Client API Consumption Inventory

- **56 Source Files** in `apple/Sources/` import `TTZipCore`.
- **31 Test Files** in `apple/Tests/` import `TTZipCore`.
- **Zero Raw C Imports**: `apple` has **0 imports of `CTTZipBridge`** and 0 raw C pointer / FFI handles.
- All boundary calls are mediated through Swift 6 `Sendable` structs, classes, actors, and enums:
  - `TTZipEngine`, `ArchiveReader`, `ArchiveWriter`, `InPlaceArchiveMutationEngine`, `PasswordVaultManager`, `ArchiveEntry`, `ArchiveTreeNode`, `ArchiveCompressionFormat`, `ArchiveProgress`, `ByteCountFormatterFlyweight`.

---

## 3. Compiler & XCFramework Parameters

### Universal macOS Binary Slices
- `aarch64-apple-darwin` (Apple Silicon M1/M2/M3/M4)
- `x86_64-apple-darwin` (Intel Mac)
- Merged via `libtool -static` and `lipo -create` into `Vendor/TTZipVendor.xcframework/macos-arm64_x86_64/libTTZipVendor.a`.

### Swift 6 Library Evolution Flags
- `-enable-library-evolution`
- `-emit-module-interface`
- `-strict-concurrency=complete`
- `-swift-version 6`
- System Linker Settings: `-larchive`, `-lbz2`, `-liconv`, `-lc++`, `-lcompression`, `-framework Security`.

---

## 4. Architectural Decision: Release Mode vs. Local Development Mode

1. **Release Mode (Architecture A)**:
   - `core` builds Universal XCFramework -> zips as `TTZipVendor-vX.Y.Z.xcframework.zip` -> computes SHA-256 -> attaches to GitHub Release.
   - `core/Package.swift` and `apple/Package.swift` consume via `.binaryTarget(name: "TTZipVendor", url: "...", checksum: "...")`.
2. **Local Mode (Auto-detection)**:
   - If sibling `../core/Package.swift` exists or `TTZIP_USE_REMOTE_CORE` is unset, `apple/Package.swift` automatically uses local path dependency for live debugging.
