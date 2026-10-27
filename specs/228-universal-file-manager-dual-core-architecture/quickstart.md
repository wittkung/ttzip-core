# Quickstart & Verification Guide: Universal File Manager Dual-Core Architecture

**Feature**: `228-universal-file-manager-dual-core-architecture`  
**Date**: 2026-09-26  

---

## 1. Prerequisites

- macOS 13.0+ (Ventura, Sonoma, or Sequoia)
- Rust 1.84+ toolchain (`cargo`, `rustc`)
- Swift 6 toolchain (`swiftc`, `swift-build`)
- Bazel 8 (Bzlmod enabled)

---

## 2. Automated Architecture & Gate Verification

Run all gate audits sequentially:

```bash
# 1. Verify LOC Architecture Gate (all files <= 800 LOC)
./products/ttzip/core/scripts/lint_loc_gate.sh

# 2. Verify Rust Microkernel and app-core compile cleanly with zero warnings
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 3. Verify Bazel 8 builds all targets cleanly
bazel test -c opt //products/ttzip/... //infra/ttmedia/...
```

---

## 3. End-to-End Scenarios

### Scenario A: Zero-Copy Archive Navigation
1. Open a multi-gigabyte ZIP/7z file via TTZip.
2. Observe Memory footprint in Activity Monitor: stays strictly below 32MB.
3. Rapidly navigate through deep nested folders with keyboard arrows.
4. Verify 0ms beachball stalls and 120 FPS display refresh.

### Scenario B: In-Place Editing via `infra/ttmedia`
1. Open a Markdown or CSV file stored inside an archive.
2. Modify text in the editor and press `Cmd+S`.
3. Inspect WAL log in microkernel: verifies delta committed without full-archive recompression.

### Scenario C: Device Transfer with Sliding-Window Metrics
1. Attach an Android phone via USB.
2. Select files and queue a 2GB transfer.
3. Observe transfer speed indicator: rates update smoothly at 2Hz based on real measured throughput (no fake 75 MB/s).
