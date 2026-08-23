# Implementation Plan: Multi-Tier Format Selection & Benchmark Architecture

## Technical Context

为了全面、科学、立体地反映数据压缩软件在不同业务场景下的真实性能，TTZip 建立 4 阶代表性评测格式矩阵（4-Tier Representative Format Benchmark Matrix）：
- Tier 1: `ZIP` (Deflate, 32KB window) —— 通用生态与日常交换基准 (L1D Cache 局部性与通用兼容性)
- Tier 2: `7Z` (LZMA2, 64MB~1GB) —— 极限空间与冷备归档基准 (DRAM 随机寻址延迟与大字典匹配查找)
- Tier 3: `TAR.ZST` (Zstandard, FSE) —— 现代工业级平衡与网络流式基准 (8-wide OoO 流水线与万兆云传输)
- Tier 4: `LZ4` / `TAR.LZ4` (LZ4) —— 内存级极限与超高 IOPS 传输基准 (统一内存 UMA 总线带宽极限)

并在 `TTZipCore/Benchmark/` 体系中实现几何平均综合效能评分（Weighted Geometric Mean Index）与 DeepSWE 帕累托轨迹联动。

## Constitution Check

- **[P0] 热路径零分配与隔离**：评测矩阵分发器与打分引擎仅运行在基准测试调度层与诊断输出层，不侵入编解码内核热路径。
- **[P1] 进程内原生与零外部依赖**：打分模型与矩阵生成基于纯 Swift/C 数学运算，零外部 Python/R 依赖。
- **[P2] 跨架构比率尺度不变性**：基于 Fleming & Wallace (1986) 几何平均模型，消除基准机器选择偏见。
- **[P3] 性能门禁兼容**：全套 4-Tier 矩阵基准执行耗时控制在 15 秒以内。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《压缩基准测试学术界与工业界格式评测矩阵标准调研 (Silesia/Hutter/lzbench/TurboBench)》：确立 4-Tier 矩阵在压缩比、压缩吞吐、解压吞吐和生态兼容性四个维度的正交完备性。
- R002 [SUBAGENT:research] 《多格式综合加权指数 (Geometric Mean Index) 与异构量纲归一化算法研究》：确立加权几何平均 Base-1000 评分公式与帕累托效率指数 (PEI) 计算模型。
- R003 [SUBAGENT:research] 《4-Tier 格式在 Apple Silicon M 系列芯片上的硬件瓶颈与加速特性分析》：确立 4-Tier 格式在 L1D Cache、L2/SLC Cache、8-wide OoO 流水线与 UMA 内存总线上的硬件瓶颈映射。

---

## Phase 1: Design Artifacts & Contracts

- `research.md`：记录 R001 ~ R003 的四大要素（Decision, Rationale, Alternatives Considered, Source）。
- `data-model.md`：定义 `BenchmarkFormatTier`、`FormatMatrixPreset`、`CompositeScoreReport`、`ParetoEfficiencyReport` 模型。
- `contracts/multi-tier-benchmark-matrix-contract.json`：符合 JSON Schema Draft-07 的零通配契约。
- `quickstart.md`：包含可执行命令、预期输出样本与失败诊断排查指南。

---

## Planned Changes by Component

### TTZipCore / Benchmark Component
- [NEW] `Sources/TTZipCore/Benchmark/FormatMatrixTaxonomy.swift`: 定义 4 阶格式矩阵模型、预设枚举与几何平均评分计算器。
- [MODIFY] `Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift`: 适配 4 阶格式标识与综合评分数据字段。
- [MODIFY] `Sources/TTZipCore/CLI/CLIOptions.swift`: 新增 `--format-matrix` 选项解析支持。
- [MODIFY] `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`: 扩充 4-Tier 全景多软件 PK 评测矩阵。
