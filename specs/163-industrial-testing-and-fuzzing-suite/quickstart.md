# Quickstart Guide: 工业级极端边界与安全测试验证 (Feature 163)

## Scenario 1: 执行 CVE 恶意畸变包安全拦截测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner cve_regressions
  ```
- **Expected Output**:
  - `[PASS] CVE-2002-0059 (Huffman code tree overflow): Successfully rejected (BAD_DATA)`
  - `[PASS] CVE-2005-1849 (Window distance overflow): Successfully rejected`
  - `[PASS] CVE-2018-25032 (Deflate hash loop overwrite): Successfully rejected`
  - `[PASS] GH-382 (Negative offset pointer): Successfully rejected`
  - 100% 优雅拦截，0 崩溃，0 假死。
- **Failure Diagnostic**:
  - 若发生 SIGSEGV，查看 ASan 栈追踪定位哪一个解压器没有做边界检查。

---

## Scenario 2: 执行跨年代远古归档兼容性测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner compat_archives
  ```
- **Expected Output**:
  - `[PASS] PKZIP 2.04g Junk bytes archive: Extracted successfully`
  - `[PASS] Streaming ZIP with Data Descriptors: Extracted successfully`
  - `[PASS] GNU Tar @LongLink path > 100 chars: Extracted successfully`
  - `[PASS] PowerShell backslash paths: Normalized and extracted`
- **Failure Diagnostic**:
  - 若路径丢失，检查 `ttzip_tar_native.c` 中的 GNU longlink 和 EOCD 偏置处理逻辑。

---

## Scenario 3: 执行 macOS APFS 扩展属性与 1GB 稀疏文件往返测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner fs_metadata
  ```
- **Expected Output**:
  - `[PASS] com.apple.quarantine and custom xattrs preserved 100%`
  - `[PASS] 1GB APFS Sparse File roundtrip preserved (physical usage < 1MB)`
- **Failure Diagnostic**:
  - 若稀疏空洞膨胀，检查 `ARCHIVE_EXTRACT_SPARSE` 选项是否启用。

---

## Scenario 4: 编译并执行原生 LibFuzzer 模糊测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_fuzzer && ./build/ttzip_fuzzer -max_total_time=5 tests/fixtures/cve/
  ```
- **Expected Output**:
  - 运行数万次输入变异，0 内存泄漏，0 地址违规。
