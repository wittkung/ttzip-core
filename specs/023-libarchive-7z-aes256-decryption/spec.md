# Feature Specification: 023-libarchive-7z-aes256-decryption

**Feature Name**: 向 libarchive 贡献 7z AES-256 密码解密支持与跨平台密码学体系 (023-libarchive-7z-aes256-decryption)  
**Status**: DRAFT  
**Priority**: P1 (最高)  
**Target Upstream**: [libarchive/libarchive](https://github.com/libarchive/libarchive) (PR targeting `master`)  

---

## 1. Background & Executive Summary

`libarchive` 是类 Unix 操作系统（包括 macOS 原生 `bsdtar`、FreeBSD、Debian/Ubuntu `libarchive-tools` 等）及数十个开源包管理器的核心归档基础库。
然而，自 2017 年开启 [Issue #878](https://github.com/libarchive/libarchive/issues/878) 以来，`libarchive` 长期无法解密受密码保护的 7z 归档（遇到加密文件时直接返回 `ARCHIVE_FAILED: "Crypto codec not supported yet (ID: 0x6F10701)"`）。

本 Feature 旨在从数学与密码学理论出发，构建严谨的 7z AES-256-SHA-256 密钥派生（KDF）与 AES-256-CBC 流式解密数学模型，依托 TTZip 已有的高性能实现（[`ttzip_7z_kdf_arm64.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c) 与 [`CTTZipBridge_7z.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_7z.c)），为 `libarchive` 扩展通用的多后端密码学抽象（`archive_cryptor`），并在 `archive_read_support_format_7zip.c` 中完整支持：
1. **数据流加密（Stream Encryption / Plain Header）**：Folder 级别的 AES-256-CBC 解密。
2. **全头加密（Header Encryption / Encoded Header `0x17`）**：中央目录树的密码解密与递归解析。
3. **多密码学后端适配**：Apple CommonCrypto、OpenSSL (1.1 & 3.x)、Windows CNG (BCrypt)、mbedTLS、Nettle 及纯 C Fallback，确保跨平台 CI 全绿。

---

## 2. Clarifications & Mathematical Assumptions

### Session 2026-08-15
- **Q1: 7z 密码的字符编码规范是什么？**  
  **A1**: 7z 规范强制要求将 UTF-8 密码转换为 **UTF-16LE**（小端序双字节），且**不包含**结尾的 `\0` 字节。若输入空密码，则密码字节序列长度为 0。
- **Q2: 若 7z 归档中的 IV 长度不足 16 字节如何处理？**  
  **A2**: 属性解析时若 `ivSize < 16`，解密器必须将剩余字节用 `0x00` 填充对齐至 16 字节。
- **Q3: AES-256-CBC 的密文对齐与填充机制？**  
  **A3**: 7z 将原始压缩流按 16 字节对齐后用 AES-256-CBC 加密。解密时按 16 字节整块解密，解密后的明文直接送入 LZMA/LZMA2 解压缩器，由 Decompressor 的 `UnpackSize` 自动截断多余数据。
- **Q4: Fork 与上游测试验证策略？**  
  **A4**: 在开发与验证阶段，在本地隔离目录建立 `libarchive` 独立构建树（CMake + Ninja），启用 `ENABLE_TEST=ON`，直接运行其官方测试二进制 `libarchive_test`，确保测试用例 100% 真实绿灯。

---

## 3. Mathematical & Cryptographic Theory Foundations

### 2.1 7z KDF 形式化定义与计算复杂度模型
设用户输入 UTF-8 密码字符串转换后的 UTF-16LE（小端序 16 位整数）字节序列为 $P \in \{0,1\}^{16 L_p}$（长度为 $2 L_p$ 字节），Salt 为 $S \in \{0,1\}^{8 L_s}$（$0 \le L_s \le 16$ 字节），迭代指数 $E = \text{NumCyclesPower} \in [0, 24]$，迭代总轮数 $R = 2^E$（默认 $E=19 \implies R=524,288$）。

在每个迭代轮次 $r \in [0, R-1]$ 中，构造消息分块：
$$M_r = S \mathbin{\Vert} P \mathbin{\Vert} \text{LE64}(r)$$
其中 $\text{LE64}(r)$ 为 64 位小端序整数编码（8 字节），总消息流为：
$$M = M_0 \mathbin{\Vert} M_1 \mathbin{\Vert} M_2 \mathbin{\Vert} \dots \mathbin{\Vert} M_{R-1}$$

令 SHA-256 初始状态为 $H_0 = \text{IV}_{\text{SHA256}}$，对连续 512-bit 分组应用压缩函数 $f_{\text{SHA256}}$：
$$H_{i+1} = f_{\text{SHA256}}(H_i, B_i)$$
最终派生密钥为：
$$K_{\text{AES}} = \text{Finalize}(H_m) \in \{0,1\}^{256}$$

- **理论复杂度**：总吞吐数据量 $|M| = R \cdot (L_s + 2L_p + 8)$ 字节。以 $L_p=8, L_s=0, R=2^{19}$ 为例，$|M| \approx 12.58\text{ MB}$。
- **CPU 周期开销**：
  - 纯标量软件实现：约 $150 \sim 250\text{ ms}$。
  - 硬件加速实现（ARMv8 Crypto / Intel SHA-NI）：约 $12 \sim 18\text{ ms}$（加速比 $> 10\times$）。

### 2.2 AES-256-CBC 解密数学模型
设密文分组序列为 $C_0, C_1, \dots, C_{n-1} \in \{0,1\}^{128}$，初始向量为 $\text{IV} \in \{0,1\}^{128}$，解密变换 $D_K$：
$$P_0 = D_K(C_0) \oplus \text{IV}$$
$$P_i = D_K(C_i) \oplus C_{i-1} \quad (\forall i \ge 1)$$

由于 $P_i$ 仅依赖于当前密文块 $C_i$ 与前一密文块 $C_{i-1}$，解密过程不存在数据反向依赖，支持任意块大小的零等待流水线解密。

---

## 3. User Stories

### User Story 1: libarchive 数据流加密 7z 归档解密 (Priority: P1)
- **As a** libarchive 开发者与终端用户（使用 `bsdtar` 或系统解压库）
- **I want** 输入密码后能够正常解压数据流被 AES-256 加密的 7z 归档
- **So that** 不再报错 `"Crypto codec not supported yet"`，成功提取所有加密文件。

#### Acceptance Scenarios
1. 对包含普通文件且 Folder 使用 `_7Z_CRYPTO_AES_256_SHA_256` 加密的 7z 归档，调用 `archive_read_add_passphrase()` 后能够正确解密并校验 CRC32。
2. 密码错误时返回 `ARCHIVE_FAILED` 或 `ARCHIVE_WARN`，不发生内存泄漏或段错误崩溃。

### User Story 2: libarchive 全头加密 (`kEncodedHeader`) 7z 归档解密 (Priority: P1)
- **As a** 安全与归档工具开发者
- **I want** 对加密了文件名和目录树的 7z 归档（`-mhe=on`）在读取 Central Directory 时完成解密
- **So that** 能够列出完整文件树并逐一提取。

#### Acceptance Scenarios
1. 对 `kEncodedHeader` 类型的 7z 文件，在 `archive_read_next_header()` 前获取密码并正确解析解密后的明文 Header。
2. 激活并绿灯通过 `test_read_format_7zip_encryption_header.c`。

### User Story 3: 跨平台多后端密码学抽象与零破坏兼容 (Priority: P1)
- **As a** libarchive 维护团队成员
- **I want** 新增的 AES-CBC 与 KDF 支持多密码学后端（CommonCrypto, OpenSSL, CNG, mbedTLS, Nettle, Stubs）
- **So that** 在全平台 CI（Linux, macOS, Windows, FreeBSD）上 100% 编译通过且测试绿灯。

---

## 4. Functional Requirements

- **FR-001**: 在 `archive_cryptor_private.h` 与 `archive_cryptor.c` 中定义并实现统一的 AES-CBC 接口：
  - `__archive_cryptor_aes_cbc_init(archive_crypto_ctx *ctx, const uint8_t *key, size_t key_len, const uint8_t *iv, size_t iv_len)`
  - `__archive_cryptor_aes_cbc_update(archive_crypto_ctx *ctx, const uint8_t *in, size_t in_len, uint8_t *out, size_t *out_len)`
  - `__archive_cryptor_aes_cbc_release(archive_crypto_ctx *ctx)`
- **FR-002**: 在 `archive_cryptor.c` 中实现基于 `archive_digest_private.h` 的标准 7z SHA-256 KDF：
  - `__archive_7z_kdf_sha256(const char *password, const uint8_t *salt, size_t salt_len, unsigned numCyclesPower, uint8_t *aes_key)`
- **FR-003**: 在 `archive_read_support_format_7zip.c` 的 `read_Folder` 中解析 `_7Z_CRYPTO_AES_256_SHA_256` 属性（NumCyclesPower, Salt, IV）。
- **FR-004**: 在 `init_decompression` 与 `decompress` 中装配 AES-CBC 流解密器，在解压数据送入 LZMA/LZMA2 前完成就地解密。
- **FR-005**: 在 `read_Header` / `slurp_central_directory` 中支持 `kEncodedHeader` 的密码解密与二次解析。
- **FR-006**: 更新 `libarchive/test/test_read_format_7zip_encryption_*.c` 测试套件，验证文件解密正确性与 CRC32 校验。

---

## 5. Success Criteria

- **SC-001**: 完整通过 `libarchive` 自带的 3 个 7z 加密测试用例（`test_read_format_7zip_encryption_data`, `test_read_format_7zip_encryption_header`, `test_read_format_7zip_encryption_partially`）。
- **SC-002**: 在 macOS (AppleClang + CommonCrypto)、Linux (GCC/Clang + OpenSSL) 与 Windows (MSVC + CNG) 环境下编译零 Warning，无内存泄漏。
- **SC-003**: 严格遵守 BSD-2-Clause 许可与 C89/C99 编码标准。
