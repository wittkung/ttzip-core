# Quickstart Validation Guide: 030-libarchive-optimizations-integration

本文档提供本特性的可执行验证流程与断言标准。

---

## Scenario 1: 验证 Vendor 静态库符号与 7z 解密能力就绪

- **Command**:
  ```bash
  nm Vendor/lib/libarchive.a | grep -E "(decrypto_aes_cbc|kdf_7z_sha256)"
  ```
- **Expected Output**:
  ```
  0000000000000450 t _decrypto_aes_cbc_init
  00000000000004e8 t _decrypto_aes_cbc_release
  000000000000049c t _decrypto_aes_cbc_update
  0000000000000518 t _kdf_7z_sha256
  ```
- **Failure Diagnostic**:
  若未输出上述符号，表明 `Vendor/lib/libarchive.a` 未成功替换为 `Vendor/libarchive-upstream/build/bin/libarchive.a`。执行 `cp Vendor/libarchive-upstream/build/bin/libarchive.a Vendor/lib/libarchive.a` 并重试。

---

## Scenario 2: 运行 7z 加密双引擎单元测试

- **Command**:
  ```bash
  swift test --filter Libarchive7zEncryptionTests
  ```
- **Expected Output**:
  ```
  Test Suite 'Libarchive7zEncryptionTests' passed at ...
  Executed 3 tests, with 0 failures (0 unexpected) in ... seconds
  ```
- **Failure Diagnostic**:
  若测试失败并提示 `TTZIP_ERR_OPEN_FAILED` 或数据校验错误，检查密码传入是否经过 UTF-8 到 UTF-16LE 转换，并排查 `archive_read_add_passphrase` 是否正确设置。

---

## Scenario 3: 验证 7z 原生 KDF 硬件加速与性能门禁

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests/testSevenZipKdf_HardwareAcceleration_DurationFloor
  ```
- **Expected Output**:
  ```
  Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testSevenZipKdf_HardwareAcceleration_DurationFloor]' passed
  ```
- **Failure Diagnostic**:
  若耗时 $> 17\text{ ms}$（Debug）或 $> 15\text{ ms}$（Release），检查 `ttzip_7z_kdf_arm64.c` 中是否残留 `malloc`/`free` 堆分配，或是否误用了标量软件循环。

---

## Scenario 4: 全格式 46 场景基准回归与零倒退审计

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests
  python3 scripts/audit_performance_regression.py
  ```
- **Expected Output**:
  ```
  [Audit Summary] Total: 46 scenarios, 0 regressions (> 3.0%), 100% floor pass.
  ```
- **Failure Diagnostic**:
  若发生性能倒退，查看 `scripts/audit_performance_regression.py` 输出的倒退项详情，确认是否侵入了热路径循环。
