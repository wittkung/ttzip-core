# Feature Specification: Google Snappy 原生引擎深度剖析与架构集成 (Google Snappy Native Engine Analysis & Architecture Integration)

**Feature Branch**: `083-snappy-native-engine-analysis-and-integration`  
**Created**: 2026-08-18  
**Status**: Ready for Planning  
**Input**: User description: "[google/snappy](https://github.com/google/snappy) BSD 3-Clause Mac: Clang C++17 Win: MSVC C++17 提供高稳定性、无不可信崩溃的 SNAPPY 格式原生解压缩支持。详细看看相关内容我们是怎么实现的，这个库又是怎么实现的，比我们真的更快更好吗，我们可以怎么利用 /speckit-specify"

---

## Clarifications

### Session 2026-08-18
- **Q1: TTZip 现有 Snappy 实现的核心痛点与缺陷是什么？**
  - **A**: 当前 TTZip 在 `ArchiveWriter+Dispatch.swift` 与 `ttzip_tar_native.c` 中对 `format == .snappy` 依赖了 libarchive 的 `archive_write_add_filter_program(a, "snappy")` 外部进程管道。由于 libarchive 源码中原生未内置 Snappy 过滤器，macOS 默认亦未附带 `snappy` 或 `snzip` CLI，导致在常规系统及 Mac App Store 沙盒环境（`-DMAS_BUILD`）下无法创建或解压 Snappy 归档，且在 `AllFormatsAndAdvancedParametersMatrixTests.swift` 中被迫跳过单测。
- **Q2: 引入 Google 官方 `google/snappy` 库的核心价值与对比维度是什么？**
  - **A**: 
    1. **100% 进程内纯原生 (Zero CLI Fork)**：将 Google Snappy C++17 / C 静态绑定直接嵌入 `CTTZipBridge`，彻底根除子进程派生，完美满足 MAS 严苛沙盒要求与跨平台纯静态分发。
    2. **不可信输入零崩溃免疫 (Untrusted Crash Immunity)**：官方 Snappy 解压器具备严格的数学边界确界、单调指针推进与 Buffer 溢出防御，通过了 Google 工业级 Fuzzing 验证，防御恶意构造压缩包导致的崩溃或内存越界。
    3. **极致吞吐 (Extreme Throughput)**：利用 4 字节哈希快速探测、SWAR / 64-bit 宽字匹配查找、Wild Copy 非对齐快速拷贝，实现单核 350~900 MB/s 压缩与 1500~4500 MB/s 极速解压。
    4. **流式分块帧格式 (Framing Format, `.sz` / `.tar.sz`) 与硬件 CRC32C**：支持标准 Snappy Framing 规范（Stream Identifier `\xff\x06\x00\x00sNaPpY`，64KB 块流），结合 Apple Silicon ARM64 PMULL 硬件 CRC32C 校验，提供高吞吐分块并行解压缩。
- **Q3: 本 Feature 的落地范围与 API 契约设计？**
  - **A**: 深度完成官方库与现有链路技术剖析报告；在 `CTTZipBridge` 中建立原生 Snappy 块与流式帧编解码接口；在 `TTZipCore` 中提供标准 Swift 流式编解码器；打通 TAR.SZ 原生管道；解除 `AllFormatsAndAdvancedParametersMatrixTests.swift` 跳过限制并补齐全量回归与模糊测试。

---

## User Scenarios & Testing

### User Story 1 - 深度技术剖析与底层原理对比报告 (Priority: P1)

系统架构师与开发者需要获得一份详尽的技术研究与架构对比报告，全面对比 TTZip 现有 Snappy 链路与官方开源 `google/snappy` 库的底层算法原理（Token 编解码、哈希表匹配查找、SWAR / 宽字非对齐拷贝、Framing Format 帧格式、CRC32C 校验、内存边界防御模型），清晰回答现有实现机制、官方库实现原理、性能与稳定性差距，以及在 TTZip 中的利用路径。

**Why this priority**: 是理解现有缺陷、确立重构方向、进行架构决策与落地的核心理论与事实依据。

**Independent Test**: 通过完整的架构分析文档与代码审计，全面覆盖当前 TTZip 代码链路与官方 Snappy 实现机制。

