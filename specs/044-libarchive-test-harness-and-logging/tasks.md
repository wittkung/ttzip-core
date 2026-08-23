# Tasks: 044-libarchive-test-harness-and-logging

**Feature Branch**: `044-libarchive-test-harness-and-logging`  
**Input Documents**: [spec.md](./spec.md), [plan.md](./plan.md), [data-model.md](./data-model.md), [research.md](./research.md), [quickstart.md](./quickstart.md), [contracts/](./contracts/)  

---

## Phase 1: Setup & Foundational Quality

- [x] T001 [P] 验证并核对 JSON Schema 契约在 `specs/044-libarchive-test-harness-and-logging/contracts/` 下的零通配与字段一致性
- [x] T002 [P] 建立测试报告目录 `docs/test_reports/` 与 `.gitignore` 忽略临时测试产物规则

---

## Phase 2: Foundational Diagnostic & Logging Infrastructure (阻塞基建)

- [x] T003 [P] 实现延迟失败诊断上下文与会话隔离中枢 in `Sources/TTZipCore/Testing/DiagnosticContext.swift`
- [x] T004 [P] 实现基于 `os_unfair_lock` 的线程安全日志缓冲与 POSIX 原子刷新器 in `Sources/TTZipCore/Testing/TestLogCollector.swift`

---

## Phase 3: User Story 1 - 本地工业级命令行测试与诊断中枢 (`ttzip-cli test`) (Priority: P1)

*Goal*: 开发者在本地通过 `ttzip-cli test` 命令快速执行测试，支持多级 Verbosity、用例正则过滤与断言计数汇总。  
*Independent Test*: 运行 `swift run ttzip-cli test --filter Zip`，输出彩色进度与汇总统计。

- [x] T005 [P] [US1] 扩展 CLI 选项与参数解析器支持测试标志 (`filter`, `verbosity`, `keepTemp`, `dumpOnFailure`, `fast`) in `Sources/TTZipCLI/CLIOptions.swift` 与 `Sources/TTZipCLI/CLIArgumentParser.swift`
- [x] T006 [P] [US1] 实现独立的测试子命令调度器 `TestCommand` in `Sources/TTZipCLI/TestCommand.swift`
- [x] T007 [US1] 在 `Sources/TTZipCLI/CLICommandRouter.swift` 中接入并分发 `test` 子命令

---

## Phase 4: User Story 2 - 对标 libarchive 的原语级诊断日志与 HexDump 上下文追踪 (Priority: P1)

*Goal*: 发生断言失败时输出 16 字节对齐 HexDump 差分切片与 UTF-8 Unicode 码点序列展开。  
*Independent Test*: 构造故意分歧的断言测试，验证控制台输出对齐的 Hex + ASCII 差分及 `[0054 0054 005A]` 码点。

- [x] T008 [P] [US2] 实现 64 字节快速跳跃与 16 字节对齐零堆分配 Hex 差分引擎 in `Sources/TTZipCore/Testing/FastHexDiffEngine.swift`
- [x] T009 [P] [US2] 实现字符串逐标量 Unicode 码点序列格式化与 APFS NFD/NFC 差分分析器 in `Sources/TTZipCore/Testing/UnicodeDiagnosticFormatter.swift`
- [x] T010 [US2] 升级 `Tests/TTZipTests/TTZipAssertions.swift` 接入 `FastHexDiffEngine` 与 `UnicodeDiagnosticFormatter`

---

## Phase 5: User Story 3 - 结构化多格式测试报告持久化生成 (Markdown & JSON) (Priority: P2)

*Goal*: 测试运行结束后自动生成人类友好的 Markdown 报告与符合 Schema 的 JSON 数据文件。  
*Independent Test*: 指定 `--json-report` 与 `--markdown-report`，验证在 `docs/test_reports/` 生成合法文件。

- [x] T011 [P] [US3] 实现多格式测试报告生成器 `TestReportGenerator` in `Sources/TTZipCLI/TestReportGenerator.swift`
- [x] T012 [US3] 编写测试报告序列化与 JSON Schema 一致性单元测试 in `Tests/TTZipTests/TestReportGeneratorTests.swift`

---

## Phase 6: User Story 4 - 自动化本地全回归编排脚本 (`run_local_ci.sh`) (Priority: P2)

*Goal*: 彻底脱离云端 GitHub Actions 额度，一键在本地运行 Lint、Native 诊断、全量回归与硬性能门禁。  
*Independent Test*: 运行 `./scripts/run_local_ci.sh --quick`，验证绿色通关徽章输出。

- [x] T013 [P] [US4] 编写工业级本地 CI/CD 自动化调度脚本 in `scripts/run_local_ci.sh`
- [x] T014 [US4] 为 `scripts/run_local_ci.sh` 赋予可执行权限并完成本地回归实测

---

## Phase 7: Polish, Verification & Performance Gate

- [x] T015 [P] 运行全量 565+ 单元测试验证零回归 (`swift test`)
- [x] T016 运行全格式 46 项基准与前端性能门禁 (`swift test --filter XCTestPerformanceMeasureTests,FrontendPerformanceGateTests`)
