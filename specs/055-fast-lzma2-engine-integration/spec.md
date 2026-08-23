# Feature Specification: Fast-LZMA2 Multi-Threaded Engine Integration (7Z / XZ / TAR.XZ High-Level Compression & Dual-Platform Concurrency)

**Feature Directory**: `specs/055-fast-lzma2-engine-integration`

**Created**: 2026-08-17

**Status**: Clarified

**Input**: User description: "conor42/fast-lzma2 (BSD/GPLv2, dual-platform macOS/Apple Silicon & Windows) 7Z/XZ/TAR.XZ 高性能多线程 LZMA2 引擎深度评估与系统级整合。破除 LZMA BT4 在高压缩等级下的多核扩展瓶颈，建立 L1 原生 NEON 与 L3~L9 Fast-LZMA2 混合调度架构，全面榨干 8~24 核 CPU 算力并保障跨平台无损兼容。"

---

## Clarifications

### Session 2026-08-17
- **Q: fast-lzma2 代码在工程中的物理打包与集成形式？**
  - **Decision**: 采用 In-tree C 源码编译模式，置于 [Sources/CTTZipBridge/fast-lzma2/](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/)，直接纳入 SPM 与 CMake 静态编译单元，保持 100% 进程内无外部动态链接依赖。
  - **Rationale**: 避免预编译二进制碎片化，确保 Clang `-O3 -flto` 与 Apple Silicon 向量化指令最佳内联。
- **Q: 线程池在 macOS 与 Windows 平台的调度与隔离策略？**
  - **Decision**: macOS 平台下复用 FL2 内置的轻量 POSIX `pthread` 线程池或 GCD 桥接，利用 `QOS_CLASS_USER_INITIATED` 高优先级绑定；Windows 平台使用 FL2 原生 Win32 线程池。
  - **Rationale**: FL2 自带的分块无锁作业队列已针对 Radix 匹配查找高度优化，直接调用原生线程池开销最小。
- **Q: 高并发多线程下的字典尺寸与内存上限控制？**
  - **Decision**: 默认字典尺寸限制为 16MB ~ 64MB，单任务常驻内存硬上限为 512MB（16 线程），超大字典（128MB+）仅在 32GB+ 内存设备且用户显式指定时开启。
  - **Rationale**: 符合四大系统工程铁律中的确定性确界（Bounds-First），防止极端多核机器上出现内存抖动或 OOM。

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 高压缩等级 (Level 3~9) 7Z / XZ 极致多核吞吐 (Priority: P0)

用户在进行 7Z、XZ 或 TAR.XZ 归档压缩时，选择中高压缩等级（如 Level 5 标准或 Level 7 极限压缩）。系统必须能够充分调动多核 CPU（8 ~ 24 核 P/E 核），打破传统 LZMA BT4 匹配查找器单核串行与巨额内存锁死瓶颈，使高压缩等级下的整体吞吐相比传统 liblzma 提升 1.5x ~ 3x，大幅缩短用户等待时间。

**Why this priority**: 解决 7Z / LZMA2 在中高压缩等级下耗时长、CPU 利用率不足的长期行业痛点，确立 TTZip 在 7Z 压缩速度上的压倒性优势。

**Independent Test**: 对 100MB+ 标准基准测试样本（如 Silesia / Enwik / 实际工程数据）执行 7Z Level 5 压缩，记录 CPU 整体利用率与压缩吞吐量，验证多核加速比 $\ge 2.0\text{x}$ 且生成合规 7Z 归档。

**Acceptance Scenarios**:
1. **Given** 待压缩体积 $\ge 100\text{MB}$ 的数据源，**When** 用户选择 7Z Level 5 压缩，**Then** 8 核以上 CPU 利用率稳定维持在 85% 以上，压缩吞吐达到 $\ge 800\text{MB/s}$（Debug）/ $\ge 1200\text{MB/s}$（Release），耗时较传统引擎缩短 50% 以上。
2. **Given** 生成的 7Z 归档文件，**When** 使用系统原生解压工具、官方 7-Zip 或 TTZip 自身解压，**Then** 校验和 100% 匹配，零字节损坏。

