# Feature Specification: TTZipCore Standalone Binary SDK Distribution & Complete Apple Isolation

**Feature Branch**: `226-core-standalone-sdk-and-apple-isolation`  
**Date**: 2026-08-26  
**Status**: DRAFT  
**Scope**: `ttzip-core` & `ttzip-apple` System Boundary & Distribution Pipeline

---

## 1. Problem Statement & User Value

### 1.1 Context
`ttzip-core` is the high-performance cross-platform archiving microkernel (Rust + Mozilla UniFFI + C-Bridge + Swift 6 facade). Currently, `ttzip-apple` consumes `ttzip-core` through local relative paths (`../core`) or SPM source targets, exposing internal Rust/C toolchains and build scripts to client application developers.

### 1.2 User Pain Points
1. **Toolchain Pollution**: External macOS/iOS developers cloning `ttzip-apple` need Rust/Cargo/Clang toolchains if they need to build or modify microkernel bridges.
2. **Build Latency**: SPM recompiles C and Swift source modules repeatedly instead of performing O(1) instant binary framework linking.
3. **Repository Coupling**: Breaking changes in `core` lack strict semantic versioning (SemVer) boundaries against `apple`.

### 1.3 Core Requirements
1. **Microkernel Standalone SDK (`ttzip-core`)**:
   - `ttzip-core` must provide an automated release pipeline (`build_sdk_framework.sh`) generating `TTZipVendor.xcframework.zip` with universal macOS (`arm64` + `x86_64`) binary slices.
   - `core/Package.swift` must support release mode with `binaryTarget(name:url:checksum:)` and local development mode.
2. **Pure Black-Box Consumption (`ttzip-apple`)**:
   - `ttzip-apple` must consume `TTZipCore` as an external versioned dependency (via official Git tag / release).
   - Zero Rust toolchain requirement for `apple` developers.
   - Dual-mode local resolution supporting sub-second local iteration for core engine engineers.

---

## 2. User Scenarios & User Stories

### User Story 1: Independent App Developer (Zero Rust)
> As an Apple platform developer, I want to `git clone https://github.com/wittkung/ttzip-apple.git` and run `swift build` or open in Xcode without having Rust, Cargo, or CMake installed on my Mac, so that I can immediately build and contribute to the macOS UI.

### User Story 2: Core Microkernel Engineer (Automated SDK Release)
> As a core engine architect, I want to run `./scripts/build_sdk_framework.sh` to generate a universal `TTZipVendor.xcframework.zip` and its cryptographic SHA-256 checksum, so that releases can be published to GitHub Releases with zero manual packaging errors.

### User Story 3: Full-Stack Engineer (Live Local Co-Development)
> As an engineer developing a new archive algorithm in Rust and its UI in SwiftUI, I want `apple/Package.swift` to automatically detect sibling `../core` on my local machine without requiring remote releases, so that I have instant sub-second feedback loops.

---

## 3. Success Metrics

1. **Zero Rust Requirement**: `swift build` in `apple/` succeeds on a pristine macOS machine without `cargo` / `rustc` in `$PATH`.
2. **SPM Link Time**: Linking precompiled `TTZipVendor.xcframework` in `apple/` takes $\le 3.0$ seconds.
3. **Cryptographic Checksum Gate**: 100% deterministic SHA-256 validation on binary zip assets.
4. **Zero Regressions**: All 170 unit and integration tests in `TTZipAppTests` pass without warnings or errors.