**Acceptance Scenarios**:
1. **Given** TTZip 现有代码库与官方 `google/snappy` 开源仓库，**When** 执行架构与算法审查，**Then** 输出涵盖数据布局、压缩/解压热路径、内存模型、多线程并发、安全性与平台特化对比的深度分析报告。
2. **Given** 现有基于 `archive_write_add_filter_program` 的外部管道与官方纯静态库，**When** 进行对比评估，**Then** 明确指出进程派生开销、沙盒阻断、吞吐差异与不可信输入崩溃防御边界。

---

### User Story 2 - 原生 C 桥接引擎强化与 100% 进程内编解码 (Priority: P1)

TTZip 核心引擎需要提供 100% 进程内、零外部 CLI 调用的原生 Snappy 内存块与分块帧流编解码能力，支持直接调用 Google Snappy 原生 C/C++ 静态绑定，支持原始块（Raw Block）与标准帧流（Framing Format）的高速处理，彻底根除对外部 `snzip`/`snappy` 二进制命令的依赖。

**Why this priority**: 核心引擎数据平面的基石，解决 MAS 沙盒环境下无法处理 Snappy 格式的致命缺陷。

**Independent Test**: 运行单测验证内存数据在原生 Snappy 引擎下的压缩与解压正确性，解压数据与源数据 100% 比特级一致。

**Acceptance Scenarios**:
1. **Given** 任意大小的内存数据块（从数字节到数百 MB），**When** 调用原生 Snappy 编解码接口，**Then** 能够以超过门禁底线的吞吐完成处理，且还原数据完全一致。
2. **Given** 包含标准 Snappy 帧头（`\xff\x06\x00\x00sNaPpY`）的流式数据，**When** 调用帧流解压器，**Then** 逐块解析并验证 CRC32C 校验码，无损输出原始字节流。

---

### User Story 3 - TAR.SZ 原生归档与流式管道穿透 (Priority: P1)

用户在压缩或解压 `.sz` / `.tar.sz` / `.snappy` 归档时，系统能够通过零拷贝内存管道与多块并发调度，实现端到端的高性能流式打包与解包，无缝适配 Finder、UI 进度展示及 CLI 管道命令。

**Why this priority**: 用户日常操作的核心格式支持，实现全 16 种归档格式无死角原生闭环。

**Independent Test**: 对大文件与多目录结构创建 TAR.SZ 归档并执行完整解压验证。

**Acceptance Scenarios**:
1. **Given** 多文件目录树，**When** 用户选择 Snappy 格式进行压缩，**Then** 系统生成符合标准 Framing 规范的 `.tar.sz` 归档文件。
2. **Given** 标准 `.tar.sz` 归档，**When** 用户执行解压操作，**Then** 系统自动识别帧头并快速还原所有文件与目录元数据，无需外部工具辅助。

---

### User Story 4 - 不可信与损坏流的 100% 内存安全防御 (Priority: P2)

当用户打开被恶意篡改、截断或损坏的 Snappy 文件时，解压引擎必须严格保证不发生段错误（SIGSEGV）、堆缓冲区溢出（Heap Out-of-Bounds Write）或无限循环，能够安全捕获异常并向 UI/CLI 抛出精确的校验错误码。

**Why this priority**: 满足 macOS 原生安全基准与企业级防崩溃稳健性要求。

**Independent Test**: 针对伪造的 Magic Header、畸形 Chunk Length、CRC32C 不匹配及截断的 Snappy 数据包执行逆向注入与模糊测试。

**Acceptance Scenarios**:
1. **Given** 随机损坏或畸形构造的 Snappy 数据流，**When** 执行解压操作，**Then** 引擎在 < 1ms 内检测出非法数据并安全返回错误状态码，进程零崩溃。
2. **Given** CRC32C 校验码被篡改的数据块，**When** 流式解压器读取该 Chunk，**Then** 立即终止当前流并报告校验失败，杜绝污染输出文件。

---

### User Story 5 - 性能门禁与全格式 PK 回归 (Priority: P2)

确保所有与 Snappy 相关的单测、性能基准测试在 Debug 与 Release 模式下均能稳定通过，维持全格式历史最优峰值，且不发生任何性能倒退。

**Why this priority**: 遵守工程宪法与性能铁律，确保重构与优化安全可信。

**Independent Test**: 执行 `XCTestPerformanceMeasureTests` 与 `AllFormatsPkSuiteTests`。

**Acceptance Scenarios**:
1. **Given** Snappy 进程内流式压缩/解压测试，**When** 执行性能门禁测试，**Then** 满足历史最优性能基准（解压吞吐 >= 4,500 MB/s）。
2. **Given** 全格式回归矩阵，**When** 运行测试集，**Then** 解除 `testFormat_SNAPPY()` 的跳过标记，525+ 项测试全部绿灯通过。

