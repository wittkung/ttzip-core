# Implementation Plan: Pure ZIP Format Dedicated Pareto Benchmark

## Technical Context

精炼并重构 `SoftwareParetoFrontierPkTests` 与 `RasterParetoPlotter`/`SVGParetoPlotter`：
1. 专注于纯 ZIP 格式多档位测试，生成 100% 纯净的 `pareto_pk_zip.png`。
2. 优化视口计算：针对 ZIP 格式高压缩率区间，自适应设定 X 轴（94.5% ~ 97.2%）与 Y 轴（5 ~ 2000 MB/s）。
3. 强化曲线连线与平滑样条（Fritsch-Carlson Monotone Spline），让 TTZip（蓝）、7-Zip（橙）、Apple Native（红）三条演进轨迹清晰呈现。

## Constitution Check

- [P0] 热路径隔离：仅在 Benchmark 测试套件与图表渲染器执行。
- [P1] 零外部 Python/CLI 绘图依赖：原生 CoreGraphics 亚像素抗锯齿渲染。
- [P2] 零 Git 污染：图像受 `.gitignore` 保护。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《ZIP 格式在 enwik8 真实语料下的压缩率分布与多档位对标研究》：分析 ZIP 格式下各软件档位的压缩率分布与吞吐特征。

---

## Phase 1: Design Artifacts & Contracts

- `research.md`
- `data-model.md`
- `contracts/pure-zip-benchmark-contract.json`
- `quickstart.md`
- `tasks.md`

---

## Planned Changes by Component

- [MODIFY] `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`: 构建纯 ZIP 专场专用测试 `testPureZipSoftwareParetoFrontier`。
- [MODIFY] `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`: 优化自适应视口下界与 X 轴精度。
- [MODIFY] `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`: 优化矢量视口与精度。
