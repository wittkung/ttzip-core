# Feature Specification: Single-Core 12-Tier Deflate Calibration and Full Pareto Frontier Supremacy

**Feature Branch**: `126-single-core-pareto-supremacy-and-12tier-calibration`  
**Created**: 2026-08-19  
**Status**: Draft  
**Input**: User description: "Deflate 引擎全语料单核帕累托最优校准：消除中段断层、档位倒挂与内部支配，全面压制 libdeflate 建立全程 Pareto 凸包统治"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 严格单调的 12 阶参数梯度校准 (Priority: P1)

作为压缩工具与开发者，用户在切换压缩级别（Level 1 至 Level 12）时，期望获得绝对可预测且单调递进的压缩收益。任何更高档位必须产生比前一档位更小的文件体积，绝不允许出现算力消耗增加但压缩率反而更差或持平的「内部自我支配与档位倒挂」现象。

**Why this priority**: 彻底解决当前在混合工作区等语料下 $L_3 \sim L_9$ 出现的压缩率倒挂与无效算力损耗，建立严格的 Pareto 前沿单调递进基础。

**Independent Test**: 在混合工作区（`Mixed Compound 100MB`）、标准英文语料（`enwik8 100MB`）及结构化日志（`Structured JSON 100MB`）上遍历执行 Level 1 到 Level 12 压缩，断言：
1. $\text{CompressedSize}(L_{k+1}) < \text{CompressedSize}(L_k)$（单调递减）；
2. $\text{Throughput}(L_k) > \text{Throughput}(L_{k+1})$（单调递减）。

**Acceptance Scenarios**:
1. **Given** 任意 100MB 混合语料，**When** 依次执行 $L_1$ 到 $L_{12}$ 压缩，**Then** 输出的体积严格单调递减，杜绝任意连续两档体积相等或倒挂。
2. **Given** 相同硬件环境，**When** 运行基准测试，**Then** 每一档位均处于有效 Pareto 前沿上，无任何被自身其他档位支配的冗余点。

---

### User Story 2 - 500 ~ 900 MB/s 中速区间 NEON 2-Way Lazy 匹配器（压制 libdeflate L6）(Priority: P1)

作为性能敏感型用户，在中高压缩比模式下进行打包时，期望获得平滑且领先竞品的中速压缩体验。当前引擎在 $L_2$ (1.02 GB/s) 到 $L_3$ (17 MB/s) 之间存在 59 倍的真空塌陷，被 `libdeflate L6` (721.8 MB/s, 3.21 MB) 攻破。通过引入轻量级 ARM64 NEON 2-way/4-way 紧凑 Lazy 匹配器，填补此真空断层。

**Why this priority**: 夺回被 libdeflate 占据的 3.20 MB / 750 MB/s 黄金均衡区帕累托前沿。

**Independent Test**: 在 `enwik8 100MB` 语料上运行中速档位测试，断言新档位产出体积 $\le 3.20\text{ MB}$ 且单核吞吐 $\ge 800\text{ MB/s}$，全面超越 `libdeflate L6`（721.8 MB/s, 3.21 MB）。

**Acceptance Scenarios**:
1. **Given** `enwik8 100MB` 语料，**When** 运行校准后的 $L_3$ 压缩，**Then** 产出文件小于 3.21 MB 且单核吞吐突破 800 MB/s。
2. **Given** Pareto 基准图，**When** 绘制 $L_2 \rightarrow L_3 \rightarrow L_4$ 曲线，**Then** 连线平滑过渡且始终位于 `libdeflate L6/L9` 的右上方。

---

### User Story 3 - 结构化 JSON 与文本的自适应 3-Byte/4-Byte 快速哈希（夺回 L1 极速前沿）(Priority: P1)

