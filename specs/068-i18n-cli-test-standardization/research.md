# Phase 0 Research: 全面国际化、CLI 标准化与测试体系专业化架构研究 (Comprehensive Research Artifact)

**Feature**: `068-i18n-cli-test-standardization`  
**Date**: 2026-08-17  
**Status**: Completed & Grounded

---

## 1. R001: 跨端统一类型安全本地化 (i18n / l10n) 架构研究

### 1.1 核心问题与场景
在 Swift 6.0 + 纯 SPM 跨 Target 架构（`TTZipCore` 底层库、`TTZipApp` 桌面应用、`TTZipCLI` 独立命令行工具）下，如何构建零外部依赖、零运行时反射、100% 内存安全、无缝支持 7 种语言（`en`, `zh-Hans`, `zh-Hant`, `ja`, `de`, `fr`, `es`），且同时满足 SwiftUI 免重启动态热更与 POSIX 环境变量/CLI 参数覆盖的高性能本地化系统？

### 1.2 架构决策 (Decision)
- **纯 Swift 嵌入式强类型字典与多层命名空间枚举 (Pure Swift Embedded In-Memory Catalog)**：
  - 在 `TTZipCore/Localization/` 下统一下沉 `AppLanguage`、`L10n` / `LocaleKey` 命名空间枚举、`TTZipLocalizationManager` 调度中枢以及 7 种语言的静态字典表。
  - **动态切语言与环境感知双通道**：
    - GUI 应用通过 `@MainActor AppLanguageStore: ObservableObject` 配合 `@Published` 属性驱动 SwiftUI 视图树热更新（无需重启应用），并通过 `NotificationCenter` 通知 AppKit 菜单栏。
    - CLI 工具通过 `resolveFromPOSIXEnv()` 自动检测 POSIX 标准环境变量（`LC_ALL` ➔ `LC_MESSAGES` ➔ `LANG`），并支持 `--lang <locale>` 命令行选项显式覆盖。
  - **级联回退链 (Cascading Fallback)**：`请求语言` ➔ `区域保底 (zh-Hant -> zh-Hans)` ➔ `基础英语 (en)` ➔ `Key RawValue`。
  - **单位与错误多语言绑定**：`ByteSizeFormatter`（支持 SI 十进制与 IEC 二进制）、`ThroughputFormatter`（本地化小数点与千分位）与 `LocalizedError` 全面绑定 `L10n.Error` 强类型键。

### 1.3 决策理由 (Rationale)
1. **彻底消除 SPM Standalone CLI 的 Bundle 丢失崩溃风险**：在无 App Bundle 包装的独立命令行分发场景（如 `/usr/local/bin` 或 CI 容器），调用 `Bundle.module` 会因找不到 `.bundle` 资源目录而触发 `fatalError`。纯 Swift 嵌入式代码化字典 100% 独立自包含。
2. **Swift 6 严格并发与零分配热路径**：静态字典天然具备 `Sendable` 特性，采用 `os_unfair_lock` / `Atomic` 快照调度，单次查表耗时 $< 20\text{ns}$，对压缩/解压热循环零额外堆分配开销。
3. **编译期严格类型安全**：所有键由嵌套 `enum` 驱动，杜绝魔法字符串拼写错误。

### 1.4 被否决方案与否决理由 (Alternatives Considered)
- **被否决方案 1**：Apple Xcode 15+ String Catalogs (`.xcstrings`) + `Bundle.module`。
  - *否决理由*：在 SPM 单独立可执行文件 CLI 场景下存在 `Bundle` 依赖丢失崩溃隐患；且在 Headless 进程与核心库中无法通过纯代码动态覆盖语言环境（`Bundle.module` 强绑定进程级 `AppleLanguages`）。
- **被否决方案 2**：传统 `.strings` / `.stringsdict` + SwiftGen 外部代码生成工具。
  - *否决理由*：引入额外外部构建依赖，破坏 SPM 纯净构建流水线；7 种语言维护 `.stringsdict` 复数文件极为臃肿繁琐。
- **被否决方案 3**：修改 `UserDefaults.standard` 中的 `AppleLanguages` 并提示用户重启 App。
  - *否决理由*：交互体验割裂，违背现代 macOS 原生应用交互标准，且对无状态的 CLI 脚本调用完全无效。

### 1.5 真实研究源 (Source)
- `/Users/kevintung/Documents/dev/TTZip/Package.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipCLI/TTZipCLIApp.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipApp/Views/SettingsView.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/ArchiveReader.swift`

---

## 2. R002: 企业级 POSIX/GNU CLI 架构、终端渲染与流式管道标准化研究

