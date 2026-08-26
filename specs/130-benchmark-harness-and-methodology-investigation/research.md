# Research: 130-benchmark-harness-and-methodology-investigation

## R001: 压缩基准测试的语料特征分类与内存隔离模型 (Corpus Diversity & In-Memory Isolation)

### 1. Decision (选定方案)
建立基于 **8 类标准特征语料库矩阵** 的全方位评测体系，并采用 **100% 预分配 RAM-to-RAM 内存驻留与物理页锁定模型**：
1. **8 类标准特征语料分类**（严格对齐 upstream `zlib-ng/test/test_data_p.h` 与 Silesia/TurboBench 标准）：
   - **`text` (文本/源码)**：包含高局部 LZ77 冗余与非均匀 ASCII 统计分布，重点评测典型动态 Huffman 编码与短距哈希匹配效率。
   - **`striped_rgb` (结构化条带图案)**：3~4 字节周期性交错通道数据，专门压测长距离滑动窗口（Long Distance Match Finder）与游程编码（RLE）。
   - **`random` (高熵伪随机数)**：均匀分布的不可压缩数据（$CR \approx 1.0$），专门评测最坏哈希碰撞遍历开销与早期失配（Early-Mismatch）快速跳出延迟。
   - **`dna` (生物碱基序列)**：4 符号字符集（2-bit 符号熵），包含大量长距离非局部重复模式，重点压测深度哈希链遍历与解冲突能力。
   - **`literals` (纯字面量)**：无 $\ge 3\text{B}$ 重复匹配的数据流，专门评测纯字面量直通发射（Literal-Only Emission）带宽与定长/动态 Huffman 树吞吐。
   - **`short_match` (短匹配序列)**：3~15 字节短重复片段与字面量交替分布，压测 4B/8B SWAR 与 SIMD 向量前导失配边界分派开销。
   - **`mixed` (复合数据)**：混合代码、二进制头部与结构化标记，评测自适应分块检测（Block Splitting）与 Huffman 头部开销。
   - **`realistic_rgb` (真实位图)**：24-bit/32-bit 未压缩光栅像素，具备水平/垂直 2D 空间相关性，压测滑动窗口跨扫描线匹配能力。
2. **100% RAM-to-RAM 隔离执行模型**：
   - 采用 `posix_memalign` 分配 64 字节对齐的输入/输出缓冲区，调用 `deflateBound()` 预先计算最大可能输出空间并完成预分配。
   - 通过 `mlock(ptr, size)` 或 `madvise(MADV_WILLNEED)` 强制锁定物理内存页，杜绝运行时 Page Fault、交换分区（Swap）与 VFS 锁争用。
   - 在进入正式计时窗口前执行 1~3 次未计时的 Warmup 预热遍历，确保输入/输出缓冲区完全驻留于 CPU L1/L2/L3 缓存与 MMU TLB。
   - 计时窗口内部实施 **零堆分配（Zero Heap Allocation Invariance）** 铁律，禁止调用 `malloc`/`free`/`realloc`。

### 2. Rationale (选择理由)
1. **全谱系覆盖消除局部偏置**：单一语料容易掩盖算法在极端情况下的缺陷（如过度激进的 SIMD 循环在 `random` 或 `literals` 出现严重失配回退惩罚）。8 类语料构成了从 $CR=1.0$ 到 $CR=1000.0+$ 的完备正交基底。
2. **消除 I/O 抖动与操作系统干扰**：传统基于磁盘文件的测试会引入 VFS 元数据锁定、页缓存脏页回写、SSD 控制器垃圾回收及 macOS Spotlight/fseventsd 索引干扰。RAM-to-RAM 隔离将基准测试标准差降低至 $\le 0.5\%$。

### 3. Alternatives Considered (已否决方案)
- **基于 Ramdisk (`/Volumes/RAMDisk`) 文件读写**：虽然消除了物理磁盘 I/O，但依然会穿透 VFS 虚拟文件系统抽象层，触发 POSIX `read()`/`write()` 系统调用上下文切换开销与文件描述符锁争用。
- **纯单一 Silesia/Calgary 语料库打包评测**：无法细粒度隔离“哈希匹配查找”、“字面量发射”、“Huffman 动态树构建”各阶段的耗时占比，不能精准指导指令级调优。

