# Quickstart & Verification Guide: 7z Zero-Configuration & Performance Hardening

**Feature Directory**: `specs/097-7z-zero-config-optimization-audit`  
**Date**: 2026-08-18

---

## Verification Scenarios

### Scenario 1: 7z Fast-LZMA2 Multi-Core Block Compression & Zero-Block Bypass

验证 7z 原生多核并行分块压缩、全零块旁路与流式生命周期。

- **Command**:
  ```bash
  swift test --filter FastLZMA2Tests
  ```
- **Expected Output**:
  ```text
  Test Suite 'FastLZMA2Tests' passed
  Executed 5 tests, with 0 failures (0 unexpected) in ~0.010 seconds
  ```
- **Failure Diagnostic**:
  若测试失败，检查 `Sources/CTTZipBridge/ttzip_fl2_bridge.c` 中 `ttzip_fl2_compress_block` 分发逻辑，确保 `ttzip_lzma2_compress_block_tuned` 与 `FL2_compressCCtx` 正确链接系统 `liblzma` 与 `fast-lzma2` 静态符号。

---

### Scenario 2: 7z Hardware Encrypted Header & AES-256 KDF Verification

验证 7z AES-256 加密归档创建、头部加密探测与 TouchID / 密码派生正确性。

- **Command**:
  ```bash
  swift test --filter TouchIDAndHeaderEncryptionTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'TouchIDAndHeaderEncryptionTests' passed
  Executed 2 tests, with 0 failures (0 unexpected) in ~0.025 seconds
  ```
- **Failure Diagnostic**:
  若测试失败，检查 `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` 中 ARMv8 NEON 向量指令与 `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c` 中 CBC 512KB 分块解密逻辑。

---

### Scenario 3: 7z Multi-Format Regression & Historical Peak Floor Matrix

验证 7z 与全 16 种格式在回归套件中的格式支持与零性能倒退。

- **Command**:
  ```bash
  swift test --filter FormatSupportTests
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.FormatSupportTests testSevenZipFormatSupport]' passed
  Test Suite 'FormatSupportTests' passed with 0 failures
  ```
- **Failure Diagnostic**:
  若 7z 格式测试失败，检查 `Sources/TTZipCore/SevenZip/SevenZipEngine.swift` 与 `SevenZipParallelWriter.swift` 中的桥接返回码。

---

### Scenario 4: 7z Native Zero-Copy Inspection (< 5ms) Integration Check

验证通过 `NativeSevenZipEngine.shared.inspectSevenZip` 零拷贝解析 7z 目录树的延迟与内存表现。

- **Command**:
  ```bash
  swift test --filter ArchiveEncryptionIntrospectionTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveEncryptionIntrospectionTests' passed
  Executed all tests with 0 failures
  ```
- **Failure Diagnostic**:
  若检视失败，检查 `Sources/CTTZipBridge/ttzip_native_archive.c` 中 `ttzip_7z_parse_header_metadata` 是否能正确解码 UTF-16LE 文件名与未压缩大小。
