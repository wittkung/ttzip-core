# Feature Specification: 190-domain-responsibility-realignment-and-core-lightweighting

## 1. Executive Summary & Strategic Motivation
A thorough architectural inspection identified 4 glaring domain misplacements in `Sources/TTZipCore`:
1. **Benchmark Engine Misplacement**: 48 benchmark and plotting files in `Sources/TTZipCore/Benchmark/` belong in `Sources/TTZipBench/` or Rust crates.
2. **Swift TUI Redundancy**: 6 files in `Sources/TTZipCore/CLI/TUI/` duplicate the standalone Rust TUI binary (`rust/ttzip-tui/`).
3. **Obsolete Concurrency Patterns**: 20 files in `Sources/TTZipCore/ConcurrencyPatterns/` contain custom pthread locks/pools obsolete under Swift 6 Structured Concurrency and Rust Rayon work-stealing.
4. **Test Fuzzing Engine in Production Security**: `MalformedStreamFuzzEngine.swift` belongs outside production security modules.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Ultra-Thin Production Core
- **Given** building `TTZipCore`
- **When** the library is compiled
- **Then** only archive/crypto/VFS/localization primitives are included; all benchmarking, plotting, and test-only mutations are isolated to `TTZipBench` or tests.

### User Scenario 2: Modern Swift 6 & Rust Concurrency
- **Given** performing parallel operations in Swift
- **When** tasks are dispatched
- **Then** tasks use Swift 6 `async/await` and TaskGroups or direct Rust Rayon scheduling, without legacy POSIX mutex wrappers.

---

## 3. Success Metrics
1. **Source Code Purge / Realignment**: Purge / migrate 75+ files (~12,000 LOC eliminated from Core).
2. **Core Lightweighting**: `Sources/TTZipCore` contains $< 50$ total files.
3. **Zero Regression**: 100% pass rate on `cargo test`, `swift test`, and `./scripts/run_local_ci_gate.sh`.
