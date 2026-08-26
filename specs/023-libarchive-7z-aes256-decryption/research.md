# Phase 0 Research: 023-libarchive-7z-aes256-decryption

## Research Item R001: 7z KDF 与 AES-CBC 多后端密码学抽象架构

### Decision
在 `archive_cryptor_private.h` 与 `archive_cryptor.c` 中扩展统一的 `aes_cbc` 对称分组加密接口，并基于 `archive_digest_private.h` 实现跨平台的 7z SHA-256 KDF。

### Rationale
- **多后端统一抽象**：`libarchive` 现有架构要求支持 CommonCrypto (`CCCryptorCreate(kCCDecrypt, kCCAlgorithmAES, 0, ...)`), OpenSSL (`EVP_CIPHER_CTX`, `EVP_aes_256_cbc()`), Windows CNG (`BCryptSetProperty(..., BCRYPT_CHAINING_MODE, BCRYPT_CHAIN_MODE_CBC)`), mbedTLS (`mbedtls_aes_crypt_cbc`) 及 Nettle。统一接口可保证 CMake/Autotools 在任何后端配置下平滑编译。
- **复用现有 SHA-256 基础设施**：`archive_digest_private.h` 已经封装了跨平台的 `archive_sha256_*` 宏与函数指针，7z KDF 的 $2^{\text{NumCyclesPower}}$ 轮迭代可直接调用，避免重复编写底层 SHA-256 算法。

### Alternatives Considered
- **方案 A：直接在 7zip 模块内引入外部第三方加密库（如 libsodium 或 7-Zip C++ 源码）**
  - **否决理由**：违反 `libarchive` 的零强制外部依赖原则与 BSD-2-Clause 许可要求，上游维护者绝不接纳引入新依赖的 PR。
- **方案 B：仅支持 OpenSSL 单一后端**
  - **否决理由**：macOS (CommonCrypto) 与 Windows (CNG) 默认不自带 OpenSSL，会导致 CI 构建矩阵中的 macOS 与 Windows 作业全线红灯。

### Source
- [libarchive/archive_cryptor.c](https://github.com/libarchive/libarchive/blob/master/libarchive/archive_cryptor.c)
- [libarchive/archive_cryptor_private.h](https://github.com/libarchive/libarchive/blob/master/libarchive/archive_cryptor_private.h)
- [libarchive/archive_digest_private.h](https://github.com/libarchive/libarchive/blob/master/libarchive/archive_digest_private.h)

---

## Research Item R002: 7z 全头加密 (`kEncodedHeader`) 的流式解析与递归解密架构

### Decision
在 `archive_read_support_format_7zip.c` 的 `read_Header` / `slurp_central_directory` 中增加对 `kEncodedHeader` 的解密拦截，将其作为内部 Folder 解析解密为明文 Header 缓冲区，再递归调用 `read_Header` 解析明文目录树。

### Rationale
- 7z 的全头加密（`-mhe=on`）机制将包含文件列表的整个 Header 打包为一个 Folder。
- 将其抽象为单个临时内存 Folder 解码任务，解密解压后的数据流即为标准的明文 Header 结构，与既有的 `read_Header` 解析器完全兼容，无需重写目录树解析逻辑。

### Alternatives Considered
- **方案 A：在扫描归档尾部时直接将密文 Header 写入临时磁盘文件后再用新句柄打开**
  - **否决理由**：破坏纯内存解压架构，增加 I/O 开销并引发临时文件泄露与沙盒安全风险。

### Source
- [libarchive/archive_read_support_format_7zip.c](https://github.com/libarchive/libarchive/blob/master/libarchive/archive_read_support_format_7zip.c)
- 7-Zip 官方规范文档 `DOC/7zFormat.txt`

---

## Research Item R003: 本地 Fork 构建验证与 CI 测试套件规范

### Decision
在本地构建验证环境中，克隆/检出 `libarchive` 官方分支，配置 CMake + Ninja + AppleClang (CommonCrypto)，编译并执行官方测试程序 `libarchive_test`，重点运行 `test_read_format_7zip_encryption_*` 套件。

### Rationale
- 官方测试套件具有极其严格的内存对齐、字符串解析与错误码校验逻辑。
- 只有通过 `libarchive_test` 的本地测试，才能确保提交至 GitHub 的 PR 能够一次性通过 upstream 全平台 CI 检查。

### Alternatives Considered
- **方案 A：仅在 TTZip 内部编写 Swift 桥接单测，不运行 `libarchive_test` 原生测试**
  - **否决理由**：无法验证 C 级标准库接口的跨平台行为与 libarchive 专有的 `assertion` 宏。

### Source
- `libarchive/test/test_read_format_7zip_encryption_data.c`
- `libarchive/test/test_read_format_7zip_encryption_header.c`