在结构化日志与 JSON 文件上，由于短键名（如 `{"`, `id`, `":`）密集，传统 4-byte 哈希难以捕获高频 3-byte 重复词，导致 `libdeflate L1` 在 JSON 语料上以 5.64 GB/s (0.92 MB) 领先 TTZip $L_1$ (4.25 GB/s, 1.10 MB)。通过引入 3-byte 向量化直接哈希与 NEON SWAR 匹配，夺回极速前沿。

**Why this priority**: 消除结构化日志和纯文本语料上的极速端落后，实现全语料 $L_1$ 绝对领先。

**Independent Test**: 在 `Structured JSON 100MB` 语料上运行 $L_1$ 压缩，断言吞吐 $\ge 5.8\text{ GB/s}$ 且压缩后体积 $\le 0.90\text{ MB}$。

**Acceptance Scenarios**:
1. **Given** 100MB 真实 JSON 语料，**When** 执行 $L_1$ 极速压缩，**Then** 吞吐量突破 5.8 GB/s 且压缩率优于或持平 libdeflate $L_1$。
2. **Given** 高频短词匹配，**When** 执行快速哈希，**Then** 无额外堆分配，零缓存抖动。

---

### User Story 4 - 全语料多模态图表收敛与无死角 Pareto 验证 (Priority: P2)

作为工程负责人与交付者，需要自动生成最新 100MB enwik8、Mixed Workspace、Structured JSON 三大核心战场的 Pareto 前沿图表，并使用多模态视觉检查，确认 TTZip 蓝色曲线在全域所有档位上全程包裹竞品（成为最外层凸包）。

**Why this priority**: 形成可度量、可复现、视觉可验证的最终帕累托最优闭环交付物。

**Independent Test**: 运行 `ZipSingleCoreParetoFrontierPkTests` 并视觉检查生成的 3 张 PNG 图表，确认无倒挂、无重叠、凸包无内凹。

**Acceptance Scenarios**:
1. **Given** 生成的三张高清 Pareto PNG，**When** 执行多模态视觉审查，**Then** 图像排版清爽、图例完备、无文字重叠。
2. **Given** 物理基准数据，**When** 计算各语料的 Pareto 包络线，**Then** TTZip 在所有主要区间均处于最外层前沿。

---

### Edge Cases

- **极端单调数据与零熵数据**：全 0 或全重复字符在各档位下的处理，确保直通 RLE 或快速静态 Huffman，避免哈希退化。
- **不可压缩高熵数据（PNG/JPEG/MP4）**：在各档位下均能在 64KB 内快速早停（Early Entropy Bypass）并直通 Store，吞吐稳定维持在 $\ge 20\text{ GB/s}$。
- **跨 Block 32KB 滑动窗口在混合突变处的平滑处理**：从不可压缩文件突变到高压缩文本时，滑动字典上下文平滑重置，不发生内存越界或脏匹配。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 引擎必须建立从 $L_1$ 到 $L_{12}$ 的严谨 12 阶参数映射矩阵，包含哈希表大小、链搜索深度（Chain Depth: 0, 1, 2, 4, 8, 16, 32, 64, 128, 256）、Good/Nice 匹配长度及 Zopfli 迭代轮数（2, 5, 15）。
- **FR-002**: 引擎在所有标准语料上必须满足压缩体积严格单调性断言：$\forall k \in [1, 11], \text{Size}(L_{k+1}) < \text{Size}(L_k)$。
- **FR-003**: 引擎必须实现专为 ARM64 调优的 NEON 2-way/4-way Fast-Lazy Matcher，在 `enwik8` 上实现 $\ge 800\text{ MB/s}$ 吞吐与 $\le 3.20\text{ MB}$ 压缩体积。
- **FR-004**: 引擎必须在 $L_1/L_2$ 匹配器中支持自适应 3-byte / 4-byte 快速哈希，在 JSON 语料上达到 $\ge 5.8\text{ GB/s}$ 吞吐与 $\le 0.90\text{ MB}$ 压缩体积。
- **FR-005**: 引擎的 12 档调度在热路径上必须保持零中间堆分配（Zero Heap Allocation），复用线程局部缓冲与栈内存。
- **FR-006**: 引擎必须保持 100% RFC 1951 / RFC 1952 规范兼容与标准解压器双向比特精确性。
- **FR-007**: 基准测试套件必须支持实时生成 12 阶平滑 Pareto 图表，并在多模态视觉检查中验证凸包外包络线。
- **FR-008**: 必须通过所有既有单元测试与 13 项硬性能门禁（`XCTestPerformanceMeasureTests` 零倒退）。