### 4. Source (实际查阅资料与代码路径)
- Upstream `zlib-ng` 官方数据生成定义：`Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/test_data_p.h`
- Google Benchmark 宏观压缩套件：`zlib-ng/test/benchmarks/benchmark_deflate.cc`
- Silesia Corpus 标准集（Deorowicz, 2003）与 TurboBench 规范：`specs/049-silesia-corpus-benchmark/spec.md`
- 本地宏观基准测试工件：`scratch/latest_rebased_macro.json` (记录了 8 种数据类型在 L1/L3/L6/L9 各级别的完整度量)

---

## R002: 纳秒级微架构比对测试与缓存对齐方法学 (Sub-Nanosecond Microbenchmark & Cache Alignment)

### 1. Decision (选定方案)
构建具备 **Google Benchmark 规范与亚纳秒级微架构遥测能力** 的轻量级基准测试 Harness：
1. **计时机制与架构对齐**：
   - 采用 `CLOCK_MONOTONIC`（POSIX）或 `clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)`（macOS 原生硬件时钟），单次单点执行 10,000,000 次循环迭代，并采集 5 组独立 Repetition 取中位数（Median），计算标准差与均值误差。
   - 评测范围覆盖 **0 ~ 256 字节连续谱系**，重点加密采样关键微架构边界（0, 1, 2, 4, 8, 10, 12, 16, 24, 32, 40, 48, 64, 80, 96, 128, 160, 192, 224, 256 Bytes）。
2. **编译器与编译器屏障控制 (Google Benchmark 语义)**：
   - 使用 `__asm__ volatile("" : "+r"(sink) : : "memory")` 或 `benchmark::DoNotOptimize(expr)` + `benchmark::ClobberMemory()`。
   - 强制编译器保留每次比较的计算结果，禁止死代码消除（DCE），同时阻止编译器将循环内内存加载提升至循环外部（Loop-Invariant Code Motion）。
3. **64 字节缓存行对齐与动态滑动窗口偏移 (Rolling Misalignment)**：
   - 测试缓冲区基地址通过 `__attribute__((aligned(64)))` 强制 64 字节对齐，彻底消除 False Sharing 与 Cross-Cache-Line Split Load 惩罚。
   - 在迭代循环中引入 **0..63 字节动态滚动偏移 (`misalign = (misalign >= 63) ? 0 : misalign + 1`)**，真实模拟 LZ77 匹配查找器在任意非对齐滑动窗口指针下的访存特征，消除特定对齐地址给 SIMD 带来的虚假增益。
4. **分支预测与指令缓存重置**：
   - 每次 Repetition 重新初始化目标字符串，并通过前置冷身（Warmup Loop）稳定分支预测器（BTB）与 L1I 指令缓存状态。

### 2. Rationale (选择理由)
1. **精准定位流水线气泡与指令延迟**：
   - 亚纳秒级精度（$< 0.1\text{ ns}$）使得观察单条 AArch64 指令的延迟成为可能：例如向量横向归约指令 `umaxv` 带来固定的 3 个 CPU 周期气泡（在 4.2GHz 下约为 0.71 ns），而 64-bit 标量提取 `fmov xN, dN` 仅需 1 个周期。
2. **抗编译优化与访存偏置**：
   - 若不施加 Memory Barrier，编译器在 `-O3` 下会将紧凑循环矢量化折叠或将常量内存提取到外层，测得虚假的 0.01 ns 跑分；
   - 固定的 64 字节对齐会导致 NEON/AVX 向量加载指令总是命中完美对齐边界，无法暴露非对齐访存（Unaligned Access）在跨缓存行时的性能骤降。

### 3. Alternatives Considered (已否决方案)
- **直接使用 CPU 周期计数器 `rdtsc` / `CNTVCT_EL0`**：现代乱序超标量 CPU（如 Apple M 系列与 Neoverse）存在剧烈的指令重排与动态频率调节（DVFS），直接读取虚拟计数器受内核态陷阱、乱序执行与流水线推测执行影响，在没有 `ISB`/`DSB` 屏障时波动剧烈。
- **单次大文件计时取平均**：粒度过粗，无法分解 0~15 字节短匹配（占 Deflate 匹配总数的 70% 以上）与 128~256 字节长匹配在流水线中的具体表现。

