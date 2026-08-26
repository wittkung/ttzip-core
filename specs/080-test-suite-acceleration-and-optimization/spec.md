# Feature Specification: 080-test-suite-acceleration-and-optimization

**Feature Branch**: `080-test-suite-acceleration-and-optimization`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "我觉得我们需要好好优化，让测试时间加快，Top 10 最耗时单个测试用例 Top 10 最耗时测试套件都好好分析，如何优化"

## Clarifications

### Session 1 (2026-08-18)
- **Q**: 在加速耗时测试时，是否允许删除测试用例或放宽性能门禁阈值？
  - **A**: **绝对禁止**。所有 883+ 个测试用例必须 100% 保持通过，所有性能门禁红线与故障注入防御必须 100% 维持原标准。加速必须通过多核并发化调度（`DispatchQueue.concurrentPerform`）、消除无效 `Thread.sleep`、内存旁路代替磁盘 IO 以及基准测试模式环境变量自适应分级（`TTZIP_RUN_BENCHMARKS`）实现。
- **Q**: 变异模糊测试是否在快速单测时缩减迭代轮次？
  - **A**: 默认单元测试执行代表性 50~100 轮并发变异注入以确保瞬时回归反馈；在指定 `TTZIP_DEEP_FUZZ=1` 时自适应展开至 1,000~5,000 轮夜间全量深度测试。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 变异模糊测试与故障注入多核并发加速与内存旁路优化 (Priority: P1)

开发者和 CI/CD 流水线在运行全量单元测试（`swift test`）时，耗时最长（占全量 45.0%、耗时 52.59 秒）的变异模糊测试套件 `ArchiveMutationFuzzTests` 能够利用多核并发（GCD `DispatchQueue.concurrentPerform` / Swift `withTaskGroup`）和纯内存微缓冲旁路进行并行化变异注入与断言，在保障 100% 故障注入安全性与边界防御的前提下，将套件执行耗时从 52.59s 缩减至 8s 以内。

**Why this priority**: `ArchiveMutationFuzzTests` 占整个测试套件近一半耗时（52.59s / 116.76s），其中单个 `testComprehensiveDeterministicFuzzMatrix` 耗时达 20.79s。优化该套件将带来立竿见影的整体加速收益。

**Independent Test**: 运行 `swift test --filter ArchiveMutationFuzzTests`，断言全部 7 个变异测试用例 100% 成功通过，单套件执行总耗时 <= 8.0 秒（提速 6.5x+）。

**Acceptance Scenarios**:

1. **Given** 包含 `testComprehensiveDeterministicFuzzMatrix`、`testTruncateStreamMutationStability`、`testCorruptCRCMutationStability` 等 7 个模糊测试用例，**When** 执行单套件测试，**Then** 所有测试用例并行调度执行且全部通过，总耗时从 52.59s 降至 <= 8.0s。
2. **Given** 模糊变异过程中的字节翻转与截断注入，**When** 运行测试，**Then** 数据在内存缓冲区中完成变异与解压探针验证，无冗余临时文件磁盘系统调用风暴。

---

### User Story 2 - 高并发压测、硬件基准与长循环用例自适应分级与同步优化 (Priority: P2)

开发者在日常编码与快速单测时，Top 2~5 耗时用例（`StrategyPatternTests` 100+ 任务并发取消、`CRC64HardwareTests` 硬件基准对比、`ExhaustiveCompressionCombinationsTests` 穷举压缩矩阵、`LZ4DeepIntegrationAndVFSTests` 语料库多轮基准）采用确定性事件同步与自适应采样分级策略，消除固定 `Thread.sleep` 空转与过度冗余循环，使单元测试回归既快又稳。

**Why this priority**: `StrategyPatternTests`（8.39s）、`CRC64HardwareTests`（7.92s）、`ExhaustiveCompressionCombinationsTests`（6.46s）、`LZ4DeepIntegrationAndVFSTests`（5.25s）合计耗时 28.02s。优化后可将其压缩至 2.5s 以内。

**Independent Test**: 分别执行这 4 个套件，验证其功能断言与性能断言 100% 达标，且耗时大幅降低。

**Acceptance Scenarios**:

