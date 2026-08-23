# Research Phase 0: Native High-Aesthetic Test Logging, Harness & Reporter (111-native-test-logging-and-reporter)

## R001: zlib-ng / CTest 与 libarchive 测试流水线与控制台渲染美学体系研究

### 1. Decision (选定方案)
确立 **TTZip 工业级测试流水线控制台渲染美学规范 (TTZip Test Terminal Aesthetic Spec)**，由以下四大支柱构成：
1. **固定宽度单行流式对齐 (Fixed-Width Single-Line Streaming Alignment)**：
   - 统一模板：`  %3d. [ BADGE ] [%-12s] %-42s (%s)`
   - 示例：`    1. [  PASS  ] [Tier 1     ] testDiagnostic_tar.zst                     (12.45 ms)`
   - 规则：序号 3 位右对齐，分类徽章固定 10 字符宽，测试目标/套件 12 字符左对齐，测试用例名 42 字符定宽填充，耗时采用自适应高精度格式化（`< 1 µs`, `µs`, `ms`, `s`）右对齐。
2. **Kintsugi 禅宗分类徽章矩阵 (Kintsugi Aesthetic Badge Matrix)**：
   - `[PASS]`：粗体翡翠绿（ANSI `\u{001B}[1;32m`）
   - `[FAIL]`：粗体绯红（ANSI `\u{001B}[1;31m`）
   - `[SKIP]`：粗体琥珀金（ANSI `\u{001B}[1;33m`）
   - `[STANDARDS]`：粗体青蓝（ANSI `\u{001B}[1;36m`）
   - `[ORACLE]`：粗体品紫（ANSI `\u{001B}[1;35m`）
   - `[FUZZ]`：高亮柠檬黄（ANSI `\u{001B}[1;93m`）
   - `[PERF]`：Kintsugi 金黄（ANSI `\u{001B}[1;33m` 或 256 色 `\u{001B}[38;5;220m`）
3. **延迟捕获静默-失败差分卡片 (Silent-on-Success, Detailed-on-Failure Diagnostic Card)**：
   - 深度借鉴 libarchive `failure()` 与 `UnicodeDiagnosticFormatter` 机制：通过路径保持终端绝对静默（Zero Console Noise），不产生任何冗余输出；
   - 失败时自动展开原子诊断卡片：包含源文件名与行号（`File:Line`）、前置注入的意图描述（`Deferred Context`）、Unicode 标量展开与 APFS NFD vs NFC 归一化根因分析、以及基于 `FastHexDiffEngine` 的双栏 Hex 差异对齐视窗（高亮标红差异字节与偏移量）。
4. **ANSI / Unicode 双线边框全景汇总仪表盘 (Summary Dashboard)**：
   - 终端支持时使用 Unicode 细双线/箱体字符（`┌───`, `├───`, `└───`, `═`），非 TTY / CI 重定向时自动降级为纯 ASCII（`+---`, `|`, `===`）；
   - 卡片完整涵盖：总用例数、通过数、失败数、跳过数、整体通过率（精确至 0.1%）、总耗时、会话 Session ID、主机架构与操作系统版本。

### 2. Rationale (选择理由)
1. **视觉韵律与认知负荷极小化**：经典基础库（如 zlib-ng / libarchive / CTest）经过数十年的工业验证，其单行固定宽度排版让工程师能在高速滚动的测试流中一眼捕捉异常。
2. **根因定位秒级闭环**：libarchive 经典的 `failure()` 延迟上下文模式将“测试意图”与“断言执行”解耦，成功时不消耗任何字符串格式化与 I/O 成本，失败时立即输出关键上下文，杜绝无意义的调试日志轰炸。
3. **全格式多维质量感知**：TTZip 涵盖 16 种归档格式、SIMD 硬件加速与跨引擎差分验证，通过明确的徽章分类使测试类型边界清晰分明。

### 3. Alternatives Considered (被否决方案及理由)
- **被否决方案 A：直接依赖 SwiftPM / XCTest 原生控制台默认输出**。
  - *否决理由*：XCTest 原生输出格式冗长、缺乏语义徽章高亮、无法对齐各列，且在并发测试场景下易出现日志交错撕裂，无法承载 Hex Diff 与 Unicode NFD/NFC 根因卡片。
- **被否决方案 B：基于 curses / ncurses 的交互式全屏 TUI 仪表盘**。
  - *否决理由*：全屏 TUI 无法良好适配 CI/CD 自动化流水线（如 GitHub Actions、Shell 管道输出重定向 `> test.log`），容易引发控制终端转义混乱，且增加了不必要的外部状态管理复杂度。

### 4. Source (查阅源)
- `Vendor/libarchive-upstream/test_utils/test_common.h`（第 158–279 行：`assert`、`assertEqualInt`、`assertEqualMem` 宏族定义；第 281 行：`failure()` 函数原型；第 277–279 行：`skipping` 宏定义）
- `Vendor/libarchive-upstream/test_utils/test_main.c`（第 472–483 行：`failure()` 缓冲注入；第 532–565 行：`failure_start()` 失败头部输出与格式化；第 598–616 行：`test_skipping()` 跳过处理）
- `Vendor/zlib-ng-upstream/test/CMakeLists.txt`（第 1–50 行：CTest 测试用例与套件配置规范）
- `Sources/TTZipCore/Testing/TestTerminalRenderer.swift`（第 15–41 行：`ANSI` 颜色常量；第 45–80 行：`Badge` 徽章渲染；第 92–147 行：`formatDuration` / `formatThroughput`；第 203–274 行：`renderSuiteTable` 与 `renderSummaryTable`；第 303–359 行：`renderHexDiffSnippet` 诊断卡片）
- `Sources/TTZipCore/Testing/UnicodeDiagnosticFormatter.swift`（第 14–25 行：`dumpScalars` 标量解构；第 28–50 行：`analyzeStringMismatch` APFS NFD/NFC 根因分析）
- `Sources/TTZipCLI/TestCommand.swift`（第 43–62 行：终端 Header 格式；第 120–123、172–188 行：徽章与单行测试结果渲染）
- `scripts/run_local_ci_gate.sh`（第 62–68 行：CI 门禁标题；第 119–154 行：阶段调度与状态输出；第 160–185 行：Summary Table 汇总表）

