# Quickstart Validation Guide: TTZip Systemic Governance & Resilience

- **Feature ID**: `002-systemic-architecture-and-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `READY`

---

## 1. Prerequisites

- macOS 14.0+ / Apple Silicon or x86_64
- Rust toolchain 1.80+ (`cargo`, `rustc`, `libtool`)
- Swift 6.0+ toolchain (`swift`, `swiftpm`)
- Python 3.10+ (for statistical A/B benchmark tooling)

---

## 2. Test Execution & Governance Scenarios

### Scenario 1: Validate C-ABI Contract Schemas
```bash
bash .specify/scripts/bash/lint-contracts.sh specs/002-systemic-architecture-and-governance/contracts
```
*Expected Outcome*: All 3 contract schemas pass validation with 0 errors.

### Scenario 2: Execute Rust Full Test & Property Suite
```bash
cargo test --manifest-path core/rust/Cargo.toml --all-targets
```
*Expected Outcome*: 100% unit tests, property tests, and TUI integration tests pass with 0 failures.

### Scenario 3: Execute Swift Core Integration & Concurrency Tests
```bash
swift test --package-path core
```
*Expected Outcome*: All 147 test cases in `TTZipCoreIntegrationTests`, `ProgressStreamingBridgeTests`, and `QuickLookPreviewTests` pass with 0 failures.

### Scenario 4: Execute Multi-Engine Hardware Codec Regression Gate
```bash
swift run --package-path core ttzip-bench gate
```
*Expected Outcome*: `✅ GATE PASSED: All hardware & codec invariants verified.`

### Scenario 5: Execute Automated Statistical A/B Benchmark Delta
```bash
python3 core/scripts/ab_performance_audit.py
```
*Expected Outcome*: Throughput $\Delta\% \ge 0\%$ (zero regression), VFS search latency consistency $\pm 9.45\text{ms}$, multi-file roundtrip latency improvement.
