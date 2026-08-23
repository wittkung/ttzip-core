# Implementation Plan: Comprehensive Corpus Orchestration & Geometric Mean Benchmark Matrix

## Technical Context

将现有的单语料测试重构为 5-Tier 科学多模态真实语料库编排体系：
1. **Tier 1: Large Text & Web (25%)**：`enwik8` (100MB) + `webster` (41.5MB) + `dickens` (10.2MB) + `reymont` (6.6MB)
2. **Tier 2: Binary Executable (20%)**：`mozilla` (51.2MB) + `ooffice` (6.2MB)
3. **Tier 3: Structured Data & DB (20%)**：`nci` (33.6MB) + `osdb` (10.1MB) + `xml` (5.3MB)
4. **Tier 4: Mixed SourceTree & VFS (20%)**：`samba` (21.6MB) + `MicroCorpus` (500 文件目录树)
5. **Tier 5: Scientific & Dense Matrix (15%)**：`mr` (10.0MB) + `x-ray` (8.5MB) + `sao` (7.3MB)

## Constitution Check

- [P0] 零中间堆分配：所有语料采用 POSIX `mmap` 与 `MAP_SHARED` 预热共享内存。
- [P1] 严格加权几何平均：杜绝算术平均引发的排序反转。
- [P2] 零退化：全量回归测试保持全绿灯。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《压缩基准测试工业界语料库科学编排方案》：完成 5-Tier 划分与生命周期设计。
- R002 [SUBAGENT:research] 《多语料综合效能指数与加权几何平均数计算体系研究》：完成数学公理证明与 CEI/SPECScore 模型推导。

---

## Phase 1: Design Artifacts & Contracts

- `research.md`
- `data-model.md`
- `contracts/composite-benchmark-contract.json`
- `quickstart.md`
- `tasks.md`

---

## Planned Changes by Component

- [NEW] `Sources/TTZipCore/Benchmark/BenchmarkTierCategory.swift`: 5 大 Tier 分类枚举与元数据。
- [NEW] `Sources/TTZipCore/Benchmark/CompositeEfficiencyModels.swift`: 综合效能与加权几何平均数据模型。
- [NEW] `Sources/TTZipCore/Benchmark/CorpusOrchestrator.swift`: 5-Tier 语料自适应发现与零拷贝只读映射调度器。
- [NEW] `Sources/TTZipCore/Benchmark/CompositeEfficiencyCalculator.swift`: 加权几何平均数与 CEI / SPECScore 计算器。
- [NEW] `Tests/TTZipTests/CorpusOrchestratorTests.swift`: 5-Tier 语料加载与几何平均单元测试。
- [NEW] `Tests/TTZipTests/ComprehensiveCorpusBenchmarkPkTests.swift`: 5-Tier 综合帕累托评测套件与图表生成。
