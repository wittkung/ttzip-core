# Implementation Plan: ZIP 7-Tier Graph-Theoretic & Advzip Conquest Hierarchy

**Feature Directory**: `specs/099-zip-graph-and-advzip-hierarchy`  
**Created**: 2026-08-18  
**Status**: Ready for Implementation  

---

## 1. Technical Context

TTZip 当前在 ZIP 格式下实现了基于 18 核心分块与 NEON 加速的极速压缩（5.4 GB/s）和超级压缩（3.0 MB/s @ 97.01%）。然而，当前产品体系存在以下两大架构缺口：
1. **中高速与高压缩比断层**：Level 4 (1.8 GB/s @ 96.65%) 与 Level 6 (3.0 MB/s @ 97.01%) 之间缺少一个吞吐在 150~400 MB/s、压缩比达 ~96.85% 的图论优化档位；
2. **极限压缩率未达全球峰值**：在极限归档场景下，现有最高档位尚未超越 `advzip -4`（2,994,957 bytes @ 97.005%）。

### Target Architecture: 7 大黄金梯队 (7-Tier Golden Hierarchy)

```
Level 1 (极速):  libdeflate L2 (5.4 GB/s @ 95.50%)   ── 4.83 MB
Level 2 (快速):  libdeflate L4 (3.8 GB/s @ 96.30%)   ── 4.45 MB
Level 3 (标准):  libdeflate L5 (3.2 GB/s @ 96.55%)   ── 3.45 MB
Level 4 (深度):  libdeflate L10 (1.8 GB/s @ 96.65%)  ── 3.35 MB
Level 5 (图论):  18-Core Bounded DAG (200 MB/s @ 96.85%) ── 3.15 MB 🌟 [NEW]
Level 6 (超级):  18-Core Zopfli DAG (3.0 MB/s @ 97.01%)  ── 2.99 MB
Level 7 (极限):  15-Pass Advzip Buster (0.5 MB/s @ 97.05%) ── 2.95 MB 🌟 [NEW - Beats advzip-4]
```

---

## 2. Constitution Check

- **Hot Paths Zero Allocation**: Level 5 与 Level 7 核心 DAG 匹配解析器完全在 C 语言栈空间和定点数组中运算，零堆分配。
- **In-Process C Static Binding**: 100% 进程内 C 静态库与 Apple Silicon SIMD 绑定，零外部 CLI 调用。
- **Zero Regression**: 现有 1~4 档与全格式 16 种格式的性能门禁必须 100% 保持绿灯。

---

## 3. Phase 0: Research Tasks
- R001 [SUBAGENT:research] 《Level 5 高速有限前瞻 DAG 最短路径图论解析器》：已完成，见 [research.md](./research.md)。
- R002 [SUBAGENT:research] 《Level 7 极限多轮重平衡与最优动态块切分器》：已完成，见 [research.md](./research.md)。

---

## 4. Phase 1: Design Artifacts
- Data Model: [data-model.md](./data-model.md)
- Interface Contracts: [contracts/zip-tier-schema.json](./contracts/zip-tier-schema.json)
- Quickstart Guide: [quickstart.md](./quickstart.md)

---

## 5. Component Modification List

### Component 1: `Sources/TTZipCore/ArchiveCompressionTypes.swift`
- 更新 `ArchiveCompressionLevel`，支持 7 档位枚举与 `effectiveZipRawLevel` 精确映射：
  - Level 1 -> rawLevel 2 (5.4 GB/s)
  - Level 2 -> rawLevel 4 (3.8 GB/s)
  - Level 3 -> rawLevel 5 (3.2 GB/s)
  - Level 4 -> rawLevel 10 (1.8 GB/s)
  - Level 5 -> rawLevel 11 (200 MB/s @ 96.85% 图论近优)
  - Level 6 -> rawLevel 12 (3.0 MB/s @ 97.01% 超级压缩)
  - Level 7 -> rawLevel 15 (0.5 MB/s @ 97.05% 极限超越 advzip-4)
- 更新 `ArchiveCompressionFormat.zip.supportedLevels` 为 `[.store, .level1, .level2, .level3, .level4, .level5, .level6, .level7]`。

### Component 2: `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`
- 接入 Level 5（有限前瞻图论解析器）与 Level 7（15-Pass 多轮动态块切分优化器）。
- 移除所有外部 `pigz` CLI 调用，全量采用进程内 C 桥接。

### Component 3: `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`
- 横坐标全面改造为**物理压缩后大小（Compressed File Size in MB, 越小越好 / 靠右为更优）**，彻底消除百分比体感钝化；
- 自适应双波段 Y 轴与 X 轴折叠引擎全面支持 7 档位。

### Component 4: `Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift`
- 更新 ZIP 格式的 7 档位 Tile 与标签描述（1: 极速 5.4G, 2: 快速 3.8G, 3: 标准 3.2G, 4: 深度 1.8G, 5: 图论 200M, 6: 超级 97%, 7: 极限 97.05%）。
