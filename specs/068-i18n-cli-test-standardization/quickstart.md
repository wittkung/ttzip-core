# Quickstart: 全面国际化、CLI 标准化与测试体系验证指南 (Validation Guide)

**Feature**: `068-i18n-cli-test-standardization`  
**Date**: 2026-08-17  
**Status**: Ready for Verification

---

## 1. 验证场景一：7 语言本地化完整性与格式化占位符自动化断言

### Command
```bash
swift test --filter LocalizationIntegrityTests
```

### Expected Output
```text
Test Suite 'LocalizationIntegrityTests' started at 2026-08-17 20:40:00.000
Test Case '-[TTZipTests.LocalizationIntegrityTests testAllKeysPresentInAllSevenLanguages]' passed (0.012 seconds).
Test Case '-[TTZipTests.LocalizationIntegrityTests testNoOrphanKeysInLanguagePacks]' passed (0.008 seconds).
Test Case '-[TTZipTests.LocalizationIntegrityTests testFormatSpecifierParityAndTypeSafetyAcrossLanguages]' passed (0.015 seconds).
Test Case '-[TTZipTests.LocalizationIntegrityTests testCascadingFallbackToEnglish]' passed (0.005 seconds).
Executed 4 tests, with 0 failures (0 unexpected) in 0.040 seconds.
```

### Failure Diagnostic
- **若报 Key 缺失**: 检查 `Sources/TTZipCore/Localization/Catalogs/` 下是否有语言包缺少对应键定义，需补充对应语言的翻译。
- **若报占位符类型不一致**: 检查报错的 Key 在不同语言包中是否混用了 `%@` (对象/字符串) 与 `%d` / `%lld` (整型)，修正占位符使其与基准英语完全一致。

---

## 2. 验证场景二：CLI POSIX 规范、参数解析与退出状态码核验

### Command
```bash
# 1. 验证标准短选项合并与 --dry-run
swift run ttzip-cli archive test_out.tar.zst Sources/ -vq --dry-run
echo "Exit Code: $?"

# 2. 验证未知选项报错与 EX_USAGE (64) 状态码
swift run ttzip-cli archive --unknown-flag-test
echo "Exit Code: $?"

# 3. 验证文件不存在报错与 EX_NOINPUT (66) 状态码
swift run ttzip-cli extract /nonexistent_archive_12345.zip -o ./out
echo "Exit Code: $?"
```

### Expected Output
```text
# 场景 1 预期输出:
[DRY-RUN] Would create archive: test_out.tar.zst from 1 source path(s)
Exit Code: 0

# 场景 2 预期输出:
ttzip-cli: unrecognized option '--unknown-flag-test'
Try 'ttzip-cli --help' for more information.
Exit Code: 64

# 场景 3 预期输出:
ttzip-cli: error: input file '/nonexistent_archive_12345.zip' does not exist
Exit Code: 66
```

### Failure Diagnostic
- **若退出码为 1 而非 64/66**: 检查 `POSIXCLIArgumentParser.swift` 与 `CLICommandRouter.swift` 是否未接入 `CLIExitCode` 强类型映射。

---

## 3. 验证场景三：非 TTY 管道模式与 NDJSON 结构化事件流验证

### Command
```bash
# 重定向管道并开启 --json 模式
swift run ttzip-cli archive -f zip - Sources/TTZipCLI/ --json | head -n 5
```

### Expected Output
```json
{"event":"progress","timestamp":1723908000.12,"progress":{"fraction":0.25,"bytes_processed":25000,"total_bytes":100000,"speed_mbs":1850.0,"current_file":"CLIOptions.swift"}}
{"event":"progress","timestamp":1723908000.14,"progress":{"fraction":0.50,"bytes_processed":50000,"total_bytes":100000,"speed_mbs":1870.0,"current_file":"CLICommandRouter.swift"}}
{"event":"completed","timestamp":1723908000.18,"completed":{"exit_code":0,"duration_seconds":0.06,"total_bytes":100000,"average_throughput_mbs":1860.0}}
```

### Failure Diagnostic
- **若输出混杂 ANSI 颜色控制字符 `\x1b[`**: 检查 `TerminalRenderEngine.swift` 的 TTY 自动探测是否未在重定向至管道时生效。

---

## 4. 验证场景四：6 层分级测试调度器与全格式性能门禁回归

### Command
```bash
# 1. 运行 Tier 0 微单元测试 (必须在 3 秒内完成)
swift run ttzip-cli test --tier 0

# 2. 运行 Tier 3 性能硬门禁回归测试
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
# Tier 0 执行输出:
[TTZip-TestRunner] Running Tier 0 (Micro/Unit Tests)...
✅ Tier 0 passed: 85 tests executed in 1.42s (0 failures)

# Tier 3 执行输出:
Test Suite 'XCTestPerformanceMeasureTests' passed.
- ZIP Level 1 Throughput: >= 1500 MB/s [PASSED]
- 7Z Level 1 Throughput: >= 3200 MB/s [PASSED]
- TAR.ZST Direct Throughput: >= 15000 MB/s [PASSED]
```

### Failure Diagnostic
- **若 Tier 3 报吞吐倒退**: 检查近期变更是否在数据热路径引入了加锁、额外堆分配或未被 bypass 的慢路径。