---

## User Story 2 - 混合双引擎智能分流 (Hybrid Fast-Path Routing) (Priority: P0)

系统根据用户选择的压缩等级与平台特征，实现双引擎自适应无缝路由：
1. **Level 1 极速模式**：直接路由至 TTZip 自研手写 ARM64 NEON 向量化匹配查找器与无分支 Range Coder，维持 $\ge 3,200\text{MB/s}$ 的硬件极限吞吐与零块快速旁路。
2. **Level 3 ~ Level 9 高压缩模式**：无缝分流至 Fast-LZMA2 并行 Radix 匹配查找器与多线程流水线，榨干多核多线程算力。

**Why this priority**: 兼顾极速场景（L1 NEON 硬件级峰值）与极限压缩比场景（L5+ 多核 Radix 扩展），避免“一刀切”导致极速路径性能回退。

**Independent Test**: 分别以 Level 1 与 Level 5 运行 7Z 压缩基准测试，验证 Level 1 不跌破现有门禁（$\ge 3,200\text{MB/s}$），Level 5 获得翻倍级性能提升。

**Acceptance Scenarios**:
1. **Given** Level 1 压缩请求，**When** 引擎执行压缩，**Then** 命中原生 NEON Fast-Path，吞吐达标且零额外抽象开销。
2. **Given** Level 5/7 压缩请求，**When** 引擎执行压缩，**Then** 自动命中 Fast-LZMA2 多线程流水线，保持高压缩比的同时实现多核并行加速。

---

## User Story 3 - macOS (Apple Silicon) 与 Windows 双平台原生并发 (Priority: P1)

在 macOS 上深度结合 Apple Silicon 统一内存架构与 POSIX pthread / GCD 调度；在 Windows 平台上利用 Fast-LZMA2 原生 Windows 线程池（Win32 Threads / CRITICAL_SECTION）与 MSVC 编译优化，为后续 Windows 版本提供开箱即用、零 POSIX 胶水层依赖的高性能 7Z 压缩引擎。

**Why this priority**: 统一双平台 LZMA2 核心算法栈，提前铺平跨平台架构路径，消除 Windows 环境下多线程调度的平台断层。

**Independent Test**: 在 macOS (ARM64/x86_64) 与跨平台 CMake / Windows 环境下分别编译并运行 LZMA2 多线程压缩测试用例，验证双端均能满载多核并行。

**Acceptance Scenarios**:
1. **Given** macOS Sonoma 运行环境，**When** 启动多线程 LZMA2 压缩，**Then** 线程池均匀调度至 P-Core 与 E-Core，内存无泄漏。
2. **Given** Windows / MSVC 编译环境，**When** 编译 Fast-LZMA2 核心模块，**Then** 零编译警告，原生 Win32 线程调度平稳运行。

---

## User Story 4 - 严格内存确界与零内存泄漏 (Priority: P1)

在高并发线程数（如 16~24 线程）下，系统对每个工作线程的字典缓冲区与 Radix 表内存进行严格确界管理，单任务常驻内存必须受控，严禁因并发度增加导致物理内存爆炸（OOM）或垃圾回收停顿。

**Why this priority**: 传统 7-Zip 多线程 LZMA 随线程数线性倍增字典内存，极易在多核高字典场景下耗尽内存。Fast-LZMA2 的共享 Radix 表与分块流式模型必须严格遵守 TTZip 的四大系统工程铁律（确定性确界）。

**Independent Test**: 在 16 线程配置下对 500MB 大文件执行压缩，采样全生命周期物理内存（RSS），验证峰值内存稳定且任务结束后 100% 归还。

**Acceptance Scenarios**:
1. **Given** 16 并发线程进行 Level 5 压缩，**When** 压缩过程持续进行，**Then** 进程物理内存峰值不超过字典预设的严格确界，无内存页清零抖动。
2. **Given** 任务正常完成或中途取消，**When** 释放资源，**Then** 句柄与内存池 100ms 内完全清理，通过 LeakSanitizer 审查。

---

## Edge Cases

