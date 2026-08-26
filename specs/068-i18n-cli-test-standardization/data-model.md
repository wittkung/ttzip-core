# Phase 1 Data Model: 全面国际化、CLI 标准化与测试体系数据模型 (Domain Data Models)

**Feature**: `068-i18n-cli-test-standardization`  
**Date**: 2026-08-17  
**Status**: Ready & Schema-Aligned

---

## 1. 国际化与多语言子系统 (Localization Subsystem)

### 1.1 `AppLanguage` (语言枚举)
- **Description**: 系统支持的 7 种标准语言代码。
- **Fields**:
  - `id: String` (如 `"zh-Hans"`, `"en"`, `"ja"`) [Required, Primary Key]
  - `displayName: String` (如 `"简体中文"`, `"English"`, `"日本語"`) [Required]
  - `bcp47Code: String` (如 `"zh-Hans"`, `"en-US"`, `"ja-JP"`) [Required]
  - `fallbackLanguage: String` (如 `zh-Hant` 降级为 `zh-Hans`，其余降级为 `en`) [Required]

### 1.2 `LocaleKey` (类型安全文案键)
- **Description**: 结构化业务域命名空间下的文案唯一标识符。
- **Namespaces**:
  - `Common`: `ok`, `cancel`, `confirm`, `done`, `error`, `warning`, `success`, `save`, `delete`, `browse`, `settings`
  - `Compress`: `title`, `startCompression`, `outputFormat`, `compressionLevel`, `encryptArchive`, `splitVolume`, `totalItemsCount`, `totalSize`
  - `Extract`: `title`, `startExtraction`, `destPath`, `enterPassword`, `passwordWrong`, `extractingItem`, `completedSuccess`
  - `Error`: `fileNotFound`, `invalidFormat`, `corruptedData`, `cancelled`, `passwordRequired`, `wrongPassword`, `zipSlipDetected`, `permissionDenied`, `diskFull`
  - `Benchmark`: `title`, `throughput`, `compressionSpeed`, `decompressionSpeed`, `historicalFloor`, `winRate`
  - `CLI`: `helpUsage`, `helpOptions`, `helpCommands`, `pipeReadingStdin`, `pipeWritingStdout`

### 1.3 `LocaleCatalog` (语言字典表)
- **Description**: 单一语种下的全量静态映射字典。
- **Fields**:
  - `language: AppLanguage` [Required]
  - `entries: Dictionary<String, String>` (键值对，值包含规范化的 `%@`, `%d`, `%lld`, `%.2f` 占位符) [Required]

### 1.4 `ByteUnitStyle` (容量单位标准)
- **Description**: 文件容量格式化风格。
- **Enum Values**:
  - `metricSI`: 十进制标准 (1000 进制, `KB`, `MB`, `GB`, `TB`) - 适配 macOS Finder
  - `binaryIEC`: 二进制标准 (1024 进制, `KiB`, `MiB`, `GiB`, `TiB`) - 适配开发者与分卷规范

---

## 2. CLI 标准化与终端子系统 (CLI Standardization Subsystem)

### 2.1 `CLICommand` (子命令枚举)
- **Description**: 标准 POSIX 一级子命令。
- **Enum Values**:
  - `archive` (别名 `a`, `create`, `c`)
  - `extract` (别名 `x`, `e`)
  - `list` (别名 `l`, `ls`)
  - `test` (别名 `t`, `verify`)
  - `bench` (别名 `b`, `benchmark`)
  - `info` (别名 `i`, `inspect`)
  - `diff` (别名 `d`)
  - `recover`
  - `repair`
  - `completion`
  - `man`

### 2.2 `CLIExitCode` (POSIX Sysexits 退出代码)
- **Description**: 强类型对齐 BSD `<sysexits.h>` 的进程退出状态码。
- **Enum Values**:
  - `ok = 0` (成功)
  - `usage = 64` (`EX_USAGE`: 命令行参数/标志使用错误)
  - `dataErr = 65` (`EX_DATAERR`: 归档数据损坏/密码错误/解密失败)
  - `noInput = 66` (`EX_NOINPUT`: 输入归档或文件不存在/不可读)
  - `unavailable = 69` (`EX_UNAVAILABLE`: 格式引擎或硬件加速不可用)
  - `software = 70` (`EX_SOFTWARE`: 内部状态机/断言异常)
  - `cantCreat = 73` (`EX_CANTCREAT`: 无法创建目标输出文件/磁盘满)
  - `ioErr = 74` (`EX_IOERR`: 物理 IO 故障/管道断裂 EPIPE)
  - `noPerm = 77` (`EX_NOPERM`: 权限拒绝)
  - `sigint = 130` (`128 + 2`: 用户 Ctrl-C 中断退出)

