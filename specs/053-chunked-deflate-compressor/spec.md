# Feature Specification: Full-Matrix libdeflate Engine Architecture (Chunked Streaming, Upstream Upgrade & Windows Cross-Platform Matrix)

**Feature Directory**: `specs/053-chunked-deflate-compressor`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "P0 超大文件分块流式压缩器 (Chunked Stream Compressor)；P1 Vendor/libdeflate 升级与构建自动化 (v1.21+, ARMv8.2-A+crypto & AVX2)；P2 Windows 跨平台 CTTZipBridge 静态库矩阵 (CMake/MSVC)。全部规划进统一规范与执行流水线。"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 超大单文件低内存常驻压缩 (Priority: P0)

用户需要将单个体积巨大（如 1GB、4GB 乃至 10GB 以上的虚拟机镜像、高清视频或数据库 dump 文件）的文件压缩打包为 ZIP 归档。在压缩过程中，系统不占用巨大内存，常驻内存增量严格受限在 64MB 以内，避免在内存紧张的设备上触发内核交换或导致系统卡顿。

**Why this priority**: 解决当前 Whole-buffer 机制在大文件压缩时内存占用随文件大小 1:1 线性膨胀的根本隐患，消除 OOM 风险，确保极端大文件下的系统稳定性。

**Independent Test**: 传入一个 4GB 的大文件执行 ZIP 压缩，实时采样进程物理常驻内存（RSS），验证峰值内存增量 $\le 64\text{MB}$ 且成功生成合法归档。

**Acceptance Scenarios**:
1. **Given** 一个未压缩体积为 4GB 的单文件输入，**When** 用户触发 ZIP 格式压缩（Level 1 或 Level 6），**Then** 压缩任务平稳运行完成，进程常驻内存峰值始终不超过基线 + 64MB，且生成的文件 SHA-256 还原后与原始文件一致。
2. **Given** 待压缩文件大小 $\le 256\text{MB}$，**When** 引擎处理该文件，**Then** 自动命中 Whole-Buffer TLS Fast-Path，维持全量向量化的高性能吞吐。

---

### User Story 2 - Upstream libdeflate v1.21+ 升级与双架构自动化构建 (Priority: P1)

系统需要升级内置的 `Vendor/libdeflate` 至官方最新稳定版（v1.21+），并提供一套全自动构建脚本，能够一键在 macOS 上交叉编译产出包含 Apple Silicon (ARMv8.2-A+crypto/PMULL) 与 Intel x86_64 (AVX2/BMI2) 的 Universal 2 静态库 `libdeflate.a`，进一步提升底层 CRC-32 校验与 Huffman 编码吞吐。

**Why this priority**: 消除陈旧静态库（当前 v1.19）潜在缺陷，吃满最新硬件指令集（如 ARMv8.2-A+crypto）的扩展性能红利，建立可复现的自动化构建机制。

**Independent Test**: 运行 `./scripts/build_libdeflate.sh`，验证产出支持 `arm64` 与 `x86_64` 的 fat 静态库，并通过 `lipo -info` 与全量回归测试。

**Acceptance Scenarios**:
1. **Given** 构建脚本执行，**When** 编译产出 `Vendor/libdeflate.a`，**Then** 二进制包含 arm64 与 x86_64 双架构切片，开启 `-O3` 与平台专用 SIMD 扩展。
2. **Given** 升级后的静态库，**When** 运行全量性能门禁与单元测试，**Then** 所有测试通过且无性能回退。

---

### User Story 3 - Windows 跨平台 CTTZipBridge 静态构建矩阵 (Priority: P2)

面向后续 Windows 平台分发，建立标准 CMake / MSVC 跨平台构建体系，能够自动化构建 Windows x86_64 与 ARM64 的 `libdeflate.lib` 与 `CTTZipBridge.dll / .lib`，统一跨平台 C 桥接层接口定义与符号导出。

**Why this priority**: Windows 平台传统 zlib 性能低下（慢 2.5x~3x）。提前建立跨平台静态库矩阵与构建脚本，为后续 TTZip 核心引擎跨平台移植提供开箱即用的基础设施。

**Independent Test**: 在 CMake 配置下通过跨平台工具链编译验证 `CMakeLists.txt`，确保 C 桥接层头文件与 Windows MSVC 语法兼容（包含 `__declspec(dllexport)`、线程局部存储宏 `__declspec(thread)` / `thread_local` 抽象）。

