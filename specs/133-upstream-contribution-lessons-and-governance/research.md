# Phase 0 Technical Research: Upstream Contribution Methodology, Lessons Learned, and Engineering Governance

**Feature Directory**: `specs/133-upstream-contribution-lessons-and-governance`  
**Target Subject**: 上游开源贡献方法论、微架构底层原理、工程治理规范与知识树沉淀  

---

## 摘要与调研背景 (Executive Summary)

在向顶级上游开源基础库（`zlib-ng` PR #2416、`libarchive` PR #3391 / #3388 / #3393）贡献底层优化的工程实践中，暴露出三大核心问题：
1. **AI 辅助贡献的虚浮与失控**：未经底层微架构物理机制推演与汇编级逐行验证，盲目向开源上游提交补丁，消耗维护者宝贵精力；
2. **“局部加速、全局倒退”陷阱**：仅针对单一微基准测试（如 256B 满匹配）优化，却在真实世界高频负载（短匹配、高熵随机、复合文本等）产生严重性能倒退；
3. **隐性工程知识的断代与流失**：底层性能工程中关于寄存器物理域切换、双构建系统验证、开源社区协作礼仪等宝贵经验，亟需从隐性直觉转化为显性工程规范，并沉淀为可复用的自动化门禁与教学知识树。

本调研针对上述问题展开 Phase 0 深度论证，确立 R001、R002、R003 的技术决策模型。

---

## R001: 自动化上游 Pre-Flight 审计门禁设计与多维度统计标准 (Automated Pre-Flight Gate & Statistical Standards)

### 1. Decision (选定方案)
构建基于 Python 3 + Google Benchmark JSON 结构化流的 **全自动零漏报 Pre-Flight 审计门禁引擎**（`scripts/upstream_audit_gate.py`），确立严格的多维度统计学与工程合规门禁体系：
1. **统一门禁流水线（Single CLI Pipeline）**：
   - **Step 1: 编译器构建参数等价性审计**：自动解析 Baseline 与 Candidate 的 `CMakeCache.txt`，强校验 `-DCMAKE_BUILD_TYPE`, `-DWITH_NATIVE_INSTRUCTIONS`, `-DWITH_ARMV8CRC32_HW`, `-DWITH_OPTIM` 等核心编译标志 100% 对齐，杜绝因优化级别不对称导致的跑分假象；
   - **Step 2: 双构建系统与编译器零警告审计**：静态比对 `Makefile.am` 与 `CMakeLists.txt` 源文件清单，执行 `-DCMAKE_C_FLAGS="-Wmissing-prototypes -Wall -Wextra" -DENABLE_WERROR=ON` 零警告编译断言与 100% CTest 通过断言；
   - **Step 3: 5 轮双向镜像交错基准采样（Mirrored Latin Square Cross-Over）**：采用 `Order A (Cand -> Base)` 与 `Order B (Base -> Cand)` 各 5 轮采样，100% RAM 驻留运行，数学消除 Apple Silicon / Linux CPU 的 DVFS 热节流与时钟漂移；
   - **Step 4: 统计离散度门禁（变异系数  \le 1.50\%$）**：
     48042CV = \frac{\sigma}{\mu} \times 100\%48042
     对全部测试点计算 $。若中位数  > 1.50\%$ 或单点  > 3.00\%$，直接判定环境热噪声或调度干扰过大，阻断门禁并提示重测；
   - **Step 5: 单点零倒退门禁（Single-Point Hard Regression Floor）**：覆盖 8 类标准语料（`text`, `striped_rgb`, `dna`, `mixed`, `short_match`, `random`, `literals`, `realistic_rgb`）与 13 个微测试点。**任何单一工作负载如果显示统计显著的耗时增加超过 0.0\%$（$\Delta\% < -2.0\%$ 吞吐），立即无条件硬阻断门禁**；
   - **Step 6: 零硬编码 Markdown / JSON 工件自生成**：直接从原生 JSON 结构化抽取各点数据，通过参数化模板生成包含 🟢/⚪/🔴 状态标记与 7 位 short SHA 的 PR 描述与 Maintainer 回复。

