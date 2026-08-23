# Feature Specification: LZ4 Engine Analysis and Architecture Integration

**Feature Branch**: `063-lz4-engine-analysis`
**Created**: 2026-08-17
**Status**: Ready for Planning
**Input**: User description: "[lz4/lz4](https://github.com/lz4/lz4) BSD 2-Clause Mac: 纯 C / 统一内存 Win: MSVC 免配置 单核 4~5 GB/s 极速解压，用于大体积 TAR.LZ4 浏览与 VFS 临时缓存。P0 (核心底座) 详细看看相关内容我们是怎么实现的，这个库又是怎么实现的，比我们真的更快更好吗，我们可以怎么利用 /speckit-specify"

---

## Clarifications

### Session 2026-08-17
- **Q1: 核心研究范围与落地目标是什么？**
  - **A**: 全面剖析官方 `lz4/lz4` 开源库原理（Token 编解码、Wild Copy、Hash 匹配、Zero-Entropy 设计），对比 TTZip 当前链路（Apple `compression.h` vs 原生 `liblz4`），评估性能差距，并提出在 TAR.LZ4 极速穿透与 VFS 临时解压缓存池中的具体利用方案与工程落地策略。
- **Q2: 是否需要变更现有 CTTZipBridge 现有 API 契约？**
  - **A**: 保持 `ttzip_lz4_compress` 与 `ttzip_lz4_decompress` 原生 C 签名向后兼容，内部提升为调用原生优化路径，并提供更高级的微秒级加速控制接口。

---

## User Scenarios & Testing

### User Story 1 - 深度技术剖析与架构全景对比报告 (Priority: P1)

系统架构师与开发者需要获得一份详尽的技术研究报告，全面对比 TTZip 现有 LZ4 链路与官方开源 `lz4/lz4` 库的底层算法原理（Token 结构、Wild Copy 向量化展开、哈希匹配探测）、硬件加速特性、API 架构分层以及性能表现，清晰回答现有实现方式、官方库实现原理、性能差距与潜在利用场景。

**Why this priority**: 是理解、决策和重构 TTZip 底层极速压缩底座的前提，直接决定后续 VFS 缓存与 TAR.LZ4 浏览的架构选型。

**Independent Test**: 通过完整的架构分析文档与代码审计，全面覆盖当前 TTZip 代码链路与官方 LZ4 实现机制。

**Acceptance Scenarios**:
1. **Given** TTZip 现有代码库与官方 `lz4/lz4` 开源仓库，**When** 执行架构与算法审查，**Then** 输出涵盖数据布局、压缩/解压热路径、内存模型、多线程并发与平台特化对比的深度分析。
2. **Given** 现有 macOS Apple `compression.h` 与原生 `liblz4`，**When** 进行对比评估，**Then** 明确指出两者的吞吐差异、调用开销与功能边界（如 Partial Decompress、定制加速因子）。

---

### User Story 2 - 原生 C 桥接引擎强化与 Fast-Path 对齐 (Priority: P1)

TTZip 核心引擎需要提供高吞吐、低延迟的 原生 LZ4 内存编解码能力，支持直接调用高性能原生 C 静态库，支持动态加速因子控制与快速前缀解压，规避系统封装的黑盒间接开销。

**Why this priority**: 核心引擎数据平面的基石，支撑所有上层归档、流式处理与内存池操作。

**Independent Test**: 运行单测验证内存数据在多档加速因子下的压缩与解压正确性，解压数据与源数据 100% 比特级一致。

**Acceptance Scenarios**:
1. **Given** 任意大小的内存数据块（从数 KB 到数百 MB），**When** 调用原生 LZ4 编解码接口，**Then** 能够以超过门禁底线的吞吐完成处理，且还原数据完全一致。
2. **Given** 变长加速因子输入，**When** 执行极速压缩，**Then** 引擎根据加速因子动态跳过采样，实现更低 CPU 占用与更高吞吐。

---

### User Story 3 - 大体积 TAR.LZ4 极速穿透与 VFS 临时缓存利用方案 (Priority: P2)

用户在浏览大体积 TAR.LZ4 归档或使用 VFS 虚拟文件系统进行临时文件解压/预览时，系统能够利用 LZ4 单核 4~5 GB/s 的极限解压速度与内存紧凑性，实现无感知的毫秒级目录树穿透与瞬时内存/临时磁盘缓存。

