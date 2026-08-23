# Phase 1 Quickstart & Validation Guide: 023-libarchive-7z-aes256-decryption

## 1. 验证场景 1：7z 密码数据流解密单测执行

### Command
```bash
cmake -B build-test -DENABLE_TEST=ON -DENABLE_TAR=OFF -DENABLE_CPIO=OFF
cmake --build build-test --target libarchive_test
./build-test/bin/libarchive_test -r test_read_format_7zip_encryption_data
```

### Expected Output
```text
Running tests on: libarchive 3.8.x
  test_read_format_7zip_encryption_data: OK
1 test passed, 0 failures, 0 skipped.
```

### Failure Diagnostic
- 若返回 `ARCHIVE_FAILED: "Crypto codec not supported yet"`，检查 `archive_read_support_format_7zip.c` 中 `case _7Z_CRYPTO_AES_256_SHA_256:` 分支是否正确跳转到 `init_decompression` 的解密初始化流程。
- 若校验码不匹配，检查 AES-CBC 解密后的明文缓冲是否按 16 字节对齐送入 LZMA 解压缩器，以及 IV 是否补齐至 16 字节。

---

## 2. 验证场景 2：7z 全头加密 (`kEncodedHeader`) 目录树解密测试

### Command
```bash
./build-test/bin/libarchive_test -r test_read_format_7zip_encryption_header
```

### Expected Output
```text
  test_read_format_7zip_encryption_header: OK
1 test passed, 0 failures, 0 skipped.
```

### Failure Diagnostic
- 若报 `Corrupted archive header`，检查 `read_Header` 中遇到 `kEncodedHeader` 时是否成功拉取 Passphrase 并解密得到临时内存 Header 流。
- 若报 `Invalid passphrase`，检查 UTF-8 到 UTF-16LE 转码是否去除了结尾 `\0`。

---

## 3. 验证场景 3：TTZip In-Process 桥接与回归门禁验证

### Command
```bash
swift test --filter SevenZipExtractorTests
```

### Expected Output
```text
Test Suite 'SevenZipExtractorTests' passed.
Executed 8 tests, with 0 failures (0 unexpected).
```

### Failure Diagnostic
- 若测试耗时异常升高，检查 `SevenZipFolderCryptoContext` 中的密钥派生缓存是否生效。
