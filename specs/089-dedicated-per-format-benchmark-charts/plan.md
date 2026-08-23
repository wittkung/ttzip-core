# Implementation Plan: Dedicated Per-Format Benchmark Charts & Multi-Software Suite

## Technical Context

升级 `SoftwareParetoFrontierPkTests` 与 `RasterParetoPlotter`/`SVGParetoPlotter`：
1. 扩充 Apple Native 评测范围：覆盖 `/usr/bin/ditto` 与 `/usr/bin/zip`（`-1` 极速、`-6` 默认），形成 Apple 家族的完整多档位轨迹线。
2. 支持单格式独立渲染：按 ZIP、7Z、TAR.ZST、LZ4 分别生成专属的帕累托 PK 图表，并输出 4-Tier 综合全景图。
3. 保持纯白极简 DeepSWE 学术规范，自适应计算各格式专属的 X 轴与 Y 轴视口。

## Constitution Check

- **[P0] 热路径隔离**：所有图表生成与评测仅在测试层与 CLI 诊断层执行，不侵入热路径。
- **[P1] 进程内与原生**：绘图基于 CoreGraphics 与标准 SVG，零 Python 依赖。
- **[P2] 零 Git 污染**：所有输出的 `.png` 与 `.svg` 均在 `.gitignore` 保护范围内。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《macOS 系统自带归档工具链 (ditto, zip, tar, gzip, bzip2) 行为特征与测试接口调研》：调研 Apple 原生二进制的命令行参数与执行特征。

---

## Phase 1: Design Artifacts & Contracts

- `research.md`：记录 R001 四要素。
- `data-model.md`：定义单格式专场与多图表导出模型。
- `contracts/dedicated-format-chart-contract.json`：强类型契约。
- `quickstart.md`：验证指南。
- `tasks.md`：实施任务清单。

---

## Planned Changes by Component

- [MODIFY] `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`: 扩充 Apple zip/ditto 测试并分别导出各格式独立图表。
- [MODIFY] `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`: 优化单格式专场标题与自适应排版。
- [MODIFY] `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`: 优化单格式专场矢量导出。
