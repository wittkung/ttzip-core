# Quickstart Validation Guide: Architectural Audit and Paradigm Evolution

- **Feature ID**: `004-architecture-audit-and-paradigm-evolution`
- **Target Subsystems**: `ttzip-engine` (Rust Core), `CTTZipBridge` (C-ABI), `TTZipCore` (Swift SDK), `TTZipApp` (SwiftUI / AppKit)
- **Status**: `READY`

---

## 1. Prerequisites & Environment Setup

Ensure Xcode 15+, Swift 6 toolchain, and Rust 1.80+ are available:

```bash
# Verify Swift 6 and Rust toolchains
swift --version
cargo --version
```

---

## 2. Validation Scenarios

### Scenario 1: Bounded-Memory Solid 7z Extraction Test
Validates that extracting a multi-gigabyte solid 7z archive consumes $\le 64\text{MB}$ peak RSS.

```bash
# Run 7z sliding window bounded memory integration test
cargo test -p ttzip-engine --test extract_single_mmap_bounded_memory -- --nocapture
```
*Expected Outcome*: Decompression streams directly to disk; peak RSS sampled via Mach `task_info` remains $\le 64.0\text{MB}$.

---

### Scenario 2: Parallel Work-Stealing Extraction DAG Speedup
Validates multi-core scaling on archives with $>1,000$ files.

```bash
# Run multi-core parallel extraction integration test
cargo test -p ttzip-engine --test standards_integration_tests test_parallel_extraction_dag -- --nocapture
```
*Expected Outcome*: Extraction scales across all available CPU cores with $\ge 3.5\times$ speedup on 8-core Apple Silicon.

---

### Scenario 3: Arena VFS O(N) Construction & Zero-Allocation Search
Validates flat arena VFS tree construction speed and zero heap allocations during search.

```bash
# Run zero-alloc VFS search & arena construction benchmark
cargo test -p ttzip-engine --test zero_alloc_vfs_search_test -- --nocapture
```
*Expected Outcome*: 100,000 entries construct in $<30\text{ms}$; 0 heap allocations recorded during fuzzy search.

---

### Scenario 4: C-ABI Symbol Parity & Contract Linting
Validates bidirectional symbol mapping and JSON contract compliance.

```bash
# Run contracts linter
bash .specify/scripts/bash/lint-contracts.sh specs/004-architecture-audit-and-paradigm-evolution/contracts
```
*Expected Outcome*: All 4 contracts pass linting with zero warnings.

---

### Scenario 5: Full Swift & Rust Unit/Integration Test Suite
```bash
# Run Swift 6 package tests
swift test --filter TTZipCoreTests

# Run Rust engine cargo test suite
cargo test --manifest-path core/rust/ttzip-engine/Cargo.toml
```
*Expected Outcome*: 100% test pass rate with zero warnings.
