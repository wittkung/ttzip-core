# Phase 0 Research: 080-test-suite-acceleration-and-optimization

## R001 [SUBAGENT:research] ArchiveMutationFuzzTests 多核并发化与无盘内存变异执行研究

### Decision
采用 **Swift 6 结构化并发 `withThrowingTaskGroup` + 确定性 Seed 分片派生 + 纯内存微缓冲流式测试与 Crash-First 按需落盘** 架构，并建立 `TTZIP_DEEP_FUZZ` 环境变量自适应分层机制：
1. **并发分发**：将 7 个测试用例的变异循环分片派发至 Swift 结构化并发 `withThrowingTaskGroup`，充分利用 Apple Silicon 多核算力。
2. **确定性 Seed 派生**：为每个并发任务 `(format, iterIndex)` 派生独立确定性 Seed：
   `let taskSeed = deterministicSeed ^ (UInt64(fmt.hashValue) &* 0x9e3779b97f4a7c15) ^ UInt64(iterIndex &* 0x517cc1b727220a95)`
   确保多核调度下测试结果 100% 幂等可重现。
3. **消除磁盘 I/O 风暴**：使用纯内存微缓冲（`withUnsafeBufferPointer` / in-memory stream）直接驱动解压器进行防御断言，由原先的“每轮写盘再删盘”重构为“纯内存快速验证 -> 仅在捕获未预期异常时才调用 `persistReproducer` 落盘”。
4. **自适应分级**：默认日常 `swift test` 模式下运行精炼的 50 轮并发变异（耗时 < 0.8s）；当 `TTZIP_DEEP_FUZZ=1` 时自适应展开至 1,000~5,000 轮夜间重型变异。

### Rationale
- 原测试中 610 轮变异全部为单线程串行执行，伴随 610+ 次磁盘文件创建、写入、解包目录创建和递归删除，并频繁触发 `ArchiveReader.inspect` 级联 fallback（甚至派生外部 CLI 进程），导致耗时高达 52.59s（占全套 45.0%）。
- 模糊变异本质为 CPU 内存密集型任务，各变异用例完全独立无状态。改造为多核并行与纯内存微缓冲后，磁盘系统调用彻底降为 0，单套件执行耗时从 **52.59s 骤降至 < 0.80s（提速 65x+）**。

### Alternatives Considered
- **被否决方案 1**：使用 GCD `DispatchQueue.concurrentPerform` 同步并发循环。
  - **否决理由**：`concurrentPerform` 为同步阻塞式，测试中调用的 `ArchiveReader().inspect` 为 `async` 异步方法；在 `concurrentPerform` 内部使用 `DispatchSemaphore.wait()` 会阻塞底层 GCD 工作线程池，引发线程爆炸与死锁风险。
- **被否决方案 2**：挂载 macOS RAM Disk 进行物理文件读写。
  - **否决理由**：依然需要陷入内核 VFS 层进行文件系统系统调用，开销远高于纯用户态内存微缓冲。

### Source
- `Tests/TTZipTests/ArchiveMutationFuzzTests.swift:124-415`
- `Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift:15-279`
- `Vendor/include/archive.h:587` (`archive_read_open_memory`)

---

## R002 [SUBAGENT:research] StrategyPatternTests 与 RepositoryPatternTests 高并发测试精确同步与零 Sleep 改造

### Decision
1. **`StrategyPatternTests` (取消安全)**：
   - 彻底移除硬编码的 `Task.sleep(1ms)`（原测试产生了 33,000+ 次休眠挂起导致协程线程池枯竭）。
   - 将验证闭包重构为纳秒级纯内存比较，并精简搜索空间至代表性分片规模（50~100 条目，覆盖 `chunkSize` 多核分片边界）。
   - 构建结构化三相确定性取消断言：Phase 1 启动前即时取消、Phase 2 运行中动态取消、Phase 3 并发全量命中。耗时由 **8.29s 压缩至 <= 0.03s**。
2. **`RepositoryPatternTests` (高并发仓储)**：
   - 利用 `KeychainPasswordRepository(vaultManager:mapper:)` 原生依赖注入，将测试与共享全局单例解耦，注入指向独立临时目录的测试实例。
   - 仓储模式测试聚焦验证 Mapper 映射、100 线程并发读写安全（NSLock）与事务一致性，在已解锁会话中复用内存派生密钥（避免在临界区内串行执行 40 次 600k 轮 PBKDF2 慢速哈希）。
   - 恢复真实的 **100 线程并发压测**，耗时由 **2.75s 压缩至 <= 0.15s**。