**Acceptance Scenarios**:
1. **Given** 跨平台 CMake 工程配置，**When** 在 Windows/MSVC 环境或 cross-compiler 下配置构建，**Then** 正确识别平台特征，成功生成 `libdeflate.lib` 与 `CTTZipBridge` 目标。
2. **Given** C 桥接层头文件与核心实现，**When** 在 MSVC 严格模式下编译，**Then** 零编译警告且无 POSIX 专有头文件（如 `<unistd.h>`）导致的阻断。

---

### User Story 4 - 跨平台与标准解压工具 100% 无损兼容 (Priority: P2)

由分块流式压缩器生成的 ZIP 文件，必须能够被 macOS 自带归档实用工具（Archive Utility）、系统命令行 `/usr/bin/unzip`、Windows 资源管理器、7-Zip 以及 TTZip 自身完美无损解压。

**Why this priority**: 归档软件的核心价值在于跨生态互操作性，任何流式切块实现必须严格遵循 PKWARE APPNOTE.TXT DEFLATE 规范，绝不能引入私有魔数或非标准格式。

**Independent Test**: 使用分块流式管道压缩生成 ZIP 归档，分别调用 `/usr/bin/unzip -t` 与 `7z t` 进行解压和校验和比对。

**Acceptance Scenarios**:
1. **Given** 由流式管道生成的 ZIP 文件，**When** 使用 `/usr/bin/unzip` 解压，**Then** 解压成功无报错，且解压所得文件与源文件逐字节一致。
2. **Given** 文件体积大于 4GB，**When** 流式管道压缩并写入 ZIP64 头部与数据描述符，**Then** 7-Zip 和系统工具均能正确识别 ZIP64 结构并无损解压。

---

### Edge Cases

- **边界大小 (恰好 256MB)**：单文件未压缩大小恰好等于 256MB 时，系统行为具有确定性确界（$\le 256\text{MB}$ 走 Whole-Buffer，$> 256\text{MB}$ 走 Chunked-Stream）。
- **不可压缩/高熵数据 (膨胀保护)**：当 1MB 块内的原始数据为已压缩视频或随机高熵数据时，若压缩后体积大于 1MB，系统自动以 Store 块（BTYPE=00）封装或合理标记，避免产生膨胀溢出。
- **超大单文件 (> 4GB)**：当单文件超过 4GB 时，自动且强制激活 ZIP64 扩展（64 位未压缩大小、64 位压缩大小、ZIP64 Extra Field 及 Data Descriptor），确保数据不发生 32 位整型截断。
- **背压控制 (Backpressure)**：当 CPU 多核压缩速度远大于磁盘 I/O 写入速度时，缓冲队列达到最大容量上限（如 32 个 In-flight 块）后，输入读取端必须自动阻塞暂停，防止内存堆积超出 64MB 门禁。
- **MSVC 与 POSIX C 兼容性断层**：Windows 环境下缺乏 `usleep`、`open(O_NOFOLLOW)`、`__thread` 等 POSIX 原语，C 桥接层必须建立跨平台抽象层（PAL, Platform Abstraction Layer）。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 根据待压缩文件的元数据大小，自动在 Whole-Buffer 模式（$\le 256\text{MB}$）与 Chunked-Stream 模式（$> 256\text{MB}$）之间无缝分流。
- **FR-002**: 在 Chunked-Stream 模式下，系统 MUST 将输入文件按 1MB 固定块尺寸进行拉取与切块。
- **FR-003**: 系统 MUST 采用有界环形缓冲或对象池（Bounded Pool）管理分块，在任何时刻限制 In-flight（正在读取、压缩、等待落盘）的数据块总数（$\le 32$ 块），确保总常驻内存增量严格恒定在 64MB 以内。
- **FR-004**: 系统 MUST 为每个 1MB 块生成标准 DEFLATE 压缩流（非末尾块设置 BFINAL=0，最终末尾块设置 BFINAL=1），并通过流式无缝拼接保证全局 DEFLATE 格式合规。
- **FR-005**: 系统 MUST 在流式切块过程中利用硬件加速指令（`libdeflate_crc32`）并发或增量计算全文件 CRC-32 校验和，并在文件末尾准确写入。
- **FR-006**: 针对超过 4GB 的超大文件，系统 MUST 自动开启 ZIP64 扩展支持，正确填充 64 位头字段与 ZIP64 Central Directory 记录。
- **FR-007**: 提供自动化构建脚本 `scripts/build_libdeflate.sh`，支持从源码编译最新 `libdeflate v1.21+` 并生成 Universal 2 (`arm64` + `x86_64`) macOS 静态库。
- **FR-008**: 提供跨平台 `CMakeLists.txt` 构建配置，支持在 Windows MSVC 环境下编译 `libdeflate.lib` 与跨平台 C 桥接层。
- **FR-009**: 系统 MUST 建立跨平台 C 原语抽象头文件（`CTTZipPlatform.h`），对 TLS 线程局部存储（`__thread` vs `__declspec(thread)`）、休眠原语与文件 I/O 标志提供跨平台统一宏定义。
- **FR-010**: 系统 MUST 具备原子级错误回收能力，在任务取消或 I/O 错误时在 100ms 内释放全部已分配的分块缓冲区与文件描述符。

