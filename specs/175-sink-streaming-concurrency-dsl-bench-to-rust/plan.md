# Implementation Plan: 175-sink-streaming-concurrency-dsl-bench-to-rust

## Technical Context
- **Target Architecture**: Self-sufficient Safe Rust core (`rust/ttzip-glue` and `rust/ttzip-tui`) + Ultra-thin Swift layer (`Sources/TTZipApp`, minimal C-ABI bridge).
- **Core Components Sinking**:
  1. **7z Solid Stream Decoder & SeekTable**: Folder-level in-memory decompression with early termination (<10ms single-item extract without disk temporary files).
  2. **Bounded Zstd Stream Pipeline**: 4MB In / 4MB Out double buffer stream processor (strictly <16MB RAM, eliminating 10GB+ OOM crashes).
  3. **Lock-Free Ring Buffers & Rayon Concurrency**: Dmitry Vyukov MPMC sequence queue, SPSC cache-padded ring buffer, and `parking_lot` Condvar thread sleeping (0.0% idle CPU, zero `Task.sleep` polling).
  4. **Zero-Allocation Archive Filter DSL & Globset**: Lifetime-borrowed `DslLexer`/`DslParser` and Aho-Corasick DFA multi-pattern path filtering.
  5. **Cross-Platform Monotonic Benchmarking & Pareto Convex Hull**: `std::time::Instant` stopwatch, 7-Zip MIPS rating, and Andrew's Monotone Chain 2D Upper Convex Hull.

---

## Constitution Check
- [x] **Principle 1: Safe Rust First**: Unsafe pointer allocations, manual `posix_memalign`, and unbounded buffer allocations in Swift are replaced by safe, bounded Rust pipelines.
- [x] **Principle 2: Zero Polling**: `Task.sleep` polling loops in worker pools are completely replaced by OS futex / event condition variables.
- [x] **Principle 3: Zero OOM & Constant Memory**: Streaming codecs enforce bounded 4MB double-buffering.
- [x] **Principle 4: Zero Breaking Changes**: All existing Swift public APIs retain backward compatibility through high-level C-ABI glue, ensuring 100% test pass rate across 860+ tests and 7/7 local CI stages.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《7z 固实流纯内存解码与零磁盘 SeekTable 方案》: Completed.
- R002 [SUBAGENT:research] 《Zstd 真正有界流式管道与零 OOM 缓冲方案》: Completed.
- R003 [SUBAGENT:research] 《无锁环形队列与 Rayon 多核调度消除 CPU 空耗方案》: Completed.
- R004 [SUBAGENT:research] 《零分配 Archive Filter DSL 与跨平台 Glob 模式匹配方案》: Completed.
- R005 [SUBAGENT:research] 《跨平台高精度单调时钟、MIPS 与 Pareto 凸包分析中枢方案》: Completed.

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. `rust/ttzip-glue/src/` Modules
- **`src/sevenz/`**:
  - `decoder.rs`: Implement `SevenZSeekIndex` and `extract_entry_bytes_stream` with early termination.
- **`src/codecs/zstd.rs`**:
  - Implement `zstd_compress_stream_pipe` and `zstd_decompress_stream_pipe` with 4MB double buffering.
- **`src/runtime/`**:
  - `ring_buffer.rs`: SPSC & MPMC lock-free sequence ring buffer.
  - `worker_pool.rs`: Rayon work-stealing pool with `parking_lot` Condvar sleeping.
- **`src/fs/filter.rs`**:
  - `DslLexer<'a>`, `DslParser<'a>`, `FilterExpr<'a>`, and `PathPatternFilter` via `globset`.
- **`src/bench/`**:
  - `clock.rs`: Monotonic stopwatch via `std::time::Instant`.
  - `mips.rs`: 7-Zip standard MIPS benchmark engine.
  - `pareto.rs`: Dilworth 2D ranking and Andrew's Monotone Chain upper convex hull.

### 2. C-ABI FFI Updates
- `src/ffi/codecs_ffi/zstd.rs`: Export `ttzip_rust_zstd_compress_file_stream`, `ttzip_rust_zstd_decompress_file_stream`.
- `src/ffi/archive_ffi/extract.rs`: Export `ttzip_rust_7z_extract_entry_memory`.
- `src/ffi/runtime_ffi.rs`: Export runtime ring buffer and worker pool C-ABIs.
- `src/ffi/fs_ffi.rs`: Export DSL and Glob filtering C-ABIs.
- `src/ffi/bench_ffi.rs`: Export MIPS and Pareto C-ABIs.
- Update `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.

### 3. Swift Thinning
- `Sources/TTZipCore/SevenZip/SevenZipSeekTable.swift`: Route single item extraction to `ttzip_rust_7z_extract_entry_memory`.
- `Sources/TTZipCore/Adapters/ZstdCAdapter.swift`: Route streaming compress/decompress to Rust stream pipes.
- `Sources/TTZipCore/ConcurrencyPatterns/ArchiveWorkerPool.swift`: Replace polling loops with Rust event-driven primitives.
- `Sources/TTZipCore/InterpreterPattern/ArchiveFilterDSLLexerParser.swift`: Forward DSL evaluation to Rust.
- `Sources/TTZipCore/Benchmark/MIPSHardwareBenchmarkEngine.swift` & `ParetoFrontierCalculator.swift`: Forward to Rust.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` across all unit, property, and integration tests.
2. `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh`.
3. `swift test` across all 860+ tests ensuring 100% green.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
