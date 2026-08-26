# Feature Specification: Native High-Aesthetic Test Logging, Harness & Reporter (zlib-ng & libarchive Paradigm)

**Feature Branch**: `111-native-test-logging-and-reporter`

**Created**: 2026-08-19

**Status**: Specified

**Input**: User description: "你不觉得测试 log 还是很丑吗，学习 zlib-ng，libarchive /speckit-specify"

## Clarifications

### Session 2026-08-19
- **Q1: 外部美化工具依赖选型 (External Formatter Tools vs Native In-Process)?**
  - **Decision**: 绝对禁止依赖 `xcbeautify`、`xcpretty` 等外部 npm/gem/CLI 工具。采用 100% 纯原生 Swift/POSIX In-Process Terminal 格式化与渲染引擎。
- **Q2: 视觉风格对标规范 (Aesthetic Standard Paradigm)?**
  - **Decision**: 深度吸纳 `zlib-ng`（CTest 紧凑单行对齐、固定宽度、毫秒精度、汇总矩阵）与 `libarchive`（测试分类标签 `[UNIT]`/`[ORACLE]`/`[FUZZ]`、高信噪比静默成功、显式红色失败卡片与差分对比）。
- **Q3: 统一日志架构改造路径 (Unified TestLogger Architecture)?**
  - **Decision**: 引入 `TestLogger` 核心模块，支持日志级别过滤 (`.silent`, `.normal`, `.verbose`, `.debug`)，并通过 Subagent 批量重构既有单测文件中的裸 `print()` 调用。


## User Scenarios & Testing *(mandatory)*

### User Story 1 - Clean & Aesthetic Terminal Test Execution Stream (Priority: P1)

As a developer or CI engineer running tests (`ttzip-cli test`, `swift test`, or `./scripts/run_all_tests.sh`), I want the terminal output to be clean, elegant, high-signal, and formatted similarly to premier C libraries like `zlib-ng` and `libarchive`, so that I immediately see progress, suite names, test execution times, and status badges without multiline XCTest clutter or noisy unformatted prints.

**Why this priority**: High-aesthetic, zero-noise test output directly improves developer velocity, eliminates log fatigue, and provides immediate visual feedback on regressions.

**Independent Test**: Execute `swift run ttzip-cli test` or `./scripts/run_all_tests.sh` and observe clean, aligned 1-line per test or suite stream with ANSI badges (`[ PASS ]`, `[ SKIP ]`, `[ FAIL ]`), millisecond timing, and zero raw output spam.

**Acceptance Scenarios**:
1. **Given** a developer executes test suites, **When** tests run, **Then** each completed test or suite outputs a compact, aligned row `[ 042/209 ] [ PASS ] SuiteName (1.2 ms)` with color badges.
2. **Given** all tests pass, **When** the run completes, **Then** a clean ASCII/Unicode box summary is rendered showing total tests, pass/skip/fail counts, total duration, and memory/throughput metrics without noisy stack traces.

---

### User Story 2 - High-Fidelity Failure Diagnostic & Diff Presentation (Priority: P2)

