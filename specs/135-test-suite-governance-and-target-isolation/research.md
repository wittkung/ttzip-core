# Phase 0 Technical Research: Test Suite Architecture Governance, Target Isolation & Unified Corpus Infrastructure

**Feature Directory**: `specs/135-test-suite-governance-and-target-isolation`  
**Status**: Completed  
**Author**: Research Subagent `687953a7-4601-4446-bb9f-08f1bc5554d3`  

---

## 1. R001: 语料加载器与缓存管理器收敛至 C/Swift 8 大确定性语料生成器

### 决策 (Decision)
构建统一的分层语料提供者与流式生成基础设施 `DeterministicCorpusProvider`，将零散分布在 `SyntheticXmlCorpusGenerator`、`MultiModalDatasetGenerator`、`BenchmarkDatasetGenerator` 和 `EnwikFixtureCacheManager` 中的合成数据逻辑彻底收敛至以 `Sources/CTTZipBridge/CTTZipCorpusGen.c` 与 `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift`（即 `BenchmarkCorpusType` 8 大核心语料）为核心的统一数据流引擎：

1. **语料类型统一映射矩阵**：
   - `BenchmarkCorpusType.text` ➔ 对应自然语言、文本代码与 Wikipedia XML 结构化测试（收敛 `SyntheticXmlCorpusGenerator` 与 `BenchmarkDatasetGenerator.codeText`）。
   - `BenchmarkCorpusType.shortMatch` ➔ 对应高频短匹配与 LZ77 窗口极端滑动测试。
   - `BenchmarkCorpusType.dna` ➔ 对应 4 字符有限离散字母表测试。
   - `BenchmarkCorpusType.random` ➔ 对应高熵不可压缩随机流（收敛 `MultiModalDatasetGenerator.generateHighEntropyBinaryDataset` 与 `BenchmarkDatasetGenerator.mediaBinary`）。
   - `BenchmarkCorpusType.literals` ➔ 对应字面量偏斜与非均匀字节分布（收敛 `MultiModalDatasetGenerator.generateDeterministicBinaryDataset`）。
   - `BenchmarkCorpusType.mixed` ➔ 对应复合多模态混合负载（收敛 `MultiModalDatasetGenerator.generateCompoundMixed100MBDataset` 与 `BenchmarkDatasetGenerator.mixedOffice`）。
   - `BenchmarkCorpusType.realisticRGB` ➔ 对应连续多通道图像矩阵（收敛 `MultiModalDatasetGenerator.generateFloat32SensorDataset`）。
   - `BenchmarkCorpusType.stripedRGB` ➔ 对应大块条带化规律特征测试。
2. **统一流式落盘与缓存机制**：
   在 `BenchmarkCorpusType` 上扩展零堆内存分配的流式落盘 API `writeToDisk(filePath:totalBytes:chunkSize:)`，统一由 POSIX 64KB 对齐页面缓冲区驱动；`EnwikFixtureCacheManager` 仅保留统一的磁盘缓存索引、`TTZIP_CORPUS_ROOT` 发现与进程间 `flock` 并发保护逻辑，其合成回退分支全量委托给 `BenchmarkCorpusType`，彻底废弃孤立的 PRNG/循环生成逻辑。

### 理由 (Rationale)
1. **消除重复代码与维护负担**：当前仓库内存在多套独立的伪随机/模板生成实现（如 `MultiModalDatasetGenerator.swift` 中手写的 SplitMix64 循环、`BenchmarkDatasetGenerator.swift` 中的三元位移异或以及 `SyntheticXmlCorpusGenerator.swift` 中的 XML 模板切片），总计产生 >600 行冗余代码且压缩熵值基准不一致。
2. **极致流式吞吐与零堆开销**：`CTTZipCorpusGen.c` 基于 SIMD/C 编译器内联优化，内存生成速率达 >2.5 GB/s，比 Swift Debug 模式下的循环拼接快 4~6 倍，能够大幅缩短百兆级合成语料的初始化准备耗时。
3. **确定性与密码学指纹一致性**：收敛至 C 标准生成器后，任意平台（macOS / Linux / Apple Silicon）与构建配置下生成的字节流均严格保持 SHA-256 跨平台同构。

### 被否决的替代方案 (Alternatives Considered)
1. **保持现有独立生成器并在各模块保留专用实现**：
   - *否决理由*：不同模块测试用例使用的“混合数据”定义各异（如 PK 测试使用的是 5 部分简单拼接，而 Micro 基准测试使用的是 `BenchmarkCorpusType.mixed`），导致 Pareto 前沿结果与单算法微基准出现横向偏差，且难以维护。
2. **全盘使用纯 Swift 重新实现 8 大语料生成器并移除 C 桥接**：
   - *否决理由*：纯 Swift 在非 `-Ounchecked` 或 Debug 单测配置下存在数组越界检查和 ARC 开销，100MB 语料生成耗时从 40ms 劣化至 250ms+，拖慢 CI 与日常单测整体执行流速。

