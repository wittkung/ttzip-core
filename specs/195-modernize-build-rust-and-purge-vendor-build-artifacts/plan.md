# Implementation Plan: 195-modernize-build-rust-and-purge-vendor-build-artifacts

## Technical Context
- Modernize `scripts/build_rust.sh` to package Rust static libraries directly into `TTZipVendor.xcframework`.
- Purge `Vendor/libTTZipVendor.a`.
- Purge upstream CMake `build*` directories in `Vendor/`.

---

## Constitution Check
- [x] Zero Cloud Quota.
- [x] Single-file LOC $\le 800$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《build_rust.sh 纯粹化与 XCFramework 直连架构》: Completed.
- R002 [SUBAGENT:research] 《Vendor 上游 CMake 构建垃圾清扫》: Completed.

---

## Phase 1: Modernize build_rust.sh & Delete libTTZipVendor.a
- Update `scripts/build_rust.sh` to output directly to `Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a` and `Headers/ttzip_rust_glue.h`.
- Delete `Vendor/libTTZipVendor.a`.

## Phase 2: Purge Upstream CMake Build Directories
- Delete `Vendor/turbobench/build`.
- Delete `Vendor/zlib-ng-upstream/build_dev` and `build_orig`.
- Delete `Vendor/libarchive-upstream/build`.
- Delete `Vendor/xz-upstream/build`.
- Delete `Vendor/zstd-upstream/build`.
- Delete `Vendor/lz4-upstream/build`.

## Phase 3: Verification & Gate
- Run `scripts/build_rust.sh`.
- Run `./scripts/lint_loc_gate.sh`.
- Run `swift test` and `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.