---

## Success Criteria *(mandatory)*

- **SC-001 (单调递进率)**：$L_1 \sim L_{12}$ 在 `Mixed Workspace`、`enwik8`、`Structured JSON` 三大语料上的体积单调递减达标率达到 **100%**（0 倒挂，0 相等）。
- **SC-002 (中速段反超)**：在 `enwik8 100MB` 均衡压缩段（$\approx 3.20\text{ MB}$），TTZip 吞吐量达到 $\ge 800\text{ MB/s}$，全面超越 `libdeflate L6`（721.8 MB/s）。
- **SC-003 (极速段反超)**：在 `Structured JSON 100MB` 上，TTZip $L_1$ 吞吐量达到 $\ge 5.8\text{ GB/s}$（超越 libdeflate $L_1$ 5.64 GB/s），压缩体积 $\le 0.90\text{ MB}$。
- **SC-004 (混合语料统治力)**：在 `Mixed Workspace 100MB` 上，所有档位消除自我支配，高压档位保持对 libdeflate 极限体积的 2.27 MB 缩减优势。
- **SC-005 (端到端正确性)**：全量 525+ 单元测试 100% 通过，解压哈希完全校验一致。
- **SC-006 (视觉图表验证)**：生成的三张高清 Pareto PNG 图表在多模态视觉审查中确认全部处于外包络线。

---

## Assumptions

- **测试平台基准**：基于 Apple Silicon M 系列芯片与 macOS 14+ 操作系统运行，时间测量基于 `mach_absolute_time`。
- **内存模型约束**：压缩过程不采用超过 64KB 的动态内存分配，所有哈希表与匹配器结构体驻留栈上或线程局部存储。
- **兼容性原则**：所有生成的压缩比特流必须能被系统自带 `unzip`、`gzip -d`、`libdeflate` 及 `7z x` 比特一致解压。

---

## Clarifications

### Session 2026-08-19
- **Q1 (12 档梯度定义与单调性)**: 如何杜绝混合工作区中 $L_3 \sim L_9$ 的档位倒挂与体积相等？
  - *Resolution*: 彻底废弃离散粗暴的分支调度，为 $L_1 \sim L_{12}$ 建立基于严格哈希链深度（0, 1, 2, 4, 8, 16, 32, 64, 128, 256）与 Zopfli 迭代数（2, 5, 15）的单调参数梯度，单测中断言严格单调性 $\text{Size}(L_{k+1}) < \text{Size}(L_k)$。
- **Q2 (中速段 500~900 MB/s 塌陷攻克)**: 如何填补 $L_2$ (1.02 GB/s) 到 $L_3$ (17 MB/s) 的巨大断层并压制 libdeflate $L_6$？
  - *Resolution*: 引入针对 ARM64 NEON 指令集特化的 2-way/4-way 紧凑 Lazy Matcher，在 enwik8 上以 $\ge 800\text{ MB/s}$ 吞吐产出 $\le 3.20\text{ MB}$，夺回帕累托前沿。
- **Q3 (JSON 极速段落后反超)**: 如何在结构化 JSON 语料上超越 libdeflate $L_1$ (5.64 GB/s, 0.92 MB)？
  - *Resolution*: 采用自适应 3-byte / 4-byte 快速直接哈希表配合 16-byte SWAR 匹配，消除键名重复查找延迟，目标吞吐 $\ge 5.8\text{ GB/s}$。