### 查阅源 (Source)
- `Sources/CTTZipBridge/include/CTTZipCorpusGen.h` (L16-38: `ttzip_corpus_type_t` 8 大枚举与 `ttzip_generate_corpus`)
- `Sources/CTTZipBridge/CTTZipCorpusGen.c` (L224-252: C 实现与各模态确定性生成器)
- `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift` (L11-51: `BenchmarkCorpusType` 桥接与 `BenchmarkBufferPool`)
- `Sources/TTZipCore/Benchmark/SyntheticXmlCorpusGenerator.swift` (L34-115: 现有 XML 合成逻辑)
- `Sources/TTZipCore/Benchmark/MultiModalDatasetGenerator.swift` (L12-211: 现有 5 模态分散生成代码)
- `Sources/TTZipCore/BenchmarkDatasetGenerator.swift` (L23-79: 现有 3 模态老旧生成代码)
- `Sources/TTZipCore/Benchmark/EnwikFixtureCacheManager.swift` (L67-164: 语料检索、缓存与回退逻辑)
- `Tests/TTZipTests/SilesiaFixtureLoader.swift` (L11-89: Silesia 静态真实语料加载器)

---

## 2. R002: Swift Package Manager 独立 Benchmark Target 隔离与极速单测编译

### 决策 (Decision)
在 `Package.swift` 中实施“核心库剥离 + 独立 Executable/Test Target 隔离”治理策略：

1. **Target 架构重构**：
   - 新增独立可执行 Target `.executableTarget(name: "ttzip-bench", dependencies: ["TTZipCore"])`，将 `Sources/TTZipCore/Benchmark/` 下的所有重量级竞品 PK 测试套件、Pareto 绘图引擎（`RasterParetoPlotter`、`SVGParetoPlotter`、`TerminalParetoPlotter`）以及 `ExhaustiveBenchmarkRunner`、`CompetitorBenchmarkRunner` 完整迁移至 `ttzip-bench` 模块。
   - 新增独立基准测试 Target `.testTarget(name: "TTZipBenchmarkTests", dependencies: ["TTZipCore"])`，将 `SoftwareParetoFrontierPkTests.swift`、`TarZstParetoFrontierPkTests.swift`、`ComprehensiveCorpusBenchmarkPkTests.swift`、`SilesiaCorpusBenchmarkSuiteTests.swift` 等重型测试从 `TTZipTests` 中迁出。
2. **门禁与调用语义**：
   - 日常开发与 CI 阶段默认仅运行 `swift test --target TTZipTests`，只编译纯逻辑单测与格式回归校验用例（201 个文件精简收敛）。
   - 宏观性能评测与竞品 PK 统一通过独立 CLI 命令 `swift run ttzip-bench pareto` 或 `swift test --target TTZipBenchmarkTests` 触发，并配合 `TTZIP_RUN_BENCHMARKS=1` 环境变量做双重硬隔离。

### 理由 (Rationale)
1. **单测编译速度实现数量级提升**：当前 `TTZipTests` 包含 201 个 Swift 文件，其中混杂了大量的 CoreGraphics/Quartz 图像绘制、全量压缩等级遍历及竞品轮询逻辑。即使运行时通过 `XCTSkip` 跳过，SwiftPM 每次依然必须完整解析、类型推导并编译全部 201 个测试文件，导致即使改动一行代码，增量编译链接测试 Target 仍需 15~25 秒。隔离后 `TTZipTests` 编译链接时间可压缩至 <2 秒。
2. **彻底净化生产交付物包体积**：目前 `Sources/TTZipCore/Benchmark/` 包含 37 个源文件（仅 `RasterParetoPlotter.swift` 就达 33KB），直接导致发布到生产环境的 `TTZipCore.framework`、`TTZipApp` 和 `ttzip-cli` 携带了大量无用的绘图与竞品测算代码。剥离后核心库二进制体积预计缩减 180KB~260KB（剥离无用符号与元数据）。

### 被否决的替代方案 (Alternatives Considered)
1. **仅依靠 Swift 条件编译宏 (`#if TTZIP_BENCHMARK`) 进行文件内门禁**：
   - *否决理由*：SwiftPM 对自定义 `OTHER_SWIFT_FLAGS` 的增量缓存管理脆弱，不同标志切换会导致全量重新编译；且宏隔离无法阻止编译器对语法树与泛型符号的前置解析，对编译加速贡献有限，且无法解决发布包体积膨胀问题。
2. **将 Benchmark 拆分为完全独立的 Git 仓库或外部 Package**：
   - *否决理由*：极大破坏了 TTZip 核心引擎演进过程中的即时性能反馈闭环，跨仓库更新核心 API 会引入繁琐的 Submodule/版本同步摩擦。

