# Feature Specification: 7z 全链路原生压缩流算法全景调研与自主无依赖引擎演进规范

**Feature ID**: `108-7z-native-compression-pipeline`  
**Feature Branch**: `108-7z-native-compression-pipeline`  
**Created**: 2026-08-19  
**Status**: Draft  
**Input**: User description: "全面调研我们的 7z 格式 下的压缩流算法情况，看看底层是怎么实现的，有哪些是调用外部库的 我觉得针对 7 z格式来说，我们需要尽量摆脱库的依赖，完全自己实现并做调优，这样才能真正大幅度领先，还有很多可以优化的点，并且看看 zip 的底层实现有哪些可以复用的 /speckit-specify"

---

## 1. Executive Summary & Goals

7z 作为高压缩比容器标准，是 TTZip 面向 macOS 14+ 核心架构中的两大主力格式之一。当前 TTZip 在 7z 格式上已经实现了部分自主能力（如极速 Store 模式 28,000+ MB/s、ARM64 硬件 SHA-256 KDF 派生、ARM NEON AES-256 并行解密、ARM64 BCJ 向量化分支过滤以及基于内存映射的零拷贝解析）。然而，在核心的 **LZMA2 压缩与解压热路径** 上，仍深度依赖外部 C 静态库 `liblzma`（XZ Utils）、`fast-lzma2`（FL2 外部源码库）、系统 `libcompression` 以及 `libarchive` 兜底。

为了彻底摆脱外部库依赖、消除外部库冗余抽象开销、实现 100% 进程内纯原生自研算法并对 Apple Silicon 进行极限调优（NEON 向量化、无锁并发环形字典、SWAR 匹配长度计算、自适应 Range Coder），本特性旨在确立 **7z 全链路压缩流算法全景调研与自主无依赖引擎演进规范**：
1. **全面底线调研与外部依赖全景审计**：逐行审计 `Sources/TTZipCore/SevenZip/` 与 `Sources/CTTZipBridge/` 中所有 7z 压缩/解压/解析流程，明确哪些是自主实现、哪些调用了 `liblzma` / `fast-lzma2` / `libarchive` / `libcompression`；
2. **ZIP 底层自研成果可复用性矩阵**：深度比对 ZIP 引擎在 `native_deflate`、SWAR/NEON 匹配查找（HC3/HC4/Double-Fast）、APFS `fstore_t` 预分配、无锁多核分块调度（`ZipExtremeBlockWriter`）与直接 I/O 上的成熟范式，制定向 7z 迁移的通用数据平面基础设施；
3. **纯自研 7z/LZMA2 引擎演进蓝图**：规划完全脱离外部库的自研 LZMA2 极速与极限编解码流水线架构，包含全自研 Range Coder、分支预测优化、环形历史字典与 ARM64 NEON 向量化指令加速；
4. **性能指标与质量门禁确立**：确立覆盖 7z 极速（Level 1）、均衡（Level 5）、极限（Level 9）及加密/解压的全矩阵性能底线，确保自主化改造后不仅零性能倒退，且实现大幅超越。

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - 7z 压缩流底层实现全景审计与外部依赖拓扑透视 (Priority: P1)

作为核心架构师与系统工程师，我需要地毯式摸清当前 TTZip 中 7z 格式下所有压缩流算法的底层调用栈与外部库依赖关系，准确识别所有性能瓶颈与外部黑盒约束。

**Why this priority**: 这是自主化改造的前提与事实依据。没有精确的依赖和热路径调用分析，盲目重写极易引入性能倒退或遗漏边界格式支持。

**Independent Test**: 能够通过物理源码核实与符号交叉比对，输出 100% 真实的 7z 压缩/解压/加解密/容器解析全景调用图与外部依赖矩阵表。

**Acceptance Scenarios**:
1. **Given** `Sources/TTZipCore/SevenZip/` 与 `Sources/CTTZipBridge/` 的源码，**When** 进行全量符号与依赖扫描，**Then** 清晰列出每个 7z 功能点（Header 解析、Store 存储、Level 1-6 LZMA2 压缩、Level 7-9 极限压缩、LZMA1/LZMA2 解压、AES-256 加解密、BCJ 过滤）的底层实现文件、行号以及是否依赖 `liblzma`、`fast-lzma2`、`libarchive` 或系统库。
2. **Given** 现有的 `Vendor/` 与 `Sources/CTTZipBridge/`，**When** 梳理外部依赖点，**Then** 明确指出当前依赖外部库引入的堆分配开销、锁竞争及无法内联 SIMD 优化的具体瓶颈点。