1. **Given** `testRound3MultiCoreBruteForce100PlusTasksGroupCancellationSafety`，**When** 执行并发取消安全测试，**Then** 使用条件变量/信号量精确同步代替长休眠等待，耗时从 8.29s 降至 <= 0.5s。
2. **Given** `testComparativeSpeedupBenchmark` 与 `testBenchmark_TLSStreamPoolVsStandard_OnSilesiaCorpus`，**When** 在默认单元测试模式下运行，**Then** 采样轮次动态收敛为验证级精确轮次（而在 `TTZIP_RUN_BENCHMARKS=1` 下保留全量深度 profiling），耗时从 12.38s 降至 <= 0.8s。
3. **Given** `testExhaustiveZipBenchmarkRunnerScenarios`，**When** 运行穷举矩阵，**Then** 采用关键边界值采样（Level 0, 1, 6, 9 与代表性载荷），耗时从 6.42s 降至 <= 0.8s。

---

### User Story 3 - 全量测试套件 10x 极速回归与零断言降级 (Priority: P3)

工程在执行全量 `swift test` 时，所有 883+ 个测试用例保持 100% 真实执行（0 跳过、0 误报、0 门禁阈值下调），整套回归耗时从 116.76s（~2 分钟）显著压缩至 15~20 秒以内。

**Why this priority**: 极速的测试套件让开发者在本地和 CI 环境中能获得即时反馈，大幅提升迭代效率与工程质量。

**Independent Test**: 执行全量 `swift test`，测量总耗时并断言所有用例通过。

**Acceptance Scenarios**:

1. **Given** 全工程 883+ 测试用例，**When** 执行全量 `swift test`，**Then** 退出码为 0，0 failures，总执行时间 <= 20.0 秒（相比基线 116.76s 提速 6x~8x）。
2. **Given** 性能门禁测试 `swift test --filter XCTestPerformanceMeasureTests`，**When** 执行门禁验证，**Then** 所有吞吐硬门禁（ZIP >= 1500 MB/s、7Z >= 3200 MB/s 等）100% 保持红线达标，零测试质量妥协。

---

### Edge Cases

- **高并发变异中的线程安全与数据竞争**：多核并发执行模糊测试变异时，每个任务的输入输出缓冲区与解压句柄必须 100% 独立隔离，严禁共享全局可变状态。
- **基准测试模式与单测模式的无缝兼容**：当指定环境变量 `TTZIP_RUN_BENCHMARKS=1` 或 `TTZIP_DEEP_FUZZ=1` 时，自动化框架必须无缝恢复执行 1,000+ 轮全量深度压测，确保基准测试报告的统计严谨性。
- **取消信号与资源死锁**：在优化 `StrategyPatternTests` 的并发同步时，必须设置超时保护机制，避免任务组在取消过程中发生死锁或悬挂。

## Functional Requirements *(mandatory)*

- **FR-001**: `ArchiveMutationFuzzTests` 必须支持多核并行变异调度，将 7 个测试用例的变异循环在 GCD / 并发线程池中并行执行，并采用内存数据流替代临时文件读写。
- **FR-002**: `StrategyPatternTests` 必须消除所有大于 50ms 的硬编码 `Thread.sleep` 等待，采用 `DispatchSemaphore` / `AsyncStream` / 条件通知实现微秒级精确同步。
- **FR-003**: `CRC64HardwareTests` 与 `LZ4DeepIntegrationAndVFSTests` 中的基准测试用例必须支持通过 `TTZIP_RUN_BENCHMARKS` 环境变量实现自适应迭代次数配置（单测模式 5~10 轮，完整基准模式 100~200 轮）。
- **FR-004**: `ExhaustiveCompressionCombinationsTests` 必须优化穷举组合策略，覆盖边界压缩级别与代表性载荷，消除完全冗余的线性递增测试。
- **FR-005**: `RepositoryPatternTests` 必须优化 100 线程并发读写同步逻辑，消除无谓的线程睡眠，确保并发安全的同时将执行时间控制在 0.5s 以内。
- **FR-006**: 全量优化后，`swift test` 的全部 883+ 测试用例必须 100% 保持通过，严禁删除测试用例或放宽性能门禁硬指标。

## Success Criteria *(mandatory)*

- **SC-001**: `ArchiveMutationFuzzTests` 套件执行时间从 **52.59s** 降至 **<= 8.0s**（提速 >= 6.5x）。
- **SC-002**: Top 10 最耗时测试用例中单用例最高耗时从 **20.79s** 降至 **<= 3.0s**。
- **SC-003**: 全量 `swift test` 883+ 测试用例总执行时间从 **116.76s** 降至 **<= 20.0s**（整体提速 >= 5.8x）。
- **SC-004**: 全量测试通过率 100%（0 failures, 0 unexpected, 0 disabled/deleted tests）。
- **SC-005**: 核心性能门禁测试 `XCTestPerformanceMeasureTests` 与各格式吞吐底线 100% 维持不变并全绿通过。
