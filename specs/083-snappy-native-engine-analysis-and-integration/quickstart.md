# Quickstart & Verification Guide: Google Snappy 原生引擎 (083-snappy-native-engine-analysis-and-integration)

**Feature Branch**: `083-snappy-native-engine-analysis-and-integration`  
**Created**: 2026-08-18  
**Feature Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/spec.md)

---

## 1. 验证场景 1：原生 Snappy 块编解码与比特一致性验证

### Command
```bash
swift test --filter SnappyBlockEngineTests
```

### Expected Output
```text
Test Suite 'SnappyBlockEngineTests' passed at 2026-08-18 16:45:00.000.
	 Executed 12 tests, with 0 failures (0 unexpected) in 0.120 seconds
```

### Failure Diagnostic
若测试失败或断言错误：
1. 检查 `Sources/CTTZipBridge/snappy/` 中的 C++ 源文件是否按 C++17 成功编译。
2. 验证 `ttzip_snappy_compress` 与 `ttzip_snappy_decompress` 返回值是否为 `0` (`TTZIP_SNAPPY_OK`)。
3. 检查输入数据与解压输出数据是否完全一致（`XCTAssertEqual(decompressedData, sourceData)`）。

---

## 2. 验证场景 2：Snappy 官方 Framing 流式帧与 ARM64 PMULL CRC32C 验证

### Command
```bash
swift test --filter SnappyFramingStreamTests
```

### Expected Output
```text
Test Suite 'SnappyFramingStreamTests' passed at 2026-08-18 16:45:00.000.
	 Executed 15 tests, with 0 failures (0 unexpected) in 0.250 seconds
```

### Failure Diagnostic
若测试失败或报 CRC 不匹配：
1. 检查首部 10 字节是否严格等于 `0xFF 0x06 0x00 0x00 's' 'N' 'a' 'P' 'p' 'Y'`。
2. 检查 ARM64 硬件指令调用是否使用的是 Castagnoli 多项式（`__builtin_arm_crc32cw`/`d`）而非 IEEE 多项式。
3. 检查 Masked CRC 公式是否使用了 `((crc >> 15) | (crc << 17)) + 0xa282ead8`。

---

## 3. 验证场景 3：100% 进程内 TAR.SZ 归档创建与解包闭环

### Command
```bash
swift test --filter TarSnappyInProcessTests
```

### Expected Output
```text
Test Suite 'TarSnappyInProcessTests' passed at 2026-08-18 16:45:00.000.
	 Executed 8 tests, with 0 failures (0 unexpected) in 0.450 seconds
```

### Failure Diagnostic
若报告 `ARCHIVE_FAILED` 或找不到外部命令：
1. 检查 `Sources/CTTZipBridge/ttzip_tar_native.c` 是否已彻底移除 `archive_write_add_filter_program(a, "snappy")`，确认已接入 `ttzip_create_tar_snappy_native_c` 内存流回调。
2. 检查 `AllFormatsAndAdvancedParametersMatrixTests.swift` 中的 `testFormat_SNAPPY()` 是否已解除 `throw XCTSkip`。

---

## 4. 验证场景 4：13 维逆向变异与损坏流内存安全防御 (Fuzzing)

### Command
```bash
swift test --filter SnappySecurityAndFuzzingTests
```

### Expected Output
```text
Test Suite 'SnappySecurityAndFuzzingTests' passed at 2026-08-18 16:45:00.000.
	 Executed 14 tests, with 0 failures (0 unexpected) in 0.080 seconds
```

### Failure Diagnostic
若出现 EXC_BAD_ACCESS 或断言崩溃：
1. 检查 C 热路径中 Copy Tag 解引用前是否严格校验了 `offset > 0 && offset <= (op - op_base)`。
2. 检查 16 字节 Wild Copy 在末端（`op + 16 > op_limit`）是否正确降级为单字节精准拷贝。
3. 检查 Swift 层 `SnappyError` 是否完整捕获了所有底层返回的负数错误码。

---

## 5. 验证场景 5：全量回归与性能门禁测试

### Command
```bash
swift test --filter AllFormatsPkSuiteTests
```

### Expected Output
```text
📊 [TTZip Suite] Full format PK tests completed, total scenarios: 16 evaluated against competitors
All tests passed without regression.
```

### Failure Diagnostic
若有性能倒退：
1. 检查哈希表大小是否固定为 32KB（确保 L1 缓存命中）。
2. 检查是否在并发循环体内引入了共享锁或 `Data(count:)` 零填充。