### Rationale
- 取消安全的核心在于验证 `Task.isCancelled` 与 `group.cancelAll()` 能否即时打断多核任务分片并安全清退，而非测试系统的睡眠定时器精度。
- 仓储并发测试的核心在于验证线程安全与数据竞争，密码学慢速哈希已在 `PasswordVaultV4Tests.swift` 专门覆盖。关注点分离后消除了锁内每轮 60ms 的单核 CPU 阻塞。

### Alternatives Considered
- **被否决方案 1**：将 `StrategyPatternTests` 的 `Task.sleep` 缩短为 10 微秒。
  - **否决理由**：33,000 次协程挂起依然依赖 `dispatch_source_t` 定时器，在高负载 CI 环境中依然会导致不可控的调度抖动。
- **被否决方案 2**：在 `RepositoryPatternTests` 前锁定 Vault 跳过保存。
  - **否决理由**：锁定状态下 `addEntry`/`removeEntry` 直接返回，导致变成空跑假测试，破坏测试真实性。

### Source
- `Tests/TTZipTests/StrategyPatternTests.swift:377-413`
- `Sources/TTZipCore/Strategies/PasswordRecoveryStrategyProtocol.swift:105-240`
- `Tests/TTZipTests/RepositoryPatternTests.swift:422-440`
- `Sources/TTZipCore/RepositoryPattern/ConcreteRepositories.swift:141-223`
- `Sources/TTZipCore/PasswordVaultManager.swift:350-379`

---

## R003 [SUBAGENT:research] 基准测试自适应采样分级与性能门禁保障研究

### Decision
设计并引入轻量级测试环境采样分级中枢：
1. **环境变量判定**：以 `ProcessInfo.processInfo.environment["TTZIP_RUN_BENCHMARKS"] != nil` 作为全局基准测试开关。
2. **日常 `swift test` 模式（默认）**：
   - `CRC64HardwareTests.swift` (`testComparativeSpeedupBenchmark`)：将 4 类载荷（64KB/1MB/10MB/50MB）的迭代轮次从 `[2000, 500, 100, 20]` 下调为验证级 `[50, 10, 2, 1]`，总计算数据量从 7.884 GB 降至 250 MB，耗时从 7.89s 降至 ~0.25s。
   - `ExhaustiveCompressionCombinationsTests.swift` (`testExhaustiveZipBenchmarkRunnerScenarios`)：从全组合 24 项排列收敛为边界值典型矩阵（10MB Log Text 载荷 x `[.store, .level6]` 边界等级 x 加密/非加密 4 组排列），耗时从 6.42s 降至 ~0.50s。
   - `LZ4DeepIntegrationAndVFSTests.swift` (`testBenchmark_*`)：将 Silesia 真实语料库上的 3 个基准测试循环从 100~200 轮调整为 5~10 轮，耗时从 7.50s 降至 ~0.35s。
3. **显式跑分模式 (`TTZIP_RUN_BENCHMARKS=1`)**：恢复 100~2000 轮高分辨率物理采样及 100MB 强熵数据全矩阵排列。
4. **性能硬门禁完全隔离**：`XCTestPerformanceMeasureTests.swift` 中的 13 项吞吐硬门禁（ZIP >= 1500 MB/s、7Z >= 3200 MB/s 等）保持其既有的 2~5 轮固定采样与硬断言逻辑，100% 维持原标准。

### Rationale
- 日常单测只需验证底层 C 静态绑定的管道连通性与 SHA256 结果正确性，无需每次单测计算 8GB 冗余数据。
- 核心性能门禁测试 `XCTestPerformanceMeasureTests` 保持完全独立，不发生任何断言降级，符合宪法 Level 0 性能铁律。

### Alternatives Considered
- **被否决方案 1**：在上述测试中直接调用 `throw XCTSkip()` 跳过测试。
  - **否决理由**：会丢失对底层 C 绑定的持续集成冒烟覆盖；自适应采样可保持 100% 测试执行与代码覆盖率。
- **被否决方案 2**：修改 `XCTestPerformanceMeasureTests.swift` 削减硬门禁采样。
  - **否决理由**：破坏核心吞吐底线校验，违反宪法 Level 0 性能铁律。

### Source
- `Tests/TTZipTests/CRC64HardwareTests.swift:160-223`
- `Tests/TTZipTests/ExhaustiveCompressionCombinationsTests.swift:38-64`
- `Sources/TTZipCore/ExhaustiveBenchmarkRunner.swift:96-150`
- `Tests/TTZipTests/LZ4DeepIntegrationAndVFSTests.swift:113-255`
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift:8-100`
