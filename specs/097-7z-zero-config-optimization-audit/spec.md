# Feature Specification: 097-7z-zero-config-optimization-audit

**Feature Branch**: `097-7z-zero-config-optimization-audit`  
**Created**: 2026-08-18  
**Status**: Draft  
**Input**: User description: "好好分析所有涉及 7z 的相关代码，我们的所有优化是否都已经配置到生产代码中了，是否遵守了反配置膨胀，能够自动使用最佳配置"

---

## Executive Summary & Architecture Overview

本规范针对 TTZip 代码库中所有涉及 **7z (7-Zip)** 格式的编解码管道、C 静态桥接层 (`CTTZipBridge`)、Swift 核心调度层 (`TTZipCore`)、UI 预设 (`TTZipApp`) 以及命令行工具 (`TTZipCLI`) 进行全栈架构审计与最佳配置固化。确保：
1. **所有底层极致优化**（Fast-LZMA2 多核切块、SWAR/Radix 匹配查找器、ARM64 NEON AES-256 加解密、NEON SHA-256 硬件 KDF 派生、动态信息熵检测、APFS 预分配、两级 `mkdir_p` 缓存）已 100% 接通生产调用热路径；
2. **严格遵守反配置膨胀原则 (Zero Configuration Creep)**：严禁将核心性能决策作为复杂配置项抛给用户或上层调用方，系统必须根据 CPU 拓扑（P/E 核心数）、内存容量、输入数据大小与信息熵分布，**全自动透明选择最佳配置**；
3. **消除孤岛与旁路残留**：打通 `ArchiveReader` 零拷贝 7z 快速检视、Native 7z 并行解压与压缩管道，确保全生态 100% 原生 In-Process 运行（零 CLI 子进程依赖）。

---

## Clarifications

### Session 2026-08-18

- Q: 是否需要在 UI/CLI 中为 7z 暴露高级分块大小（Block Size）或匹配算法选项？ → A: 不需要。严格遵循反配置膨胀（Zero Configuration Creep），系统基于硬件拓扑和输入数据自适应决策，保持零配置最高性能。
- Q: 7z 不可压缩数据（高熵）如何处理？ → A: 自动在压缩前执行动态信息熵采样，熵 > 7.90 时自动无感降级为 Store (Level 0) 格式，防止 CPU 算力浪费。
- Q: 7z 目录检视如何提升响应速度？ → A: 优先走零拷贝 mmap 原生 Header 数据库解析通道，消除 libarchive 或解压到临时目录的开销。

## User Scenarios & Testing *(mandatory)*


### User Story 1 - 零配置全自动极速 7z 压缩与自适应分流 (Priority: P1)

作为普通用户或开发人员，通过 GUI、CLI 或 API 压缩文件为 `.7z` 格式时，无需手动配置线程数、字典大小、块大小或快速模式开关；系统全自动根据文件特性（大小、稀疏性、信息熵）和硬件核心拓扑，自适应选用最佳匹配引擎与切块参数，实现极致吞吐。

**Why this priority**: 7z 压缩是用户最高频的生产力操作之一，自适应最佳配置能让用户无需学习复杂的 7z 参数即可获得数倍于传统工具的压缩吞吐。

**Independent Test**: 使用不同特性的样本（不可压缩的视频/安装包、高压缩比文本/源码、稀疏全零文件）执行默认 7z 压缩，验证无需任何额外参数即可自动达成最佳吞吐并生成合法 7z 归档。

**Acceptance Scenarios**:
1. **Given** 用户选择一个不可压缩的 50MB 媒体文件，**When** 用户使用默认设置执行 7z 压缩，**Then** 系统自动检测到信息熵 $> 7.90$，自动降级为 Store (Level 0) 直通模式，在 $< 10\text{ms}$ 内完成打包且零 CPU 算力浪费。
2. **Given** 用户选择包含 100MB 源码与文本的文件夹，**When** 用户执行 Level 1 快速 7z 压缩，**Then** 系统自动按照 CPU 物理核心数动态划分 $8\text{MB} \sim 32\text{MB}$ 任务块，利用并行 Fast-LZMA2 与 NEON 向量化匹配查找器，吞吐稳定达到 $\ge 3,200\text{ MB/s}$。
3. **Given** 用户提供包含大面积连续零字节的稀疏文件，**When** 执行 7z 压缩，**Then** 系统通过 NEON 快速扫描感知零块并直接调用 RLE 极速通道，压缩体积 $< 1\text{KB}$ 且吞吐超 $10\text{ GB/s}$。

