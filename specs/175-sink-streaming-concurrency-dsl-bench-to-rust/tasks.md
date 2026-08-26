# Tasks: 175-sink-streaming-concurrency-dsl-bench-to-rust

## Phase 1: 7z Solid Stream Decoder & Zstd Bounded Streaming Pipeline (US1, US2)
- [x] T001 [P] [US1] Implement `SevenZSeekIndex` and `extract_entry_bytes_stream` with early termination in `rust/ttzip-glue/src/sevenz/decoder.rs` and `header.rs`.
- [x] T002 [P] [US2] Implement `zstd_compress_stream_pipe` and `zstd_decompress_stream_pipe` with 4MB In / 4MB Out double buffers in `rust/ttzip-glue/src/codecs/zstd.rs`.
- [x] T003 [P] [US1, US2] Export `ttzip_rust_7z_extract_entry_memory`, `ttzip_rust_zstd_compress_file_stream`, and `ttzip_rust_zstd_decompress_file_stream` in `rust/ttzip-glue/src/ffi/`.
- [x] T004 [P] [US1, US2] Thin `Sources/TTZipCore/SevenZip/SevenZipSeekTable.swift` and `Sources/TTZipCore/Adapters/ZstdCAdapter.swift` to directly forward to new C-ABI stream functions.

## Phase 2: Lock-Free Ring Buffers & Rayon Zero-Polling Worker Pools (US3)
- [x] T005 [P] [US3] Implement `SpscRingBuffer` (cache-padded) and `MpmcRingBuffer` (Dmitry Vyukov atomic sequence queue) in `rust/ttzip-glue/src/runtime/ring_buffer.rs`.
- [x] T006 [P] [US3] Implement `EventDrivenWorkerPool` with `parking_lot::Condvar` OS kernel sleeping (0.0% idle CPU) in `rust/ttzip-glue/src/runtime/worker_pool.rs`.
- [x] T007 [P] [US3] Export runtime concurrency C-ABI functions in `rust/ttzip-glue/src/ffi/runtime_ffi.rs`.
- [x] T008 [P] [US3] Thin `Sources/TTZipCore/ConcurrencyPatterns/ArchiveWorkerPool.swift` and `BoundedProducerConsumerQueue.swift` to eliminate `Task.sleep` polling.

## Phase 3: Zero-Allocation Archive Filter DSL & Globset Path Filter (US4)
- [x] T009 [P] [US4] Implement `DslLexer<'a>`, `DslParser<'a>`, and `FilterExpr<'a>` with lifetime-borrowed zero-allocation AST in `rust/ttzip-glue/src/fs/filter.rs`.
- [x] T010 [P] [US4] Implement `PathPatternFilter` (via `globset` Aho-Corasick DFA) and zero-allocation `strip_leading_components` in `rust/ttzip-glue/src/fs/filter.rs`.
- [x] T011 [P] [US4] Export filter and DSL C-ABI functions in `rust/ttzip-glue/src/ffi/fs_ffi.rs`.
- [x] T012 [P] [US4] Thin `Sources/TTZipCore/InterpreterPattern/ArchiveFilterDSLLexerParser.swift` and `Security/PathPatternFilterEngine.swift`.

## Phase 4: Cross-Platform Monotonic Benchmarking & Pareto Convex Hull (US5)
- [x] T013 [P] [US5] Implement `MonotonicStopwatch` (`std::time::Instant`) and `MIPSHardwareBenchmarkEngine` in `rust/ttzip-glue/src/bench/clock.rs` & `mips.rs`.
- [x] T014 [P] [US5] Implement Dilworth multi-tier ranking and Andrew's Monotone Chain 2D Upper Convex Hull in `rust/ttzip-glue/src/bench/pareto.rs`.
- [x] T015 [P] [US5] Export benchmark and Pareto C-ABI functions in `rust/ttzip-glue/src/ffi/bench_ffi.rs`.
- [x] T016 [P] [US5] Thin `Sources/TTZipCore/Benchmark/MIPSHardwareBenchmarkEngine.swift` and `ParetoFrontierCalculator.swift`.

## Phase 5: Verification, CI Gates & Standalone CLI Validation (US6)
- [x] T017 [US6] Run `cargo test --workspace` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T018 [US6] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh` and test standalone `bin/ttzip`.
- [x] T019 [US6] Run `swift test` ensuring all 860+ tests pass with 0 failures and 0 warnings.
- [x] T020 [US6] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
