# Implementation Plan: Native High-Aesthetic Test Logging, Harness & Reporter (111-native-test-logging-and-reporter)

## Technical Context

- **Target Ecosystem**: Swift 6.0 (`swift-tools-version: 6.0`), macOS 14.0+ Sonoma, Apple Silicon & Intel.
- **Core Motivation**: Eliminate cluttered, unformatted multiline test logs from XCTest / CLI test runs, replacing them with a pristine, high-signal, aligned terminal stream and diagnostic reporting system inspired by `zlib-ng` and `libarchive`.
- **Key Invariants**:
  - 100% In-Process Swift / POSIX implementation (Zero external tool dependencies like `xcbeautify` or `xcpretty`).
  - Zero lock contention on hot test execution paths via TaskLocal per-task buffering.
  - Silent-on-success, detailed-on-failure diagnostic cards.
  - 100% SPDX copyright compliance and zero compiler warnings.

## Constitution Check

- [x] 熱路徑零成本抽象 (Zero-cost abstraction on hot execution paths)
- [x] 跨架構確定性与安全 (Cross-architecture determinism & memory bounds)
- [x] 100% 原生 C / Swift 绑定 (Zero external subshell or CLI tool glue)
- [x] 统一 SPDX 版权头部 (SPDX headers on all new/modified files)
- [x] 零编译告警门禁 (-warnings-as-errors compliance)

## Phase 0: Research Index

- - R001 [SUBAGENT:research] 《zlib-ng / CTest 与 libarchive 测试流水线与控制台渲染美学体系研究》：分析经典 C 基础库的固定宽度单行对齐、分类徽章与全景汇总仪表盘设计。
- - R002 [SUBAGENT:research] 《Swift 原生无锁高效 TestLogger 体系设计》：研究 TaskLocal 隔离的无锁每任务日志缓冲、四级日志级别与 TTY / NO_COLOR 探测。

## Phase 1: Design Artifacts Index

- `data-model.md`: Data structures for `TestLogLevel`, `TestBadgeType`, `TestExecutionRecord`, `TestFailureContext`, `TestRunSummary`.
- `contracts/test_telemetry_event.json`: Discriminated NDJSON schema for test execution and telemetry stream.
- `contracts/test_run_summary.json`: Schema for end-of-run executive summary reports.
- `quickstart.md`: Automated validation scenarios and failure diagnostics.

## Component Modifications

### 1. `Sources/TTZipCore/Testing/`
- **`TestLogger.swift` [NEW]**: Centralized, thread-safe test logging subsystem supporting `.silent`, `.normal`, `.verbose`, `.debug` log levels, TaskLocal buffer capturing, and atomic single-chunk dumping on failure.
- **`TestTerminalRenderer.swift` [MODIFY]**: Enhance fixed-width alignment, Kintsugi gold badge aesthetics, Unicode/ASCII dual box rendering, and seamless TTY auto-detection.
- **`TestTelemetryStream.swift` [MODIFY]**: Enhance NDJSON serialization and dispatch with `TestLogger` integration.

### 2. `Sources/TTZipCLI/`
- **`TestCommand.swift` [MODIFY]**: Upgrade `ttzip-cli test` runner to render the full zlib-ng/libarchive aligned streaming table and end-of-run executive summary card.

### 3. `scripts/`
- **`run_local_ci_gate.sh` [MODIFY]**: Upgrade stage table and test execution log formatting to use high-contrast ANSI status badges and structured stage timing.
- **`run_all_tests.sh` [MODIFY]**: Wrap full test run in high-aesthetic terminal frame.

### 4. `Tests/TTZipTests/`
- **`TestTelemetryAndRendererTests.swift` [MODIFY]**: Add unit tests for `TestLogger`, TaskLocal isolation, TTY detection, and aligned table rendering.