---

### User Story 2 - 原生硬件加速 7z 并行解压与加密直通 (Priority: P2)

作为 macOS 用户，解压任意标准 7z 归档（包含 AES-256 加密、Solid 固实流、Zstd/Deflate/LZMA1 混合编码）时，无需额外安装命令行工具，系统以 100% 进程内 C 静态引擎直接解压，并通过 ARM64 硬件指令集加速密钥派生与数据解密。

**Why this priority**: 解压是用户交互的关键路径，冷启动外部进程会带来不可接受的延迟与沙盒权限问题；硬件加速解密保证了在 macOS 上的安全与极致流畅体验。

**Independent Test**: 对加密 7z 和多块 7z 归档进行解压测试，断言 100% 走通 `ttzip_7z_extract_native_parallel_c`，验证解压数据完整性与 CRC32 校验一致。

**Acceptance Scenarios**:
1. **Given** 一个使用 AES-256 加密的 7z 归档，**When** 用户输入正确密码执行解压，**Then** 系统的 ARM64 NEON SHA-256 模块在 $\le 15\text{ms}$ 内完成 524,288 次密钥哈希派生，并通过 NEON AES-256-CBC 向量化指令高速解密数据。
2. **Given** 一个包含多个 LZMA2 / Zstd 数据块的 7z 固实归档，**When** 执行解压，**Then** 系统自动解析 Header 块字典重置标记，通过 GCD 多核并行解码各数据块，写入时利用两级 `mkdir_p` 缓存消除重复目录系统调用，解压吞吐达到 $\ge 6,600\text{ MB/s}$。

---

### User Story 3 - 归档极速检视与多卷穿透零摩擦 (Priority: P3)

作为用户，在归档浏览器或 Finder 中双击浏览 7z 归档结构或多卷分割归档（`.7z.001`）时，系统能够秒级呈现文件列表，无需将整个归档解压到临时目录，且对多卷文件能够自动透明合并定位。

**Why this priority**: 穿透浏览是桌面 App 的核心用户体验；直接基于内存映射解析 Header 避免无谓的磁盘 I/O。

**Independent Test**: 调用 `ArchiveReader.inspect` 查看大型 7z 与分卷 7z 文件列表，验证无需解压即可在 $< 5\text{ms}$ 内返回准确的条目结构、大小及加密属性。

**Acceptance Scenarios**:
1. **Given** 一个未加密的 1GB 7z 归档，**When** 用户在应用中打开浏览，**Then** 系统通过 `mmap` 零拷贝直接读取尾部 7z Header 数据库并解析文件树，耗时 $\le 5\text{ms}$，内存占用 $\le 2\text{MB}$。
2. **Given** 一组分卷归档（`.7z.001`, `.7z.002`），**When** 用户触发解压或浏览，**Then** 系统自动识别分卷序列并透明合并，正确解压出原始文件。

---

### Edge Cases

