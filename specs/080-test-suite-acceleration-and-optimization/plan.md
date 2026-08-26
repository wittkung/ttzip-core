# Implementation Plan: 080-test-suite-acceleration-and-optimization

## Technical Context
- **Project**: TTZip (Native High-Performance Archiving & Compression Engine)
- **Problem**: Full test suite (`swift test`) contains 883 test cases taking **116.76 seconds** (~2 minutes). 45.0% of the runtime (52.59s) is dominated by sequential disk I/O fuzzing in `ArchiveMutationFuzzTests`, and ~25% (28.02s) is consumed by un-synchronized sleep loops and non-adaptive benchmark loops in `StrategyPatternTests`, `CRC64HardwareTests`, `ExhaustiveCompressionCombinationsTests`, and `LZ4DeepIntegrationAndVFSTests`.
- **Target**: Accelerate full test suite runtime to **<= 20.0s** (6x~10x speedup) with 100% test case pass rate, zero assertion drops, and zero compromise on constitution Level 0 performance floors.

## Constitution Check
- **Zero-Cost Abstraction**: All optimizations are in test orchestration, memory buffering, and concurrency dispatch without modifying hot-path production code.
- **Invariants**: 100% test case count (883+ tests) retained. Hard performance floor tests in `XCTestPerformanceMeasureTests` remain 100% untouched and active.

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《ArchiveMutationFuzzTests 多核并发化与无盘内存变异执行研究》: 使用 Swift 6 结构化并发 `withThrowingTaskGroup`、确定性 Seed 派生与纯内存流微缓冲，消除磁盘 I/O 风暴，配合 `TTZIP_DEEP_FUZZ` 自适应分级。
- - R002 [SUBAGENT:research] 《StrategyPatternTests 与 RepositoryPatternTests 高并发测试精确同步与零 Sleep 改造》: 消除 33,000+ 次 `Task.sleep` 挂起，重构为微秒级三相确定性取消与依赖注入测试隔离。
- - R003 [SUBAGENT:research] 《基准测试自适应采样分级与性能门禁保障研究》: 建立 `TTZIP_RUN_BENCHMARKS` 采样分级中枢，日常模式缩减 96.8% 冗余计算，显式模式保留全量统计。

## Phase 1: Contracts & Data Models
- [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/080-test-suite-acceleration-and-optimization/data-model.md)
- [contracts/test-suite-config-schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/080-test-suite-acceleration-and-optimization/contracts/test-suite-config-schema.json)
- [contracts/fuzz-parallel-task-schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/080-test-suite-acceleration-and-optimization/contracts/fuzz-parallel-task-schema.json)
- [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/080-test-suite-acceleration-and-optimization/quickstart.md)

## Component Breakdown & Changes

### 1. Fuzzing Acceleration (`Tests/TTZipTests/ArchiveMutationFuzzTests.swift`)
- Refactor all 7 test methods to execute across formats concurrently using `withThrowingTaskGroup`.
- Replace disk file writing with in-memory buffer probing via `archive_read_open_memory` / `SecurityProtectionProxy` in-memory demuxers.
- Use `taskSeed` per-iteration PRNG isolation for 100% deterministic reproducibility.
- Enable `TTZIP_DEEP_FUZZ` environment detection for deep vs fast fuzzing.

### 2. Concurrency Synchronization (`Tests/TTZipTests/StrategyPatternTests.swift` & `RepositoryPatternTests.swift`)
- Remove `Task.sleep` in `testRound3MultiCoreBruteForce100PlusTasksGroupCancellationSafety` and use microsecond in-memory validation with 3-phase structured cancellation checks.
- Inject independent `PasswordVaultManager` test instance in `testHighConcurrency100ThreadsPasswordRepositoryReadWrite` to eliminate shared singleton 600k PBKDF2 lock serialization.

### 3. Benchmark Sampling (`CRC64HardwareTests.swift`, `ExhaustiveCompressionCombinationsTests.swift`, `LZ4DeepIntegrationAndVFSTests.swift`)
- Add `TTZIP_RUN_BENCHMARKS` environment checks to scale down iterations in unit test mode (5~10 rounds) while preserving 100~2000 rounds when benchmark mode is requested.
- Use boundary sampling matrix for exhaustive benchmark permutations.

### 4. Verification
- Run individual test suites to verify sub-second runtimes.
- Run full `swift test` and measure total runtime under 20.0s.
- Run `swift test --filter XCTestPerformanceMeasureTests` to assert zero performance regressions.
