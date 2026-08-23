# Implementation Plan: Academic-Grade Pareto Frontier Visualization Engine (DeepSWE & Gemini 3.7 Flash Style)

## Technical Context

TTZip 在高并发压缩/解压热路径和 16 种格式编解码基准测试上已具备原生 C 静态库级性能与门禁矩阵。为了让性能测试输出具备与 Google 官方 DeepSWE V1.1 / Gemini 3.7 Flash 评测图表相同的顶级极简学术美感与极佳可读性，本特性在 `TTZipCore/Benchmark/` 体系中重构渲染引擎，支持多软件家族轨迹连线（Software Family Trajectory Curves）、Hero 蓝色胶囊药丸徽章（Hero Blue Pill Badges）、半透明演进光晕带（Evolution Halo Ribbon）以及基于 8 槽位 AABB 碰撞检测的标签智能避让机制。

## Constitution Check

- **[P0] 热路径零分配与隔离**：图表渲染器严格运行于 CLI/测试调度层与诊断冷路径，与 `ZipParallelExtractor`、`ZipWrite`、`ArchiveOperationAbstraction` 等编解码热路径 100% 物理隔离。
- **[P1] 进程内原生与零外部依赖**：基于 macOS 原生 CoreGraphics (`CGContext`, `CGPath`) 与标准 SVG XML 渲染，零外部 CLI 进程（如 Python/Matplotlib/Node.js）调用。
- **[P2] 确定性与零 Git 污染**：生成的 `.png` 与 `.svg` 文件受到 `.gitignore` 规则拦截，不向 Git 提交二进制图片；渲染算法无随机种子，比特精确幂等。
- **[P3] 双模渲染性能门禁**：PNG 渲染耗时 $\le 15\text{ ms}$，SVG 生成耗时 $\le 2\text{ ms}$。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《DeepSWE / Gemini 3.7 Flash 矢量图表设计系统与 CoreGraphics 亚像素渲染对齐研究》：完成纯白底色（#FFFFFF）、水平单向网格线（#F1F5F9）、Royal Blue（#2563EB）胶囊药丸与 24pt 光晕带的参数确界。
- R002 [SUBAGENT:research] 《软件家族轨迹聚类与自动多项式/折线平滑算法研究》：完成 `SoftwareFamily` 数据聚类与 Fritsch-Carlson 单调三次 Hermite 样条转换为贝塞尔曲线的数学建模。
- R003 [SUBAGENT:research] 《自适应视口边界裁剪与标签交错避让 (Collision Avoidance) 几何算法研究》：完成动态步长选择器（Nice Step Selector）与 8 槽位 AABB 确定性空间占用避让算法设计。

---

## Phase 1: Design Artifacts & Contracts

- `research.md`：记录 R001 ~ R003 的 Decision、Rationale、Alternatives Considered 与 Source。
- `data-model.md`：定义 `SoftwareFamily`、`SoftwareFamilyTrajectory`、`ParetoPoint`、`VisualRenderingToken` 的强类型模型。
- `contracts/pareto-benchmark-render-contract.json`：符合 JSON Schema Draft-07 的零通配强类型契约。
- `quickstart.md`：包含可执行命令、预期输出样本与失败诊断排查指南。

---

## Planned Changes by Component

### TTZipCore / Benchmark Component
- [MODIFY] `Sources/TTZipCore/Benchmark/ParetoFrontierModels.swift`: 扩展 `SoftwareFamily` 枚举、`SoftwareFamilyTrajectory` 容器模型与 `SoftwareFamilyClassifier` 分类器。
- [MODIFY] `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`: 落地 Fritsch-Carlson 样条曲线、Hero 药丸卡片与 AABB 8 槽位标签避让渲染。
- [MODIFY] `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`: 落地纯白极简 DeepSWE 矢量 SVG 生成器。
- [MODIFY] `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`: 增强真实语料全矩阵对决断言。
