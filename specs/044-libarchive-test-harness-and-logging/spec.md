# Feature Specification: 044-libarchive-test-harness-and-logging

**Feature Branch**: `044-libarchive-test-harness-and-logging`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: User description: "cicd 我们本地去跑就行。然后我觉得人家 libarchive 的测试系统构建的也很好，log 日志也非常漂亮，好好调研并学习 /speckit-specify"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 本地工业级命令行测试与诊断中枢 (`ttzip-cli test`) (Priority: P1)

作为一名系统架构师与底层核心开发者，我希望能够在本地终端直接运行专有的高性能测试驱动工具 `ttzip-cli test`，获得美观、清晰、结构化且零外部 CI 依赖的测试流水线体验，包含彩色状态矩阵、耗时统计、断言计数与分级详细度控制。

**Why this priority**: 摆脱对远程 GitHub Actions 额度的依赖，将 libarchive 20 年积累的自研 `test_main.c` 驱动精髓原生化为 TTZip 的本地测试中枢，提供毫秒级反馈与极速诊断体验。

**Independent Test**: 在终端运行 `swift run ttzip-cli test --filter Zip` 或 `swift run ttzip-cli test -v`，能够独立完成被过滤测试套件的高速执行，并在控制台输出标准美观的 ASCII 进度卡片与摘要报表。

**Acceptance Scenarios**:
1. **Given** 开发者在终端执行 `swift run ttzip-cli test`, **When** 默认无参数运行, **Then** 测试中枢按模块并发执行测试，以点阵/色块形式实时显示进展，并在完成后输出统一格式的汇总看板（包含套件数、用例数、断言数、通过率与总耗时）。
2. **Given** 某个测试用例发生断言失败, **When** 启用 `-v` (Verbose) 或 `--dump-on-failure`, **Then** 自动高亮输出失败发生的源码位置（文件:行号）、期待值 vs 实际值对比、上下文字符串 Hex Dump 及最小复现诊断命令。
3. **Given** 开发者指定 `--filter "GoldenCorpus"`, **When** 执行命令, **Then** 仅匹配并执行对应的测试集，跳过无关用例。

---

### User Story 2 - 对标 libarchive 的原语级诊断日志与 HexDump 上下文追踪 (Priority: P1)

作为一名负责极端边缘 Bug 定位的工程师，当遇到畸形包解析失败或加密校验不一致时，我希望测试系统能够像 libarchive 的 `failure()` 与 `strdump()` 一样，自动捕获失败前置上下文，并格式化输出二进制 Hex Dump 与 UTF-8 码点反解，而不是仅输出空泛的布尔值断言失败。

**Why this priority**: 归档解析排错极其依赖二进制字节对齐与字符编码。原生具备 HexDump / Unicode 码点差分是顶级开源系统区别于普通项目的核心工程标志。

**Independent Test**: 构造一个故意破坏字节的测试用例，触发断言失败，验证控制台与日志中能精准呈现 16 字节对齐的 Hex + ASCII 对照视图与 Unicode 码点展开。

**Acceptance Scenarios**:
1. **Given** 两个二进制数据切片比对失败, **When** 触发 `TTZipAssertDataEqual`, **Then** 诊断器自动在首个分歧字节处输出带有偏移量、Hex 字节与 ASCII 可见字符的格式化对比窗口。
2. **Given** 路径字符串编码解析出现分歧, **When** 触发字符串比对失败, **Then** 诊断器不仅输出字符串字面量，还输出每个字符的 UTF-8 Unicode 码点（如 `[0054 0054 005A 0069 0070]`）与字节长度。
3. **Given** 在测试逻辑执行前通过 `DiagnosticContext.setFailureReason("Extracting LFH header at offset 0x20")` 注入上下文, **When** 后续断言失败, **Then** 该诊断描述自动随同失败堆栈一起打印；若断言通过则零开销不打印。

---

### User Story 3 - 结构化多格式测试报告持久化生成 (Markdown & JSON) (Priority: P2)

作为一名持续集成的维护者，我希望本地测试运行完成后，能够自动在 `docs/test_reports/` 生成人类友好的 Markdown 视觉报告与机读的 JSON 结构化数据，方便追溯历史测试趋势与单测覆盖基准。

**Why this priority**: 使本地 CI 具备同等于甚至超越 GitHub Actions Summary 的可视化分析能力，方便生成归档交付物与性能/质量追踪。

**Independent Test**: 运行带有 `--report-dir docs/test_reports` 的测试命令，验证磁盘生成带时间戳的 `.md` 与 `.json` 报告文件，且 Markdown 报告包含表格、折叠块与通过率图表。

**Acceptance Scenarios**:
1. **Given** 测试执行结束, **When** `--json-report` 参数被指定, **Then** 输出符合强类型 Schema 的测试事件流与统计 JSON。
2. **Given** 测试执行结束, **When** `--markdown-report` 参数被指定, **Then** 输出包含整体 KPI 仪表盘、按分类折叠明细、失败用例 HexDump 证据链的完整 Markdown 报告。

---

### User Story 4 - 自动化本地全回归与硬件隔离调度器 (`./scripts/run_local_ci.sh`) (Priority: P2)

