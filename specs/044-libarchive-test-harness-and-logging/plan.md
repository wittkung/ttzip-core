# Implementation Plan: 044-libarchive-test-harness-and-logging

**Feature Branch**: `044-libarchive-test-harness-and-logging`  
**Created**: 2026-08-17  
**Status**: Plan Ready  
**Spec**: [spec.md](./spec.md)  

---

## 1. Technical Context & Motivation

为了彻底解决 GitHub Actions 云端算力配额耗尽的问题，同时吸收 libarchive 20 年工业级测试体系精髓，TTZip 需要构建原生的**本地测试中枢与高精诊断系统**：
1. **高性能 HexDump 差分引擎 (`FastHexDiffEngine`)**：支持 16 字节对齐窗口截断、首个分歧偏移量快速跳跃与查表法零堆分配。
2. **Unicode 标量深度展开与编码诊断 (`UnicodeDiagnosticFormatter`)**：输出 UTF-8 字符序列的精确 Unicode 码点与 APFS NFD 冲突检测。
3. **线程安全原子化日志收集器 (`TestLogCollector`)**：`os_unfair_lock` + 内存隔离，成功用例 0% 日志噪音，失败用例单次原子输出，彻底消除多线程日志撕裂。
4. **升级 `ttzip-cli test` 驱动器**：提供 `-v`、`-vv`、`-q`、`-k`、`--filter`、`--fast`、`--dump-on-failure` 等标准化参数，并输出美观的 Markdown/JSON 报告。
5. **本地 CI 编排脚本 (`./scripts/run_local_ci.sh`)**：一键在本地调度静态代码分析、Native 诊断、全量回归与性能门禁。

---

## 2. Constitution & Performance Invariant Check

- [x] **热路径零成本抽象**：`failure()` 延迟上下文机制在断言成功时不执行任何字符串拼接；`FastHexDiffEngine` 查表法单次缓冲，无中间堆碎片。
- [x] **Swift 6 严格并发安全**：`TestLogCollector` 与 `DiagnosticFormatter` 均保证 `@unchecked Sendable` 或不可变值语义，杜绝数据竞争。
- [x] **四大系统工程铁律**：
  - **流式第一性**：HexDump 窗口截断在 256 字节，防止全量大对象刷屏。
  - **确定性确界**：所有差分窗口计算经过 `min/max` Clamp 保护，防止越界。
  - **真实预言机**：全流程对接 libarchive 30+ 黄金语料库与 Native 往返校验。
- [x] **开源上游隔离硬闸门**：所有改动严格收敛在 TTZip 本地仓库内部，严禁对 upstream 远端产生副作用。

---

## 3. Phase 0: Research Outline

- R001 [SUBAGENT:research] 《Libarchive 原生测试驱动框架 (test_main.c) 调度与失败上下文机制》：调研 libarchive 参数解析、用例注册、`failure()` 延迟上下文与 `strdump()` 输出。 (见 [research.md](./research.md#r001-libarchive-原生测试驱动框架-test_mainc-调度与失败上下文机制))
- R002 [SUBAGENT:research] 《TTZip 现有命令行工具 (TTZipCLI) 架构与本地测试中枢扩展》：调研 `Sources/TTZipCLI/` 结构与 `CLIBenchmarkRunner` 接口，规划 `ttzip-cli test` 升级。 (见 [research.md](./research.md#r002-ttzip-现有命令行工具-ttzipcli-架构与本地测试中枢扩展))
- R003 [SUBAGENT:research] 《Swift 6 平台下高性能 HexDump、Unicode 标量展开与线程安全日志缓冲》：设计快速分歧跳跃算法、Unicode 码点展开与 `TestLogCollector` 原子派发。 (见 [research.md](./research.md#r003-swift-6-平台下高性能-hexdumpunicode-标量展开与线程安全日志缓冲))

---

## 4. Phase 1: Design Artifacts Index

- **数据模型**：[data-model.md](./data-model.md)（定义 `TestExecutionSession`, `TestCaseResult`, `TestFailureEvidence`, `HexDiffSlice`, `UnicodeDiffDetail`）
- **契约规范**：
  - [test_report_schema.json](./contracts/test_report_schema.json)（测试执行报告强类型 JSON Schema）
  - [cli_options_schema.json](./contracts/cli_options_schema.json)（CLI 测试参数强类型 JSON Schema）
- **快速验证指南**：[quickstart.md](./quickstart.md)（4 大场景：`--fast` 快速诊断、`--filter` 详细报告、持久化生成、`./scripts/run_local_ci.sh` 一键回归）

---

## 5. Planned Codebase Changes

### Layer 1: 核心诊断与断言框架 (`Sources/TTZipCore/Testing/`)
- **[NEW] `DiagnosticContext.swift`**: 线程/Task 局部的延迟失败上下文机制 (`failure(message)`)。
- **[NEW] `FastHexDiffEngine.swift`**: 64 字节快速跳跃分歧扫描与 16 字节对齐零堆分配 HexDump 格式化。
- **[NEW] `UnicodeDiagnosticFormatter.swift`**: 逐标量 Unicode 码点格式化与 NFD/NFC 冲突检测。
- **[NEW] `TestLogCollector.swift`**: `os_unfair_lock` 保护的线程安全日志缓冲器与 POSIX 原子刷新。
- **[MODIFY] `Tests/TTZipTests/TTZipAssertions.swift`**: 接入 `FastHexDiffEngine` 与 `UnicodeDiagnosticFormatter`，升级原语断言为高精诊断输出。

### Layer 2: 命令行测试中枢 (`Sources/TTZipCLI/`)
- **[MODIFY] `Sources/TTZipCLI/CLIOptions.swift`**: 扩展测试相关参数 (`filterPattern`, `verbosity`, `keepTempFiles`, `dumpOnFailure`, `fast`, `jsonReportPath`, `markdownReportPath`)。
- **[MODIFY] `Sources/TTZipCLI/CLIArgumentParser.swift`**: 支持解析上述新增测试标志。
- **[NEW] `Sources/TTZipCLI/TestCommand.swift`**: 独立实现测试驱动命令处理器，支持 Native 诊断模式与 XCTest 进程外桥接。
- **[NEW] `Sources/TTZipCLI/XCTestEventStreamParser.swift`**: 原生 Swift 解析 `swift test` 输出事件流，驱动 ANSI 树状渲染。
- **[NEW] `Sources/TTZipCLI/TestReportGenerator.swift`**: 统一生成控制台 ANSI 树、Markdown 报告与 JSON 数据。
- **[MODIFY] `Sources/TTZipCLI/CLICommandRouter.swift`**: 转发 `test` 子命令到 `TestCommand`。

### Layer 3: 本地自动化 CI 流水线 (`scripts/`)
- **[NEW] `scripts/run_local_ci.sh`**: 工业级一键式本地 CI/CD 编排脚本，支持 `--quick`、`--full`、`--sanitize`。
