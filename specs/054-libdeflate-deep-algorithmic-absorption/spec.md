# Feature Specification: Deep Algorithmic Absorption of libdeflate Core Techniques

**Feature Directory**: `specs/054-libdeflate-deep-algorithmic-absorption`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "好好研究 libdeflate 代码仓库，全面剖析并深度吸收其算法精髓（SIMD 快速重置 Matchfinder、PMULL/PCLMUL 宽折叠 Adler-32/CRC-32、无分支 64-bit 位流解压与缓存行对齐架构），全链路赋能 TTZip 全格式引擎"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 硬件级 Adler-32 / CRC-32 宽折叠校验和统一赋能 (Priority: P1)

TTZip 在处理 ZLIB、GZIP、ZIP、PNG 以及通用数据块校验时，需要对 Adler-32 与 CRC-32 进行极致的硬件加速。系统需要吸收 `libdeflate` 的 PMULL / PCLMUL 64 字节宽多项式折叠（Wide Polynomial Folding）与 NEON/AVX2 双重循环累加技术，将全格式校验和计算吞吐全面推升至 25~35 GB/s，彻底消除校验和在解压/压缩管道中的 CPU 瓶颈。

**Why this priority**: 校验和是解压和压缩管道中不可跳过的固定算力开销。将 Adler-32 和 CRC-32 推至硬件单周期极限，能够直接使全格式解压管道获得 15%~25% 的端到端吞吐收益。

**Independent Test**: 对比原生 C 语言标量查表算法与宽折叠实现，在 100MB 缓冲区上执行吞吐测试，断言吞吐量 $\ge 20\text{ GB/s}$ 且结果与标准 zlib `adler32()` / `crc32()` 逐比特一致。

**Acceptance Scenarios**:
1. **Given** 任意长度与对齐的内存数据块，**When** 调用硬件级 Adler-32 计算，**Then** 计算速度达到 $\ge 20\text{ GB/s}$ (Apple Silicon / AVX2)，且输出与标准 RFC 1950 Adler-32 完全一致。
2. **Given** 跨页大内存块，**When** 调用硬件级 CRC-32 宽折叠计算，**Then** 吞吐量达到 $\ge 25\text{ GB/s}$，且无边界越界或短读取错误。

---

### User Story 2 - SIMD Matchfinder 快速重置与双哈希链集成 (Priority: P1)

TTZip 自研的压缩引擎（如 LZMA2 快速模式、7z Store/Deflate 及 Zstd 辅助匹配器）在多块连续压缩时，需要高效管理滑动窗口与哈希表。吸收 `libdeflate` 的 `matchfinder_rebase()` 向量化索引重置技术（利用 NEON / AVX2 在 32KB 窗口满时仅需数微秒将全表索引减去 32768，而非整表清零），结合 3 字节/4 字节双哈希链（`load_u24_unaligned`），大幅提升小文件批处理与中等压缩级别的吞吐。

**Why this priority**: 传统匹配查找器在跨块时频繁 `memset` 清零哈希表导致大量的 L1/L2 缓存抖动。向量化快速重置能够将匹配器重置耗时降低 90% 以上，显著提升连续分块压缩性能。

**Independent Test**: 在连续压缩 1,000 个 32KB 数据块场景下，测试匹配器重置耗时，验证重置时间占总压缩时间比 $\le 1\%$。

**Acceptance Scenarios**:
1. **Given** 滑动窗口达到 32KB 边界，**When** 触发匹配器重置，**Then** 自动执行 SIMD `matchfinder_rebase` 向量化减法，无需重新分配或全量清零内存。
2. **Given** 输入数据流，**When** 执行哈希查找，**Then** 采用未对齐 3 字节宽字加载计算哈希，短距离重复子串命中率提升 $\ge 15\%$。

---

### User Story 3 - 64-bit 无分支位流解码与双层 Huffman 极速解码 (Priority: P2)

在解压缩热路径上，全面吸收 `libdeflate` 的 64 位机器字长位累加器（Word-sized bitbuf）与无分支位流预加载（`REFILL_BITS_BRANCHLESS`），配合双层直接解析 Literal 与 Length+Distance 的预计算查找表（Fast Decode Tables），将解压过程中的指令依赖与分支预测失败降至最低。

**Why this priority**: DEFLATE 解压的核心性能损耗主要集中在位流逐 bit 读取与 Huffman 树逐级回溯。无分支位流与单次访存 Huffman 解析是解压速度突破 10 GB/s 的核心技术基石。

**Independent Test**: 对比标准 zlib inflate 与无分支位流解码器在 Silesia / Enwik 语料库上的单核解压速度，断言解压吞吐达到标准 zlib 的 $\ge 2.5\times$。

**Acceptance Scenarios**:
1. **Given** 任意符合 RFC 1951 的 DEFLATE 压缩流，**When** 解压器进行位流消费，**Then** 使用 64 位累加器无分支预充，单次 Huffman 符号解码在 1 次内存访问内完成。
2. **Given** 发生重叠拷贝（Overlapping match, $D < L$），**When** 执行字节还原，**Then** 采用 16 字节 SIMD 宽字无分支展开拷贝，杜绝逐字节循环。

---

### User Story 4 - 32/64 字节缓存行对齐与平坦结构体内存布局 (Priority: P3)