作为一名即将提交代码的开发者，我希望能够一键执行涵盖“代码风格检查 + 全量单元测试 + 黄金语料库验证 + 动态 Sanitizers 检测 + 性能硬门禁”的本地全回归脚本，自动隔离多核资源，并在本地跑通全部流水线。

**Why this priority**: 彻底解决云端 GitHub Actions 配额耗尽问题，让开发者在本地 100% 模拟并超越云端 CI 的全套质量防护网。

**Independent Test**: 执行 `./scripts/run_local_ci.sh --quick` 或 `./scripts/run_local_ci.sh --full`，能够顺序或隔离调度各项检查，遇到任意一项目红灯即时阻断并输出排查指南。

**Acceptance Scenarios**:
1. **Given** 开发者运行 `./scripts/run_local_ci.sh`, **When** 所有门禁均达标, **Then** 最终打印绿色的全通通关徽章与耗时分析，返回 exit code 0。
2. **Given** 开发者运行 `./scripts/run_local_ci.sh`, **When** 出现单测或性能门禁跌破, **Then** 自动留存崩溃与诊断日志，以红字精准标定失败步骤，返回 exit code 1。

---

## Edge Cases

- **ANSI 彩色输出在非 TTY 环境中的适配**：当重定向输出到文件或管道（`| grep`）时，自动剥离 ANSI 颜色控制字符，防止日志乱码。
- **并发测试下的日志交织防护**：多测试套件并行执行时，每个用例的详细诊断日志在内存中独立缓冲，仅在用例结束或失败时原子化刷新到输出通道，杜绝并发日志撕裂。
- **巨型数据（>10MB）差分截断保护**：当两个数百兆的 Buffer 断言失败时，HexDump 自动定位到首个差异点并仅展示前后 256 字节的差分窗口，防止终端被日志淹没。
- **SIGSEGV / SIGBUS 崩溃现场先落盘机制**：在执行高危变异 Fuzzing 时，利用信号捕获或前置落盘，确保崩溃时依然留存最后执行的用例与输入种子。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统必须提供独立的命令行测试驱动器 `ttzip-cli test`，支持 `--filter`、`-v`、`-q`、`--keep-temp`、`--dump-on-failure` 等标准化参数。
- **FR-002**: 系统必须实现 `DiagnosticFormatter`，支持 16 字节对齐的 Hex + ASCII 对照 Dump 与字符串 UTF-8 Unicode 码点序列化展开。
- **FR-003**: 系统必须实现延迟失败上下文机制 `DiagnosticContext.failure(msg)`，支持在断言失败时输出上下文意图描述，断言通过时零开销。
- **FR-004**: 系统必须实现原子化测试日志收集器 `TestLogCollector`，支持并发执行时的日志线程局部隔离与原子派发。
- **FR-005**: 系统必须支持将测试结果输出为标准 Markdown 报告（含 KPI 仪表盘与失败详情折叠块）与 JSON 结构化数据文件。
- **FR-006**: 系统必须提供一键式本地 CI/CD 编排脚本 `./scripts/run_local_ci.sh`，覆盖 Lint、单元测试、黄金语料库、Sanitizer 矩阵与性能硬门禁。
- **FR-007**: 命令行工具必须支持自动探测 TTY 终端，在交互式终端输出 ANSI 视觉颜色，在重定向或非 TTY 管道中自动切换为纯纯文本模式。

### Key Entities

- **TestExecutionSession**: 单次测试运行会话实体，记录启动参数、环境信息（macOS 版本、CPU 架构、Xcode 版本）、开始/结束时间戳。
- **TestCaseResult**: 单个测试用例执行结果实体，包含用例标识、所属套件、耗时、状态（Passed / Failed / Skipped）、断言计数、诊断上下文与 HexDump 证据。
- **TestSuiteSummary**: 测试套件聚合汇总实体，包含用例统计、耗时统计、通过率及失败用例索引。
- **HexDumpSlice**: 二进制差分切片实体，记录首个分歧偏移量、预期与实际字节窗口及 ASCII 字符投影。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `ttzip-cli test` 自身运行调度开销极低，全量 565+ 用例执行调度开销控制在 50ms 以内。
- **SC-002**: 发生数据差分断言失败时，HexDump 与上下文格式化在 5ms 内生成并输出精准定位信息。
- **SC-003**: 开发者在本地通过 `./scripts/run_local_ci.sh` 执行全量质量验证，完全无需依赖远端 GitHub Actions 计算配额。
- **SC-004**: 多线程并发测试输出日志实现 100% 零交织、零撕裂（Log Thread-Safety Invariant）。
- **SC-005**: 无论测试成功还是失败，均能可靠生成标准 Markdown 与 JSON 报告，持久化于 `docs/test_reports/`。

---

## Assumptions

- 开发者在 macOS 14+ (Sonoma) 苹果芯片或 Intel 设备上进行本地开发。
- 本地环境已安装 Xcode 15+ 或 Swift 6.0 编译器。
- 外部 GitHub Actions 触发已关闭，所有日常研发门禁以本地 `./scripts/run_local_ci.sh` 与 `ttzip-cli test` 为准。