---

## Edge Cases

- **极小数据块 (< 16 字节)**：处理不可压缩小块时的容量计算与 `MaxCompressedLength` 安全溢出余量预留。
- **损坏或截断的 Framing Stream**：Chunk Length 超过剩余流大小或大于 64KB 限制时，立即安全报错退出。
- **非法 Stream Identifier**：首个 Chunk 必须为 `0xff` 且载荷严格匹配 `sNaPpY`，非合法帧头时拒绝作为 Framed Stream 处理。
- **超大数据流 (> 2GB / > 4GB)**：64 位偏移量向 32 位安全窄化保护（Clamp to SSIZE_MAX），分块流式处理杜绝单块超限。
- **零长度输入与空归档**：安全短路返回空数据，不执行无意义的动态内存分配。
- **CRC32C 硬件指令兼容性**：在 Apple Silicon ARM64 上利用 PMULL 硬件指令加速 CRC32C，在 x86_64 或无指令支持环境下安全回退至查表法。

---

## Requirements

### Functional Requirements

- **FR-001**: 系统必须提供完整的 Google Snappy 官方开源库与 TTZip 现有链路深度技术对比剖析报告（涵盖算法、架构、吞吐、安全性与工程利用路径）。
- **FR-002**: 系统必须支持基于 100% 进程内纯 C/C++ 静态绑定的原生 Snappy 块编解码能力（`ttzip_snappy_compress` / `ttzip_snappy_decompress`）。
- **FR-003**: 系统必须完整支持 Snappy Framing 规范（`framing_format.txt`），提供标准的流式分块写入器与读取器。
- **FR-004**: 编解码过程必须集成 Castagnoli CRC32C 校验，支持 Apple Silicon ARM64 硬件加速。
- **FR-005**: 系统必须消除对外部 `snzip`/`snappy` CLI 的任何依赖，确保在 MAS 沙盒模式（`-DMAS_BUILD`）下全功能可用。
- **FR-006**: 系统必须支持 `.sz` 与 `.tar.sz` 格式的原生流式打包与解压提取。
- **FR-007**: 系统必须通过逆向注入与畸形流模糊测试，证明对不可信损坏数据的零崩溃免疫能力。
- **FR-008**: 系统必须解除 `AllFormatsAndAdvancedParametersMatrixTests.swift` 中对 Snappy 格式的跳过标记，并在全量测试中保持全绿通过。

---

## Key Entities

- **SnappyBlockEngine**: 负责原始字节块（Raw Block）的高速编解码中枢，提供容量预估、有效性校验与内存到内存的零拷贝操作。
- **SnappyFramingStream**: 负责遵循 Snappy 官方 Framing 格式的分块流式处理器，包含 Stream Identifier、Chunk 头部解析与封包、CRC32C 校验与滑窗缓冲管理。
- **SnappyTarPipeline**: 负责连接 TAR 归档引擎与 Snappy 压缩流的双向流式传输中继，支持分块并行与背压控制。
- **SnappyCRC32CChecksum**: 硬件加速的 Castagnoli CRC32C 校验计算器，支持 ARM64 PMULL / CRC32 指令与软件查表回退。

---

## Success Criteria

### Measurable Outcomes

- **SC-001**: 产出包含算法、数据结构、架构设计、安全性与性能多维度的深度对比研究报告。
- **SC-002**: 实现 100% 进程内原生 Snappy 编解码引擎，彻底消除 `archive_write_add_filter_program` 与外部 CLI 调用。
- **SC-003**: Snappy 内存解压吞吐达到门禁底线（Debug 模式 >= 4,500 MB/s，Release 模式 >= 6,000 MB/s）。
- **SC-004**: 100 组畸形损坏数据包与截断流测试 100% 安全捕获，进程零崩溃、零内存越界。
- **SC-005**: 全量测试套件（525+ tests）与 `AllFormatsAndAdvancedParametersMatrixTests` 100% 绿灯通过。

---

## Assumptions

- 编译环境支持 C++17 标准（macOS Apple Clang 15+，Windows MSVC 2019+）。
- 外部归档交换遵循官方 Snappy Framing 规范（RFC 兼容 Framing Format），块大小默认为 64KB。
- 硬件 CRC32C 加速优先使用 ARMv8 / ARMv8.1 指令集。
