# Phase 0 Research: 182-sink-benchmark-pareto-concurrency-and-cli-to-rust

## Research Item R001: Benchmark Suite & Pareto Convex Hull in Rust
- **Decision**: Sinking the Andrew monotone chain Pareto computation and micro-benchmark orchestration into `rust/ttzip-glue/src/benchmark/`.
- **Rationale**: 
  - Sinks ~1,500 LOC of Swift benchmark boilerplate into unified Safe Rust, enabling identical Pareto analysis in CLI, TUI, and GUI.
- **Alternatives Considered**: 
  - *Maintaining separate Swift and Rust Pareto algorithms*: Risk of convex hull divergent calculations.
- **Source**: 
  - `Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift`
  - `Sources/TTZipCore/BenchmarkEngine.swift`

---

## Research Item R002: Concurrency Pipeline & Worker Pool
- **Decision**: Sinking the pipeline producer-consumer stage executor to use `rayon` and `crossbeam-channel`.
- **Rationale**: 
  - Eliminates Swift thread synchronization lock contentions and bounds memory queue spikes.
- **Alternatives Considered**: 
  - *Swift `DispatchQueue.concurrentPerform`*: High overhead for small granular chunks.
- **Source**: 
  - `Sources/TTZipCore/ConcurrencyPatterns/ArchivePipelineProducerConsumerEngine.swift`

---

## Research Item R003: Password Vault with Zeroize Compiler Fence
- **Decision**: Implement `rust/ttzip-glue/src/crypto/vault.rs` using `zeroize` and AES-256-GCM.
- **Rationale**: 
  - Guarantees memory clearing without DSE removal by LLVM `-O3`.
- **Alternatives Considered**: 
  - *Swift `memset`*: Can be eliminated by LLVM dead-store elimination passes.
- **Source**: 
  - `Sources/TTZipCore/PasswordVaultManager.swift`
