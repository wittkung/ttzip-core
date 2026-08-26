# Quickstart & Verification Guide: Systemic Quality & Architecture Governance

- **Feature ID**: `003-systemic-quality-and-architecture-governance`
- **Created**: 2026-08-24

---

## 1. Prerequisites

- macOS 14.0+ (Apple Silicon `aarch64` / Intel `x86_64`)
- Xcode 16+ Command Line Tools
- Rust 1.80+ (`cargo`, `rustc`, `rustfmt`, `clippy`)
- Python 3.10+

---

## 2. Automated Multi-Stage Verification Suite

### Stage 1: Fast Single-File LOC & C-ABI Symbol Alignment Gate
```bash
# 1. Single-File LOC Defense Gate (Hard Threshold: <= 800 LOC)
./core/scripts/lint_loc_gate.sh

# 2. C-ABI Symbol Parity Check
./core/scripts/verify_cabi_symbols.sh
```

### Stage 2: Swift 6 Strict Concurrency & Facade Test Suite
```bash
swift test --package-path core
```

### Stage 3: Rust Core Industrial Suite (Proptest, Fuzz, Differential)
```bash
cargo test -p ttzip-engine --manifest-path core/rust/Cargo.toml
```

### Stage 4: AddressSanitizer & ThreadSanitizer Diagnostics
```bash
# Run Swift AddressSanitizer on Vault & Memory Tests
swift test --package-path core --sanitize=address --filter VaultMemorySanitizationTests

# Run Swift ThreadSanitizer on Integration Tests
swift test --package-path core --sanitize=thread --filter TTZipCoreIntegrationTests
```

### Stage 5: Automated Git Worktree A/B Performance Zero-Regression Gate
```bash
python3 core/scripts/run_comprehensive_ab_benchmark.py
```
**Expected Outcome**: All 5 stages output `[PASS]` with 0 failures, $\ge 0\%$ throughput delta, and 0 memory leaks.
