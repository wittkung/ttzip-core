# Feature Specification: 7Z Comprehensive Conquest (全面超越 7-Zip 官方引擎)

**Feature Branch**: `006-7z-comprehensive-conquest`

**Created**: 2026-08-15

**Status**: Draft

**Input**: User description: "详细分析和调研，再来优化，我们要全面打过7Z ，要大幅度领先"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 超大文件 (500MB+) 极速 7Z 压缩超越官方 CLI (Priority: P1)

作为一名专业用户或开发者，当我需要将 500MB 或更大的单一数据文件、虚拟机镜像或数据库备份快速打包为 `.7z` 归档时，TTZip 能够以突破 5,800+ MB/s 的极速吞吐完成压缩，彻底击败 7-Zip 官方 `7zz` 命令行工具（5,498 MB/s），节省宝贵时间。

**Why this priority**: 500MB Level 1 压缩是当前 7Z 竞品对决中仅存的物理差距项（TTZip 4,520 MB/s vs 7zz 5,498 MB/s），直接决定了 7Z 格式是否能实现 100% 全胜统治。

**Independent Test**: 对 500MB 结构化与高重复数据流执行 Level 1 压缩，测算端到端吞吐率，验证输出 `.7z` 归档能被官方 7-Zip 无损校验解压。

**Acceptance Scenarios**:

1. **Given** 存在 500MB 单体数据文件，**When** 用户请求以 7Z 格式 Level 1 极速压缩（无加密），**Then** TTZip 压缩吞吐达到 $\ge 5,600\text{ MB/s}$，全流程超越 7zz 并生成合规 `.7z` 归档。
2. **Given** 存在 500MB 单体数据文件，**When** 用户请求以 7Z 格式 Level 1 极速压缩并开启 AES-256 加密，**Then** TTZip 压缩吞吐达到 $\ge 5,600\text{ MB/s}$，全面超越 7zz（5,382 MB/s）。

---

### User Story 2 - 海量小文件 (100+ files) 7Z 极速打包全面胜出 (Priority: P2)

作为 macOS 开发者或设计师，当我需要将包含数百个源代码或静态资源的小文件目录打包为 `.7z` 归档时，TTZip 能以零冗余系统调用快速完成固实流初始化与多核 LZMA2 编码，以超过 950+ MB/s 的吞吐超越 7-Zip 官方工具（883 MB/s）。

**Why this priority**: 小文件归档是桌面用户最高频的操作之一，目前 TTZip（855 MB/s）与 7zz（883 MB/s）仅差 28 MB/s，优化固实流流水线即可实现绝对领先。

**Independent Test**: 对包含 100 个小文件（共 10MB）的目录执行 7Z Level 1 压缩与解压，对比耗时与吞吐。

**Acceptance Scenarios**:

1. **Given** 包含 100 个小文件的测试目录，**When** 用户请求以 7Z 格式 Level 1 压缩（无加密），**Then** 压缩吞吐达到 $\ge 950\text{ MB/s}$，稳胜 7zz。
2. **Given** 包含 100 个小文件的测试目录，**When** 用户请求以 7Z 格式 Level 1 压缩并开启 AES-256 加密，**Then** 压缩吞吐达到 $\ge 950\text{ MB/s}$，保持显著领先。

---

### User Story 3 - 7Z 解压与全维度对决无死角碾压 (Priority: P3)

作为终端用户，在日常解压与压缩任意 7Z 归档时，TTZip 在所有等级（Level 1 ~ Level 6）、所有载荷类型（日志文本、高熵物理数据、海量小文件、超大单文件）及加密模式下，均能大幅超越 7-Zip 官方工具，实现 32/32 项 100% 全胜统治。

**Why this priority**: 巩固解压端 16/16 全胜的巨大优势（最高领先 6.6x），确保全矩阵基准测试中无任何失分项。

**Independent Test**: 运行全量 32 项 7Z 竞品对决基准测试，统计胜局比例。

**Acceptance Scenarios**:

1. **Given** 7Z 格式全矩阵 32 项压测场景，**When** 执行竞品 1v1 对决，**Then** TTZip 取得 32 胜 0 负（100% 胜率），且核心场景领先比例 $\ge 1.10\text{x}$。

---

### Edge Cases

- **超大单流切分边界**: 500MB 单文件切分为多核块时，块大小与核数匹配避免尾块碎片化。
- **高熵与低熵混合载荷**: 归档内既包含高熵不可压缩文件又包含高压缩比文本文件时，流式自适应编码器零惩罚自动切换。
- **AES-256 流式原地加密**: 消除临时数据拷贝与额外内存堆分配，保证加密路径与无加密路径吞吐高度接近。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 7Z 引擎 MUST 在 Level 1 极速压缩模式下，对大文件单流采用核数自适应分块与极简哈希匹配（`depth=1` / `nice_len=8`），消除多余字典查找开销。
- **FR-002**: 7Z AES-256 压缩管道 MUST 支持单流水线（In-Place Pipeline），消除加解密与 LZMA2 编码之间的中间缓冲区拷贝。
- **FR-003**: 7Z 小文件 Solid 打包流程 MUST 支持预分配连续内存流与紧凑元数据构建，消除遍历与状态机重置开销。
- **FR-004**: 7Z 解压引擎 MUST 保持流式直接写盘与 NEON 向量化 CRC32 校验，确保解压吞吐稳定在 8,500+ MB/s。
- **FR-005**: 任何 7Z 编解码优化 MUST 保证生成的 `.7z` 归档严格符合 7z 容器规范，能被官方 `7zz` 与第三方工具无损解压校验。

---

### Key Entities

- **7z Compression Stream**: 代表单一或固实流的多核 LZMA2 编码通道，包含块切分、线程局部字典与自适应匹配器状态。
- **7z Encryption Context**: 代表单次归档任务的 AES-256 与 SHA-256 KDF 会话上下文，保证跨块跨文件单次派生与硬件原地加密。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 7Z 500MB Level 1 压缩（无加密）吞吐突破 **$\ge 5,600\text{ MB/s}$**，超越 7-Zip 7zz CLI（5,498 MB/s）。
- **SC-002**: 7Z 500MB Level 1 压缩（AES-256）吞吐突破 **$\ge 5,600\text{ MB/s}$**，超越 7-Zip 7zz CLI（5,382 MB/s）。
- **SC-003**: 7Z 海量小文件 Level 1 压缩（无加密）吞吐突破 **$\ge 950\text{ MB/s}$**，超越 7-Zip 7zz CLI（883 MB/s）。
- **SC-004**: 7Z 格式在全维度竞品 1v1 基准测试中实现 **32 胜 0 负（100% 胜率）**。
- **SC-005**: 全量 560+ 项单元测试与安全性防御测试（Zip Slip / 密码混淆）保持 100% 绿灯通过。

---

## Assumptions

- 运行平台为 Apple Silicon (M 系列芯片，ARM64 架构) macOS 14.0+。
- 竞品基准工具为 7-Zip 官方最新原生 ARM64 版本 `7zz` CLI，参数全开多线程 (`-mmt=on`)。
- 压缩与解压过程中数据完整性由 CRC32 / SHA-256 硬件校验保障。
