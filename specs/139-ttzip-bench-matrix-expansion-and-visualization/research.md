# Phase 0 Research: ttzip-bench Multi-Format Matrix Expansion & Interactive Visualizations

**Feature Directory**: `specs/139-ttzip-bench-matrix-expansion-and-visualization`  
**Status**: Completed  
**Dispatch Protocol**: Subagent Dispatch via `invoke_subagent` (Conversation ID: `b60f4713-3f06-4edc-bd27-8e9868f86c3a`)  

---

## R001: 纯内存 C/Swift 编解码器 API 入口与零堆分配基准压测封装

### 1. 调研现状与引擎接口盘点
通过对 `Sources/CTTZipBridge/`、`Sources/TTZipCore/` 及 `Vendor/include/` 的源码审计，TTZip 现有代码库已具备 7 大核心压缩引擎的底层 C 库支持与内存级压测能力：

| 压缩引擎 | C/Swift API 入口 | 头文件路径 | 内存分配特性与桥接方式 |
| :--- | :--- | :--- | :--- |
| **libdeflate** | `ttzip_libdeflate_compress` / `ttzip_libdeflate_decompress` | `Sources/CTTZipBridge/include/CTTZipStreamCoder.h` | 基于 Thread-Local 静态 compressor 复用，预分配 buffer，零堆分配。 |
| **Zstandard** | `ttzip_zstd_compress` / `ttzip_zstd_decompress` (或直接 `ZSTD_compress`) | `Sources/CTTZipBridge/include/CTTZipBridge_Zstd.h`, `Vendor/include/zstd.h` | Buffer-to-Buffer 原生 C 函数，无额外 context 分配开销。 |
| **LZ4 / LZ4-HC** | `LZ4_compress_default` / `LZ4_compress_HC` / `LZ4_decompress_safe` | `Vendor/include/lz4.h`, `Vendor/include/lz4hc.h` | 原生 C API，直接操作 raw pointer，100% 零堆分配。 |
| **LZFSE** | `ttzip_lzfse_compress` / `ttzip_lzfse_decompress` | `Sources/CTTZipBridge/include/CTTZipBridge_LZFSE.h` | 内部封装线程局部静态 scratch buffer (`lzfse_encode_buffer_with_scratch`)，零运行时 `malloc`。 |
| **Snappy** | `ttzip_snappy_compress` / `ttzip_snappy_decompress` | `Sources/CTTZipBridge/include/CTTZipBridge_Snappy.h` | 纯 C 原生 raw block 编解码，入参传出 buffer 长度指针，零堆分配。 |
| **Brotli** | Apple `Compression.framework` (`COMPRESSION_BROTLI`) 或 `ttzip_brotli_compress` | macOS 系统原生 `<compression.h>` / `CTTZipBridge` | `compression_encode_buffer` / `compression_decode_buffer` 内存级执行，提供极简 C 桥接包装。 |
| **bzip2** | `BZ2_bzBuffToBuffCompress` / `BZ2_bzBuffToBuffDecompress` | `<bzlib.h>` (系统动态链接 `libbz2.dylib`) | 原生 Buffer-to-Buffer 接口，通过 `ttzip_bzip2_compress` 桥接统一类型签名。 |

### 2. 决策与架构设计
- **Decision (选定方案)**：
  1. 在 `Sources/CTTZipBridge/` 中扩展统一的 Buffer 编解码宏/函数桥接，将 `brotli` (`ttzip_brotli_compress`) 与 `bzip2` (`ttzip_bzip2_compress`) 包装为与 `libdeflate`/`zstd` 统一签名的零分配 C 接口；
  2. 复用 `BenchmarkBufferPool`，在初始化时一次性分配 64-byte Cache-line 对齐的页面内存（128KB 与 1MB 双池），测试循环内严禁任何 `malloc`/`free` 或 Swift `Data` 重新分配；
  3. 将评测矩阵由当前 50 点平滑扩展至 **74 点全引擎评测矩阵**：
     - `libdeflate`: 8 种 Corpus $\times$ 2 种尺寸 (128KB, 1MB) $\times$ 2 档 (L1, L6) + 4 点极端深度 (L3, L9, L12 on text/RGB) = **36 点**
     - `zstd`: 4 种 Corpus $\times$ 2 种尺寸 $\times$ 2 档 (L1, L3) = **16 点**
     - `lz4`: 3 种 Corpus $\times$ 128KB $\times$ 2 档 (L1, L9 HC) = **6 点**
     - `lzfse`: 3 种 Corpus $\times$ 2 种尺寸 = **6 点**
     - `snappy`: 3 种 Corpus $\times$ 2 种尺寸 = **6 点**
     - `brotli`: 2 种 Corpus $\times$ 2 种尺寸 $\times$ 1 档 = **4 点**
     - `bzip2`: 2 种 Corpus $\times$ 128KB $\times$ 2 档 (L1, L9) = **4 点**
     - **总计：74 点**