### 2. Rationale (选择理由)
1. **消除热漂移偏置（DVFS Drift Elimination）**：在 M 系列芯片多分钟单核饱和运行时，先跑的二进制享有冷硅片优势（频率高出 1%~2%）。双向交错运行并取均值，能将系统误差从 $\pm 2.5\%$ 压缩至 $< 0.3\%$。
2. **严防“局部微优化导致的全局倒退”**：在 Deflate 引擎中，短匹配和不可压缩字面量占据了 70%~90% 的执行流。若对 256B 优化带来 15B 匹配 2% 的微小开销，在整体解压缩/压缩吞吐上就会造成致命负收益。2.0% 的单点阻断线是保障上游安全合并的物理红线。

### 3. Alternatives Considered (已否决方案)
- **固定单向跑分取平均值（Fixed Baseline-First Runs）**：因先跑进程占用冷态 CPU，测出虚假的 Baseline 优势或 Candidate 虚高，已被实验证实存在系统性系统偏差（高达 3.2%）。
- **参数化 Student's t-test 显著性判定**：微架构测试在出现分支预测抖动和内存页抖动时耗时分布呈现长尾分布，严重违背正态分布假设，导致 p-value 误报。
- **纯人工 Checklist 审查**：极易因开发者疏忽遗漏编译器参数或某些压缩级别（如 L9），导致缺陷流入上游仓库。

### 4. Source (查阅文献与代码路径)
- TTZip 现有双向交错测试与参数化报告脚本：`scripts/upstream_crossover_bench.py`, `scripts/upstream_report_gen.py`
- Google Benchmark 统计规范与 `compare.py` 实现：`zlib-ng/test/benchmarks/benchmark_deflate.cc`
- 性能回归审计脚本：`scripts/audit_performance_regression.py`
- PR #2416 跨域测试实测数据记录：`scratch/pr2416_maintainer_comment.md`, `specs/131-upstream-contribution-and-benchmark-protocol/research.md`

---

## R002: AArch64 微架构寄存器域时延（GPR vs FPR）与长短匹配分层优化模型 (AArch64 Register File Latencies & Tiered Hybrid Comparison)

### 1. Decision (选定方案)
采用 **四阶段分层混合比对模型（Tiered Hybrid Comparison Model）**：
- **Stage 1 (0..15 Bytes, 短匹配探测区)**：采用 **纯 64-bit 标量通用寄存器（GPR）`zng_memread_8` + `^` 异或探测 + `UNLIKELY` 早期跳出**，100% 避免进入向量寄存器文件（FPR/SIMD）；
- **Stage 2 (16..47 Bytes, 中距过渡区)**：采用 16 字节 NEON 向量后变址加载（`LOAD_16B_PAIR`）+ 标量子寄存器通道提取（`vgetq_lane_u64` $\to$ `fmov xD, dN`），以低发射开销平滑过渡；
- **Stage 3 (48..240 Bytes, 长匹配高速循环区)**：采用 **2x 向量双展开（32 字节/迭代）+ `vorrq_u8` 差异合并 + 单次 `vmaxvq_u8` 向量横向最大值归约**，充分打满双 128-bit 向量加载端口并均摊归约时延；
- **Stage 4 (240..256 Bytes, 尾部收敛区)**：执行最后 16 字节向量比对并直接返回满匹配 256。

### 2. Rationale (微架构流水线、发射端口与时钟周期深度物理剖析)

#### A. 寄存器物理域（GPR vs FPR）与跨域时延分析
在 Apple Silicon (Firestorm / Avalanche / M4 / M5) 与 ARM Neoverse (V1 / V2) 等现代顶级 AArch64 超标量微架构中：
1. **整数通用寄存器文件（GPR File, `x0..x30`）**：
   - 拥有 6~8 个全功能整数 ALU 发射端口（Port 0..5）；
   - `ldr x0, [ptr]`（LSU 端口 4/5）加载到使用延迟（Load-to-Use）为 **3~4 周期**；
   - `eor x2, x0, x1` 为单周期延迟（**1 周期**），吞吐可达 0.16~0.25 周期/条；
   - `cbz x2, label` / `cbnz x2, exit` 在分支预测单元内以 **0~1 周期** 评估，分支不跳转时直通执行零气泡；
   - 字节差异定位 `zng_first_diff_byte64`（`rbit` + `clz` / `ctz`）耗时 **1 周期**；
   - **0..7 字节失配关键路径总时延**：Load (3c) + EOR (1c) + CBNZ (1c) = **$\approx 5$ 个时钟周期（约 1.1ns）**。

