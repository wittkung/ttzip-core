# Feature Specification: 7Z Final Two 500MB Conquest (500MB 终极战役)

**Feature Branch**: `007-7z-final-two-500mb-conquest`

**Created**: 2026-08-15

**Status**: Draft

**Input**: User description: "没打过的两项也需要打过，加油，我们大幅度超越！"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 500MB 大文件 Level 1 无加密压缩突破 5,800+ MB/s (Priority: P1)

作为一名需要高频压缩超大虚拟机镜像或日志数据包的用户，使用 TTZip 压缩 500MB 大文件时，能以超过 5,800 MB/s 的极致速度完成压缩，大幅超越 7-Zip 官方 `7zz` CLI（5,245 ~ 5,434 MB/s）。

**Why this priority**: 这是 7Z 对决 32 个场景中仅存的 2 个未打过项之一，完成此项将实现无加密全场景 100% 胜出。

**Independent Test**: 对 500MB 大文件执行 7Z Level 1 压缩，验证吞吐 $\ge 5,600\text{ MB/s}$ 且归档可被官方 7zz 无损解压。

**Acceptance Scenarios**:

1. **Given** 存在 500MB 数据块，**When** 用户请求以 7Z Level 1 极速压缩（无加密），**Then** 压缩吞吐达到 $\ge 5,600\text{ MB/s}$，全面领先 7zz（5,245 MB/s）。

---

### User Story 2 - 500MB 大文件 Level 1 AES-256 加密压缩突破 5,800+ MB/s (Priority: P2)

作为对安全性有极高要求的专业用户，在对 500MB 大文件进行 AES-256 加密 7Z 压缩时，TTZip 能够利用 ARMv8 NEON 原地流式加密与零拷贝写盘，达到 $\ge 5,600\text{ MB/s}$ 吞吐，全面超越 7-Zip 官方 `7zz` CLI（5,347 ~ 5,386 MB/s）。

**Why this priority**: 这是 7Z 加密场景仅存的未胜项，攻克后 7Z 格式实现 32/32 全胜（100% 统治）。

**Independent Test**: 对 500MB 大文件执行 7Z Level 1 AES-256 压缩，验证吞吐 $\ge 5,600\text{ MB/s}$ 且解密校验一致。

**Acceptance Scenarios**:

1. **Given** 存在 500MB 数据块，**When** 用户请求以 7Z Level 1 极速压缩（带 AES-256 密码），**Then** 压缩吞吐达到 $\ge 5,600\text{ MB/s}$，全面领先 7zz（5,347 MB/s）。

---

### Edge Cases

- **CPU 线程池满载与内存带宽平衡**: 避免线程过多引发 L2 Cache 震荡或线程过少引起核心空转。
- **AES-256 CBC Padding 对齐**: 确保对齐填充不引入额外内存分配。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 500MB 单流压缩 MUST 采用动态多通道无锁环形队列（Lock-Free Ring Pipeline），让每个 P-Core 持续饱和处理压缩块。
- **FR-002**: 针对全零/长重复流，LZMA2 匹配器 MUST 启用极速直接匹配（Direct Match Run-Length Shortcut），消除 Range Coder 状态机无效深搜。
- **FR-003**: AES-256 加密 MUST 与块输出线程完全合并，单指令流直接写入内存映射文件。

---

## Success Criteria *(mandatory)*

- **SC-001**: 7Z 500MB Level 1 无加密压缩吞吐达到 **$\ge 5,600\text{ MB/s}$**（超越 7zz 5,245 MB/s）。
- **SC-002**: 7Z 500MB Level 1 AES-256 压缩吞吐达到 **$\ge 5,600\text{ MB/s}$**（超越 7zz 5,347 MB/s）。
- **SC-003**: 7Z 格式在全维度 32 个竞品对决项中实现 **32 战 32 胜（100% 胜率）**。
- **SC-004**: 所有既有测试场景性能**零倒退**。
