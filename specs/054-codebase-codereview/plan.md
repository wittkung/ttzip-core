# Implementation Plan: Full Codebase Architecture, Security & Testing Remediation

**Branch**: `054-codebase-codereview` | **Date**: 2026-08-17 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/spec.md)

**Input**: Full Codebase Code Review findings across Systems C, Swift Core Engines, 28 Design Patterns, Desktop UI, and Test/Benchmark oracles.

---

## Summary

This plan outlines the end-to-end technical remediation across the 5 core project domains to fix all 30 `[MUST]` blockers and 27 `[SHOULD]` recommendations uncovered during the multi-agent code review. It establishes a dedicated CI codebase invariant linter, eliminates C memory overflow and insecure crypto fallbacks, removes duplicate execution and lock contention in Swift 6 engines, implements UI progress event throttling ($\le 60\text{Hz}$), and completes the dual-direction system differential test oracle with strict 90% historical peak performance floor protection.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.  
**Primary Dependencies**: `libarchive.a`, `liblzma.a`, `liblz4.a`, `libb2.a`, `libzstd.a`, `libdeflate.a`, Apple Silicon ARM NEON Crypto Extensions, AppKit, SwiftUI.  
**Storage**: Memory-mapped files (`mmap`), POSIX direct I/O, macOS Keychain Services (`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`).  
**Testing**: XCTest, `SystemDifferentialTests`, `ArchiveGoldenCorpusTests`, `ArchiveMutationFuzzTests`, `XCTestPerformanceMeasureTests`, `PerformanceRegressionGuardTests`.  
**Target Platform**: macOS 14.0+ (Apple Silicon arm64 prioritized, x86_64 compatible).  
**Project Type**: High-performance Native macOS Desktop Application + CLI + In-process C FFI Core Engine.  
**Performance Goals**:
- ZIP L1 Compression: $\ge 1500\text{ MB/s}$ (Debug) / $\ge 2000\text{ MB/s}$ (Release)
- ZIP Decompression: $\ge 7500\text{ MB/s}$ (Debug) / $\ge 10000\text{ MB/s}$ (Release)
- 7Z L1 Compression: $\ge 3200\text{ MB/s}$ (Debug) / $\ge 3900\text{ MB/s}$ (Release)
- 50k Directory Tree Build: $\le 250\text{ ms}$ ($\ge 250,000\text{ items/s}$)
- UI Progress Event Dispatch: Throttled to $\le 60\text{Hz}$ with $\ge 97\%$ event suppression.  
**Constraints**: Zero bare `print`/`printf`/`NSLog`, zero `Data(count:)` in hot paths, zero `NSLock` inside `concurrentPerform`, zero hardcoded `/Users/` paths.  
**Scale/Scope**: ~230 source & test files across `Sources/CTTZipBridge/`, `Sources/TTZipCore/`, `Sources/TTZipApp/`, `Sources/TTZipCLI/`, `Tests/TTZipTests/`.

---

## Constitution Check

*GATE: All items evaluated against `.specify/memory/constitution.md`.*

| Invariant / Rule | Pre-Design Status | Post-Remediation Design Status | Status |
| :--- | :--- | :--- | :---: |
| **Stream-First (Zero-Memory Assumption)** | `Data(count:)` in pipeline engine; monolithic solid alloc | Uninitialized raw pointer buffers + `Data(bytesNoCopy:)` | ✅ PASS |
| **Invariant-First (POSIX AT-API & TOCTOU)** | Missing `O_NOFOLLOW` in direct I/O & tar zstd | Explicit `O_NOFOLLOW` + `ARCHIVE_EXTRACT_SECURE_SYMLINKS` | ✅ PASS |
| **Bounds-First (Magic Sentinel & DSE Erasure)** | Missing clamp on 7z `numFilesVal`; DSE key erase bypass | Proportional slice bounds check + `ttzip_secure_zero` | ✅ PASS |
| **Oracle-First (Golden Corpus & Differential)** | Golden tests missing extraction; 1-way tar diff | Full 2-way Diff (`TTZip <-> System`) + Extractor assertions | ✅ PASS |
| **Zero Bare Logging** | Bare `print(...)` in test helpers & test files | 100% routed through `TTLogger` / `ttzip_log` | ✅ PASS |
| **Hot-Path Floor Integrity** | Guard test floor diluted to 50% | `floorRatio = 0.90` restored; 90% floor strictly enforced | ✅ PASS |