2. **浮点/向量寄存器文件（FPR / SIMD Register File, `v0..v31`）**：
   - 拥有独立的向量 ALU 端口（Port 2/3）与 2 个 128-bit 向量 LSU 端口（Port 4/5）；
   - `ldr q0, [ptr]` 向量加载延迟为 **4 周期**；
   - `veorq_u8`（`eor v2.16b, v0.16b, v1.16b`）耗时 **1~2 周期**；
   - **跨域数据搬移惩罚（Cross-Domain Forwarding Penalty）**：
     - 指令 `vgetq_lane_u64(cmp, 0)` 编译为 `fmov x2, d0`，必须从 FPR 物理寄存器堆经过专用的跨域旁路网络（FP-to-INT Forwarding Bus）传递到 GPR 寄存器堆，带来 **2~3 个时钟周期的物理跨域时延**；
     - 指令 `vgetq_lane_u64(cmp, 1)`（提取高 64 位）更需先执行元素解包（Unpack/Extract）再跨域，耗时 **3~4 周期**；
   - **向量横向归约时延（Horizontal Reduction Latency）**：
     - 指令 `vmaxvq_u8`（`umaxv b3, v2.16b`）必须在 16 个 8-bit 字节通道间执行 4 级二叉树规约比较（ \to 8 \to 4 \to 2 \to 1$），在硬件执行流水线上具有 **不可减免的 3~4 个周期固定物理延迟**；
     - 归约完成后仍需 `fmov w4, s3` 跨域到 GPR 供 `cbz` 评估，再耗费 1~2 周期；
   - **单向量 16B 探测失配关键路径总时延**：Load (4c) + EOR (2c) + UMAXV (3c) + FMOV (2c) + Branch (1c) = **$\approx 12$ 个时钟周期（约 2.8ns）**。

#### B. 为什么短匹配（0..15B）纯标量 GPR 具有确定性绝对优势？
- 在 LZ77 Deflate 匹配查找器（`longest_match`）中，哈希链遍历与候选匹配探测有 **70%~90% 以上在 0..8 字节内即发生失配**（哈希假冲突或短字面量）；
- 纯标量 GPR 路径完全在整数执行域内闭环，零 FPR 跨域开销，耗时比向量归约短 **60% 以上**；
- 配合 `UNLIKELY(diff)` 宏，编译器生成直通下行指令，将失配跳出代码剥离至冷基本块，大幅削减指令缓存挤压与前向跳转。

#### C. 为什么长匹配（48..256B）2x 双展开 + `vmaxvq` 能决定性胜出？
- 当匹配越过 32 字节时，数据具有高局部相关性，继续匹配的先验概率极高（`LIKELY`）；
- Apple Silicon / Neoverse 具备 **双 128-bit 向量并发加载能力**，单周期可从 L1D Cache 吞吐 32 字节；
- 2x 双展开将原本 16 轮单向量迭代（16 次分支）骤降为 **8 轮 32 字节迭代**；
- 通过 `vorrq_u8`（1 周期）将两个 16B 差异合并，使得 32 字节内只需执行 **1 次 `vmaxvq` 归约**，将 3 周期归约时延摊薄至 **0.09 周期/字节**；
- 在长匹配场景（如 `striped_rgb` L6）下，耗时从 Baseline 的 0.184 ms 缩短至 0.149 ms，实现 **+19.1% 的净吞吐跃升**。

### 3. Alternatives Considered (已否决方案)
- **纯 64-bit 标量 SWAR 全程遍历（Pure 64-bit SWAR）**：在短匹配尚可，但在 32..256B 长匹配下需要执行 32 次 64-bit 循环与分支，导致 `striped_rgb` 出现 **-13.7% 严重性能倒退**。
- **单向量 16B 逐块 UMAXV（PR #2416 初始提交方案）**：每 16 字节硬塞一次 3 周期 `umaxv` 归约，在不可压缩字面量 `literals` 上因归约开销无法均摊而出现 **+4.3% 性能倒退**。
- **4x 16B 展开（64 字节/迭代）**：导致函数体过大，内联后挤爆 `longest_match` 的 uop Cache 与 L1I 缓存，导致编译器寄存器溢出（Stack Spilling）。

