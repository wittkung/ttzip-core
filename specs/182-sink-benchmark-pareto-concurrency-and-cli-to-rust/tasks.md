# Tasks: 182-sink-benchmark-pareto-concurrency-and-cli-to-rust

## Phase 1: Pareto Convex Hull & Monotonic Timing in Rust (US1)
- [x] T001 [P] [US1] Implement `rust/ttzip-glue/src/benchmark/pareto.rs` with Andrew monotone chain 2D convex hull algorithm.
- [x] T002 [P] [US1] Export C-ABI `ttzip_rust_calculate_pareto_frontier` in `rust/ttzip-glue/src/ffi/` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T003 [P] [US1] Refactor `Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift` and `Sources/TTZipCore/BenchmarkEngine.swift` to delegate to Rust C-ABI, maintaining LOC < 350.
- [x] T004 [P] [US1] Add unit tests for Pareto frontier calculation in `rust/ttzip-glue/src/benchmark/pareto.rs` and `Tests/TTZipTests/`.

## Phase 2: Secure Password Vault with Zeroize Compiler Fence (US2)
- [x] T005 [P] [US2] Implement `rust/ttzip-glue/src/crypto/vault.rs` using `zeroize` memory clearing and AES-GCM encryption.
- [x] T006 [P] [US2] Export C-ABI for password vault operations in `rust/ttzip-glue/src/ffi/crypto_ffi/` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T007 [P] [US2] Refactor `Sources/TTZipCore/PasswordVaultManager.swift` to delegate to Rust C-ABI, maintaining LOC < 350.
- [x] T008 [P] [US2] Add unit tests for password vault operations and memory zeroization in `Tests/TTZipTests/`.

## Phase 3: Concurrency Pipeline & CLI Consolidations (US3)
- [x] T009 [P] [US3] Refactor `Sources/TTZipCore/ConcurrencyPatterns/ArchivePipelineProducerConsumerEngine.swift` and `WorkerPoolConformances.swift`, maintaining LOC < 350.
- [x] T010 [P] [US3] Refactor `Sources/TTZipCore/CLI/POSIXCLIArgumentParser.swift`, `CLIOptions.swift`, and `ShellCompletionGenerator.swift`, maintaining LOC < 350.
- [x] T011 [P] [US3] Refactor `BenchCommandRunner.swift`, `InMemoryBenchmarkEngine.swift`, and `MultiCoreBreakdownRunner.swift`, maintaining LOC < 350.
- [x] T012 [P] [US3] Run `swift test` across all suites.

## Phase 4: Verification, CI Gates & Standalone Validation (US4)
- [x] T013 [US4] Run `cargo test` across all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T014 [US4] Run `./scripts/build_rust.sh --release && ./scripts/build_tui.sh` and verify universal libraries and `bin/ttzip`.
- [x] T015 [US4] Run `swift test` ensuring all 886+ tests pass with 0 failures and 0 warnings.
- [x] T016 [US4] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
