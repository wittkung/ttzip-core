# Quickstart: 024-full-matrix-peak-consolidation-and-zero-regression

## 1. 验证 DMG AES 加密解压与直通路由

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  [▶ DMG 500MB L6 (AES) Extract] Throughput: >= 9933.1 MB/s -> PASS [PERF_OPTIMAL]
  ```
- **Failure Diagnostic**:
  - 检查 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 是否对 `password != nil` 走了 `SevenZipEngine` 硬件 AES 路径。

---

## 2. 验证 TAR 变体小文件解压吞吐提升

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  [▶ TAR Batch Small Files L6 Extract] Throughput: >= 1304.1 MB/s -> PASS [PERF_OPTIMAL]
  ```
- **Failure Diagnostic**:
  - 检查 `Sources/CTTZipBridge/ttzip_tar_native.c` 中 `mkdir_cache` 是否正确跳过了重复系统调用。
