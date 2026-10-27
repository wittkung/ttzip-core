# Quickstart & Verification Guide: Rust Core & Glue Layer Architectural Reconstruction

**Feature**: `223-rust-core-and-glue-architectural-reconstruction`  
**Date**: 2026-08-24  
**Spec Reference**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/core/specs/223-rust-core-and-glue-architectural-reconstruction/spec.md) | **Plan**: [`plan.md`](file:///Users/kevintung/Documents/dev/TTZip/core/specs/223-rust-core-and-glue-architectural-reconstruction/plan.md)

---

## 1. Prerequisites & Environment Setup

Ensure Xcode 16+, Swift 6.0, and Rust 1.80+ are installed:

```bash
rustc --version
swift --version
```

---

## 2. Compilation & C-ABI Parity Gate

Verify that the reconstructed engine compiles cleanly with zero warnings and passes the symbol gate:

```bash
# 1. Build and verify Rust engine
cd core/rust
cargo check --workspace --all-targets

# 2. Run unit & property tests
cargo test --workspace

# 3. Verify C-ABI symbol parity between header and static library
cd ../..
./scripts/verify_cabi_symbols.sh
```

---

## 3. End-to-End Test Scenarios

### Scenario 1: Memory-Safe Single Entry Preview on 50GB Archive

```bash
# Extract single 5KB file from 50GB test archive and assert bounded memory RSS
cargo test -p ttzip-engine --test extract_single_mmap_bounded_memory -- --nocapture
```

**Expected Outcome**: Memory peak $\le 64\text{MB}$, zero full-file heap loading.

---

### Scenario 2: Streaming Parallel ZIP Multi-Core Creation

```bash
# Compress synthetic 10GB corpus via streaming parallel writer
cargo test -p ttzip-engine --test streaming_parallel_zip_test -- --nocapture
```

**Expected Outcome**: Multi-core CPU utilization $> 700\%$, peak RSS $< 1\text{GB}$, disk output verified with `unzip -t`.

---

### Scenario 3: 100k-Entry VFS Search Zero-Allocation Latency

```bash
# Run 10 successive fuzzy search queries on 100k-entry RustVfsSession
swift test --filter RustVfsSessionSearchBenchmarkTests
```

**Expected Outcome**: 0 tree handle rebuilds across queries, query latency $< 5\text{ms}$ per search, 0 heap growth.

---

### Scenario 4: Error Context Diagnostics Retrieval

```bash
# Trigger corrupted header error and inspect thread-local error message
cargo test -p ttzip-engine --test error_diagnostics_test -- --nocapture
```

**Expected Outcome**: `ttzip_rust_last_error_message()` returns formatted offset and path details without memory leaks.
