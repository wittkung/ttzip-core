# Feature Specification: 084-lzham-branchless-decompression-and-circular-dict

**Feature Directory**: `specs/084-lzham-branchless-decompression-and-circular-dict`  
**Created**: 2026-08-18  
**Status**: Specified  
**Input**: 借鉴 richgel999/lzham_codec 解压状态机中的“分支消除”与环形解压字典更新设计，分析实现差异、性能优势并规划利用方案。

---

## 1. Executive Summary & Problem Space

### 1.1 Context & Background
在超高压缩比场景（LZMA/LZMA2/LZX 级别），传统的解码瓶颈主要在于**逐比特范围算术解码 (Bit-by-Bit Range Decoding) 的流水线气泡**以及**非对齐环形字典回绕时的分支预测失败与逐字节拷贝开销**。

[richgel999/lzham_codec](https://github.com/richgel999/lzham_codec) 是由 Rich Geldreich 开发的无损压缩编解码器（MIT / Public Domain），其核心目标是**在保持甚至匹敌 LZMA 极高压缩比的同时，实现 1.5x ~ 8x 于经典 LZMA 的解压吞吐速度**。

本规范聚焦于解压核心链路的系统化解构与借鉴：
1. **解压状态机分支消除**：准自适应哈夫曼模型 (Quasi-Adaptive Huffman)、11-bit 直接查表映射、64-bit 预取缓冲与局部变量寄存器驻留。
2. **环形解压字典高效更新**：$2^N$ 掩码索引回绕 (`dict_size_mask`)、无回绕 Fast-Path（SIMD / NEON 批量复制）与跨边界回绕 Slow-Path 分流、RLE Byte Run 特化。
3. **TTZip 引擎升级路径**：在 TTZip 现有 `CTTZipBridge` 体系下提炼高性能分支消除解压技术，既赋能自研 LZHAM 解码器支持，又反哺现有 LZMA2/LZX/7z 热路径解压吞吐。

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - 架构与性能对比调研分析 (Priority: P1)
作为 TTZip 的架构师与性能工程师，需要全面掌握当前自研引擎（`liblzma`, `fast-lzma2`, `ttzip_lzma2_dec_native.c`, `ttzip_lzma2_branchless_rc.c`）与 `lzham_codec` 在状态机分支处理、字典回绕管理、内存对齐与吞吐指标上的实质性差异，产出具备确定性结论的技术全景分析报告。

**Why this priority**: 摸清两者底层物理执行模型差异与性能差距是所有算法优化与代码集成的绝对前置条件，避免盲目重构。

**Independent Test**:
- 运行针对典型高压缩数据集（Silesia、Enwik8、二进制 Executables）的微基准测试与指令分支分析（Branch Mispredictions、IPC、CPU L1 Cache Misses），量化对比分支消除与字典回绕设计的理论及实测收益。

**Acceptance Scenarios**:
1. **Given** LZHAM 解码器与 TTZip 现有 LZMA/LZMA2 解码器源码，**When** 进行状态转移与分支拓扑分析时，**Then** 清晰标定 LZHAM 如何通过准自适应哈夫曼查表替代逐比特二叉树遍历，并量化其分支消除率。
2. **Given** 循环解压字典更新流程，**When** 对比掩码回绕机制与当前分块拷贝机制时，**Then** 输出明确的流水线与缓存命中率收益评估。

---

### User Story 2 - 分支消除与掩码环形字典核心技术提炼与移植方案 (Priority: P2)
作为核心引擎开发者，需要基于 LZHAM 的设计思想，为 TTZip 规划可落地的架构改进方案，包括：
1. 将 64-bit 批量预取与 11-bit 一级哈夫曼直接查表技术整合到解压状态机。
2. 将 $2^N$ 掩码式环形字典更新模型与双路径（Fast-Path NEON 复制 / Slow-Path 边界回绕）无缝融入 C Bridge 解码热路径。

**Why this priority**: 将高价值算法思想工程化转化为 TTZip 系统的实际性能资产。

**Independent Test**:
- 提供独立的 C11/POSIX 原型实现或针对 LZMA2/LZHAM 的解压状态机单元测试，验证在不破坏数据完整性的前提下解压吞吐获得显著提升。

**Acceptance Scenarios**:
1. **Given** 连续的 Match Copy 载荷，**When** 源与目标区间均在字典当前有效线性窗口内时，**Then** 100% 走入无分支 NEON/SIMD 批量复制，零额外边界判断开销。
2. **Given** 跨越字典边界的末尾 Match 数据，**When** 触发回绕时，**Then** 安全无缝切换到 Slow-Path 字节级回绕，并正确维护 `dst_ofs` 与滑动窗口。

---

### User Story 3 - 集成 LZHAM Codec 作为 TTZip 原生支持格式 (Priority: P3)
作为终端用户或 CLI 使用者，能够在 TTZip 中直接无缝解压使用 LZHAM 压缩的高压缩比归档，享受极速解压体验。

**Why this priority**: 扩充 TTZip 的超算/游戏资源类压缩格式版图，满足对超高压缩比与高解压速度兼顾的专业场景需求。

**Independent Test**:
- 使用 LZHAM 原生压缩文件作为输入，TTZip 能够完整、校验正确（Adler-32/CRC32 一致）地还原原始文件。

**Acceptance Scenarios**:
1. **Given** 一个使用 LZHAM 压缩的测试文件，**When** 调用 TTZip 解码流水线时，**Then** 数据解压成功且解压吞吐达到原生理论基准。

---

## 3. Edge Cases & Boundary Conditions

- **EC-001 (0 距离或超界 Match 距离)**: 当解压流中出现损坏或恶意构造的 `match_dist > current_dict_size` 或 `match_dist == 0` 时，状态机必须安全捕获并报错，严禁内存下溢或野指针读写。
- **EC-002 (极小重叠 Match, RLE Byte Runs)**: `match_dist == 1`（连续单字节重复填充）在字典回绕处需特殊处理，防止 SIMD 批量读取自身未写入的数据。
- **EC-003 (跨越字典边界的巨型 Match)**: `match_len` 超过当前位置到字典末尾距离时，必须精准切分为前半段和后半段或回绕单字节写入，确保字典完整性。
- **EC-004 (未对齐的非 $2^N$ 输入/字典)**: 强制校验并对齐字典大小为 2 的整数次幂，若非 $2^N$ 拒绝初始化或平滑规整，确保位与掩码运算的数学正确性。

---

## 4. Requirements *(mandatory)*

### 4.1 Functional Requirements

- **FR-001**: 系统 MUST 深入剖析 `richgel999/lzham_codec` 的解压状态机（`lzham_lzdecomp.cpp`, `lzham_symbol_codec.h`）实现机制，对比 TTZip 现有解压模型（`ttzip_lzma2_dec_native.c`, `ttzip_lzma2_branchless_rc.c`, `fast-lzma2`）。
- **FR-002**: 系统 MUST 明确分析 LZHAM 的“分支消除”机制（64-bit 预取、11-bit 直接查表、`LZHAM_BUILTIN_EXPECT` 拓扑与局部寄存器保持）。
- **FR-003**: 系统 MUST 明确分析 LZHAM 的环形字典更新机制（$2^N$ 掩码回绕 `dict_size_mask`、`LZHAM_MAX(src_ofs, dst_ofs) + match_len > dict_size_mask` 边界分流、RLE byte run 特化）。
- **FR-004**: 系统 MUST 给出客观、精确的性能与架构评估（在压缩比、解压吞吐、内存占用三维度的权衡矩阵），回答“是否真的更快更好”。
- **FR-005**: 系统 MUST 制定清晰的分阶段落地与利用技术路线（包括自研 C Bridge 原生接入方案与对现有格式的优化建议）。
- **FR-006**: 系统 MUST 符合 TTZip 工程宪法（四大系统工程铁律：流式第一性、纵深防御、确定性确界、真实预言机）。

---

## 5. Success Criteria *(mandatory)*

### 5.1 Measurable Outcomes

- **SC-001**: 架构对比与技术深度剖析 100% 覆盖状态机分支、前缀解码、字典回绕三大核心维度。
- **SC-002**: 产出的技术决策与选型建议具备唯一性与明确理由，不给出无倾向性选项罗列。
- **SC-003**: 方案设计确保移植后的分支消除状态机与环形字典更新在主流 Apple Silicon / ARM64 与 x86_64 平台上保持零性能回退与零未对齐故障。

---

## 6. Assumptions

- 目标平台以 macOS (Apple Silicon ARM64, NEON) 为首要优化目标，同时保证 x86_64 POSIX C11 兼容性。
- LZHAM 解压字典大小限定为 $2^N$（$2^{15}$ 至 $2^{29}$ 字节，即 32KB 至 512MB），符合其格式原生规范。

---

## 7. Clarifications

### Clarification Session 2026-08-18
- **Q1: 本次落地的技术边界与架构形态是什么？**  
  **A1**: 确立双轨落地策略：(1) 将 LZHAM 的 11-bit 直接查表加速与 $2^N$ 掩码环形字典更新模型作为独立 C 模块提炼并反哺至 TTZip 原生解压链路；(2) 在 `CTTZipBridge` 中预留原生 LZHAM Codec 解压流式接口。
- **Q2: 硬件指令集与平台特化范围？**  
  **A2**: Fast-Path 重点特化 Apple Silicon ARM64 (NEON 向量指令与 PMULL/CRC)，同时保持 x86_64 标准 C11/POSIX 跨平台一致性。
- **Q3: 内存与字典生命周期管理？**  
  **A3**: 严格遵守 TTZip “热路径零中间堆分配”与“流式第一性”铁律，字典采用固定页对齐分配（`ttzip_platform_aligned_alloc`），状态机核心变量栈上寄存器化。