### 4. Source (实际查阅资料与代码路径)
- Google Benchmark 核心实现：`benchmark::DoNotOptimize` 与 `benchmark::ClobberMemory` 原理
- Upstream `zlib-ng` compare256 基准测试：`zlib-ng/test/benchmarks/benchmark_compare256.cc`
- 本地微基准测试验证代码：`scratch/bench_compare256_official_short.c`
- Maintainer 评审与微架构分析记录：`scratch/pr2416_maintainer_comment.md`

---

## R003: 自动化基准数据对比、Pareto 前沿分析与 PR 报表生成规范 (Comparative Analysis & Reporting Standards)

### 1. Decision (选定方案)
建立基于 **JSON 结构化度量、非参数假设检验（Mann-Whitney U 检验）、四级双层判定闸门与 Pareto 前沿凸包分析** 的自动化报表系统：
1. **统计学对比方法学**：
   - **相对速度提升公式**：
     $$\Delta\%_{\text{Time}} = \frac{T_{\text{baseline}} - T_{\text{candidate}}}{T_{\text{baseline}}} \times 100\% \quad (\Delta\% > 0 \text{ 表示提速})$$
     $$\Delta\%_{\text{Throughput}} = \frac{\text{MB/s}_{\text{candidate}} - \text{MB/s}_{\text{baseline}}}{\text{MB/s}_{\text{baseline}}} \times 100\%$$
   - **显著性检验**：采用 **Mann-Whitney U 检验**（对齐 Google Benchmark `compare.py` 规范，$\alpha = 0.05$），在非正态微架构耗时分布下判定提速是否具备统计显著性。
2. **四级双层性能回归判定闸门**：
   - 🟢 **GAIN**: $\Delta\% > +3.0\%$ 且 $p < 0.05$（显著提升）
   - ⚪ **FLAT / NOISE**: $-3.0\% \le \Delta\% \le +3.0\%$（统计持平/容忍噪声）
   - 🟡 **WARNING**: $-10.0\% \le \Delta\% < -3.0\%$（轻微倒退预警）
   - 🔴 **CRITICAL REGRESSION**: $\Delta\% < -10.0\%$（严重倒退，硬性阻断 CI/CD 流水线，退出码返回 1）
3. **Pareto 前沿凸包与 Markdown 报表呈现规范**：
   - **Pareto 最优判定**：在压缩率（Compression Ratio, $CR$）与压缩吞吐量（Throughput, $\text{MB/s}$）的二维坐标系中，当且仅当不存在任何引擎级别在“压缩率更高且速度更快”时，该点处于 Pareto 前沿（Convex Hull）。
   - **GitHub Markdown 表格规范**：输出包含语料分类、压缩级别、Baseline 耗时/吞吐、Candidate 耗时/吞吐、$\Delta\%$ 百分比、状态图标（🟢/⚪/🟡/🔴）及 Bitstream 校验码。

### 2. Rationale (选择理由)
1. **杜绝偶发性与主观误判**：单纯依赖均值容易受到单次系统调度毛刺干扰；结合 Mann-Whitney U 检验与中位数过滤，能以 95% 置信度拦截虚假提速或误报倒退。
2. **工程可读性与 PR 审查闭环**：结构化 Markdown 报表与清晰的 Unicode 状态标记使上游 Maintainer 和团队能够一目了然地确认零倒退（Zero Regression）与全面胜出，大幅缩短 PR 评审合并周期。

### 3. Alternatives Considered (已否决方案)
- **Student's / Welch's t-test (参数检验)**：t 检验假设样本数据严格服从正态分布，而在 CPU 降频、缓存抖动和上下文切换场景下，耗时分布通常呈现长尾分布，导致 t 检验的 p-value 计算严重失真。
- **纯绝对阈值判定（如固定 $\pm 1\%$ 闸门）**：忽略了系统固有噪声底噪，在多核高负载环境中会导致 CI 频繁误报红灯。

### 4. Source (实际查阅资料与代码路径)
- Google Benchmark 比较工具：`tools/compare.py` (基于 `scipy.stats.mannwhitneyu` 与几何均值计算)
- TTZip 自动化性能审计与阻断脚本：`scripts/audit_performance_regression.py`
- TTZip Pareto 前沿可视化引擎：`Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift` 与 `SVGParetoPlotter.swift`
- 真实 PR 评审表格格式化实现：`scratch/compare_json_bench.py` 与 `scratch/format_annotated_table.py`
