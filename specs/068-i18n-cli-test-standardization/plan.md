# Implementation Plan: 全面国际化、CLI 标准化与测试体系专业化构建 (Implementation Plan)

**Branch**: `068-i18n-cli-test-standardization` | **Date**: 2026-08-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/068-i18n-cli-test-standardization/spec.md`

---

## Summary

构建 TTZip 工业级三大基础设施基石：
1. **全面类型安全国际化与动态多语言体系**（下沉至 `TTZipCore`，支持 7 种主流语言包、SwiftUI 免重启热更、POSIX 环境变量自动解析与单位/复数/错误模板化绑定）。
2. **企业级 POSIX / GNU 规范 CLI 与 TTY 自适应双模流式架构**（支持短选项合并、`--` 截断符、60Hz 帧率收敛 Unicode 进度条、NDJSON 机器可读流、POSIX `<sysexits.h>` 强类型退出码、`-` 标准 I/O 管道流式对接及 Shell 补全与 Man Page 自动生成）。
3. **6 层分级自动化测试体系与全格式性能门禁中枢**（划分 Tier 0 ～ Tier 5 执行分层、构建 100% 国际化双向完整性与占位符类型安全断言套件、全 16 格式历史峰值硬门禁守门、以及 JUnit XML / JSON / Markdown 三模测试报告引擎）。

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.  
**Primary Dependencies**: 100% In-Process C 静态库 (`Vendor/*.a`: libarchive, libdeflate, fast-lzma2, zstd, libb2, liblz4, uchardet, Sparkle 2.6.0 for Direct channel). Zero external CLI subprocesses.  
**Target Platform**: macOS 14.0+ (Sonoma), Apple Silicon (ARM64 NEON) 优先，兼容 Intel (x86_64).  
**Project Architecture**:
- `Sources/CTTZipBridge`: C 底层桥接与硬件加速。
- `Sources/TTZipCore`: Swift 核心引擎、归档管道、设计模式体系、国际化中枢与测试基础设施。
- `Sources/TTZipApp`: SwiftUI + AppKit 桌面应用 (MVVM + `@MainActor`)。
- `Sources/TTZipCLI`: 独立命令行工具 (`ttzip-cli`)。
- `Tests/TTZipTests`: 127+ 测试套件 (Tier 0 ～ Tier 5)。  
**Testing Framework**: XCTest + 自研 `TestRunnerScheduler` + `TTZipAssertions` + `AsyncBenchmarkRunner`。  
**Performance Goals**:
- i18n 查表耗时 $< 20\text{ns}$，零中间堆分配。
- CLI TTY 终端渲染频率严格收敛在 $\le 60\text{Hz}$，CPU 额外开销 $< 0.5\%$。
- 16 种格式性能门禁 100% 锚定历史最优峰值 `604d44d`（$\Delta < -10.0\%$ 绝对阻断）。  
**Constraints**:
- 严禁在热路径（`Zip/`、`CTTZipExtract.c` 等）引入动态对象树或分配。
- 严禁在独立 CLI Target 中依赖 `Bundle.module`（避免无 Bundle 独立二进制崩溃）。
- 严禁使用普通 `memset` 擦除敏感密码（必须使用 `volatile` / `memset_s`）。

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

- [x] **Core Architecture & Tech Stack Boundaries**: Swift 6.0, in-process C static library bindings, MAS sandbox / Direct dual distribution supported.
- [x] **Zero-Cost Abstraction on Hot Paths**: 国际化查表与 CLI 渲染逻辑完全处于控制平面/调度层，严禁侵入编解码并行数据平面。
- [x] **No Shared Locks on Concurrent Paths**: 国际化字典采用不可变静态表，无锁并发读取。
- [x] **Streaming-First & Zero-Memory Assumption**: CLI 管道流支持零拷贝流式透传与 APFS 双阶 Spooling，内存恒定 $O(1)$。
- [x] **Invariant-First & Defensive Security**: POSIX 原语级路径安全，ZipSlip 防御，密码与敏感内存物理擦除。
- [x] **Deterministic Bounds & Magic Lifecycles**: 结构体 magic 生命周期，整型 narrowing clamp。
- [x] **Oracle-First & Crash-First Fuzzing**: 历史缺陷黄金语料库 (`.uu`)，系统原生工具差分预言机，模糊测试变异样本优先落盘。
- [x] **Logging Discipline**: 零裸 `print(...)` 泄露至数据管道，全部收敛至 `TTLogger` 与 `TerminalRenderEngine`。

---

## Phase 0: Outline & Research

- [x] - R001 [SUBAGENT:research] 《跨端统一类型安全本地化与动态多语言架构》：SPM 跨模块嵌入式字典、7 语言包、SwiftUI 免重启热更与 POSIX 环境变量解析。
- [x] - R002 [SUBAGENT:research] 《企业级 POSIX/GNU CLI 架构、终端渲染与流式管道标准化》：长短选项、`--` 截断符、TTY 自适应与 60Hz 节流、NDJSON 流、`<sysexits.h>` 退出码、标准流管道与 Shell 补全/Man Page 生成。
- [x] - R003 [SUBAGENT:research] 《6 层分级自动化测试体系与性能门禁/预言机标准化架构》：Tier 0 ～ Tier 5 执行分层、100% 国际化双向完整性与占位符类型断言、全 16 格式历史峰值硬门禁、JUnit XML / JSON / Markdown 三模报告引擎。

**Phase 0 Output**: [research.md](./research.md) (全部 R001-R003 决策与研究源已闭环落地)

---

## Phase 1: Design & Contracts Index

### 1.1 Data Model
- [data-model.md](./data-model.md) 详细定义本地化、CLI 交互模型、TTY 渲染上下文与测试分层/报告模型。

### 1.2 Interface Contracts (Zero Bare Objects)
- [SUBAGENT:research] [contracts/i18n-catalog-contract.json](./contracts/i18n-catalog-contract.json): 7 语言本地化资源包强类型 JSON Schema。
- [SUBAGENT:research] [contracts/cli-command-contract.json](./contracts/cli-command-contract.json): POSIX CLI 参数、选项与退出码 JSON Schema。
- [SUBAGENT:research] [contracts/cli-event-ndjson-contract.json](./contracts/cli-event-ndjson-contract.json): 机器可读 NDJSON 进度与事件流协议 JSON Schema。
- [SUBAGENT:research] [contracts/test-report-contract.json](./contracts/test-report-contract.json): JUnit XML 与 JSON 结构化测试报告 JSON Schema。

### 1.3 Validation Quickstart Guide
- [quickstart.md](./quickstart.md) 包含可执行验证命令、预期输出与失败排查路径。

---

## Project Structure & Component Breakdown

```text
TTZip/
├── Sources/
│   ├── TTZipCore/
│   │   ├── Localization/                     # [NEW] 强类型国际化与多语言中枢
│   │   │   ├── AppLanguage.swift             # 7 种语言枚举 (en, zh-Hans, zh-Hant, ja, de, fr, es)
│   │   │   ├── LocaleKey.swift               # 强类型命名空间枚举 (Common, Compress, Extract, Error, Bench)
│   │   │   ├── TTZipLocalizationManager.swift# 本地化调度器 (级联回退、线程安全、POSIX 解析)
│   │   │   ├── Catalogs/                     # 7 种语言静态嵌入式字典
│   │   │   │   ├── LocaleCatalog+En.swift
│   │   │   │   ├── LocaleCatalog+ZhHans.swift
│   │   │   │   ├── LocaleCatalog+ZhHant.swift
│   │   │   │   ├── LocaleCatalog+Ja.swift
│   │   │   │   ├── LocaleCatalog+De.swift
│   │   │   │   ├── LocaleCatalog+Fr.swift
│   │   │   │   └── LocaleCatalog+Es.swift
│   │   │   └── Formatters/                   # 本地化单位与复数格式化器
│   │   │       ├── ByteSizeFormatter.swift   # IEC KiB vs SI KB 格式化
│   │   │       ├── ThroughputFormatter.swift # 吞吐速率与本地化千分位/小数点
│   │   │       └── PluralRuleEngine.swift    # 零分配复数规则引擎
│   │   └── Testing/                          # [NEW] 测试框架基础设施
│   │       ├── TestTier.swift                # Tier 0 ～ Tier 5 强类型枚举与调度策略
│   │       ├── TestReportModel.swift         # 结构化测试报告领域模型
│   │       └── JUnitReportBuilder.swift      # JUnit XML 标准报告构建器
│   ├── TTZipCLI/
│   │   ├── POSIXCLIArgumentParser.swift      # [NEW] POSIX/GNU 长短选项与截断符解析器
│   │   ├── CLIExitCode.swift                 # [NEW] POSIX <sysexits.h> 强类型退出代码
│   │   ├── TerminalRenderEngine.swift        # [NEW] TTY 感知、宽度自适应与 60Hz 节流渲染器
│   │   ├── StreamPipeAdapter.swift           # [NEW] '-' stdin/stdout 管道流式对接与双阶 Spooling
│   │   ├── CLICommandSpec.swift              # [NEW] 声明式命令元数据与 Shell 补全/Man Page 生成器
│   │   ├── CLICommandRouter.swift            # [MODIFY] 全面升级子命令分发、国际化与管道支持
│   │   ├── CLIOptions.swift                  # [MODIFY] 扩展标准选项与语言参数
│   │   └── TestCommand.swift                 # [MODIFY] 支持 --tier, --format, --report-junit
│   └── TTZipApp/
│       ├── Services/
│       │   └── AppLanguageStore.swift        # [NEW] SwiftUI @Observable 动态响应式语言管理
│       └── Views/                            # [MODIFY] 全量替换硬编码文本为 L10n 键
└── Tests/
    └── TTZipTests/
        ├── LocalizationIntegrityTests.swift  # [NEW] 国际化 100% 完备性与占位符类型断言套件
        ├── CLIPOSIXStandardTests.swift       # [NEW] CLI POSIX 规范、退出码与管道测试
        └── TestTierClassificationTests.swift # [NEW] 6 层分级执行与调度测试
```

---

## Verification Plan

### Automated Tests
1. **i18n 完整性与占位符类型断言测试**:
   `swift test --filter LocalizationIntegrityTests`
   - 断言 100% `LocaleKey` 存在于全部 7 种语言包。
   - 断言所有语言包格式化占位符（`%@`, `%d`, `%lld`）数量与类型绝对一致。
2. **CLI POSIX 规范与退出码测试**:
   `swift test --filter CLIPOSIXStandardTests`
   - 验证长短选项合并、`--` 截断符、标准退出码 `0/64/65/66/73/74`。
   - 验证非 TTY 管道下的 NDJSON 输出与零 ANSI 乱码泄漏。
3. **全格式性能硬门禁回归测试**:
   `swift test --filter XCTestPerformanceMeasureTests`
   - 验证 16 种格式吞吐不跌破历史最优基准 `604d44d`。
4. **全量回归测试**:
   `swift test`
   - 确保全套 530+ 测试 100% 通过。