### 2.1 核心问题与场景
如何将 `ttzip-cli` 升级为具备 POSIX Utility Syntax / GNU 规范对齐（长短参数、紧凑标志、`--` 截断符）、TTY 设备自适应与 60Hz 帧率节流双模渲染、`<sysexits.h>` 标准退出码体系、标准 I/O 管道（`-` 符号）流式处理以及 Shell 自动补全 / UNIX Man Page 动态生成的现代化工业级 CLI？

### 2.2 架构决策 (Decision)
- **POSIX / GNU 双模参数解析器 (`POSIXCLIArgumentParser`)**：
  - 支持短选项合并（如 `-vq`、`-yq`）、GNU 标准等号语法（`--level=9`）、`--` 参数截断符与子命令作用域独立解析。
  - 核心子命令体系：`archive` (`a`), `extract` (`x`), `list` (`l`), `test` (`t`), `bench` (`b`), `info` (`i`), `diff` (`d`), `recover`, `repair`, `completion`, `man`。
- **TTY 自适应与双模渲染引擎 (`TerminalRenderEngine`)**：
  - 探测 `isatty(STDOUT_FILENO)` 与 `ioctl(TIOCGWINSZ)` 动态获取终端列宽 `ws_col`。
  - 交互式终端模式：遵循 [NO_COLOR](https://no-color.org) 规范，渲染自适应宽度的 Unicode 流线进度条，使用 `ContinuousClock`（`mach_continuous_time`）强制实施 $\le 60\text{Hz}$ 的派发收敛（最小刷新间隔 $16.6\text{ms}$）。
  - 非交互式 / 管道模式：自动禁用 ANSI 转义字符与光标控制，支持 `--json` 输出结构化 NDJSON 机器可读事件流；当使用管道输出数据时，`STDOUT_FILENO` 100% 独占用于二进制数据，人类可读日志全部自动导向 `STDERR_FILENO`。
- **POSIX `<sysexits.h>` 强类型退出代码体系 (`CLIExitCode`)**：
  - 严格映射标准退出码：`EX_OK` (0), `EX_USAGE` (64), `EX_DATAERR` (65), `EX_NOINPUT` (66), `EX_UNAVAILABLE` (69), `EX_SOFTWARE` (70), `EX_CANTCREAT` (73), `EX_IOERR` (74), `EX_NOPERM` (77), `SIGINT` (130)。
- **标准 I/O 管道 (`-` 符号) 与流式适配**：
  - 流式格式（TAR, ZSTD, LZ4, BROTLI 等）直接通过 `STDIN_FILENO`/`STDOUT_FILENO` 零拷贝单遍流式处理。
  - 随机访问格式（ZIP, 7Z）在 Stdin 侧采用内存（$\le 64\text{MB}$）与 APFS 临时匿名文件（$> 64\text{MB}$）双阶自适应 Spooling，在 Stdout 侧启用 ZIP Data Descriptor 流式发射。注册 `SIGPIPE` 忽略信号，防止管道破裂硬崩溃。
- **声明式元数据内省生成 Shell 补全与 Man Page**：
  - 基于 `CLICommandSpecification` 元数据模型，自包含生成 Zsh (`_ttzip-cli`)、Bash (`ttzip-cli.bash`)、Fish (`ttzip-cli.fish`) 与 UNIX `mdoc` 格式的 `ttzip-cli.1`。

### 2.3 决策理由 (Rationale)
1. 解决脚本重定向时 ANSI 转义字符污染输出数据的顽疾。
2. 消除多核高频微秒级解压回调导致的控制台 I/O 假死与 CPU 浪费。
3. 提供符合 Unix 哲学的标准退出码与数据管道支持，便于与 `curl`、`ssh`、`tar` 以及 GitHub Actions 流水线无缝组合。
4. 单一数据源（Single Source of Truth）保证 CLI 选项、Shell 补全与 Man Page 100% 同步。

### 2.4 被否决方案与否决理由 (Alternatives Considered)
- **被否决方案 1**：引入外部 `swift-argument-parser`。
  - *否决理由*：增加了额外外部依赖与编译时间；且在底层二进制管道流对接、精细化 `SIGPIPE` 信号拦截和自适应双模渲染方面不如自研架构灵活可控。
- **被否决方案 2**：在所有环境下无条件输出 ANSI 转义符或仅提供简单的 `--no-color` 手动开关。
  - *否决理由*：在非 TTY 管道（如 `ttzip-cli ... | grep`）下残留的控制字符会导致脚本解析崩溃，产生大量乱码。
- **被否决方案 3**：仅使用 `0`（成功）与 `1`（失败）二元退出码。
  - *否决理由*：自动化运维与 CI 脚本无法根据 `$?` 区分参数错误、文件损坏还是 IO 故障，丧失自动化重试能力。

### 2.5 真实研究源 (Source)
- IEEE Std 1003.1-2017 (POSIX.1-2017) Utility Syntax Guidelines
- macOS SDK: `/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/sysexits.h`
- [NO_COLOR Specification (no-color.org)](https://no-color.org)
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipCLI/CLIArgumentParser.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipCLI/CLICommandRouter.swift`

---

## 3. R003: 6 层分级自动化测试体系与性能门禁/预言机标准化研究

### 3.1 核心问题与场景
如何将现有 127+ 测试文件系统化重构与标准化为 Tier 0 (Micro/Unit) 到 Tier 5 (Stress/Scale) 的分层运行体系，并建立针对 16 种格式峰值吞吐门禁、系统原生差分测试与本地化完整性自动断言的统一测试执行与 JUnit/JSON 报告生成中枢？

### 3.2 架构决策 (Decision)
- **6 层分级测试体系 (Tier 0 ～ Tier 5)**：
  - **Tier 0: Micro / Unit**：纯内存算法、SIMD 硬件算子、28 大设计模式合规性测试，$\le 5\text{ms}$ 单用例，全套 $\le 3\text{s}$。
  - **Tier 1: Integration / Contract**：16 种格式往返闭环、AES-256 加密、分卷切割、UI 状态绑定与 ZipSlip 路径穿透防御。
  - **Tier 2: System / Differential Oracle**：历史缺陷黄金语料库 (`.uu`) 解码验证、macOS 原生 `/usr/bin/tar` 与 `/usr/bin/unzip` 双向交叉差分验证。
  - **Tier 3: Performance Regression Gates**：全格式历史最优性能硬门禁（262 维度矩阵）、前端 50k 节点渲染与 20k 实时搜索门禁，基于单调物理时钟核验，吞吐倒退 $> 10\%$ 强行阻断。
  - **Tier 4: Crash-First Fuzzing**：基于变异算子对底层 C 桥接层轰炸，采用**崩溃现场优先落盘机制 (Crash-First Persistence)**，每次变异调用前强制落盘 `fuzz_crash_reproducer.bin`。
  - **Tier 5: Stress / Scale / PK**：1.0GB 流式解压、2.0GB 加密分卷切割压力测试与全 16 格式竞品 1v1 PK。
- **国际化 100% 完整性自动化断言套件 (`LocalizationIntegrityTests`)**：
  - **双向完备性断言**：代码中引用的所有 `LocaleKey` 100% 存在于 7 种语言包中，且各语言包无未定义的孤立死键。
  - **格式化占位符类型与个数状态机断言**：通过正则状态机比对各语言包中的 `%@`, `%d`, `%lld` 等格式化占位符，断言数量与类型签名绝对一致，杜绝 64 位指针/整型混淆引发的 Crash。
- **标准化 CLI / XCTest 统一调度与三模报告引擎 (`TestReportGenerator`)**：
  - 原生生成标准 **JUnit XML**（供 GitHub Actions 解析）、**JSON Schema 强契约报告** 与 **Markdown 格式化报告**（带 KPI 徽标与 HexDump 差分）。

### 3.3 决策理由 (Rationale)
1. 解决混合测试运行导致的 CI 耗时过长或超时误报，实现高频提交 3 秒反馈、PR 深度闭环、Nightly 极限轰炸。
2. 崩溃现场优先落盘机制确保底层 C 静态库在遭遇极端数据发生不可控硬崩溃时，能够在沙盒第一现场留存最小复现用例。
3. 自动化占位符一致性断言彻底防范多语言文本格式化引发的运行时内存越界与崩溃。
4. 原生三模报告生成器避免引入外部 Python/Ruby 脚本，维持 100% 纯净的原生 Swift/C 工程体系。

### 3.4 被否决方案与否决理由 (Alternatives Considered)
- **被否决方案 1**：保持单套扁平的 `swift test`，依赖开发者手动传参过滤。
  - *否决理由*：开发者易漏跑门禁，或在常规 CI 中意外触发 2GB 压力测试导致超时阻断。
- **被否决方案 2**：纯内存生成变异 Buffer 并在 Swift `catch` 中落盘崩溃样本。
  - *否决理由*：底层 C 语言的硬崩溃（如 SIGSEGV、SIGBUS）不会触发 Swift 的 `catch`，导致崩溃现场永久丢失。
- **被否决方案 3**：依赖外部第三方 Python 脚本解析控制台文本生成测试报告。
  - *否决理由*：破坏了 TTZip 100% 独立自包含的工程铁律，且文本格式脆弱易碎。

### 3.5 真实研究源 (Source)
- `/Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- `/Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ArchiveMutationFuzzTests.swift`
- `/Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/SystemDifferentialTests.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipCLI/TestCommand.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/TTZipCLI/TestReportGenerator.swift`
