# Phase 1 Data Model: 023-libarchive-7z-aes256-decryption

## 1. Core Cryptographic Entities

### 1.1 `SevenZipCryptoProperties` (7z AES Codec 属性结构体)
```c
struct _7z_crypto_properties {
    uint8_t num_cycles_power;   /* 循环轮数指数: 0 <= E <= 24, 迭代总轮数 R = 2^E */
    uint8_t salt_size;          /* Salt 字节长度: 0 <= salt_size <= 16 */
    uint8_t iv_size;            /* IV 字节长度: 0 <= iv_size <= 16 */
    uint8_t salt[16];           /* Salt 缓冲区 */
    uint8_t iv[16];             /* 16 字节对齐的 IV 缓冲区 (不足部分以 0x00 补齐) */
};
```

| 字段名 | 类型 | 必填性 | 描述 |
| :--- | :--- | :--- | :--- |
| `num_cycles_power` | `uint8` | Required | 密钥派生指数，取值范围 $[0, 24]$ |
| `salt_size` | `uint8` | Required | Salt 实际有效长度，取值范围 $[0, 16]$ |
| `iv_size` | `uint8` | Required | 归档中存储的 IV 长度，取值范围 $[0, 16]$ |
| `salt` | `bytes (16)` | Required | Salt 数据缓冲区 |
| `iv` | `bytes (16)` | Required | 16 字节对齐的初始化向量 |

---

### 1.2 `SevenZipFolderCryptoContext` (7z Folder 解密上下文)
```c
struct _7z_folder_crypto_ctx {
    int is_encrypted;                           /* 标识当前 Folder 是否包含加密 Coder */
    struct _7z_crypto_properties props;         /* 解析得到的加密属性 */
    uint8_t derived_key[32];                    /* 256 位派生 AES 密钥 */
    int key_is_cached;                          /* 标识当前密钥是否已经派生完成 */
    archive_crypto_ctx aes_ctx;                 /* 底层对称分组解密句柄 */
};
```

| 字段名 | 类型 | 必填性 | 描述 |
| :--- | :--- | :--- | :--- |
| `is_encrypted` | `int` | Required | 是否为加密 Folder (0=明文, 1=加密) |
| `props` | `SevenZipCryptoProperties` | Required | 加密属性元数据 |
| `derived_key` | `bytes (32)` | Required | 256 位 AES 密钥 |
| `key_is_cached` | `int` | Required | 是否命中 KDF 缓存 |
| `aes_ctx` | `archive_crypto_ctx` | Required | `archive_cryptor` 分组解密句柄 |

---

### 1.3 `SevenZipPassphraseRequest` (密码查询与校验请求)
```c
struct _7z_passphrase_req {
    const char *passphrase;     /* 输入的 UTF-8 密码字符串 */
    size_t passphrase_len;      /* 密码字符长度 */
    uint16_t utf16le_pw[256];   /* 转码后的 UTF-16LE 缓冲区 */
    size_t utf16le_bytes;       /* UTF-16LE 字节数 (2 * 字符数) */
};
```

| 字段名 | 类型 | 必填性 | 描述 |
| :--- | :--- | :--- | :--- |
| `passphrase` | `string` | Required | 用户输入的 UTF-8 格式明文密码 |
| `passphrase_len` | `size_t` | Required | 密码字符串长度 |
| `utf16le_bytes` | `size_t` | Required | 转换后 UTF-16LE 字节流大小 |
