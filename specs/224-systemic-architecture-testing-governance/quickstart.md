# Quickstart: Systemic Architecture & Testing Governance

**Feature ID**: `224-systemic-architecture-testing-governance`  
**Classification**: `[Full SDD]`  

---

## 1. Running Resource-Invariant Tests

### 50GB Sparse File RSS Invariant Test
```bash
cargo test -p ttzip-engine --test sparse_fixture_rss_test -- --nocapture
```
*Expected: Creates 50GB APFS file in <5ms, verifies `peak_rss < 16MB` during entire inspection & streaming.*

### 100k Nodes Zero-Allocation VFS Search Test
```bash
cargo test -p ttzip-engine --test zero_alloc_vfs_search_test -- --nocapture
```
*Expected: 100,000 nodes searched with exactly 0 heap allocations.*

### Zero Disk-IO Leakage Assertion Test
```bash
swift test --filter ZeroDiskIOLeakHarnessTests
```
*Expected: 100 in-memory extractions executed with 0 bytes written to `/tmp`.*

---

## 2. Running Bidirectional C-ABI & Struct Field Linter

```bash
# Standard run
python3 scripts/lint_cabi_context.py

# Strict gate run (blocks on any error)
python3 scripts/lint_cabi_context.py --strict
```

---

## 3. Running Full-Pipeline APFS & FFI Tax Benchmark

```bash
swift run ttzip-bench pipeline --json-out docs/benchmarks/latest_pipeline_telemetry.json
```
