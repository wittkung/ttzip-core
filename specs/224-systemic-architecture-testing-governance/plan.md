# Implementation Plan: Systemic Architecture, Resource Invariants & Testing Governance Hardening

**Feature ID**: `224-systemic-architecture-testing-governance`  
**Classification**: `[Full SDD]`  
**Date**: 2026-08-24  
**Status**: Draft  

---

## 1. Technical Context & Constraints

- **Language & Platforms**: Swift 6 (Strict Concurrency), Rust 2021 (Memory Safe, Send/!Sync), C-ABI (`CTTZipBridge`), macOS Apple Silicon (arm64, Darwin Mach Kernel, APFS).
- **Quality Gates**:
  - LOC Defense Gate: Single-file $\le 800$ LOC limit.
  - C-ABI Alignment Gate: 100% header-binary parity & 0 orphaned exports.
  - Zero-Regression Gate: 50-point Pareto throughput and resource invariant compliance.
  - Local Pre-Push Gate: Total runtime $< 30\text{s}$.

---

## 2. Proposed Changes & Implementation Phases

### Phase 1: Resource Invariant Testing Fixtures (P1)
1. **APFS 50GB Sparse File Generator & RSS Monitor**:
   - Create `rust/ttzip-engine/tests/sparse_fixture_rss_test.rs` with `MachTaskBasicInfo` sampler and `ApfsSparseZipFixture`.
2. **Zero-Allocation Scoped Allocator & VFS Search**:
   - Create `rust/ttzip-engine/tests/zero_alloc_vfs_search_test.rs` with `TrackingAllocator` and `fuzzy_match_zero_alloc`.
   - Create `Tests/TTZipTests/ZeroAllocVfsBridgeTests.swift`.
3. **Zero-Disk-IO Leakage Watchdog**:
   - Create `Tests/TTZipTests/ZeroDiskIOLeakHarnessTests.swift` using `FSEventTempWatcher` and `proc_pid_rusage`.

### Phase 2: Bidirectional C-ABI & Struct Context Linter (P1)
1. **Linter Script**:
   - Create `scripts/lint_cabi_context.py` implementing `ClangAstExtractor` (via `clang -Xclang -ast-dump=json`) and `SwiftConsumptionScanner`.
2. **Exemptions Database**:
   - Create `scripts/cabi_exemptions.json` for intentional test/fuzzing symbols.
3. **CI Gate Integration**:
   - Update `scripts/verify_cabi_symbols.sh` to execute `lint_cabi_context.py --strict` in Stage 2.

### Phase 3: End-to-End Engine Dispatch Provenance & Anti-Fallback (P2)
1. **Rust Microkernel Telemetry**:
   - Add `TTZipEngineTag` and `TTZipExecutionProvenance` in `rust/ttzip-engine/src/types.rs`.
   - Add thread-local execution recorder and C-ABI export in `rust/ttzip-engine/src/ffi/archive_ffi/`.
   - Inject execution tags into `StreamingParallelZipWriter`, `SevenZArchive`, `in_place_edit_zip`, `in_place_edit_sevenz`.
2. **C-ABI Header Declaration**:
   - Declare provenance types and `ttzip_rust_get_last_execution_provenance` in `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
3. **Swift Core Telemetry**:
   - Add `Sources/TTZipCore/Pipeline/EngineDispatchProvenance.swift` and `EngineProvenanceCollector.swift`.
   - Add `createArchiveWithReport` and `extractWithReport` methods in `ArchiveWriter` / `ArchiveReader` / `InPlaceArchiveMutationEngine`.
4. **Anti-Fallback Assertions**:
   - Add `Tests/TTZipTests/TTZipAssertions+Provenance.swift` and `Tests/TTZipTests/E2EEnginePathTracerTests.swift`.

### Phase 4: Full-Pipeline APFS & FFI Tax Benchmark (P3)
1. **TTZipBench CLI Extension**:
   - Add `pipeline` subcommand in `Sources/TTZipBench/main.swift` to measure `Swift -> C-ABI -> Rust -> APFS` throughput and calculate `FFI Tax %`.

---

## 3. Verification Plan

```bash
# 1. Run C-ABI Linter
python3 scripts/lint_cabi_context.py --strict

# 2. Run Rust Invariant Tests
cargo test -p ttzip-engine --test sparse_fixture_rss_test -- --nocapture
cargo test -p ttzip-engine --test zero_alloc_vfs_search_test -- --nocapture

# 3. Run Swift Tests & Provenance Assertions
swift test --filter ZeroDiskIOLeakHarnessTests
swift test --filter E2EEnginePathTracerTests
swift test

# 4. Run Pipeline Benchmark
swift run ttzip-bench pipeline

# 5. Full 5-Stage Local CI Gate
./scripts/run_local_ci_gate.sh
```