---

## Phase 0: Outline & Research

- [x] - R001 [SUBAGENT:research] 《CI 宪法静态扫描脚本架构》：设计 `scripts/lint_codebase_invariants.sh` 扫描 `/Users/` 路径、裸 print、热路径 `Data(count:)` 与并发循环锁。
- [x] - R002 [SUBAGENT:research] 《C 桥接 7z 头部安全确界与加密 Fail-Safe 防护》：在 `ttzip_7z_header_parser.c` 限制 `numFilesVal` 内存分配上限，并在 `ttzip_lzma2_enc_native.c` 杜绝密码模式降级为明文 store。
- [x] - R003 [SUBAGENT:research] 《核心引擎零拷贝微缓冲与无锁原子并发热路径改造》：在 `ArchivePipelineProducerConsumerEngine.swift` 与 `ZipMemoryEngine.swift` 使用 `UnsafeMutablePointer` 消除 `Data(count:)`，在 `SevenZipBlockParallelDecompressor.swift` 使用原子 CAS 消除 `NSLock`。
- [x] - R004 [SUBAGENT:research] 《策略与模板方法单一真理源重构与责任链并发安全迭代》：在 `ArchiveEngineStrategy.swift` 移除重复的 Bridge 执行，在 `ArchiveValidationPipeline.swift` 改为纯迭代循环消除指针竞态。
- [x] - R005 [SUBAGENT:research] 《高频 UI 进度 $\le 60\text{Hz}$ 纳秒单调时钟节流与 AppKit 状态机闭环》：在 `CompressModalView.swift` 接入 `ThrottledProgressPublisher` 与 `defer` 状态重置，在 `MainView.swift` 替换硬编码路径为 `Bundle.main`。
- [x] - R006 [SUBAGENT:research] 《双向系统差分预言机与 90% 历史峰值门禁恢复》：在 `SystemDifferentialTests.swift` 补全与系统 `/usr/bin/unzip` 及 `/usr/bin/tar` 的双向 SHA256 校验，在 `PerformanceRegressionGuardTests.swift` 恢复 `floorRatio = 0.90`。

---

## Phase 1: Design & Contracts

### Interface Contracts
- [x] - [SUBAGENT:research] [`contracts/ci_invariant_linter.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/contracts/ci_invariant_linter.schema.json): CI 静态扫描结果与违规项数据契约。
- [x] - [SUBAGENT:research] [`contracts/engine_strategy_template.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/contracts/engine_strategy_template.schema.json): 策略与模板编排上下文及执行结果契约。
- [x] - [SUBAGENT:research] [`contracts/ui_progress_throttler.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/contracts/ui_progress_throttler.schema.json): UI 纳秒级单调时钟节流事件契约。
- [x] - [SUBAGENT:research] [`contracts/system_differential_oracle.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/contracts/system_differential_oracle.schema.json): 双向系统差分测试比对预言机契约。

### Data Models & Quickstart
- [x] [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/data-model.md): 强类型数据模型与字段约束定义。
- [x] [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-codebase-codereview/quickstart.md): 4 大核心验证场景（Linter、Differential Oracle、Golden Corpus、Performance Gate）运行指南。

---

## Project Structure & Changes by Component

