# Feature Specification: TTZip 现代化终端交互式 TUI 与独立 CLI 引擎 (Feature 170)

**Feature ID**: `170-rust-interactive-tui-engine`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Terminal User Experience, Standalone CLI Distribution, In-Process Zero-Cost TUI)

---

## 1. Executive Summary & Background

TTZip 当前已拥有高性能的 Rust 核心胶水与计算引擎（`ttzip-glue`），并通过了 859 个 Swift 单元测试与 85+ 个 Rust 工业级测试。
然而，终端用户在使用 CLI 时通常面临以下痛点：
1. **静态黑盒执行缺乏直观反馈**: 命令行解压/压缩几十 GB 大文件时，传统 CLI 仅能输出单行进度或无交互文本，无法直观浏览归档内部的多级目录树；
2. **免解压查看成本高**: 用户若只想快速查看某个归档中的某个文本文件、配置或图片元数据，传统方案必须全量或部分解压到临时目录；
3. **缺乏即时模糊搜索**: 在包含数万个条目的巨型归档中，难以在终端内毫秒级定位目标文件并选择性提取；
4. **分发依赖环境**: 现有的 Swift CLI (`Sources/TTZipCLI`) 依赖 macOS 系统环境与 Swift 运行时，难以独立打包为纯净的跨平台单文件二进制。

本特性的目标是：**在 `rust/` 工作区构建全新的 `ttzip-tui` 原生终端应用程序，基于 `ratatui` + `crossterm` 提供极速交互式归档浏览器、即时模糊搜索、终端免解压 QuickLook 预览、实时多核算力/吞吐看板，并输出独立的 Universal Mach-O 单可执行文件。**

---

## 2. User Scenarios

### User Scenario 1 (US1) - 终端即时交互式归档浏览与选择性提取 (Interactive Tree Navigation & Selective Extraction)
- **As a**: 深度使用终端与 Tmux/Ghostty 的开发者
- **I want to**: 在终端输入 `ttzip archive.7z` 直接进入全屏 TUI 界面
- **So that**: 使用键盘方向键或 `j/k` 键上下滚动多级目录树，按 `Space` 标记文件，按 `Enter` 仅解压选中的文件至当前目录。

### User Scenario 2 (US2) - 终端免解压即时预览 (In-Terminal QuickLook Preview)
- **As a**: 审查归档内容的工程师
- **I want to**: 选中归档内某个 `.rs` / `.json` / `.txt` 或二进制文件并按下 `p` 或 `Tab`
- **So that**: TUI 侧边栏即时以语法高亮（`syntect`）展示文本内容，或以格式化 Hex Dump 呈现二进制数据，完全不触碰磁盘 I/O。

### User Scenario 3 (US3) - 毫秒级即时模糊搜索 (Instant Fuzzy Search & Filtering)
- **As a**: 在包含 50,000+ 文件的源码备份包中寻找特定头文件的用户
- **I want to**: 按下 `/` 键呼出搜索栏并输入文件名片段
- **So that**: 目录树以毫秒级响应过滤出匹配结果并高亮匹配字符，按下 `Enter` 直接跳转。

### User Scenario 4 (US4) - 实时多核负载与吞吐性能看板 (Live Performance & Throughput Dashboard)
- **As a**: 处理大体积归档的用户
- **I want to**: 在执行解压或压缩操作时观察进度弹窗
- **So that**: 实时看到 Apple Silicon P-Core / E-Core 算力使用率、瞬时吞吐量（GB/s）、已处理字节与剩余时间，且随时按下 `Esc` 或 `q` 在毫秒级内原子取消。

### User Scenario 5 (US5) - 单二进制 CLI 与 TUI 统一命令行调度 (Unified CLI Subcommands & Headless Mode)
- **As a**: 编写 Shell 自动化脚本的用户
- **I want to**: 既可以使用 `ttzip x archive.zip -o ./out` 进行无头快速解压，也可以直接运行 `ttzip` 进入交互界面
- **So that**: 一个单文件可执行程序满足日常交互与 CI/脚本自动化双重诉求。

---

## 3. Functional Requirements

### REQ-001: Rust TUI Crate 结构与 CLI 参数分发 (Crate Setup & Clap CLI Dispatch)
- 在 `rust/Cargo.toml` 中新增成员 `ttzip-tui`，输出 binary 可执行文件；
- 使用 `clap` (Derive) 解析命令行参数：
  - 交互模式: `ttzip <archive_path>` 或直接运行 `ttzip` 开启文件选择器；
  - 命令行子命令: `ttzip x/extract <archive>`, `ttzip c/create <archive> <sources...>`, `ttzip l/list <archive>`.

### REQ-002: 基于 `ratatui` 的即时模式渲染布局 (Immediate-Mode UI Layout)
- 根布局划分为三段式：
  - Header: 归档基本信息（格式、文件大小、总条目数、压缩算法、加密方式）；
  - Body: 双栏/三栏布局（左侧：目录树与条目表格；右侧：条目详细属性元数据与 QuickLook 预览窗口）；
  - Footer: 实时状态栏、全局快捷键提示与 CPU 核心负载指标。

### REQ-003: 归档 VFS 虚拟文件系统与即时模糊搜索 (Archive VFS & Fuzzy Matcher)
- 解析 `ttzip-glue` 导出的归档元数据，在内存中构建层级 `VfsTree`；
- 集成 `fuzzy-matcher`，在用户键入时实现 $< 5\text{ms}$ 响应的实时模糊匹配与条目高亮过滤。

### REQ-004: 免解压流式即时预览引擎 (In-Terminal Stream Preview Engine)
- 文本文件：通过 `ttzip-glue` 内存流式解压前 64KB 数据，基于 `syntect` 进行 ANSI 语法高亮渲染；
- 二进制文件：格式化输出 16 字节对齐的 Hex Dump（带 ASCII 侧边栏与地址偏移）；
- 严格限制预览缓冲区内存，常驻内存 $\le 16\text{MB}$。

### REQ-005: 多核流式吞吐监控与原子取消弹窗 (Live Dashboard & Atomic Cancellation)
- 实时通过通道（`crossbeam-channel`）接收底层解压/压缩管道的字节计数；
- 计算实时瞬时吞吐量（MB/s）、ETA 剩余时间并绘制 Ratatui `Gauge` 进度条与 Sparkline 吞吐波动图；
- 监听 `Esc` / `q` / `Ctrl+C` 信号，调用 `CancellationToken::cancel()` 实现 $< 5\text{ms}$ 确定性安全终止与临时文件回滚。

### REQ-006: 独立 Universal 静态二进制编译 (Universal Standalone Packaging)
- 编写 `scripts/build_tui.sh`，生成合并 `aarch64` 与 `x86_64` 的 Universal 二进制产物 `bin/ttzip`；
- 零动态依赖系统 Swift 运行时，可独立分发与直接运行。

---

## 4. Success Criteria

1. **渲染帧率与交互流畅度**: 终端目录树滚动与模糊搜索达到 60+ FPS，输入到屏幕重绘延迟 $\le 16\text{ms}$；
2. **大文件浏览瞬时打开**: 在包含 50,000+ 条目的归档中，TUI 启动到完整目录树渲染完成耗时 $\le 100\text{ms}$；
3. **取消确界**: 任务中途取消后，工作线程 100% 优雅退出，无悬挂句柄与磁盘残留；
4. **单文件二进制独立性**: 生成的 `bin/ttzip` 在纯净 macOS 系统上单文件直接运行，无外部 dylib 依赖；
5. **单元与集成测试全绿**: TUI 状态机、VFS 构建、模糊过滤与命令行参数解析测试 100% 通过。