---

### Key Entities

- **ChunkedCompressionPipeline**: 负责协调大文件读取、有界分块调度、多线程压缩与有序流式写入的核心管道调度器。
- **StreamChunk**: 代表单一 1MB 输入数据块及其压缩结果、序号（Sequence Number）、校验和以及就绪状态的实体结构。
- **BoundedChunkPool**: 容量固定（32 个槽位）的内存享元池，负责分配与回收固定大小的未初始化内存页。
- **PlatformAbstractionLayer (PAL)**：跨平台 C 桥接定义，包含 `TTZIP_THREAD_LOCAL`、`ttzip_sleep_ms`、`ttzip_open_secure` 等跨平台宏与内联包装。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 压缩任意超大单文件（测试覆盖 1GB、4GB、10GB）时，进程物理常驻内存（RSS）峰值增量严格 $\le 64\text{MB}$。
- **SC-002**: 在 Apple Silicon (M 系列芯片) 上，Chunked-Stream 模式在 4GB 单文件场景下的压缩吞吐量达到 $\ge 800\text{MB/s}$ (Level 1) 与 $\ge 500\text{MB/s}$ (Level 6)。
- **SC-003**: 100% 通过双向差分测试：生成的文件在 macOS `/usr/bin/unzip`、系统 Archive Utility 以及 7-Zip 上解压比对 SHA-256 校验和 100% 一致。
- **SC-004**: 小文件与常规文件（$\le 256\text{MB}$）的压缩基准测试吞吐量保持零回退（$\Delta \ge 0\%$）。
- **SC-005**: `Vendor/libdeflate.a` 升级到 v1.21+，全量测试套件（525+ tests）与性能门禁 100% 绿灯。
- **SC-006**: CMake 跨平台构建脚本在启用 Windows/MSVC 目标配置下零语法错误通过校验。

---

## Assumptions

- 运行环境为 macOS 14.0+，同时代码需具备 Windows MSVC C11 跨平台编译能力。
- 底层编解码引擎基于 `libdeflate` 官方最新稳定版源码。
- 单文件未压缩大小可通过文件系统元数据或流式探查准确获取。

---

## Clarifications

### Session 2026-08-17

- **Q1 (三大核心板块范围)**: 本规范将 P0（超大文件流式切块压缩器）、P1（Vendor/libdeflate v1.21+ 升级与自动化构建）与 P2（Windows CMake/MSVC 跨平台静态库矩阵）三位一体统筹规划。
- **Q2 (分块尺寸与背压确界)**: 设定 `MAX_IN_FLIGHT_CHUNKS = 32`。每个 In-flight 块包含 1MB 输入缓冲与至多 1MB 输出缓冲，理论峰值内存为 $32 \times 2\text{MB} = 64\text{MB}$。当队列满载时输入线程挂起等待，严格满足 $\le 64\text{MB}$ 常驻内存铁律。
- **Q3 (ZIP 规范 DEFLATE 块无缝拼接方式)**: 每一个 1MB 块使用 `libdeflate_deflate_compress` 生成独立的非终止 DEFLATE 块（前 $N-1$ 块标记非末尾块，最后 1 个块标记末尾块），或通过标准 Raw Deflate 块流对齐写入，确保系统与标准解压器无感知解压。
- **Q4 (Windows 兼容性抽象)**: 引入 `CTTZipPlatform.h`，统一隔离 `__thread` / `__declspec(thread)`、原子操作与文件描述符操作，确保 C 桥接层在 MSVC 与 Clang 下均能无缝编译。