---

### User Story 2 - ZIP 引擎底层优秀架构向 7z 迁移与复用设计 (Priority: P1)

作为算法优化工程师，我需要评估并提取 ZIP 引擎底层（`ZipExtremeBlockWriter`、`native_deflate`、`ZipDirectoryScanner`、NEON 匹配查找器）中已验证的高性能设计模式与 C 基础设施，复用至 7z 引擎。

**Why this priority**: ZIP 模块在 TTZip 中已达到极高吞吐（Store 7,500+ MB/s、Level 1 2,000+ MB/s、并行 Deflate 1,500+ MB/s），其零拷贝架构、无锁并发环形池、SWAR/NEON 匹配查找与 APFS 预分配机制可以直接赋能 7z。

**Independent Test**: 形成 ZIP 基础设施复用清单与接口契约规范，证明各模块在 7z 场景下的可迁移性与收益。

**Acceptance Scenarios**:
1. **Given** ZIP 引擎中的 `ttzip_hc4_neon.c` 与 `ttzip_hybrid_match_len_neon`，**When** 应用于 7z LZMA2 匹配查找，**Then** 证明其可以无缝支持 LZMA2 的 2-byte、3-byte、4-byte 哈希表与最长 273 字节匹配探测。
2. **Given** ZIP 引擎中的 APFS 空间预分配与多线程 `pwrite` 分块写入模型，**When** 应用于 7z Solid 归档生成，**Then** 消除中间磁盘写入碎片与动态扩容系统调用。

---

### User Story 3 - 纯自研 0-外部依赖 7z/LZMA2 引擎全套架构规范 (Priority: P2)

作为核心开发人员，我需要一套完整的 100% 自研 7z 引擎架构方案（涵盖容器解析/生成、LZMA2 编解码、Range Coder、BCJ 变换与硬件加密），摆脱所有第三方静态库。

**Why this priority**: 摆脱 `liblzma` 与 `fast-lzma2` 依赖能够彻底消除外部 C 结构体封装与间接调用开销，并能够针对 Apple Silicon M 系列芯片深度定制 NEON 向量寄存器分配与分支预测。

**Independent Test**: 定义纯自研 7z 引擎的模块划分、API 契约、数据结构与关键算法伪代码，并可在后续实现中独立替换外部库。

**Acceptance Scenarios**:
1. **Given** 7z 容器格式规范与 LZMA2 流规范，**When** 设计自研引擎，**Then** 完整定义自研 Range Coder（`ttzip_rc_*`）、状态转移机（12 状态模型）、概率更新表（2048 概率模型）、环形字典缓冲区以及自研 LZMA2 解码器。
2. **Given** 针对 Level 1 极速与 Level 9 极限压缩，**When** 设计算法分级，**Then** 极速档位采用 Double-Fast / HC3 匹配查找与快速 Range 编码，极限档位采用自研多线程 Radix 匹配查找与最优解析（Optimal Parser / Price Calculation）。

---

### User Story 4 - 性能基准、门禁与回归验证矩阵 (Priority: P3)

作为质量保证与性能工程师，我需要确立 7z 自研引擎与当前引擎及竞品（7zz、Keka、The Unarchiver）的全矩阵基准测试规范与硬性性能门禁。

**Why this priority**: 任何算法重写必须保证 100% 格式兼容性与确凿的性能领先，杜绝性能倒退。

**Independent Test**: 运行全格式性能门禁与 7z 专项测试套件，验证压缩率与吞吐指标全面达标。

**Acceptance Scenarios**:
1. **Given** Silesia Corpus、Enwik9 与大文件（10MB/50MB/500MB）样本，**When** 执行基准对比，**Then** 7z Level 1 压缩吞吐达到 $\ge 3,500\text{ MB/s}$，7z 解压达到 $\ge 7,500\text{ MB/s}$，且压缩比与标准 7-Zip 差异 $\le 1.0\%$。
2. **Given** 加密与分卷场景，**When** 执行自研引擎解压，**Then** 原生系统 `7zz` 能够 100% 正确解压自研引擎生成的归档，反之自研引擎也能 100% 正确解压外部生成的归档。