**Why this priority**: 极大提升大归档穿透浏览、单文件快速提取与临时工作区的响应性能。

**Independent Test**: 针对 100MB+ 的 TAR.LZ4 归档及多层级目录，执行列表提取与单文件随机预览测试。

**Acceptance Scenarios**:
1. **Given** 一个包含多文件的 TAR.LZ4 归档，**When** 用户请求目录列表或预览特定条目，**Then** 系统能以极低延迟完成微缓冲流式解压并呈现内容。
2. **Given** VFS 缓存管理模块，**When** 缓存大型中间对象，**Then** 可利用 LZ4 进行零瓶颈压缩驻留，节约内存与 I/O 带宽。

---

### User Story 4 - 性能基准门禁与零倒退回归 (Priority: P2)

确保所有与 LZ4 相关的基准测试、吞吐门禁在 Debug 与 Release 模式下均能稳定通过，维持全格式历史最优峰值，且不发生任何性能倒退。

**Why this priority**: 遵守工程宪法与性能铁律，确保重构与优化安全可信。

**Independent Test**: 执行 `XCTestPerformanceMeasureTests` 与 `AllFormatsPkSuiteTests`。

**Acceptance Scenarios**:
1. **Given** LZ4 进程内流式压缩/解压测试，**When** 执行性能门禁测试，**Then** Debug 下吞吐 >= 6000 MB/s，Release 下 >= 10000 MB/s。
2. **Given** 全格式回归矩阵，**When** 运行测试集，**Then** 525+ 项测试全部绿灯通过。

---

## Edge Cases

- **极小数据块 (< 16 字节)**：边界对齐与不可压缩数据膨胀处理（保证分配容量有安全余量，防止越界写入）。
- **损坏或截断的 LZ4 流**：解压器必须返回明确错误码，严禁发生内存越界（Buffer Overflow）或崩溃。
- **超大单文件 (> 2GB / > 4GB)**：64 位偏移量向 32 位安全窄化保护（Clamp to SSIZE_MAX）。
- **零长度输入**：安全短路返回空数据，不执行无意义的内存分配。

---

## Requirements

### Functional Requirements

- **FR-001**: 系统必须提供完整的 LZ4 官方开源库与 TTZip 现状深度技术对比报告。
- **FR-002**: 系统必须支持基于原生 C 原语的高吞吐内存数据块 LZ4 压缩与解压能力。
- **FR-003**: 编解码过程必须保证单任务内存常驻稳定在安全界限内，消除未经控制的全量内存膨胀。
- **FR-004**: 系统必须支持在 TAR.LZ4 归档与 VFS 临时缓存场景中利用 LZ4 进行极速解码与暂存。
- **FR-005**: 所有 LZ4 编解码实现必须满足 100% 比特级无损数据一致性验证。

---

## Key Entities

- **LZ4BlockPayload**: 裸 LZ4 数据块，包含原始数据长度、压缩数据与加速配置。
- **LZ4FrameDescriptor**: LZ4 帧格式元数据，包含块独立性、内容校验和与字典标识。
- **VFSTempCacheBlock**: VFS 虚拟文件系统专用的 LZ4 临时解压缓存块，用于高速读写与内存紧缩。

---

## Success Criteria

### Measurable Outcomes

- **SC-001**: LZ4 内存编解码吞吐在 Debug 模式下达到 >= 6000 MB/s，Release 模式下达到 >= 10000 MB/s。
- **SC-002**: TAR.LZ4 解压吞吐维持 >= 4000 MB/s 历史峰值水平。
- **SC-003**: 全量单元测试（525+ 测试用例）100% 通过，零回归、零崩溃。
- **SC-004**: 产出系统化的开源库技术对比与 VFS / 归档利用架构指南。

---

## Assumptions

- 平台运行环境为 macOS 14.0+，支持 Apple Silicon ARM64 及 Intel x86_64 架构。
- 项目已内置 `Vendor/libTTZipVendor.a` 及对应 `lz4.h` 头文件，具备原生 C 静态链接能力。
- 本次任务遵循 Spec Kit 全闭环自驱协议，按规范推进。
