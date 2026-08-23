# Implementation Plan: 182-sink-benchmark-pareto-concurrency-and-cli-to-rust

## Technical Context
- **Objective**: Complete the seventh round of deep non-Rust sinking, moving Benchmark engines, Pareto convex hull, Concurrency pipelines, and Password Vault into Safe Rust (`rust/ttzip-glue`), while consolidating Swift CLI tools.

---

## Constitution Check
- [x] **Safe Rust Engine**: Benchmark, Pareto, Concurrency, and Vault all backed by Safe Rust.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **SRP & LOC Budget**: All files maintained strictly under $< 350\sim 500\text{ LOC}$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《基准测试与帕累托前沿凸包算法全面下沉至 Rust》: Completed.
- R002 [SUBAGENT:research] 《生产者消费者并发管道引擎下沉》: Completed.
- R003 [SUBAGENT:research] 《敏感内存安全保险库与 Zeroize 屏障》: Completed.

---

## Phase 1: Component Change List

### 1. Rust Glue Layer
- **`rust/ttzip-glue/src/benchmark/pareto.rs`**: Andrew monotone chain convex hull calculation.
- **`rust/ttzip-glue/src/crypto/vault.rs`**: Secure zeroize password vault.
- **`rust/ttzip-glue/src/ffi/`**: Export unified C-ABIs.

### 2. Swift Facades & Bridges
- **`Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift`**: Delegate Pareto calculation to Rust C-ABI.
- **`Sources/TTZipCore/PasswordVaultManager.swift`**: Delegate key wiping and secure memory to Rust C-ABI.
- **`Sources/TTZipCore/ConcurrencyPatterns/ArchivePipelineProducerConsumerEngine.swift`**: Consolidate with Rust concurrency.
- **`Sources/TTZipCore/CLI/`**: Thin out and consolidate CLI parsers and completion generators.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `./scripts/build_rust.sh --release && ./scripts/build_tui.sh`.
3. `swift test` ensuring all 886+ tests pass with 0 failures and 0 warnings.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