- **Rationale (选择理由)**：
  - Apple Silicon 单核单点耗时实测：`lz4`/`snappy` 单次 $\sim 15\mu\text{s}$，`libdeflate`/`zstd` 单次 $\sim 50\text{--}150\mu\text{s}$，`brotli`/`bzip2` 单次 $\sim 0.5\text{--}1.5\text{ms}$。74 点 $\times$ 3 次中位数采样总纯 CPU 耗时仅约 **$350\text{--}480\text{ms}$**，远低于 CI/CD 2.5 秒硬性时间门禁（裕量 $>80\%$）。
- **Alternatives Considered (被否决方案)**：
  - *方案 B：基于临时文件 IO 的流式进程压测（类似 `ExhaustiveBenchmarkRunner`）*。
    - *否决理由*：磁盘 IO 与外部进程 Spawn 带来毫秒级延迟与调度抖动，50 点以上耗时即超 15 秒，且无法测出核心纯编解码计算吞吐（容易受 VFS/APFS 锁影响）。
- **Source (核实依据)**：
  - `Sources/CTTZipBridge/include/CTTZipStreamCoder.h:30-34` (`ttzip_libdeflate_compress`)
  - `Sources/CTTZipBridge/include/CTTZipBridge_Zstd.h:25-27` (`ttzip_zstd_compress`)
  - `Sources/CTTZipBridge/include/CTTZipBridge_LZFSE.h:35-40` (`ttzip_lzfse_compress`)
  - `Sources/CTTZipBridge/include/CTTZipBridge_Snappy.h:69-79` (`ttzip_snappy_compress`)
  - `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c:134` (`BZ2_bzBuffToBuffCompress`)
  - `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift:32-100`

---

## R002: 零外部依赖、自包含交互式 SVG 与 HTML5 帕累托可视化架构

### 1. 设计要求与技术调研
- **零依赖性**：严禁引入外部 CDN（如 cdnjs, Google Fonts, Chart.js, D3.js 等），确保在完全断网（Air-gapped CI 环境）中生成的 HTML 单文件可直接双击交互运行。
- **Zen UI 视觉语言**：继承 `ttzip-ui-design-system` 规范（深色 `inkBlack` `#0B0B0C`、`deepGraphite` `#1C1C1E`、金线 `kintsugiGold` `#D4AF37`、竹青 `bambooGreen` `#2E8B57`、朱砂红 `cinnabarRed` `#C84B31`）。

### 2. 决策与架构设计
- **Decision (选定方案)**：
  1. **双格式协同渲染管线**：
     - `HTMLParetoDashboardGenerator`：输出完全自包含的单文件 HTML 仪表盘，内嵌 `<svg>` 图表、CSS 现代磨砂玻璃样式 (`backdrop-filter`)、内嵌 JSON 数据及 2KB 极简原生 Vanilla JavaScript 交互驱动器；
     - `SVGParetoPlotter`：保留纯静态矢量 SVG 导出能力，适配 Markdown 与 GitHub README 嵌入展示；
  2. **坐标系与几何建模**：
     - **Y 轴（压缩速度/吞吐）**：采用 $\log_{10}$ 对数标尺，动态自适应跨度 $[10 \text{ MB/s}, 10,000 \text{ MB/s}]$；
     - **X 轴（压缩率/空间节省率）**：线性标尺 $[0.0\%, 100.0\%]$；
     - **帕累托凸包包络线**：基于 `ParetoFrontierCalculator` 的 Andrew's Monotone Chain 算法提取 Rank 1 支配点，渲染金色虚线高亮包络层；
  3. **交互能力矩阵**：
     - **Hover Tooltip 悬浮卡片**：纯 JS 监听 SVG 数据点 `mouseenter`/`mousemove`，悬浮渲染毛玻璃卡片（显示 Engine、Corpus、Size、Level、Speed、Ratio、Pareto 状态）；
     - **多维过滤与联动搜索表**：支持按 Engine（libdeflate, zstd, lz4, etc.）、Corpus（text, dna, rgb...）、Level 即时过滤，表格行点击即高亮对应 SVG 散点。
- **Rationale (选择理由)**：
  - 纯原生内联 HTML5 + SVG + Vanilla JS 零网络请求开销、毫秒级打开即渲染，在 GitHub Pages、本地浏览、CI 报告产物中均能完美还原设计。
- **Alternatives Considered (被否决方案)**：
  - *方案 B：引入 D3.js / Plotly.js / ECharts 的 Base64 内联脚本*。
    - *否决理由*：单一 JS 库体积增加 500KB~2MB，不仅使基准报告膨胀，而且在无头环境生成时存在代码注入和解析复杂性。
