# Phase 1 Data Model: 030-libarchive-optimizations-integration

## 1. Entity Definitions

### 1.1 7z Crypto Session (`ttzip_7z_crypto_session_t`)
表示 7z AES-256-SHA-256 加解密会话的生命周期状态实体。

| 字段名 | C 类型 / Swift 类型 | 必填 | 约束与描述 |
| :--- | :--- | :--- | :--- |
| `is_active` | `bool` / `Bool` | 是 | 是否处于有效加密解密状态 |
| `aes_key` | `uint8_t[32]` / `[UInt8]` (32 字节) | 是 | 经 7z KDF 派生出的 AES-256 密钥（32 字节） |
| `aes_iv` | `uint8_t[16]` / `[UInt8]` (16 字节) | 是 | AES-256-CBC 初始向量 IV（16 字节） |
| `num_cycles_power` | `uint32_t` / `UInt32` | 是 | SHA-256 迭代幂次（$0 \le N \le 24$，默认 19 即 524,288 轮） |

### 1.2 归档条目加密自省元数据 (`ArchiveEntryEncryptedMetadata`)
描述穿透检查时获取到的单个条目的加密特征实体。

| 字段名 | 类型 | 必填 | 约束与描述 |
| :--- | :--- | :--- | :--- |
| `pathname` | `String` | 是 | 归档内相对路径，非空字符串 |
| `uncompressedSize` | `Int64` | 是 | 解压后原始字节数，$\ge 0$ |
| `isDirectory` | `Bool` | 是 | 是否为目录节点 |
| `isDataEncrypted` | `Bool` | 是 | 载荷数据是否被 AES 加密 |
| `isMetadataEncrypted` | `Bool` | 是 | 条目元数据（文件名/属性）是否在加密头中 |

### 1.3 归档提取执行结果 (`ArchiveExtractionResult`)
描述解压操作完成后的物理状态实体。

| 字段名 | 类型 | 必填 | 约束与描述 |
| :--- | :--- | :--- | :--- |
| `status` | `Int32` | 是 | `TTZIP_OK` (0) 或具体错误码 ($< 0$) |
| `extractedFilesCount` | `Int` | 是 | 成功解压落盘的物理文件总数，$\ge 0$ |
| `engineUsed` | `String` (Enum) | 是 | 解密/解压使用的底层引擎：`"native_parallel"` 或 `"libarchive_fallback"` |
| `errorMessage` | `String` (Optional) | 否 | 错误发生时的详细诊断文本 |

---

## 2. Validation Rules & Invariants

1. **KDF Stack Buffer Invariant**: `kdf_buf` 大小固定为 536 字节。UTF-16LE 密码序列长度严禁超过 512 字节（256 个 UTF-16 字符），超过时必须返回 `TTZIP_ERR_INVALID_PARAM` 并安全截断，杜绝栈溢出。
2. **Scrubbing Invariant**: 当 `ttzip_7z_crypto_session_t` 析构或 `ttzip_7z_kdf_sha256_armv8` 函数退出时，所有暂存密码、Salt 及派生密钥的栈/堆内存必须通过 `memset_s` 填充零。
3. **Consistency Invariant**: `data-model.md` 中的实体定义与 `contracts/*.json` 中的 JSON Schema 字段名、类型和枚举保持 100% 严格一致。
