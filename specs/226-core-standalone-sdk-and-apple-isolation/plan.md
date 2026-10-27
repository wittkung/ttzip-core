# Implementation Plan: TTZipCore Standalone Binary SDK Distribution & Complete Apple Isolation

**Branch**: `226-core-standalone-sdk-and-apple-isolation` | **Date**: 2026-08-26 | **Spec**: [specs/226-core-standalone-sdk-and-apple-isolation/spec.md](file:///Users/kevintung/Documents/dev/products/ttzip/core/specs/226-core-standalone-sdk-and-apple-isolation/spec.md)

---

## 1. Summary

Establish a complete, industrial-grade standalone binary SDK distribution model for `ttzip-core`, decoupling `ttzip-apple` from internal Rust/C toolchains. Provide an automated packaging script (`build_sdk_framework.sh`) generating Universal Apple Silicon + Intel `TTZipVendor.xcframework.zip`, compute cryptographic SHA-256 checksums, and verify standalone zero-Rust client building.

---

## 2. Technical Context

- **Language & Standards**: Rust 2021 edition, Swift 6.0, Mozilla UniFFI 0.28, Apple XCFramework Format 1.0.
- **Architectures**: macOS Universal (`arm64` + `x86_64`).
- **Platform Invariants**: macOS 14.0+, iOS 17.0+, `-strict-concurrency=complete`, `Zero-Warning Hard Gate`.
- **Packaging Pipeline**: Cargo multi-target cross-compilation -> `libtool` -> `uniffi-bindgen` -> `xcodebuild -create-xcframework` -> `zip` -> `swift package compute-checksum`.

---

## 3. Constitution Check

- **100% Mozilla UniFFI Standard**: Verified (0 raw C pointers or unmanaged memory handles exposed to `apple`).
- **Swift 6 Presentation Boundary**: Verified (`apple` is purely UI/macOS frameworks, all compute in Rust).
- **Single-File LOC Threshold ($\le 800$ LOC)**: Verified (all scripts and modules conform to strict threshold).
- **Zero In-Tree Path Invariant**: Verified (eliminating required relative path assumptions).

---

## 4. Execution Phases

### Phase 0: SDK Packaging Automation (`core/scripts/build_sdk_framework.sh`)
1. Create `core/scripts/build_sdk_framework.sh` supporting `--release` mode.
2. Automate multi-architecture compilation:
   - `cargo build --target aarch64-apple-darwin --release`
   - `cargo build --target x86_64-apple-darwin --release`
3. Combine Rust static archives and native codec archives via `libtool` / `lipo`.
4. Run `uniffi-bindgen` Swift generation and apply `postprocess_uniffi_swift.py`.
5. Construct Universal `TTZipVendor.xcframework` (Info.plist + `macos-arm64_x86_64`).
6. Compress into `dist/TTZipVendor-vX.Y.Z.xcframework.zip` and compute SPM SHA-256 checksum.

### Phase 1: Local CI Gate Integration (`core/scripts/run_local_ci_gate.sh`)
1. Integrate XCFramework validation into `core/scripts/run_local_ci_gate.sh`.
2. Verify `lipo -info` confirms both `arm64` and `x86_64` slices.

### Phase 2: Client Standalone Verification (`apple/`)
1. Verify `apple/Package.swift` dual-mode resolution.
2. Test `swift build` and `swift test` in `apple/` in isolated environment.
3. Test `./scripts/bundle_app.sh --channel direct` in `apple/`.

---

## 5. Verification Plan

- `core/scripts/build_sdk_framework.sh` exits with code 0 and produces valid universal XCFramework.
- `swift package compute-checksum` returns deterministic 64-char SHA-256 string.
- `swift test` in `apple/` passes 170 tests with 0 failures.
- `bundle_app.sh` in `apple/` produces signed `dist/TTZip.app`.
