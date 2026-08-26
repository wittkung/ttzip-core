# TTZip Core — Agent Execution Rules

> **Core Mandate**: All interactions, reasoning cycles, and tool invocations MUST strictly adhere to the **Pi Framework Philosophy** (Minimalist Core, Surgical Edits, Append-Only Determinism, and High Signal-to-Noise Ratio) with **100% Autonomous Proactive Subagent Dispatching**, while upholding the full **TTZip Systemic Engineering Invariants**.

---

## Ⅰ. Pi Framework Operating System Mandate

1. **直接行动，拒绝废话 (Act First, Speak Minimal)**:
   - 禁止在工具调用前输出冗长套话。
   - 直接进行精准的工具调用（`view_file` / `replace_file_content` / `run_command`）。
2. **纯粹事实陈述 (Pure Factuality)**:
   - 回复仅陈述做了什么、看到了什么、验证了什么。
   - 彻底剥离情绪化、戏剧化修辞与自我评价。
3. **全自动子智能体调度铁律 (Autonomous Proactive Subagent Dispatch)**:
   - $\ge 2$ 个独立文件并发修改、并发调研、高噪日志扫描或独立差分验证时，必须自主发起 `invoke_subagent` 并行推进。
4. **Surgical Tool Discipline**:
   - 精确切片只读 (`view_file` StartLine/EndLine)、外科手术式替换 (`replace_file_content`)、安全原子写入 (`write_to_file`)、确定性终端命令 (`run_command`)。

---

## Ⅱ. TTZip Core 架构与工程规范

### 一、 核心子系统与职责拓扑

- **`rust/ttzip-engine`**: 纯 Safe Rust 编解码微内核（Rayon 并发压缩, APFS `fstore_t` 预分配, 零拷贝 `memmap2`, SOTA Codecs, UniFFI 0.28 导出）。
- **`rust/ttzip-tui`**: 跨平台 POSIX CLI 与 Ratatui 终端 TUI 浏览器（编译产物 `bin/ttzip`）。
- **`rust/ttzip-python`**: PyO3 0.22 原生 Python C-Extension 模块（释放 GIL, Buffer Protocol）。
- **`Sources/TTZipCore`**: Swift 6 编排层（Actor 隔离, 命令模式与 APFS CoW 回滚, VFS 享元树, 60Hz 进度节流）。
- **`Sources/CTTZipBridge`**: Mozilla UniFFI Scaffolding (`ttzip_engineFFI.h`) 与 C-ABI 2.0 (`ttzip_rust_glue.h`) 桥接层。
- **`Sources/TTZipBench`**: 端到端基准测试与 CI 门禁工具（`ttzip-bench gate`, `ttzip-bench pipeline`）。
- **`Vendor/TTZipVendor.xcframework`**: 预编译静态库 `libTTZipVendor.a`（包含 Rust 微内核与全部原生编解码引擎）。

---

### 二、 构建与测试命令

```bash
# 1. 构建 Rust 微内核并生成 UniFFI 绑定
./scripts/build_rust.sh

# 2. 编译 Swift 6 Core 库与测试目标
swift build
swift build -c release

# 3. 运行 Swift 单元与集成测试
swift test --parallel

# 4. 运行 Rust Workspace 测试套件
cd rust && cargo test --workspace && cd ..

# 5. 运行全管线基准测试与 CI 门禁
swift run ttzip-bench gate
swift run ttzip-bench pipeline

# 6. 运行本地完整 CI 门禁
./scripts/run_local_ci_gate.sh
```

---

### 三、 核心架构宪章与系统工程铁律

1. **100% Mozilla UniFFI 跨语言标准**:
   - 所有 Tier-1 SDK 必须 100% 基于 Mozilla UniFFI 自动生成安全绑定与内存屏障。
   - 严禁手写跨语言非受管裸指针。
   - 计算密集、I/O 密集、密码哈希、VFS 树形结构与原地改写逻辑统一在 Rust 内核实现。

2. **四大系统工程铁律**:
   - **流式第一性 (Stream-First)**: 消除全量内存假设；数据流面向微缓冲与分块流式管道；单任务常驻内存 $\le 64\text{MB}$。
   - **纵深防御 (Invariant-First)**: 零内存分配路径消毒 (`path_sanitizer.rs`) 彻底免疫 Zip-Slip 与 TOCTOU 攻击；算术调用 CPU 防溢出指令。
   - **确定性确界 (Bounds-First)**: 密码与敏感内存必须调用 `zeroize` / `SecureBytes` 擦除；跨语言数值必须经过 `SSIZE_MAX` Clamp。
   - **真实预言机 (Oracle-First)**: 自动化测试面向真实边界用例；自研引擎与系统原生 `/usr/bin/tar` / `/usr/bin/unzip` 执行双向差分测试。

3. **严格单文件 LOC 门禁 ($\le 800$ LOC)**:
   - 单文件行数硬性上限 800 行，目标均值 $\le 350$ 行。超限阻断 CI 流水线。

4. **绝对零编译告警 (Zero-Warning Hard Gate)**:
   - 无论是 Debug、Release 还是 Test Target，必须无条件通过 `-warnings-as-errors`。

---

### 四、 统一 SPDX 版权与注释标准

```swift
// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
```
- **作者署名**: `Witt Kung <witt.w.kung@gmail.com>`
- **英文注释标准**: 源码内部所有注释、Docstrings、常量与错误说明统一采用专业英语书写。