### 2.3 `CLICommandContext` (CLI 运行上下文)
- **Description**: 命令执行周期的上下文环境。
- **Fields**:
  - `command: CLICommand` [Required]
  - `options: CLIOptions` [Required]
  - `positionalArguments: Array<String>` [Required]
  - `isInteractiveTTY: Bool` [Required]
  - `terminalColumns: Int` (物理列宽，默认 80) [Required]
  - `colorMode: ColorMode` (`.disabled`, `.ansi16`, `.ansi256`, `.trueColor`) [Required]
  - `language: AppLanguage` [Required]

### 2.4 `CLIEventNDJSON` (机器可读事件流模型)
- **Description**: `--json` 模式下向 stdout 逐行输出的 NDJSON 事件。
- **Discriminated Union (`event`)**:
  - `event = "progress"`:
    - `fraction: Double` ($0.0 \dots 1.0$)
    - `bytes_processed: Int64`
    - `total_bytes: Int64`
    - `speed_mbs: Double`
    - `current_file: String`
  - `event = "completed"`:
    - `exit_code: Int32`
    - `duration_seconds: Double`
    - `total_bytes: Int64`
    - `average_throughput_mbs: Double`
  - `event = "error"`:
    - `exit_code: Int32`
    - `error_code: String`
    - `message: String`
    - `target_path: String?`

---

## 3. 测试分层与报告中枢 (Test Infrastructure Subsystem)

### 3.1 `TestTier` (测试执行分层)
- **Description**: 6 级自动化测试分层。
- **Enum Values**:
  - `tier0Micro`: 纯内存算法、SIMD 硬件算子、28 模式调度 (耗时 $\le 5\text{ms}$)
  - `tier1Integration`: 16 种格式往返闭环、AES-256 加密、分卷切割、ZipSlip 防御
  - `tier2Differential`: 历史缺陷黄金语料库 (`.uu`)、macOS 原生 `/usr/bin/tar` 与 `/usr/bin/unzip` 交叉差分
  - `tier3PerformanceGate`: 全格式 262 维度历史最优吞吐硬门禁、UI 50k 树渲染门禁
  - `tier4CrashFuzz`: 崩溃现场优先变异模糊测试 (`fuzz_crash_reproducer.bin`)
  - `tier5StressScale`: 1GB/2GB 巨型分卷压力测试与竞品 PK 霸榜

### 3.2 `TestExecutionSession` (测试执行会话)
- **Description**: 统一测试调度会话元数据。
- **Fields**:
  - `sessionId: String` (UUID) [Required]
  - `timestamp: String` (ISO-8601) [Required]
  - `hostHardware: String` (如 `"Apple M3 Max (16 cores), 64GB RAM"`) [Required]
  - `osVersion: String` (如 `"macOS 14.5 (23F79)"`) [Required]
  - `targetTiers: Array<TestTier>` [Required]
  - `testCases: Array<TestCaseResult>` [Required]
  - `totalTests: Int` [Required]
  - `passedTests: Int` [Required]
  - `failedTests: Int` [Required]
  - `skippedTests: Int` [Required]
  - `totalDurationSeconds: Double` [Required]

### 3.3 `TestCaseResult` (用例执行结果)
- **Description**: 单个测试用例执行度量。
- **Fields**:
  - `name: String` [Required]
  - `className: String` [Required]
  - `tier: TestTier` [Required]
  - `durationSeconds: Double` [Required]
  - `status: String` (`"passed"`, `"failed"`, `"skipped"`) [Required]
  - `failureMessage: String?` [Optional]
  - `hexdumpDiff: String?` [Optional, 16 字节对齐差分分析]
  - `throughputMBs: Double?` [Optional, 性能测试专用度量]
