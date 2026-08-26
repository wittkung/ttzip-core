# TTZip Project & Architecture Unified Mandate

> **Core Mandate**: All interactions, reasoning cycles, and tool invocations MUST strictly adhere to the **Pi Framework Philosophy** (Minimalist Core, Surgical Edits, Append-Only Determinism, and High Signal-to-Noise Ratio) with **100% Autonomous Proactive Subagent Dispatching**, while upholding the full **TTZip Systemic Engineering Invariants**.

---

## Ⅰ. 项目全景基本情况与三仓独立拓扑 (Repository Topology & Architecture)

TTZip 采用**三大完全独立的 Git 仓库**解耦分发架构。开发与维护时必须时刻牢记各个仓库的边界与职责分工：

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                                 TTZip 三仓独立实体拓扑架构                                  │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. ttzip-apple (GitHub: wittkung/ttzip-apple, 许可证: GPL-3.0-or-later)                    │
│    • 实体: 完全独立的 macOS 原生桌面客户端 Git 仓库                                          │
│    • 核心职责:                                                                           │
│      - macOS 原生应用 (Sources/TTZipApp)                                                 │
│      - 🌟 默认内置通用插件 SDK (Sources/TTZipPluginKit, 包含 13 个工件)                      │
│      - 🌟 默认内置插件中心与商店 UI (PluginsView.swift / TTZipMarketplaceService)          │
│      - 宿主能力注入 (TTZipHostContextImpl.swift, 提供租户隔离 Keychain / 事件总线 / 压缩代理)    │
│      - 系统级扩展: Finder 右键菜单 (TTZipFinderSync) 与 QuickLook 快速预览 (TTZipQuickLook)  │
│      - 多渠道打包与签名流水线 (scripts/bundle_app.sh --channel direct | mas)                │
│    • 🌟 独立开箱即编铁律:                                                                  │
│      - 外部开发者单独 `git clone https://github.com/wittkung/ttzip-apple.git` 下来，必须能    │
│        直接运行 `swift build` 与 `bundle_app.sh` 编译打包成功，绝对零路径阻断！               │
│      - 依赖解析双模机制: 本地存在 `../core` 时自动优先使用本地微内核；独立克隆时自动通过 Git URL  │
│        `https://github.com/wittkung/ttzip-core.git` 拉取，严禁硬编码外部机器私有路径！         │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ 2. ttzip-core (GitHub: git@github.com:wittkung/ttzip-core.git, 许可证: BSD-3 / Apache-2.0)│
│    • 实体: 完全独立的跨平台归档与压缩微内核 Git 仓库                                          │
│    • 核心职责:                                                                           │
│      - 纯跨平台 Rust 微内核 (rust/ttzip-engine, 覆盖 16 种格式全矩阵)                         │
│      - Mozilla UniFFI 0.28 跨语言桥接自动生成 (Sources/CTTZipBridge / ttzip_engineFFI.h)   │
│      - 预编译跨平台静态库与头文件 (Vendor/TTZipVendor.xcframework)                            │
│      - Swift 6 门面与强类型编排层 (Sources/TTZipCore)                                      │
│      - 纯 Rust POSIX CLI 与终端 TUI 浏览器 (rust/ttzip-tui -> bin/ttzip)                  │
│      - 多语言 SDK 矩阵 (sdk/ 覆盖 C11, C++20, Go, JVM/Kotlin, C#, Dart, Node, Python)     │
│      - 全管线基准测试与差分测试套件 (Sources/TTZipBench)                                   │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ 3. LarkSync (GitHub: https://github.com/wittkung/LarkSync.git, 独立业务插件仓库)             │
│    • 实体: 完全独立的飞书知识库双向同步生态插件 Git 仓库 (本地位于 /studio-lab/larksync)        │
│    • 核心职责:                                                                           │
│      - 飞书 OpenAPI 差异状态机与 Rust FFI (LarkSyncCore / larksync_ffiFFI)               │
│      - 飞书专属 UI 面板与米勒列 (LarkSyncUI)                                              │
│      - 动态插件生命周期入口 (LarkSyncPlugin.swift)                                        │
│      - 所见即所得 Markdown 渲染引擎 (TTMarkdownKit)                                       │
│    • 分发机制:                                                                           │
│      - 独立编译打包为 `LarkSync-v1.0.1.ttplugin.zip` (含 Ed25519 签名与 SHA-256)            │
│      - 托管在 GitHub Releases，并通过官方 `marketplace.json` 索引供 TTZip 客户端一键安装      │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Ⅱ. 构建与测试命令速查 (Build & Test Matrix)

### 1. Apple 桌面客户端 (工作区 `apple/`)
```bash
cd apple

# 编译桌面客户端及内置插件框架 (Debug / Release)
swift build
swift build -c release

# 运行 UI、设计系统、状态机与插件注册测试套件
swift test

# 运行单文件 LOC 架构门禁 (Hard Threshold: 800 LOC)
./scripts/lint_loc_gate.sh

# 打包分发版独立应用 (.app)
./scripts/bundle_app.sh --channel direct

# 打包 Mac App Store 沙盒应用
./scripts/bundle_app.sh --channel mas
```

### 2. Core 引擎微内核 (工作区 `core/`)
```bash
cd core

# 构建 Rust 微内核并生成 UniFFI 绑定与 XCFramework
./scripts/build_rust.sh

# 编译 Swift 6 Core 库与测试目标
swift build
swift build -c release

# 运行 Swift 单元与集成测试 (Actor 隔离, VFS 搜索, UniFFI 桥接)
swift test --parallel

# 运行 Rust Workspace 单元测试
cd rust && cargo test --workspace && cd ..

# 运行全管线基准测试与 CI 门禁
swift run ttzip-bench gate
swift run ttzip-bench pipeline

# 运行本地完整 CI 门禁
./scripts/run_local_ci_gate.sh
```

---

## Ⅲ. 核心架构宪章与系统工程铁律

1. **100% Mozilla UniFFI 跨语言标准**:
   - Swift 6、Python、Kotlin 等所有 Tier-1 SDK 必须 100% 基于 Mozilla UniFFI Proc-Macro 自动生成安全绑定与内存屏障。
   - 严禁手写跨语言非受管裸指针。
   - 所有计算密集、I/O 密集、密码哈希、VFS 树形结构与原地改写逻辑统一在 Rust 内核实现。

2. **Swift 6 纯表现层边界**:
   - Swift 专职负责 SwiftUI 声明式渲染、`@Observable` 状态流管理、macOS 专有框架（QuickLook, FinderSync, Keychain, AVFoundation）及 UniFFI 强类型调用。

3. **四大系统工程铁律**:
   - **流式第一性 (Stream-First)**: 消除全量内存假设；数据流面向微缓冲与分块流式管道；单任务常驻内存 $\le 64\text{MB}$。
   - **纵深防御 (Invariant-First)**: 零内存分配路径消毒 (`path_sanitizer.rs`) 彻底免疫 Zip-Slip 与 TOCTOU 攻击；算术调用 CPU 防溢出指令。
   - **确定性确界 (Bounds-First)**: 密码与敏感内存必须调用 `zeroize` / `SecureBytes` 擦除；跨语言数值必须经过 `SSIZE_MAX` Clamp。
   - **真实预言机 (Oracle-First)**: 自动化测试面向真实边界用例；自研引擎与系统原生 `/usr/bin/tar` / `/usr/bin/unzip` 执行双向差分测试。

4. **严格单文件 LOC 门禁 ($\le 800$ LOC)**:
   - 单文件行数硬性上限 800 行，目标均值 $\le 350$ 行。超限阻断 CI 流水线。

5. **绝对零编译告警 (Zero-Warning Hard Gate)**:
   - 无论是 Debug、Release 还是 Test Target，必须无条件通过 `-warnings-as-errors`。

---

## Ⅳ. 统一 SPDX 版权与注释标准 (Strict Invariants)

所有源文件顶部必须严格包含对应的标准 SPDX 版权声明，严禁混杂错误声明：

### 1. `apple/` 目录源码（macOS 桌面端与插件 SDK）
```swift
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
```

### 2. `core/` 目录源码（跨平台微内核与核心库）
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

---

## Ⅴ. 交互纪律与零冗余执行原则 (Pi Operating Discipline)

1. **直接行动，拒绝废话 (Act First, Speak Minimal)**:
   - 回复只陈述事实（做了什么、看到了什么、验证了什么），彻底剥离情绪化、戏剧化修辞与自我评价。
2. **全自动自主子 Agent 调度 (Proactive Subagent Dispatch)**:
   - 凡涉及 $\ge 2$ 个独立文件的修改、大范围探索、测试回归，100% 自主发起 `invoke_subagent` 并行推进。
3. **外科手术式精确替换 (`replace_file_content`)**:
   - 严禁盲目全量覆写修改已有代码，必须通过精确子串切片替换，确保上下文唯一匹配。
4. **物理落盘工作账本 (Master Scratchpad)**:
   - 多步任务必须在物理账本中全量记录，步步对账核销，杜绝长上下文下的注意力衰减与掉球。
