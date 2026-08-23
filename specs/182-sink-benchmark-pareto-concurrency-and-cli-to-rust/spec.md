# Feature Specification: 182-sink-benchmark-pareto-concurrency-and-cli-to-rust

## 1. Executive Summary & Strategic Motivation
This feature represents the seventh round of deep non-Rust code sinking and structural unification across TTZip, focusing on:
1. **Benchmark Engine & Pareto Convex Hull Calculation (`rust/ttzip-glue/src/benchmark/`)**:
   - High-precision monotonic nanosecond timing, throughput and compression ratio evaluation across all 17 codecs, and $O(N \log N)$ Andrew monotone chain convex hull calculation.
   - Sinks `BenchmarkEngine.swift` (456 LOC), `InMemoryBenchmarkEngine.swift` (385 LOC), `ParetoFrontierModels.swift` (377 LOC), and `MultiCoreBreakdownRunner.swift` (360 LOC).
2. **Concurrency Pipeline & Worker Pool Dispatch (`rust/ttzip-glue/src/concurrency/`)**:
   - Multi-stage producer-consumer pipelines, work-stealing job queues, and hardware thermal-aware throttling.
   - Sinks `ArchivePipelineProducerConsumerEngine.swift` (438 LOC) and `WorkerPoolConformances.swift` (368 LOC).
3. **Password Vault & Secure Key Management (`rust/ttzip-glue/src/crypto/vault.rs`)**:
   - Zero-allocation AES-GCM password vault with `zeroize` compiler fences and salt-stretching KDF.
   - Sinks `PasswordVaultManager.swift` (416 LOC).
4. **POSIX CLI Argument Parsing & Shell Completion Generation**:
   - Thinning `POSIXCLIArgumentParser.swift` (441 LOC), `CLIOptions.swift` (340 LOC), `ShellCompletionGenerator.swift` (348 LOC), and `CLICommandSpec+Generators.swift` (343 LOC).

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Multi-Core Microsecond Benchmark Suite
- **Given** running `ttzip-bench` across 10 compression algorithms
- **When** benchmarking throughput and memory consumption
- **Then** execution runs with zero Swift garbage collection interference and microsecond nanosecond clock precision.

### User Scenario 2: Zeroize Password Vault
- **Given** master passwords and encrypted key bundles stored in memory
- **When** closing or locking the vault
- **Then** all memory pages are securely wiped using atomic volatile memory fences preventing LLVM DSE.

---

## 3. Success Metrics
1. **Engine Sinking**: 100% of Benchmark, Pareto, and Concurrency logic runs directly in Rust.
2. **SRP & LOC Budget**: 100% of first-party source files kept under $< 350\sim 500\text{ LOC}$.
3. **Zero Regression**: 100% pass rate across 175+ Rust tests, 886+ Swift tests, and 7/7 local CI stages.
