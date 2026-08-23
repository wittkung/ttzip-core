# Feature Specification: Academic-Grade Pareto Frontier Visualization Engine (DeepSWE & Gemini 3.7 Flash Style)

## 1. 业务背景与用户价值 (Context & User Value)

在高性能数据压缩与系统基准测试领域，单纯依靠控制台 ASCII 字符表或散乱的终端数字无法直观传达多软件、多算法在“吞吐速率（MB/s）vs 空间节省率（%）”之间的综合帕累托权衡（Trade-off）。

借鉴 Google 官方发布的 **DeepSWE V1.1 / Gemini 3.7 Flash** 标志性学术评测图表的设计范式，TTZip 需要在其基准测试子系统中构建一套**顶级极简学术风格的高分辨率帕累托前沿与性能演进轨迹矢量/位图渲染引擎**。该引擎需支持将 TTZip 与业界标杆（7-Zip ARM64、macOS 系统原生 ditto、Keka、The Unarchiver 等）在真实大样本上的对决结果，以高质感纯白背景、软件家族轨迹线（Family Trajectory Curves）、Hero 蓝色药丸徽章（Blue Pill Badge）与半透明演进光晕带（Evolution Halo Ribbon）的形式直接渲染落盘为出版级 PNG/SVG 图像文件，使读者一目了然地识别出帕累托最优前沿与软件优势。

---

## 2. 核心设计哲学与可读性深度解构 (Readability Principles Breakdown)

通过对 DeepSWE / Gemini 3.7 Flash 官方基准图的逆向工程与视觉认知分析，本特性确立以下 7 大视觉与可读性铁律：

```
+---------------------------------------------------------------------------------------+
|  ✦ TTZip Engine 2026                                                                  |
|  macOS Native Archiving & Compression Benchmark (enwik8 100MB)                        |
|                                                                                       |
|  100% ┤                                                      most efficient ↗         |
|       │                                              ╭─────────────────╮              |
|       │                                              │ ttzip-tar-zst   │ (Hero Badge) |
|   97% ┤                           7-zip-7z-l1  ╭─────┴─────────────────╯              |
|       │                          ╭─────────────╯ (Halo Ribbon Beam)                   |
|   96% ┤        7-zip-zip-l1 ─────╯  ★ ttzip-zip-l1                                    |
|       │       ╭─────────────╯                                                         |
|   95% ┤ ──────╯ (7-Zip Family Curve)                                                  |
|       │ 7-zip-7z-ultra                                                                |
|    0% └─────────────────────────────────────────────────────────────────────────────  |
|        10 MB/s      50 MB/s      100 MB/s      500 MB/s      1,000 MB/s    10,000 MB/s|
|                                                                                       |
|  Source: TTZip Benchmark Engine · Apple Silicon Native (mach_absolute_time)           |
+---------------------------------------------------------------------------------------+
```

1. **象限语义对齐（Top-Right Efficiency Anchor）**：
   - X 轴为对数吞吐速度（越往右越快），Y 轴为空间节省率（越往上压缩比越高）。
   - 图表右上角（Top-Right）天然代表“又快又省”的最优帕累托象限，右上角标注微型 `most efficient ↗`。
2. **多软件家族轨迹曲线（Software Family Trajectory Curves）**：
   - 将同一软件的不同配置/级别（如 `TTZip ZIP L1 -> L6 -> 7Z -> TAR.ZST`、`7-Zip Ultra -> ZIP L6 -> ZIP L1 -> 7Z Fast`）以品牌色平滑折线/曲线连接，形成“家族能力边界”，大幅降低认知负荷。
3. **Hero 重点强化与光晕光束（Hero Callout & Glowing Halo Beam）**：
   - TTZip 作为主角软件，采用 Google 经典 Royal Blue（`#2563EB`）圆角胶囊药丸（Rounded Pill Card）与白色粗体字突出关键前沿点。
   - 沿 TTZip 演进轨迹渲染一条半透明天蓝色光晕带（`rgba(37, 99, 235, 0.18)`），呈现代际统治力。
4. **背景降噪与水平单向参考线（Visual De-noising & Unidirectional Grid）**：
   - 采用学术纯白底色（`#FFFFFF`），彻底移除垂直竖向网格线，仅保留极淡的水平参考线（`#F1F5F9`）。
5. **动态焦点自适应展开（Adaptive Focus Window）**：
   - 算法和软件在真实语料上的压缩率通常集中在 90%~100% 之间，动态计算下界（如 $90\%$ 或 $\lfloor \min(Y) - 5\% \rfloor$），避免所有数据点被挤压在顶部 3% 的狭窄缝隙中。