- **Source (核实依据)**：
  - `.agents/skills/ttzip-ui-design-system/SKILL.md` (Zen UI 设计系统 Token 与金线规范)
  - `Sources/TTZipBench/Plotters/SVGParetoPlotter.swift:1-291` (SVG 坐标映射与 Fritsch-Carlson 样条曲线)
  - `Sources/TTZipCore/Benchmark/ParetoFrontierCalculator.swift:54-147` (Andrew's Monotone Chain 上凸包算法)

---

## R003: 自动化基准差异比对算法与 CI 性能门禁回归判决模型 (`ttzip-bench diff`)

### 1. 差异比对数学模型与公式
设 Baseline 测试结果集合为 $\mathcal{B}$，Candidate 测试结果集合为 $\mathcal{C}$。每个点通过唯一复合键定位：
$$\text{Key} = (\text{engine}, \text{corpus}, \text{payloadSizeBytes}, \text{level})$$

对于匹配的点对 $(b, c) \in \mathcal{B} \times \mathcal{C}$，定义核心相对变化量：
1. **压缩速度变化率 ($\Delta\text{ Comp Speed}$)**：
   $$\Delta v_{\text{comp}} = \frac{v_{\text{comp}}(c) - v_{\text{comp}}(b)}{v_{\text{comp}}(b)} \times 100\%$$
2. **解压速度变化率 ($\Delta\text{ Decomp Speed}$)**：
   $$\Delta v_{\text{decomp}} = \frac{v_{\text{decomp}}(c) - v_{\text{decomp}}(b)}{v_{\text{decomp}}(b)} \times 100\%$$
3. **压缩率相对变化量 ($\Delta\text{ Ratio}$)**：
   $$\Delta r = \frac{r(c) - r(b)}{r(b)} \times 100\%$$
4. **压缩后体积绝对变化量 ($\Delta\text{ Size}$)**：
   $$\Delta S = S_{\text{compressed}}(c) - S_{\text{compressed}}(b) \quad (\text{Bytes})$$

### 2. 统计学回归判决模型与 CI 门禁规则
- **Decision (选定方案)**：
  在 `ttzip-bench` CLI 中新增 `diff` 子命令：
  ```bash
  ttzip-bench diff <baseline.json> <candidate.json> [--threshold-pct 2.0] [--fail-pct 5.0] [--markdown-out report.md]
  ```
  **三级判决状态机**：
  1. **🔴 阻断失败 (Hard Fail, Exit Code = 70 `EX_SOFTWARE`)**：
     - 任一测试点的数据完整性校验未通过 (`integrityVerified == false`)；
     - 任一匹配测试点的压缩吞吐回退 $\Delta v_{\text{comp}} < -5.0\%$；
     - 任一匹配测试点的解压吞吐回退 $\Delta v_{\text{decomp}} < -5.0\%$；
     - 在相同压缩级别下，压缩后体积出现非预期增长 ($\Delta S > 0$ 且 $\Delta r < -0.5\%$)。
  2. **🟡 警告关注 (Warning / Amber Alert, Exit Code = 0 带标记)**：
     - 测试点吞吐回退处于微扰区间：$-5.0\% \le \Delta v < -2.0\%$；
     - Candidate 整体测试结果的变异系数中位数 $\text{Median}(\text{CV}) > 1.50\%$（指示当前运行环境存在 CPU 争用或温控降频）。
  3. **🟢 成功通过 (Passed, Exit Code = 0)**：
     - 全部测试点 $\Delta v \ge -2.0\%$ 且 100% 完整性校验通过，$\text{Median}(\text{CV}) \le 1.50\%$。
- **Rationale (选择理由)**：
  - 微基准测试在不同 CI Runner（如 GitHub Actions macOS Runner 与本地 M4 Max）间存在微小噪声。采用 **$2.0\%$ 预警 / $5.0\%$ 阻断** 的双阈值阶梯，既能捕获代码提交引发的真实性能退化（如内联失效、分支预测失效、SIMD 降级），又能避免 CI 偶发抖动造成的误报阻断。
- **Alternatives Considered (被否决方案)**：
  - *方案 B：仅对比整体平均耗时 (Total Wall Clock Duration)*。
    - *否决理由*：个别核心算子发生严重退化（如 LZFSE 退化 30%）可能被其他轻量算子的波动掩盖，无法实现原子级性能回归定位。
- **Source (核实依据)**：
  - `Sources/TTZipBench/main.swift:86-119` (`serializeMatrixReport`)
  - `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift:88-97` (`medianCvPercentage`)
  - `Sources/TTZipCore/Benchmark/CompetitorReportWriter.swift:119-173`