- **高熵混合内容**：当输入文件包含部分已压缩文件（如 `.zip`, `.mp4`）和部分纯文本时，分块引擎能够独立处理各块，高熵块自动降低压缩开销，文本块充分压缩。
- **空文件与空目录**：归档中包含零字节文件、空目录树、深层嵌套空路径时，7z Header 能够正确记录 `kEmptyStream` 与 `kEmptyFile` 属性，解压时完整恢复目录层级。
- **超大字典与内存约束**：处理超大文件时，字典大小根据文件实际体积动态对齐（$64\text{KB} \sim 32\text{MB}$），单块内存常驻严格控制在 $\le 64\text{MB}$，杜绝 OOM。
- **错误密码与损坏 Header**：输入错误密码或遇到被截断的损坏文件时，快速返回明确的 `TTZIP_ERR_INVALID_PASSWORD` 或 `TTZIP_ERR_CORRUPT_HEADER`，严禁悬挂或崩溃。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 在 7z 压缩热路径中默认启用基于硬件 CPU 拓扑的动态多核切块（`ttzip_create_7z_lzma2_native_c`），禁止上层传入硬编码的低线程数。
- **FR-002**: 系统 MUST 在 7z 压缩前自动执行轻量级动态信息熵采样（`ttzip_estimate_buffer_entropy_dynamic`）；当信息熵 $> 7.90$ 时自动无感切换至 7z Store 模式，防止算力空耗。
- **FR-003**: 系统 MUST 默认在单文件 $\ge 1\text{MB}$ 时启用 `mmap` 零拷贝读取与 `madvise` 顺序预读，消除内核页重复拷贝。
- **FR-004**: 系统 MUST 默认启用 ARM64 NEON SIMD 指令集加速全部关键环节：零块快速探测（`ttzip_is_block_all_zero_neon`）、CRC32 计算、AES-256 加解密与 SHA-256 密钥派生。
- **FR-005**: 系统 MUST 保证 7z 原生并行解码器（`ttzip_7z_decode_payload_parallel`）支持 LZMA2、Zstandard、Deflate 及 Store 等主流编码方法的自动鉴别与并行调度。
- **FR-006**: 系统 MUST 在磁盘写入热路径中使用两级 `mkdir_p` 缓存机制（L1 字符串缓存 + L2 64-slot 哈希表），减少文件系统重复目录创建调用。
- **FR-007**: 系统 MUST 在 `ArchiveReader` 检视逻辑中优先通过零拷贝 `mmap` 7z Header 数据库读取条目（`ttzip_native_inspect_archive` / `NativeSevenZipEngine.inspectSevenZip`），避免回退到全量解压或外部进程。
- **FR-008**: 系统 MUST 遵守反配置膨胀铁律，GUI 和 CLI 默认配置即为最佳配置，不向用户暴露晦涩的内部调度参数（如块大小、匹配查找器类型、多线程阈值等）。

### Key Entities

- **SevenZipArchivePackage**: 表示 7z 归档实体，包含 32 字节 Signature Header、压缩数据流集合（Packed Streams）、编码属性（Coders / Filters）及 Header 数据库（文件元数据、CRC32、时间戳）。
- **LZMA2BlockTask**: 表示一个独立的并行 LZMA2 压缩/解压任务单元，包含解压偏移量、目标容量、字典规格、信息熵特征及状态标志。
- **SevenZipCryptoSession**: 表示 7z 加密会话上下文，包含 256 位 AES 密钥、16 字节 IV、Salt 盐值及循环幂次（`num_cycles_power` = 19）。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 7z Level 1 极速压缩在标准测试语料（10MB/50MB）下的吞吐稳定达到 $\ge 3,200\text{ MB/s}$（Debug）/ $\ge 3,900\text{ MB/s}$（Release）。
- **SC-002**: 7z 极速解压吞吐稳定达到 $\ge 6,600\text{ MB/s}$（Debug）/ $\ge 7,200\text{ MB/s}$（Release）。
- **SC-003**: 7z AES-256 硬件 KDF 派生耗时稳定保持在 $\le 17\text{ ms}$（Debug）/ $\le 15\text{ ms}$（Release）。
- **SC-004**: 7z 归档目录检视响应时间 $\le 5\text{ ms}$，内存分配峰值 $\le 2\text{ MB}$。
- **SC-005**: 100% 格式测试与回归用例通过，零外部 CLI 进程强依赖，零性能倒退（$\Delta < 3.0\%$）。

---

## Assumptions

- 运行环境为 macOS 14.0+，优先在 Apple Silicon (M1/M2/M3/M4 系列) 上启用 NEON SIMD 硬件指令加速。
- 所有 7z 归档遵循 7z Format Specification 7z 签名（`0x37 0x7A 0xBC 0xAF 0x27 0x1C`）与 LZMA2 标准。
- 采用反配置膨胀原则，默认策略为自适应最大性能与最优压缩率权衡，高级选项仅作为特殊开发调试扩展。