吸收 `libdeflate` 的内存架构设计准则（`MATCHFINDER_MEM_ALIGNMENT 32` 与 `MATCHFINDER_SIZE_ALIGNMENT 1024`），将压缩器/解压器上下文设计为单块连续平坦内存结构体（Contiguous Flat Struct），彻底消除指针二次跳转与冷热数据跨缓存行惩罚（Split-cache-line penalties）。

**Why this priority**: 现代 CPU（特别是 Apple Silicon 宽发射架构与现代 x86）对缓存行对齐极其敏感。消除指针间接引用与内存碎片，能够使 L1D Cache 命中率维持在 98% 以上。

**Independent Test**: 通过性能分析工具断言匹配器结构体地址 100% 满足 32/64 字节对齐，且结构体内无多余指针解引用。

**Acceptance Scenarios**:
1. **Given** 压缩器与上下文初始化，**When** 内存分配执行，**Then** 返回的首地址严格 32/64 字节对齐，且全部哈希表与链表内嵌于同一连续内存块内。

---

### Edge Cases

- **非对齐缓冲区首地址**：当输入输出指针未按 8/16 字节对齐时，底层算法利用 `load_u32_unaligned` / `load_u64_unaligned` 安全访问，不触发硬件 Bus Error。
- **超短数据块 ($< 16$ 字节)**：在数据尾部不足 16/64 字节时，向量化折叠与 SIMD 拷贝安全平滑回退至标量保护循环，严禁越界加载触碰野指针内存页。
- **高频短距离重叠匹配 ($D \in [1, 15]$)**：在解压距离小于 16 字节的匹配串时，无分支向量化算法通过固定展开或位移安全回写，确保数据正确性与高吞吐并存。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 提供硬件级多项式宽折叠 Adler-32 实现（`ttzip_adler32_fast`），在 ARM64 上利用 NEON 双累加器、在 x86_64 上利用 AVX2，吞吐量达到 $\ge 20\text{ GB/s}$。
- **FR-002**: 系统 MUST 提供硬件级 CRC-32 宽折叠实现（`ttzip_crc32_pmull_wide`），支持 64 字节/128 字节单步多项式折叠与 Barrett 归约，吞吐量达到 $\ge 25\text{ GB/s}$。
- **FR-003**: 系统 MUST 在 `Sources/CTTZipBridge/` 中引入基于 SIMD 的匹配查找器快速重置机制（`ttzip_matchfinder_rebase`），通过向量化减法在 $\le 5\mu s$ 内完成 32KB 滑动窗口索引回退。
- **FR-004**: 系统 MUST 引入 64 位无分支位流预充（`ttzip_bitbuf_refill_branchless`）与单访存双层 Huffman 解码表生成逻辑。
- **FR-005**: 系统 MUST 确保所有匹配查找器与状态上下文结构体满足 32 字节物理内存对齐约束（`TTZIP_ALIGNED(32)`），并采用平坦内嵌数组消除堆碎片。
- **FR-006**: 系统 MUST 将上述底层算法封装为 Swift 安全接口（`HardwareChecksumAdapter`、`FastMatchFinderBridge`），对全量归档格式（ZIP/7Z/GZ/TAR/ZSTD）开放。

---

### Key Entities

- **HardwareChecksumEngine**: 统一提供硬件级 Adler-32 与 CRC-32 宽多项式折叠加速的 C/Swift 核心引擎。
- **FastMatchFinderContext**: 32 字节对齐的平坦查找器结构体，内嵌 Hash-3/Hash-4 表与向量化重置逻辑。
- **BranchlessBitDecoder**: 64 位宽机器字无分支位流解析器，用于极速 Huffman 符号与流边界提取。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `ttzip_adler32_fast` 在 Apple Silicon (M 系列芯片) 上处理 100MB 缓冲区时，物理实测吞吐量达到 $\ge 20\text{ GB/s}$，比传统 zlib 标量实现提升 $\ge 8\times$。
- **SC-002**: `ttzip_crc32_pmull_wide` 物理实测吞吐量达到 $\ge 25\text{ GB/s}$。
- **SC-003**: 匹配查找器重置测试中，32KB 窗口索引重置耗时 $\le 5\mu s$（比传统全量清零提速 $\ge 10\times$）。
- **SC-004**: 所有优化算法通过 100% 黄金预言机与标准库差分校验（比对 RFC 1950 / RFC 1951 / RFC 1952 官方预言机，零比特差异）。

---

## Assumptions

- 运行平台为 macOS 14.0+ 与 Windows x86_64/ARM64，编译器支持 C11 标准与 SIMD 内联函数（ARM NEON, Intel AVX2）。
- 所有算法基于开源自由的 MIT 许可证逻辑进行吸收与重构。

---

## Clarifications

### Session 2026-08-18

- **Q1 (吸收的核心技术范畴)**: 本次规范聚焦 `libdeflate` 五大核心技术：(1) 宽多项式折叠 Adler32/CRC32；(2) SIMD 向量化 matchfinder 快速重置；(3) 64-bit 无分支位流解码；(4) 32/64 字节平坦缓存行对齐；(5) 运行时 CPU 特性探测中枢。
- **Q2 (与已有模块的关系)**: 作为 TTZip 底层算力基础设施层（`CTTZipBridge`），上层无缝为 ZIP、7Z、GZ、TAR.ZST 及 In-Memory 基准测试提供硬件级算力赋能。
