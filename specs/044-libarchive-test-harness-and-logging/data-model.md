# Data Model: 044-libarchive-test-harness-and-logging

**Feature**: [spec.md](./spec.md)  
**Created**: 2026-08-17  
**Status**: Completed  

---

## 1. TestExecutionSession (测试执行会话)

记录单次本地测试执行的全局上下文、运行参数、环境硬件与聚合统计。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `sessionId` | `String` | 是 | 唯一会话 UUID（如 `TEST-20260817-035500-A1B2`） |
| `startTime` | `String` | 是 | ISO-8601 起始时间戳 |
| `endTime` | `String` | 是 | ISO-8601 结束时间戳 |
| `durationMs` | `Double` | 是 | 总执行耗时（毫秒） |
| `environment` | `EnvironmentInfo` | 是 | 宿主系统与硬件规格 |
| `options` | `TestOptions` | 是 | CLI 传入的运行参数配置 |
| `summary` | `TestSuiteSummary` | 是 | 总体汇总统计数据 |
| `suites` | `[TestSuiteResult]` | 是 | 各测试套件执行明细清单 |

---

## 2. EnvironmentInfo (执行环境信息)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `osVersion` | `String` | 是 | 操作系统版本（如 `macOS 14.5 (23F79)`） |
| `cpuArchitecture` | `String` | 是 | CPU 架构（如 `arm64 (Apple M3 Max)`） |
| `logicalCores` | `Int` | 是 | 逻辑核心数（如 `16`） |
| `swiftVersion` | `String` | 是 | Swift 编译器版本（如 `6.0`） |

---

## 3. TestOptions (测试运行参数)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `filterPattern` | `String` | 否 | 测试用例/套件过滤正则（为空时运行全部） |
| `verbosity` | `Int` | 是 | 详细度（-1: 极简, 0: 默认, 1: 详细, 2: 全量） |
| `keepTempFiles` | `Bool` | 是 | 是否保留失败/成功的沙盒临时文件 |
| `dumpOnFailure` | `Bool` | 是 | 断言失败时是否保留崩溃现场与产生 Dump |
| `jsonReportPath` | `String` | 否 | JSON 报告持久化路径 |
| `markdownReportPath` | `String` | 否 | Markdown 报告持久化路径 |

---

## 4. TestSuiteResult (测试套件结果)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `suiteName` | `String` | 是 | 测试套件名称（如 `LibarchiveGoldenCorpusTests`） |
| `passedCount` | `Int` | 是 | 通过用例数 |
| `failedCount` | `Int` | 是 | 失败用例数 |
| `skippedCount` | `Int` | 是 | 跳过用例数 |
| `totalAssertions` | `Int` | 是 | 断言总次数 |
| `durationMs` | `Double` | 是 | 套件耗时（毫秒） |
| `cases` | `[TestCaseResult]` | 是 | 包含的各测试用例结果 |

---

## 5. TestCaseResult (单个测试用例结果)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `caseName` | `String` | 是 | 测试用例名称（如 `testZipWinzipAES256Corpus`） |
| `status` | `String` | 是 | 执行状态（`passed` / `failed` / `skipped`） |
| `durationMs` | `Double` | 是 | 用例执行耗时（毫秒） |
| `assertionCount` | `Int` | 是 | 用例内部执行的断言次数 |
| `failure` | `TestFailureEvidence` | 否 | 若失败，附带的精确诊断证据链 |

---

## 6. TestFailureEvidence (失败诊断证据链)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `file` | `String` | 是 | 失败发生的源码文件相对路径 |
| `line` | `Int` | 是 | 失败发生的行号 |
| `expression` | `String` | 是 | 断言表达式描述（如 `actualData == expectedData`） |
| `deferredMessage` | `String` | 否 | 延迟注入的上下文意图描述（如 `Extracting LFH at 0x20`） |
| `hexDiff` | `HexDiffSlice` | 否 | 二进制断言失败时的 HexDump 对比切片 |
| `unicodeDiff` | `UnicodeDiffDetail` | 否 | 字符串/路径断言失败时的 Unicode 码点差分 |
| `underlyingError` | `String` | 否 | C/POSIX 句柄底层的 errno 或 error_string |

---

## 7. HexDiffSlice (二进制差分切片)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `firstMismatchOffset` | `Int` | 是 | 首个不一致字节的偏移量（十进制） |
| `firstMismatchOffsetHex`| `String` | 是 | 首个不一致字节的十六进制偏移量（如 `0x00000042`） |
| `expectedLength` | `Int` | 是 | 期望缓冲区总字节数 |
| `actualLength` | `Int` | 是 | 实际缓冲区总字节数 |
| `formattedDiffWindow` | `String` | 是 | 16 字节对齐的 Hex + ASCII 可视化差分文本 |

---

## 8. UnicodeDiffDetail (Unicode 码点差分)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `expectedString` | `String` | 是 | 期望字符串字面量 |
| `expectedScalars` | `String` | 是 | 期望 Unicode 标量序列（如 `[0043 0061 00E9]`） |
| `actualString` | `String` | 是 | 实际字符串字面量 |
| `actualScalars` | `String` | 是 | 实际 Unicode 标量序列（如 `[0043 0061 0065 0301]`） |
| `isNfdNfcMismatch` | `Bool` | 是 | 是否属于 APFS/NFD 与 NFC 正规化差异 |