- **极小文件与空文件 (< 4KB / 0B)**：分块多线程开销超过计算收益时，自动退化为单线程直接编码或 Store 块，避免多线程调度抖动。
- **高熵不可压缩数据**：当输入块无法被 LZMA2 压缩时，Fast-LZMA2 必须能快速短路并输出 Uncompressed Chunk，防止 CPU 无效空转。
- **动态字典尺寸匹配**：根据输入总数据量自适应选择 64KB ~ 64MB 字典尺寸，避免为小数据分配大字典浪费内存。
- **任务中途取消 (Cancellation)**：在多线程 Radix 匹配查找中途收到取消信号时，各工作线程必须在 $\le 50\text{ms}$ 内安全退出并释放已分配资源。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 将 `fast-lzma2` 核心算法源码以 BSD-2-Clause 兼容方式整合至 `Sources/CTTZipBridge/` 或 `Vendor/` 模块，对外暴露纯 C 静态接口。
- **FR-002**: 系统 MUST 建立 `ttzip_fl2_encoder` 桥接封装，支持配置线程数、压缩等级（1~9）、字典大小与内存限制。
- **FR-003**: 系统 MUST 保留现有的 Level 1 ARM64 NEON 自研快速编码器与全零块快速旁路，作为 L1 模式下的默认 Fast-Path。
- **FR-004**: 针对 Level 3 ~ Level 9 的 7Z、XZ、TAR.XZ 压缩请求，系统 MUST 默认分发至 Fast-LZMA2 多线程流水线执行。
- **FR-005**: 编码器生成的所有 LZMA2 数据块 MUST 100% 严格符合 LZMA2 官方二进制规范，可被标准解压器无损还原。
- **FR-006**: 在 macOS 平台上，系统 MUST 支持通过 pthread 或 GCD 驱动 Fast-LZMA2 任务；在 Windows 平台上 MUST 支持 Win32 Threads 原生调度。
- **FR-007**: 系统 MUST 在 `TTZipCore` 中提供高层 Strategy / Bridge 适配器，使得 7Z 归档管道与 XZ 流式管道能够透明消费 Fast-LZMA2 编码能力。
- **FR-008**: 系统 MUST 为 Fast-LZMA2 上下文与内存池实现确定性确界管理，构造填充 Magic，释放强制归零，杜绝 UAF 与内存泄漏。
- **FR-009**: 严禁在 Fast-LZMA2 内部热循环中使用 `printf` / `fprintf` / `NSLog`，所有诊断信息必须经由 `TTLogger` 统一拦截。
- **FR-010**: 系统 MUST 在现有性能门禁套件中补充 Fast-LZMA2 专项多核吞吐与压缩比回归测试用例。

---

## Success Criteria *(mandatory)*

- **SC-001**: 在 8 核及以上 Apple Silicon 芯片上，7Z Level 5 压缩吞吐较现有 liblzma 单块/串行路径提升 $\ge 100\%$（达到 $\ge 800\text{MB/s}$ Debug / $\ge 1200\text{MB/s}$ Release）。
- **SC-002**: 7Z Level 1 压缩性能保持现有门禁不倒退（$\ge 3,200\text{MB/s}$ Debug / $\ge 3,900\text{MB/s}$ Release）。
- **SC-003**: 全格式 46 项基准测试及回归测试套件 100% 通过（零回归，$\Delta \ge -3.0\%$）。
- **SC-004**: 生成的 7Z / XZ 归档经官方 7-Zip、`/usr/bin/tar` 与 macOS Archive Utility 双向差分测试，100% 无损解压。
- **SC-005**: 16 并发线程下峰值常驻内存受控在预期确界内，任务结束后零内存泄漏。

---

## Key Entities

- **FL2CompressionContext**: Fast-LZMA2 编码器 C 级会话上下文，管理 Radix 匹配查找器表、字典缓冲区与线程池。
- **SevenZipLZMA2HybridStrategy**: Swift 层 7Z 压缩策略分发器，根据压缩等级智能分流至 L1 NEON Fast-Path 或 L3~L9 Fast-LZMA2 引擎。
- **SevenZipArchivePipeline**: 7Z 归档核心流水线，负责协调文件流、分块压缩、Solid 块组织与头部序列化。