As a developer debugging a failed test, I want failure details, error messages, and binary/text diffs to be formatted in a dedicated, high-contrast failure block (inspired by libarchive's failure diagnostics and zlib-ng assertion reporting), so that the root cause, expected value, and actual value are crystal clear in 2 seconds.

**Why this priority**: When tests fail, developers need maximum clarity on the exact failure point without wading through thousands of lines of logs.

**Independent Test**: Trigger a mock failing assertion and verify the failure block formats the file path, line number, failure reason, and side-by-side diff clearly highlighted in red/yellow.

**Acceptance Scenarios**:
1. **Given** a test fails, **When** the failure is reported, **Then** a dedicated `[ FAIL ]` diagnostic card is printed containing file, line number, failure description, and clean diff.
2. **Given** non-failing tests in the same run, **When** they succeed, **Then** their internal verbose logs remain suppressed so stdout remains clean.

---

### User Story 3 - Unified Native Test Logger & Telemetry Pipeline (Priority: P3)

As a developer writing or maintaining unit tests, I want a centralized `TestLogger` / `TestTelemetry` API that replaces raw `print()` statements with structured log levels (`.trace`, `.debug`, `.info`, `.warn`, `.error`), so that subagents and codebase tests can be systematically refactored to use a consistent logging interface.

**Why this priority**: Centralizing test logging ensures tests don't pollute stdout with arbitrary ad-hoc prints and allows toggling verbosity flags (`--quiet`, `--verbose`, `--json`) across all 80+ test files.

**Independent Test**: Call `TestLogger.info(...)` and `TestLogger.debug(...)` across multiple test files and verify output conforms to current verbosity level without requiring external tools.

**Acceptance Scenarios**:
1. **Given** `--quiet` flag, **When** tests execute, **Then** only suite summaries and failures are displayed.
2. **Given** `--verbose` or `--debug` flag, **When** tests execute, **Then** detailed diagnostic traces are rendered with timestamps and subsystem tags.
3. **Given** `--json` flag, **When** tests execute, **Then** NDJSON telemetry stream is emitted for machine ingestion.

---

### Edge Cases

- **Non-TTY Environments**: When output is redirected to a file or CI pipe (`isatty(1) == false`), ANSI color escape sequences are automatically disabled to prevent raw escape code pollution.
- **Concurrent Test Logs**: When multiple tests or background subagents emit logs simultaneously, output lines are serialized via thread-safe write locks to prevent interleaving characters.
- **Terminal Width Variations**: Narrow terminals (< 80 columns) automatically abbreviate long test identifiers without wrapping lines awkwardly.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a native, zero-dependency terminal test renderer (`TestTerminalRenderer` / `TestLogger`) inspired by `zlib-ng` and `libarchive` aesthetic standards.
- **FR-002**: System MUST format test results with aligned status badges (`[ PASS ]`, `[ FAIL ]`, `[ SKIP ]`, `[ ORACLE ]`, `[ PERF ]`, `[ FUZZ ]`) and millisecond execution timings.
- **FR-003**: System MUST provide a unified, thread-safe `TestLogger` subsystem with configurable log levels (`.silent`, `.normal`, `.verbose`, `.debug`).
- **FR-004**: System MUST suppress internal diagnostic prints on passing tests by default, revealing detailed failure cards only when assertions fail.
- **FR-005**: System MUST provide an end-of-run executive summary card displaying Total, Passed, Failed, Skipped, Execution Wall Time, and System Hardware Context.
- **FR-006**: System MUST automatically detect TTY capabilities and disable ANSI color codes when piping to files or non-color terminals.
- **FR-007**: System MUST provide support for both human-readable terminal rendering and machine-readable NDJSON export (`--json`).

### Key Entities

- **TestExecutionRecord**: Represents a completed test execution with suite name, case name, status (`passed`, `failed`, `skipped`), duration in milliseconds, and optional failure details.
- **TestRunSummary**: Aggregates total suites, total cases, pass/fail/skip counts, total wall clock time, and hardware telemetry.
- **TestLoggerConfig**: Holds output stream, verbosity level, color enablement, and line width constraints.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Test output signal-to-noise ratio increased by > 80% (zero unstructured multiline noise on successful test runs).
- **SC-002**: Terminal test execution stream renders 100% aligned, clean rows across all 209 test suites in TTZip.
- **SC-003**: Native rendering has 0 ms perceptible overhead (< 1% total test runtime impact).
- **SC-004**: Zero external CLI dependencies (no `xcbeautify`, no `xcpretty`, no Ruby/Node tooling required).
- **SC-005**: 100% compliance with SPDX copyright headers and zero-warning compilation.

---

## Assumptions

- Tests are run via standard `swift test`, `./scripts/run_all_tests.sh`, `./scripts/run_local_ci_gate.sh`, or `swift run ttzip-cli test`.
- All macOS Sonoma / Apple Silicon and Intel terminal environments supporting UTF-8 / ANSI colors are targeted.
- Direct standard error / stdout redirection is respected.