```text
TTZip/
├── scripts/
│   ├── [NEW] lint_codebase_invariants.sh          # 宪法级静态扫描脚本
│   ├── [NEW] lint_codebase_invariants.py          # Python 多行 AST / 闭包扫描器
│   └── [MODIFY] run_local_ci.sh                   # 注入 linter 检查阶段
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── [MODIFY] ttzip_7z_header_parser.c      # 7z Header 内存边界防护与空指针拦截
│   │   ├── [MODIFY] ttzip_lzma2_enc_native.c      # 消除密码加密失败时的明文 Store 降级 & alloca
│   │   ├── [MODIFY] ttzip_lzma2_dec_native.c      # 消除 Range-Coded 伪明文拷贝
│   │   ├── [MODIFY] CTTZipBridge_ZipWriterCore.c  # offsets 数组 malloc 空指针检查
│   │   ├── [MODIFY] CTTZipBridge_ZipWrite.c       # 密钥派生异常路径 secure_zero 内存清零
│   │   ├── [MODIFY] CTTZipUtils.c                 # 对齐分配与释放配对
│   │   └── [MODIFY] ttzip_tar_zstd_direct.c       # 对齐释放配对 & 0字节 open O_NOFOLLOW
│   ├── TTZipCore/
│   │   ├── ConcurrencyPatterns/
│   │   │   └── [MODIFY] ArchivePipelineProducerConsumerEngine.swift # 消除 Data(count:)
│   │   ├── Zip/
│   │   │   ├── [MODIFY] ZipMemoryEngine.swift     # 消除 Data(count:) & 无锁状态结果
│   │   │   └── [MODIFY] ZipDirectIOWriter.swift   # open 添加 O_NOFOLLOW
│   │   ├── SevenZip/
│   │   │   ├── [MODIFY] SevenZipBlockParallelDecompressor.swift # 消除 concurrentPerform 内 NSLock
│   │   │   └── [MODIFY] SevenZipCryptoEngine.swift# 消除 NSLock & 修复 withUnsafeBytes 指针逃逸
│   │   ├── Flyweights/
│   │   │   └── [MODIFY] MemoryPageFlyweightPool.swift # clearPool 清空 pool16K & 修复分配器混用
│   │   ├── ChainOfResponsibility/
│   │   │   └── [MODIFY] ArchiveValidationPipeline.swift # 消除并发修改 nextHandler 指针
│   │   ├── TemplateMethod/
│   │   │   ├── [MODIFY] ArchiveTemplateContext.swift    # 补齐 Builder 链式 Setter
│   │   │   └── [MODIFY] ArchiveEngineTemplateRegistry.swift # passwordRecoveryTemplate 改为 let
│   │   ├── Strategies/
│   │   │   └── [MODIFY] ArchiveEngineStrategy.swift     # 消除 Strategy 与 Bridge 双重执行
│   │   ├── Decorators/
│   │   │   └── [MODIFY] ProgressMonitoringDecorator.swift # 消除重型 ArchiveComponentTree 分配
│   │   └── PasswordVaultManager+Keychain.swift          # 添加 kSecAttrAccessible 属性
│   └── TTZipApp/
│       ├── Views/
│       │   ├── [MODIFY] MainView.swift            # 移除开发机硬编码路径
│       │   ├── [MODIFY] CompressModalView.swift   # 接入 60Hz 进度节流 & defer 状态重置
│       │   ├── [MODIFY] PasswordPromptSheetView.swift # 接入 TTSecureTextField
│       │   ├── [MODIFY] PasswordVaultView.swift   # 挂载 resetSheet / recoverSheet
│       │   └── Explorer/
│       │       └── [MODIFY] NativeArchiveOutlineView.swift # 修复节点标识脏检查
│       └── ViewModels/
│           └── [MODIFY] AppViewState.swift        # Memento 协议 MainActor 隔离
└── Tests/TTZipTests/
    ├── [MODIFY] PerformanceRegressionGuardTests.swift # 恢复 floorRatio = 0.90
    ├── [MODIFY] FrontendPerformanceGateTests.swift    # 恢复 50k 树构建 250,000 items/s
    ├── [MODIFY] SystemDifferentialTests.swift         # 补全双向解压 SHA256 差分预言机
    ├── [MODIFY] ArchiveGoldenCorpusTests.swift        # 连接解压器验证 .uu 样本提取
    ├── [MODIFY] LibarchiveGoldenCorpusTests.swift     # 添加非空 fixture 断言
    └── [MODIFY] CLICommandRouter.swift                # 移除硬编码 Silesia 路径
```

---

## Complexity Tracking

| Modification | Why Needed | Alternative Rejected Because |
| :--- | :--- | :--- |
| Uninitialized Pointer Allocation in `ZipMemoryEngine` | Eliminates $O(N)$ kernel page-zeroing CPU stalls on multi-MB decompression | `Data(count:)` is mandated by Foundation to synchronously zero memory, reducing throughput by 50%+ |
| Lock-free Atomics in `SevenZipBlockParallelDecompressor` | Eliminates lock contention in parallel GCD worker threads | `NSLock` traps to the kernel under contention, violating hot-path lock-free invariants |
| Throttled 60Hz Progress Publisher in `CompressModalView` | Prevents main thread starvation under high-speed compression | Unthrottled `Task { @MainActor }` floods runloop with tens of thousands of tasks per second |