---

## 3. Edge Cases & Boundary Handling

- **EC-001: 极小文件与全零/低熵数据流**：当文件大小 $< 4\text{KB}$ 或包含大段全零块时，自研 LZMA2 引擎必须短路至 2MB 极速全零块编码（`0xE0` / `0x80` 控制字节）或 Store 原始块（`0x01` / `0x02`），避免无谓的 Range Coder 状态迭代。
- **EC-002: 高熵不可压缩数据（如已压缩视频、JPEG、加密数据）**：自适应熵估算器检测到熵值 $> 7.90$ 时，必须自动降级到 Store 原始块直通，防止压缩后体积膨胀并实现 $10,000+\text{ MB/s}$ 极速穿透。
- **EC-003: 跨块边界历史字典预热 (Cross-Block Dictionary Preconditioning)**：在多线程并行压缩连续数据块时，每个子块必须继承前序块末尾 $64\text{KB} \sim 8\text{MB}$ 的字典历史，避免块边界导致压缩比严重恶化。
- **EC-004: 巨型文件与内存受限环境**：单文件超过数 GB 时，严禁全量一次性读取，必须采用 mmap 分页按需加载（`MADV_WILLNEED` / `MADV_SEQUENTIAL`）与分块流式处理，单任务内存常驻限制在 $\le 64\text{MB} \sim 128\text{MB}$。
- **EC-005: 损坏或恶意构造的 7z 头部 (Fuzzing & Security)**：对畸形 Varint、超长文件名、重叠 Folder 编码和循环 BindPair 进行边界防御与 `__builtin_add_overflow` 算术溢出防护，返回确界错误码而非崩溃。

---

## 4. Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (调研与审计)**: 建立 7z 现有全部 14 个 Swift 源文件与 16 个相关 C 源文件的全量资产与依赖映射表，详细标明每一个模块的实现归属（自研 / `liblzma` / `fast-lzma2` / `libarchive` / `libcompression`）。
- **FR-002 (ZIP 复用分析)**: 梳理 ZIP 引擎中 5 大核心可复用基础设施（`ttzip_hc4_neon` 匹配查找器、`ttzip_hybrid_match_len_neon` SWAR/NEON 长度计算器、APFS 预分配、无锁多核并发分块、位流/内存池管理），制定 7z 迁移复用契约。
- **FR-003 (纯自研 Range Coder 规范)**: 设计符合 RFC/7z 标准的纯自研 Range Coder，包含无分支（Branchless）位编码、8-bit 字节树快速编码、Direct Bit 批量发射与 Range 归一化（Normalization）。
- **FR-004 (纯自研 LZMA2 解码引擎)**: 设计完全脱离 `liblzma` 的自研 LZMA2 解码器，支持 0x00 结束标记、0x01/0x02 未压缩块直通、0x80..0xFF LZMA 块解码、状态机重置与属性更新。
- **FR-005 (纯自研 LZMA2 编码引擎 - 极速档 Level 1-2)**: 设计基于 NEON 硬件加速的双向匹配查找器（Double-Fast / HC3）与快速贪婪/延迟解析器（Greedy/Lazy Parser），目标单核编码吞吐 $\ge 300\text{ MB/s}$，多核并发 $\ge 3,500\text{ MB/s}$。
- **FR-006 (纯自研 LZMA2 编码引擎 - 均衡与极限档 Level 5-9)**: 设计基于自研 Radix / BT4 匹配查找器与代价模型（Price Table）的最优解析器（Optimal Parser），支持大字典（8MB - 64MB）与深度匹配搜索。
- **FR-007 (ARM64 硬件原生安全流)**: 保持并固化纯自研的 ARM64 SHA-256 硬件 KDF 派生（`vsha256hq_u32`）与 ARM NEON AES-256-CBC 加解密，确保凭据内存 `ttzip_secure_zero` 物理擦除。
- **FR-008 (ARM64 BCJ 向量化分支过滤)**: 固化纯自研的 ARM64 B/BL 指令跳转地址绝对化/相对化转换器，提升可执行二进制文件的压缩比。
- **FR-009 (双向差分预言机测试)**: 自研引擎生成的 7z 归档必须能被 macOS 系统 `7zz`、`tar`、`unar` 完美解压；反之亦然。

