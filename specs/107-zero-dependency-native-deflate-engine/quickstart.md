# Quickstart: 100% 自研零外部依赖原生 Apple Silicon DEFLATE 引擎体系

**Feature ID**: `107-zero-dependency-native-deflate-engine`  
**Status**: APPROVED  

---

## 1. 验证场景 1：原生 Deflate 单元测试与边界健全性
- **Command**:
  ```bash
  swift test --filter NativeDeflateEngineTests
  ```
- **Expected Output**:
  ```
  Test Suite 'NativeDeflateEngineTests' passed.
  Executed 6 tests, with 0 failures (0 unexpected).
  ```
- **Failure Diagnostic**:
  检查 `Sources/CTTZipBridge/native_deflate/` 中的位流累加器溢出保护与退化字母表补充逻辑。

---

## 2. 验证场景 2：18 核心 Tile 并发压缩与系统 `/usr/bin/unzip -t` 差分验证
- **Command**:
  ```bash
  swift test --filter ZipExtremeBlockWriterTests
  ```
- **Expected Output**:
  ```
  Test Suite 'ZipExtremeBlockWriterTests' passed.
  Executed 3 tests, with 0 failures (0 unexpected).
  ```
- **Failure Diagnostic**:
  检查前 $N-1$ 块是否正确写入 `BFINAL=0` 与 `Z_SYNC_FLUSH` (`0x00, 0x00, 0xFF, 0xFF`)。

---

## 3. 验证场景 3：18 核心帕累托全生态 PK 与全量图表导出
- **Command**:
  ```bash
  TTZIP_BENCH_ALL_LIVE=1 swift test --filter ZipMultiCoreParetoFrontierPkTests
  ```
- **Expected Output**:
  ```
  🏆 纯 ZIP 格式 18 核心满载极限对决图表已生成: pareto_pk_zip_multicore.png
  ```
- **Failure Diagnostic**:
  检查各档位吞吐与体积是否完全压制 `pigz`、`7-Zip`、`ouch` 与 `advzip`。
