# Quickstart & Verification Guide: 7z 全链路原生压缩流算法与自主无依赖引擎

**Feature ID**: `108-7z-native-compression-pipeline`  
**Date**: 2026-08-19  

---

## 1. 验证场景 1：全量单元测试与 7z 基础测试套件验证

### Command
```bash
swift test --filter SevenZip
```

### Expected Output
```text
Test Suite 'SevenZipTests' passed at ...
Executed 15 tests, with 0 failures (0 unexpected) in 0.420 seconds
```

### Failure Diagnostic
- **排查路径**: 若测试失败，检查 `Sources/TTZipCore/SevenZip/` 与 `Sources/TTZipCore/Adapters/SevenZipCAdapter.swift` 的 C 桥接签名绑定；
- **常见原因**: C 函数符号未在 `Sources/CTTZipBridge/include/CTTZipBridge.h` 或 `module.modulemap` 中正确导出。

---

## 2. 验证场景 2：7z 性能门禁与吞吐压测验证

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
[XCTestPerformanceMeasureTests] 7Z Level 1 throughput: >= 3200 MB/s (PASSED)
[XCTestPerformanceMeasureTests] 7Z Extraction throughput: >= 6600 MB/s (PASSED)
[XCTestPerformanceMeasureTests] 7Z LZMA2 Level 5 throughput: >= 480 MB/s (PASSED)
Test Suite 'XCTestPerformanceMeasureTests' passed
```

### Failure Diagnostic
- **排查路径**: 检查 `ttzip_lzma2_enc_native.c` 中的分块大小、Arena 预分配和 GCD 线程池分配；
- **常见原因**: 线程池过度竞争（Thread Oversubscription）或热路径中遗留了 `malloc` 动态分配。

---

## 3. 验证场景 3：7z CLI 基准测试与格式完整性

### Command
```bash
swift run ttzip-cli bench -f 7z
```

### Expected Output
```text
================================================================================
TTZip 7z Benchmark Suite
================================================================================
Store (Level 0):       >= 25000 MB/s
Fast (Level 1):        >= 3500 MB/s
Normal (Level 5):      >= 500 MB/s
Decompression:         >= 7000 MB/s
AES-256 KDF Derivation: <= 15 ms
Status: ALL BENCHMARKS PASSED
```

### Failure Diagnostic
- **排查路径**: 检查 `ttzip_7z_kdf_arm64.c` 与 `ttzip_7z_crypto_neon.c` 的 ARMv8 Crypto 指令执行；
- **常见原因**: 未开启 Apple Silicon 硬件加速或文件系统 I/O 受到非 APFS 磁盘挂载影响。

---

## 4. 验证场景 4：双向差分预言机测试 (System 7zz vs Native Engine)

### Command
```bash
# 使用自研引擎创建 7z 归档并使用官方 7zz 解压验证
swift run ttzip-cli create -a /tmp/test_native.7z -i /tmp/corpus_dir -l 5
/opt/homebrew/bin/7zz x /tmp/test_native.7z -o/tmp/verify_7zz -y
diff -r /tmp/corpus_dir /tmp/verify_7zz
```

### Expected Output
```text
Everything is Ok
(diff returns exit code 0, 0 byte differences)
```

### Failure Diagnostic
- **排查路径**: 检查 `ttzip_7z_header_writer.c` 与 `ttzip_lzma2_enc_native.c` 的 7z 容器格式字节布局；
- **常见原因**: LZMA2 Chunk 边界控制字节（0x00, 0x80, 0xE0）或属性字节 props 解析不对齐。