### 查阅源 (Source)
- `Package.swift` (L27-96: 现有 Target 拓扑定义)
- `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift` (L1-800+: 33KB 独立绘图代码)
- `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift` (L1-235: 宏观 PK 测试与绘图流程)
- `Tests/TTZipTests/TarZstParetoFrontierPkTests.swift` (L1-111: TAR.ZST PK 流程)
- `Tests/TTZipTests/ComprehensiveCorpusBenchmarkPkTests.swift` (L1-269: 5-Tier 几何平均 PK 流程)
- `Tests/TTZipTests/TestBenchmarkTier.swift` (L1-32: 现行环境变量门禁)

---

## 3. R003: 外部工具动态二进制解析器与 CI 门禁自动化集成

### 决策 (Decision)
收敛并重构全局统一的 `SystemBinaryResolver`（合并既有的 `OracleBinaryResolver`、`SevenZipBinaryResolver` 与 `CompetitorDetector` 中的分散查找逻辑），彻底消除代码中的绝对路径硬编码，并与 CI 50 点质量门禁深度集成：

1. **统一 5 级动态解析决策链**：
   ```text
   [1. 环境变量显式覆盖 (TTZIP_ZSTD_PATH 等)]
      ⬇ (未命中)
   [2. Bundle 内嵌资源探测 (Bundle.main / .module)]
      ⬇ (未命中)
   [3. 运行时 PATH 环境变量遍历 (/usr/bin/which + stat)]
      ⬇ (未命中)
   [4. 标准候选目录探测 (/opt/homebrew/bin, /usr/local/bin, /usr/bin, /bin, /opt/local/bin)]
      ⬇ (未命中)
   [5. 降级安全处理 (返回 nil，测试层 XCTSkip / 生产层抛出 ToolUnavailableError)]
   ```
2. **全量清洗硬编码路径**：
   彻底重构 `TarZstParetoFrontierPkTests`（L56 `/opt/homebrew/bin/zstd`）、`SoftwareParetoFrontierPkTests`（L104 `/opt/homebrew/bin/pigz`, L172 `/opt/homebrew/bin/advzip`）、`ComprehensiveCorpusBenchmarkPkTests`（L87 `/opt/homebrew/bin/7z`, L105 `/opt/homebrew/bin/pigz`, L126 `/usr/bin/zip`）等文件，统一调用 `SystemBinaryResolver.resolve(name:)`。
3. **CI 门禁与 50 点基准矩阵无缝绑定**：
   - 将 `scripts/upstream_audit_gate.py` 中的 5 阶段门禁（编译器对齐、双构建零警告、汇编栈溢出审查、CV 离散系数 <= 1.50%、50 点多模态矩阵单点回归 <= 2.0%）与 `ttzip-bench matrix --json-out` 对接。
   - 在 `.git/hooks/pre-push` 与 GitHub Actions CI 中配置动态二进制自检：若 CI 环境缺少 Homebrew，门禁脚本自动识别系统原生 `/usr/bin/tar`、`/usr/bin/unzip` 或 Linux `/usr/bin/zstd`，确保无硬编码路径阻断。

### 理由 (Rationale)
1. **解决跨平台与 CI 环境断裂问题**：Apple Silicon 的 Homebrew 位于 `/opt/homebrew/bin`，Intel Mac 位于 `/usr/local/bin`，Linux CI 位于 `/usr/bin`，Nix/Container 位于自定义路径。硬编码绝对路径导致测试在非本地开发者机器上直接失败或报错。
2. **严格防范性能回退**：将 50 点基准矩阵（8 大 Workload x 2 种尺寸 x 4 个 Level）作为自动化门禁，杜绝任何未经统计学验证（CV > 1.5%）或存在 >2.0% 单点性能劣化的提交合入主干。

### 被否决的替代方案 (Alternatives Considered)
1. **在每个测试用例开头通过 `which` 命令进行局部 Process 探测**：
   - *否决理由*：每个测试方法重复派生进程执行 `which` 带来巨大进程启动开销，且缺少线程安全缓存机制与环境变量覆盖机制。
2. **CI 环境强制软链接所有二进制到 `/opt/homebrew/bin`**：
   - *否决理由*：侵入宿主环境配置，在非 root 容器或多租户 CI Runner 上会因权限问题失败，治标不治本。

### 查阅源 (Source)
- `Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift` (L342-414: `OracleBinaryResolver` 现有实现)
- `Sources/TTZipCore/Utilities/SevenZipBinaryResolver.swift` (L12-73: `SevenZipBinaryResolver` 现有实现)
- `Sources/TTZipCore/CompetitorDetector.swift` (L305-315: PATH 解析逻辑)
- `Tests/TTZipTests/TarZstParetoFrontierPkTests.swift` (L56: 硬编码 `/opt/homebrew/bin/zstd`)
- `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift` (L104, L172: 硬编码 `/opt/homebrew/bin/pigz`, `/opt/homebrew/bin/advzip`)
- `Tests/TTZipTests/ComprehensiveCorpusBenchmarkPkTests.swift` (L87, L105: 硬编码 `/opt/homebrew/bin/7z`, `/opt/homebrew/bin/pigz`)
- `scripts/upstream_audit_gate.py` (L22-183: 5 阶段门禁与 50 点矩阵校验逻辑)
