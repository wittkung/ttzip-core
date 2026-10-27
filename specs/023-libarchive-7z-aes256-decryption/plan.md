# Implementation Plan: 023-libarchive-7z-aes256-decryption

**Feature Name**: 向 libarchive 贡献 7z AES-256 密码解密支持与跨平台密码学体系  
**Target Upstream**: `libarchive/libarchive` (GitHub Pull Request)  

---

## 1. Technical Context & Academic Background

`libarchive` 是开源生态中最重要的流式归档解析库，但自 2017 年以来长期缺少 7z 格式的密码解密支持（Issue #878）。
7z 加密体系结合了 $2^{\text{NumCyclesPower}}$ 轮 SHA-256 迭代 KDF 与 AES-256-CBC 对称分组加密。
本方案从密码学数学模型出发，设计了具备多后端兼容性（Apple CommonCrypto、OpenSSL、Windows CNG、mbedTLS、Nettle）的流式解密抽象，并消除解密热路径中的多余堆分配与内存拷贝。

---

## 2. Constitution Check

- [x] **Zero-Cost Abstraction**: 解密热路径采用 16 字节块就地对齐流解密，零中间临时文件落地。
- [x] **BSD-2-Clause Compatibility**: 纯 C 实现，零 GPL 传染风险。
- [x] **C89 / C99 Standard**: 严禁 C++，严禁编译器专有宏无隔离暴露。
- [x] **Multi-Backend Resilience**: 覆盖全部 6 种密码学后端与无密码环境优雅降级。

---

## 3. Phase 0: Research Items

- R001 [SUBAGENT:research] 《7z KDF 与 AES-CBC 多后端密码学抽象架构》：针对 CommonCrypto、OpenSSL、Windows CNG 等 6 类后端的对称加密统一接口设计。
- R002 [SUBAGENT:research] 《7z 全头加密 (`kEncodedHeader`) 递归解密架构》：在 `archive_read_support_format_7zip.c` 中内存级解密与目录树二次解析。
- R003 [SUBAGENT:research] 《本地 Fork 构建验证与 CI 测试套件规范》：CMake 隔离构建与 `libarchive_test` 原生测试集配置。

---

## 4. Phase 1: Design & Contracts

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/023-libarchive-7z-aes256-decryption/data-model.md)
- **Contracts**:
  - [`contracts/sevenzip_crypto_properties.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/023-libarchive-7z-aes256-decryption/contracts/sevenzip_crypto_properties.json)
  - [`contracts/sevenzip_folder_crypto_context.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/023-libarchive-7z-aes256-decryption/contracts/sevenzip_folder_crypto_context.json)
- **Validation Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/023-libarchive-7z-aes256-decryption/quickstart.md)

---

## 5. Proposed Changes by Component

### Component 1: Cryptographic Abstraction (`libarchive/archive_cryptor.*`)
- **[MODIFY] `libarchive/archive_cryptor_private.h`**:
  - 声明 `__archive_cryptor_aes_cbc_init`、`__archive_cryptor_aes_cbc_update`、`__archive_cryptor_aes_cbc_release`。
  - 声明 7z 专用 KDF `__archive_7z_kdf_sha256`。
- **[MODIFY] `libarchive/archive_cryptor.c`**:
  - 实现各加密后端（CommonCrypto, OpenSSL, CNG, mbedTLS, Nettle）的 AES-256-CBC 分组解密驱动。
  - 实现基于 `archive_digest_private.h` 的 $2^E$ 轮 SHA-256 迭代与 UTF-16LE 转换。

### Component 2: 7z Reader Core (`libarchive/archive_read_support_format_7zip.c`)
- **[MODIFY] `archive_read_support_format_7zip.c`**:
  - 在 `read_Folder` 中解析 `_7Z_CRYPTO_AES_256_SHA_256` 属性字节流并存储至 `struct _7z_folder`。
  - 在 `init_decompression` 中挂载 AES-CBC 解密管道并完成 Passphrase 密钥派生。
  - 在 `decompress` 中将数据流先行解密后再输送至 LZMA 解压缩器。
  - 在 `read_Header` 中支持 `kEncodedHeader` 的就地解密。

### Component 3: Test Suite Activation (`libarchive/test/test_read_format_7zip_encryption_*.c`)
- **[MODIFY] `test_read_format_7zip_encryption_data.c`**: 更新预期断言为解密成功并校验文件内容；增加错误密码负面测试分支。
- **[MODIFY] `test_read_format_7zip_encryption_header.c`**: 更新断言为正确列出并提取被保护文件；增加错误密码负面测试分支。
- **[MODIFY] `test_read_format_7zip_encryption_partially.c`**: 覆盖混合多 Folder 解密与负面测试。

### Component 4: Code Quality & C89 Hardening
- **[MODIFY] `libarchive/archive_cryptor.c`**:
  - `utf8_to_utf16le()`: 将循环变量与临时变量（`uint16_t high, low;`、`size_t k;`）全部前置至函数顶部声明，严格符合 C89 语法规范。
- **[MODIFY] `libarchive/archive_read_support_format_7zip.c`**:
  - `extract_pack_stream()`: 将 `size_t to_read, aligned_in, dec_out;` 等临时变量提升至 `for (;;)` 循环体顶部声明。
  - `setup_decode_folder()`: 消除 `static const struct _7z_coder coder_copy` 的作用域内重复声明，杜绝 `-Wshadow` 编译警告。

---

## 6. Verification Plan

### Automated Tests
- 执行 `libarchive_test` 官方测试套件：
  ```bash
  ./build/bin/libarchive_test -r libarchive/test test_read_format_7zip_encryption_data
  ./build/bin/libarchive_test -r libarchive/test test_read_format_7zip_encryption_header
  ./build/bin/libarchive_test -r libarchive/test test_read_format_7zip_encryption_partially
  ./build/bin/libarchive_test -r libarchive/test test_read_format_7zip
  ```
- 严格执行全量 300+ 回归测试，确保零编译告警（-Wall -Werror -Wshadow -Wextra）：
  ```bash
  ./build/bin/libarchive_test -r libarchive/test -q
  ```