### 4. Source (查阅文献与代码路径)
- 源码实现与逐行注释：`docs/study/compare256_neon_annotated.c`, `docs/study/compare256_neon_original_annotated.c`
- 真实 PR 讨论与 Nathan 指导：`scratch/pr2416_maintainer_comment.md`, `docs/an_open_letter_of_apology_to_nathan.md`
- 反汇编物理验证：`otool -tv Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build/CMakeFiles/zlib-ng-static.dir/arch/arm/compare256_neon.c.o` (149 指令, 0 栈溢出)
- LLVM AArch64 调度模型源码：`llvm/lib/Target/AArch64/AArch64SchedM1.td` (关于 `fmov` 跨域与 `UMAXV` 延迟定义)
- 物理实测基准数据：`specs/110-aarch64-compare256-zero-overhead-optimization/research.md`

---

## R003: 开源治理宪章、隐性知识显性化与创业教学知识依赖树集成规范 (Open Source Governance, Tacit Knowledge & Educational Integration)

### 1. Decision (选定方案)
将上游贡献实战中凝结的硬核经验，双向沉淀进 **TTZip 核心工程宪章**（`.specify/memory/constitution.md`）与 **面向自动化专业与年轻工程师的创业教学知识依赖树**（`docs/study/`）：
1. **工程宪章入宪（Constitution Section 6 规范）**：
   - 固化 **5 大上游贡献不可逾越铁律（Five Upstream Invariants）**：
     - *Invariant 1: 硬件机理确界律 (Hardware Grounding)*：严禁向开源上游提交任何未经过汇编级指令周期、发射端口与寄存器域分析的 AI 生成代码；
     - *Invariant 2: 多维全负载零倒退律 (Multi-Workload Zero Regression)*：任何优化必须通过 8 类基准语料与多尺度 Payload 压测，单点倒退 $>2\%$ 一票否决；
     - *Invariant 3: 单变量消融确证律 (Single-Variable Ablation Testing)*：必须通过消融实验隔离每个宏、内联汇编与编译选项，严禁复合修改混淆归因；
     - *Invariant 4: Maintainer 关注度敬畏律 (Maintainer Attention Reverence)*：严禁机械化 LLM 套话，坚持真实人际沟通、逐项客观闭环回复与真诚致谢；
     - *Invariant 5: 原子提交整洁律 (Atomic Commit Hygiene)*：严格按照 `Refactor/Macro Infra` $\to$ `Feat/Optimization` $\to$ `Test/Docs` 拆分独立可编译的原子 Commit。
2. **教学知识依赖树集成（Educational Knowledge Dependency Tree）**：
   - 在 `docs/study/` 建设完整案例教学模块（`case_study_arm64_simd_journey.md`），提炼面向年轻工程师的 **四阶段认知升维教学范式**：
     48042\text{直觉推测 (Intuition)} \longrightarrow \text{算法模型 (Algorithmic Model)} \longrightarrow \text{硬件物理确界 (Hardware Bounds)} \longrightarrow \text{开源敬畏 (Open Source Humility)}48042
   - 建立系统化的前置知识依赖链接图谱。

### 2. Rationale (选择理由)
1. **隐性知识显性化（Tacit to Explicit Knowledge）**：顶尖工程师的微架构直觉往往是默会的。通过对 PR #2416 和 PR #3391 的系统复盘，将“何时用标量、何时用向量、跨域时延多大、如何与 Maintainer 沟通”显式转化为可推演的代码注释与文本，实现知识在组织与社区中的无损流转。
2. **对齐创业教学使命**：创始人孔维涛（Witt Kung）毕业于同济大学自动化专业，创立教育科技项目致力于构建跨学科知识依赖树。将真实的工业级 AArch64 优化历程提炼为案例，能让年轻学子在具体场景中深刻体会计算机科学的严谨与开源社区的精神内核。

### 3. Alternatives Considered (已否决方案)
- **仅保留代码变更，不留文字沉淀**：极易在未来开发中由新参与者（或后续 AI 对话）重复踩中相同的跨域时延与构建系统坑位。
- **将案例写成枯燥的纯学术论文**：缺乏真实 PR 冲突、道歉信背景与思维转变的张力，无法激发年轻学子的工程共鸣与敬畏心。

### 4. Source (查阅文献与代码路径)
- 宪章核心文档：`.specify/memory/constitution.md`
- 上游贡献复盘白皮书：`docs/upstream-contribution-lessons-learned.md`
- 致 Nathan 的公开致歉与反思信：`docs/an_open_letter_of_apology_to_nathan.md`
- libarchive 上游协作回复日志：`docs/upstream-pr-responses-2026-08-17.md`
- 现有学习专版带注释代码：`docs/study/compare256_neon_annotated.c`