---

## 5. Key Entities *(Data Model & Schema)*

- **Entity: `SevenZipStreamAuditRecord`**:
  - `componentName`: 模块名称（如 `LZMA2Encoder`, `HeaderParser`, `AESDecryption`）。
  - `sourceFile`: 物理源码文件路径与行号。
  - `currentBackend`: 当前底层依赖（`In-House C`, `liblzma.a`, `fast-lzma2`, `libarchive.a`, `System libcompression`）。
  - `dependencyType`: 依赖类别（`None (Pure Native)`, `Static Vendor Archive`, `System Library`）。
  - `eliminationFeasibility`: 自主重构可行性（`High`, `Medium`, `Completed`）。
  - `zipReusableAsset`: 对应的 ZIP 可复用组件。

- **Entity: `LZMA2StreamContext`**:
  - `dictSize`: 字典大小（$4\text{KB} \sim 64\text{MB}$）。
  - `rangeEncoder`: 自研 Range Coder 状态结构体（`low`, `range`, `cache`, `cache_size`, `out_buf`）。
  - `probabilities`: LZMA 概率状态表（`is_match`, `is_rep`, `is_rep0`, `match_len`, `literal` 等）。
  - `historyBuffer`: 环形字典历史缓冲区指针与掩码。
  - `state`: 当前 LZMA 状态（0..11）。

- **Entity: `SevenZipContainerHeader`**:
  - `signature`: 7z 签名（`0x37 0x7A 0xBC 0xAF 0x27 0x1C`）。
  - `version`: 主版本号（`0x00 0x04`）。
  - `nextHeaderOffset`: 主元数据偏移量。
  - `nextHeaderSize`: 主元数据大小。
  - `nextHeaderCRC`: 主元数据 CRC-32 校验码。

---

## 6. Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001 (外部库脱耦度)**: 7z 核心热路径（Header 读写、Store 写入、Level 1-6 压缩、LZMA2 解压、AES 加解密）外部静态库依赖减少到 **0**，完全由 `Sources/CTTZipBridge/` 内的高性能 C 代码与 NEON 汇编接管。
- **SC-002 (压缩性能跃升)**:
  - 7z Level 1 极速压缩（10MB 样本）：吞吐从当前的 $\ge 3,200\text{ MB/s}$ 提升至 **$\ge 3,800\text{ MB/s}$**；
  - 7z Level 5 均衡压缩（10MB 样本）：吞吐从当前的 $\ge 480\text{ MB/s}$ 提升至 **$\ge 600\text{ MB/s}$**；
  - 7z Store 极速打包（50MB 样本）：吞吐保持在 **$\ge 25,000\text{ MB/s}$** 历史巅峰。
- **SC-003 (解压性能跃升)**:
  - 7z 极速解压（10MB 样本）：吞吐从当前的 $\ge 6,600\text{ MB/s}$ 提升至 **$\ge 7,500\text{ MB/s}$**；
  - 7z AES-256 解密解压：硬件 KDF 派生耗时 $\le 15\text{ ms}$，解密解压吞吐 $\ge 2,500\text{ MB/s}$。
- **SC-004 (双向差分兼容性 100%)**: 自研 7z 引擎生成的所有归档在官方 `7zz x` 下测试通过率 **100%**，在 `unar` 下测试通过率 **100%**。
- **SC-005 (测试套件零倒退)**: 全量 525+ 单元测试与 46 项基准测试全绿通过，`XCTestPerformanceMeasureTests` 门禁 100% 达标。

---

## 7. Assumptions & Dependencies

- **Platform Target**: macOS 14.0+，以 Apple Silicon（ARM64 / NEON / ARMv8 Crypto Extensions）为第一公民，兼容 x86_64。
- **In-Process Invariant**: 严格遵守 100% 进程内 C 静态绑定，生产代码绝不使用 `posix_spawn` / `Process` 调用外部 `7zz` 二进制，满足 Mac App Store 沙盒（MAS Sandbox）要求。
- **Memory Safety & Zero-Allocation**: 热循环中严格杜绝 `Data(count:)` 内核清零页中断，使用未初始化的裸指针与固定栈缓冲区。