---

## R002: Swift 原生无锁高效 TestLogger 体系设计

### 1. Decision (选定方案)
设计并实现 **Swift 原生无锁每任务隔离缓冲 `TestLogger` 架构**：
1. **四级日志级别枚举 (TestLogLevel)**：
   - `.silent` (0): 仅在测试失败或门禁红灯时输出
   - `.normal` (1): 默认级别：仅输出单行进度徽章与失败诊断卡片
   - `.verbose` (2): 详细级别：输出每项子断言与阶段明细
   - `.debug` (3): 调试级别：输出原始字节 Dump、系统调用与 SIMD 寄存器状态
2. **TaskLocal 隔离的无锁每任务日志缓冲 (Lock-Free Per-Task Log Buffer)**：
   - **执行期完全无锁**：利用 Swift 6 并发模型的 `@TaskLocal` 注入 `TaskLogSession` 上下文。每个并发 Task 内部持有独立的私有内存日志缓冲区（`[String]`），在执行测试与断言期间仅向私有缓冲区追加记录，热路径零锁、零 CAS、零全局同步开销。
   - **同步线程兼容**：针对非 async 的 POSIX / C 桥接测试线程，采用基于 `pthread_key_t` 的线程局部存储（TLS）分配独立 Buffer。
   - **通过即丢弃 (Zero-Cost on Pass)**：测试通过（Pass）时，直接释放私有缓冲区，不执行任何 I/O 与字符串拼接，总体开销趋近于 0。
   - **失败原子单块写入 (Atomic Single-Chunk Dump on Failure)**：测试失败（Fail）时，将私有缓冲区全部日志拼装为单个连续字节块，通过 POSIX `flockfile(stdout)` / `fputs` / `funlockfile(stdout)` 执行单次系统调用原子落盘，杜绝并发测试时的终端交织混乱。
3. **自适应终端能力与 ANSI 剥离器 (TTY & Color Auto-Detection)**：
   - 封装 `TerminalCapabilities` 探测器：检查 `isatty(fileno(stdout))`、`getenv("NO_COLOR") == nil`、`getenv("TERM") != "dumb"`；
   - 内置基于线性扫描状态机的高性能 `stripANSI(from:)` 纯文本清洗函数，在管道重定向或 CI 纯文本捕获时自动生成干净的标准 ASCII 输出。

### 2. Rationale (选择理由)
1. **多核并行测试零争用**：测试套件包含 1,109 个单测用例与 209 个套件。若使用传统全局互斥锁或串行 Actor，所有并发线程都将在日志记录处发生严重锁碰撞，导致流水线停顿并严重污染微基准测试耗时精度。
2. **内存局部性与缓存友好**：通过 TaskLocal / TLS 局部化缓冲，符合现代 CPU L1/L2 缓存亲和性，并在测试成功时瞬时析构。
3. **输出原子性与确定性**：POSIX 文件锁 `flockfile` 保证失败卡片以完整块输出，避免乱序破损；TTY 自动探测保证在本地终端、Xcode Console、GitHub Actions 以及文本重定向工具中均能呈现最佳视觉效果。

### 3. Alternatives Considered (被否决方案及理由)
- **被否决方案 A：基于 Swift Actor 的全局串行记录器 (`actor GlobalTestLogger`)**。
  - *否决理由*：Swift Actor 依赖单一串行消息队列。在高频并发断言场景下（每秒数十万次断言），所有并发 Task 都将产生协作式让步与排队，引入虚假调度延迟。
- **被否决方案 B：直接调用系统 `print()` 或裸 `os_log` 逐行输出**。
  - *否决理由*：并发调用 `print()` 会导致输出在终端中交织混乱；且无法实现“成功静默、失败全量吐出”的延迟诊断语义。

### 4. Source (查阅源)
- `Sources/TTZipCore/Testing/TestLogCollector.swift`（第 16–46 行：`TestLogCollector` 原型、`os_unfair_lock_s`、`@TaskLocal currentSessionID`；第 48–71 行：`flushOnFailure` 与 `flockfile(stdout)` 原子块输出）
- `Sources/TTZipCore/Testing/DiagnosticContext.swift`（第 10–39 行：`DiagnosticContext`、`@TaskLocal taskLocalPendingMessage`、`withFailureMessage` 作用域绑定；第 48–68 行：`consumePendingMessage` 与清空逻辑）
- `Sources/TTZipCore/Testing/TestTelemetryStream.swift`（第 223–264 行：`TestTelemetryStream`、`NSLock` 与 `FileHandle` 流式输出）
- `Sources/TTZipCLI/TestCommand.swift`（第 36–43 行：会话初始化与 `verbosity` 阈值分流；第 261–274 行：`TestTelemetryEvent.runFinished` 完成事件发送）