6. **无碰撞标签智能排布（Collision-Free Staggered Label Placement）**：
   - 竞品点采用精巧低饱和度小字（9~11px），根据点所在象限自动上下交错避让，杜绝重叠。
7. **本地私密性与零 Git 污染（Local Privacy & Zero Git Bloat）**：
   - 渲染出的所有 PNG/SVG 图像仅作为本地预览与工件展示，严格受 `.gitignore` 保护，绝不提交至 Git 远程仓库。

---

## 3. 功能需求 (Functional Requirements)

### FR-001: 软件家族轨迹线与分组数据模型 (Software Family Grouping Model)
- 数据模型必须支持将散点按照所属软件（`vendor` / `softwareFamily`）进行聚合与排序。
- 自动识别并归类：`TTZip`、`7-Zip`、`Apple Native`、`The Unarchiver`、`Keka` 等软件家族。

### FR-002: 自适应对数/线性视口与焦点缩放 (Adaptive Dynamic Focus Engine)
- 动态扫描全部数据点的 $(X, Y)$ 分布，自动计算最佳刻度步长与下界 $Y_{\min}$。
- 保证数据点垂直分布覆盖图表有效绘图高度的 $70\% \sim 90\%$，留出充足的呼吸空间。

### FR-003: DeepSWE 风格光栅与矢量双模渲染 (DeepSWE Raster & Vector Plotters)
- `RasterParetoPlotter.swift`：基于 CoreGraphics / AppKit 渲染 1600x900 2x Retina 高清 PNG 位图。
- `SVGParetoPlotter.swift`：零依赖生成自包含、轻量级、响应式 SVG 矢量图。
- 包含：顶部 Sparkle 徽标、居中主标题、右上角效率提示、淡色水平网格、软件轨迹曲线、Hero 蓝色药丸卡片与半透明光晕带。

### FR-004: 真实语料软件级 1v1 PK 测试与渲染管道 (Real-World Software Benchmark Harness)
- 自动化运行真实 100MB Wikipedia 语料（`enwik8.xml`）对决。
- 一键完成：真实软件测速 $\to$ 数据聚合 $\to$ 帕累托前沿计算 $\to$ DeepSWE 风格 PNG/SVG 渲染 $\to$ 本地工件展示。

---

## 4. 非功能需求与性能约束 (Non-Functional Invariants)

1. **渲染延迟（Zero Latency Overhead）**：
   - PNG 生成耗时 $\le 15\text{ ms}$，SVG 生成耗时 $\le 2\text{ ms}$。
2. **内存零泄漏（Zero Memory Leaks）**：
   - CoreGraphics `CGContext`、`CGImage` 与 `NSBitmapImageRep` 内存必须在渲染结束时确定性即时释放。
3. **零 Git 污染与本地安全**：
   - 所有生成的 `.png` 与 `.svg` 默认写入 `docs/benchmarks/` 或缓存目录，并 100% 被 `.gitignore` 规则拦截。

---

## 5. 验收标准与成功指标 (Success Criteria)

- **AC-001**：在 100MB 真实语料上运行测试，成功输出 1600x900 纯白底色 DeepSWE 风格 PNG 与 SVG 图像。
- **AC-002**：图表中 TTZip 轨迹具备深蓝连接线与半透明光晕高亮带，关键点具备高对比度圆角药丸 Badge。
- **AC-003**：7-Zip 与 Apple Native 等竞品点具备各自独立的品牌色轨迹与无碰撞标签。
- **AC-004**：本地 CI/CD 流水线与单元测试 100% 全绿通过，且 Git 工作区保持零图片污染。

---

## 6. 需求消歧与澄清记录 (Clarifications)

- **Q1: 评估对比的粒度是算法维度还是软件应用维度？**
  - **Decision**: 必须为**真实软件产品维度（Software Application PK）**，将 TTZip（Swift 6/C 架构）、7-Zip（官方 ARM64 7zz）、Apple Native（macOS ditto）作为独立的软件系列（Software Families）进行多档位测速并连线。
- **Q2: 测试样本的选择策略？**
  - **Decision**: 优先使用本地缓存的 100MB 真实 Wikipedia 测试语料（`enwik8.xml`，95.37 MB）或真实工程源码树，不依赖临时合成的随机样本。
- **Q3: 图片生成物的版本控制管理策略？**
  - **Decision**: 渲染生成的 `.png` 与 `.svg` 仅作为开发者本地调试与工件展示，统一落盘在 `docs/benchmarks/` 并加入 `.gitignore` 保护，绝对不向 Git 提交任何二进制图片文件。
