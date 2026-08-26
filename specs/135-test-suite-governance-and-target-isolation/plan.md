# Implementation Plan: Test Suite Architecture Governance, Target Isolation & Unified Corpus Infrastructure

**Feature Directory**: `specs/135-test-suite-governance-and-target-isolation`  
**Target Subject**: 语料基础设施统一收敛、SPM 独立 Target 治理、外部工具动态解析与 CI 性能门禁  
**Status**: Approved  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **现有问题**：
  1. `SilesiaFixtureLoader.swift`、`EnwikFixtureCacheManager.swift`、`SyntheticXmlCorpusGenerator.swift`、`MultiModalDatasetGenerator.swift` 存在分散的生成代码，未完全复用 Feature 134 建立的高性能 `BenchmarkCorpusGenerator`。
  2. `TarZstParetoFrontierPkTests.swift`、`SoftwareParetoFrontierPkTests.swift` 存在 `/opt/homebrew/bin/` 绝对路径硬编码，跨平台与干净 CI 容器兼容性差。
  3. `Package.swift` 中宏观 PK 测试与绘图引擎混杂在主测试 Target 中，增加了日常测试构建时间。
- **治理目标**：
  1. 统一语料生成与落盘流：全量对接 `BenchmarkCorpusType` 8 大负载。
  2. 统一外部二进制动态查找器 `SystemBinaryResolver`，消除 100% 绝对路径硬编码。
  3. 优化 `Package.swift` 与测试套件分类，确保 `swift test` $\le 2.5$ 秒完成。

### 1.2 Constitution Check
- ✅ **Upstream Invariant 1 (Zero-Overhead Hot Path)**：语料生成全部通过 C / 64B 对齐池原地进行。
- ✅ **Upstream Invariant 2 (Deterministic Grounded Benchmarks)**：50 点矩阵 $CV \le 1.50\%$，单点回归 $\le 2.0\%$。
- ✅ **Upstream Invariant 5 (100% Zero-Warning & Clean Hook Policy)**：0 警告，真实通过所有 CI/Git Hook。

---

## 2. Phase 0: Technical Research Index

- R001 [SUBAGENT:research] 《语料加载器与缓存管理器收敛至 C/Swift 8 大确定性语料生成器》：统一分层语料提供者与流式生成基础设施 `DeterministicCorpusProvider`。
- R002 [SUBAGENT:research] 《Swift Package Manager 独立 Benchmark Target 隔离与极速单测编译》：Target 架构隔离与宏观 PK 门禁策略。
- R003 [SUBAGENT:research] 《外部工具动态二进制解析器与 CI 门禁自动化集成》：统一 5 级动态解析决策链，全量清洗硬编码路径。

---

## 3. Phase 1: Design Artifacts & Contracts

- `data-model.md`: `BenchmarkCorpusDescriptor`, `SystemBinaryDescriptor`, `BenchmarkMatrixExecutionReport` 实体定义。
- `contracts/benchmark_corpus_manifest.json`: 8 大确定性语料契约。
- `contracts/system_binary_resolver_config.json`: 外部二进制解析配置契约。
- `contracts/benchmark_matrix_report.json`: 50 点矩阵与 CI 门禁报表契约。
- `quickstart.md`: 2 大核心场景验收手册。

---

## 4. Component Changes & Architecture

### 4.1 Component 1: Unified Corpus Provider
- [MODIFY] `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift`: 增加文件流式写入扩展 `writeToDisk(filePath:totalBytes:)`。
- [MODIFY] `Sources/TTZipCore/Benchmark/EnwikFixtureCacheManager.swift`: 收敛合成回退逻辑至 `BenchmarkCorpusType.text`。
- [MODIFY] `Tests/TTZipTests/SilesiaFixtureLoader.swift`: 收敛静态加载与合成回退。

### 4.2 Component 2: System Binary Resolver
- [NEW] `Sources/TTZipCore/Platform/SystemBinaryResolver.swift`: 统一 5 级动态二进制解析器。
- [MODIFY] `Tests/TTZipTests/TarZstParetoFrontierPkTests.swift`: 替换硬编码路径为 `SystemBinaryResolver.resolve("zstd")`。
- [MODIFY] `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`: 替换硬编码路径为 `SystemBinaryResolver.resolve(...)`。
- [MODIFY] `Tests/TTZipTests/ComprehensiveCorpusBenchmarkPkTests.swift`: 替换硬编码路径为 `SystemBinaryResolver.resolve(...)`。

### 4.3 Component 3: Build & CI Regression Integration
- [MODIFY] `scripts/upstream_audit_gate.py`: 适配最新的 50 点矩阵三维输出。
- [MODIFY] `Tests/TTZipTests/TestBenchmarkTier.swift`: 完善门禁级别。
