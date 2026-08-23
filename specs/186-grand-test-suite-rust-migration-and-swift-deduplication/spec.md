# Feature Specification: 186-grand-test-suite-rust-migration-and-swift-deduplication

## 1. Executive Summary & Strategic Motivation
With all core domain engines (compression, encryption, parsing, recovery, VFS, and differential verification) residing in **Safe Rust (`rust/ttzip-glue`)**, the dual maintenance of tests in Swift `Tests/TTZipTests/` is an architectural burden.
1. **Migrate Algorithmic & Invariant Tests to Rust (`rust/ttzip-glue/tests/`)**:
   - Establish comprehensive Rust integration suites (`tests/patterns_and_pipelines.rs`, `tests/crypto_and_recovery.rs`, `tests/memory_and_buffers.rs`).
2. **Purge Redundant Swift Low-Level Tests**:
   - Delete 40+ redundant Swift test files in `Tests/TTZipTests/` (~8,000 LOC of duplicated test scaffolding).
3. **Streamline Swift Tests for Native UI & Public Facades**:
   - Retain only high-level Swift public facade verification and macOS system integration tests (`TTZipAppTests/`, QuickLook, Finder, etc.).

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Cross-Platform Native Cargo Test Suite
- **Given** building TTZip on Linux, Windows, or macOS
- **When** running `cargo test --workspace`
- **Then** all 220+ unit, property, fuzzing, and invariant tests execute natively in $<2\text{s}$ without Swift/Darwin dependencies.

### User Scenario 2: Instant Swift Packaging & Test Execution
- **Given** running `swift test` on macOS
- **When** executing tests
- **Then** the suite executes high-level facade tests in $<3\text{s}$ with zero boilerplate overhead.

---

## 3. Success Metrics
1. **Test De-Duplication**: Delete 40+ redundant Swift test files (~8,000 LOC).
2. **Rust as Single Test Oracle**: 100% of engine invariants covered in `rust/ttzip-glue/tests/`.
3. **Zero Regression**: 100% pass rate on `cargo test`, `swift test`, and `./scripts/run_local_ci_gate.sh`.
